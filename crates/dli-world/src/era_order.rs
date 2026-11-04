//! Symbolic event ordering derived from era memberships.
//!
//! The worklist uses the evaluator's conditioned-support algebra, retaining
//! alternative paths and defeaters without a general datalog cross-product.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use dli_core::ann::Ann;
use dli_eval::world::{conjoin_annotations, disjoin_annotations};
use dli_time::chronology::TimePoint;

use crate::model::{EraEdge, WorldModel};

/// Version of the derived era-path cache, not the stored world format.
pub(crate) const INFERENCE_REVISION: u8 = 3;

type Adjacency = BTreeMap<u32, Vec<(u32, Ann)>>;

fn reachable(source: u32, adjacency: &Adjacency) -> BTreeMap<u32, Ann> {
    let mut reached = BTreeMap::from([(source, Ann::unconditional())]);
    let mut pending = VecDeque::from([source]);
    while let Some(point) = pending.pop_front() {
        let prior = reached[&point].clone();
        for (next, edge) in adjacency.get(&point).into_iter().flatten() {
            let candidate = conjoin_annotations(&prior, edge);
            let joined = reached.get(next).map_or_else(
                || candidate.clone(),
                |old| disjoin_annotations(old, &candidate),
            );
            if reached.get(next) != Some(&joined) {
                reached.insert(*next, joined);
                pending.push_back(*next);
            }
        }
    }
    reached
}

pub(crate) fn infer(model: &WorldModel) -> Vec<EraEdge> {
    if model.era_memberships.is_empty() {
        return Vec::new();
    }
    let membership_eras: BTreeSet<_> = model
        .era_memberships
        .iter()
        .map(|entry| entry.era.0)
        .collect();
    let mut sources = membership_eras.clone();
    if model.era_points_complete {
        sources.extend(&model.era_points);
    } else {
        // Older serialized models lack this registry. This fallback is
        // conservative; point-in-time reads require a recompile before use.
        for entity in &model.entities {
            if entity.sort.as_deref() == Some("era")
                && let Some(point) = model.world.time_points.lookup(&entity.name)
            {
                sources.insert(point.0);
            }
        }
    }
    let mut adjacency = Adjacency::new();
    let mut reverse = Adjacency::new();
    for (index, &(earlier, later)) in model.world.chronology.edges.iter().enumerate() {
        if !sources.contains(&earlier.0) || !sources.contains(&later.0) {
            continue;
        }
        let ann = model
            .chronology_anns
            .get(index)
            .cloned()
            .unwrap_or_else(Ann::unconditional);
        adjacency
            .entry(earlier.0)
            .or_default()
            .push((later.0, ann.clone()));
        reverse.entry(later.0).or_default().push((earlier.0, ann));
    }
    // Only paths touching an era with an event can generate a lifted edge.
    // Walk both directions from those eras, avoiding an all-era path matrix.
    let mut paths: BTreeMap<(u32, u32), Ann> = BTreeMap::new();
    for &era in &membership_eras {
        for (point, ann) in reachable(era, &adjacency) {
            if point != era {
                paths
                    .entry((era, point))
                    .and_modify(|old| {
                        *old = disjoin_annotations(old, &ann);
                    })
                    .or_insert(ann);
            }
        }
        for (point, ann) in reachable(era, &reverse) {
            if point != era {
                paths
                    .entry((point, era))
                    .and_modify(|old| {
                        *old = disjoin_annotations(old, &ann);
                    })
                    .or_insert(ann);
            }
        }
    }
    paths
        .into_iter()
        .map(|((earlier, later), ann)| EraEdge {
            earlier: TimePoint(earlier),
            later: TimePoint(later),
            ann,
        })
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::field_reassign_with_default
)]
mod tests {
    use super::*;
    use crate::model::EraMembership;
    use dli_ast::ident::Ident;
    use dli_ast::term::{AtomExpr, Term};
    use dli_core::atom::Atom;
    use dli_core::posbool::PosBool;
    use dli_eval::world::{GroundAtom, World};

    fn atom(pred: &str, names: &[&str]) -> AtomExpr {
        AtomExpr {
            pred: Ident::synthetic(pred),
            args: names
                .iter()
                .map(|name| Term::Var(Ident::synthetic(*name)))
                .collect(),
        }
    }

    #[test]
    fn worklist_matches_the_general_conditioned_fixpoint() {
        let mut model = WorldModel::default();
        let a = Ann::new(PosBool::pos(Atom(0)), PosBool::pos(Atom(2)));
        let b = Ann::supported(PosBool::pos(Atom(1)));
        model.world.theory.add_exclusive([Atom(0), Atom(1)]);
        model.world.chronology.edges = vec![
            (TimePoint(0), TimePoint(1)),
            (TimePoint(1), TimePoint(2)),
            (TimePoint(0), TimePoint(2)),
            (TimePoint(2), TimePoint(0)),
        ];
        model.chronology_anns = vec![a.clone(), b.clone(), Ann::unconditional(), b.clone()];
        model.era_memberships = vec![
            EraMembership {
                event: TimePoint(10),
                era: TimePoint(0),
                ann: a,
            },
            EraMembership {
                event: TimePoint(11),
                era: TimePoint(1),
                ann: b,
            },
            EraMembership {
                event: TimePoint(12),
                era: TimePoint(2),
                ann: Ann::unconditional(),
            },
        ];
        let mut reference = World::new();
        reference.theory = model.world.theory.clone();
        for (edge, ann) in model
            .world
            .chronology
            .edges
            .iter()
            .zip(&model.chronology_anns)
        {
            reference.assert_fact(
                GroundAtom {
                    pred: "edge".into(),
                    args: vec![edge.0.0.to_string(), edge.1.0.to_string()],
                },
                ann.clone(),
            );
        }
        for member in &model.era_memberships {
            reference.assert_fact(
                GroundAtom {
                    pred: "member".into(),
                    args: vec![member.event.0.to_string(), member.era.0.to_string()],
                },
                member.ann.clone(),
            );
        }
        reference
            .define_rule(
                "base",
                atom("path", &["a", "b"]),
                [atom("edge", &["a", "b"])],
            )
            .unwrap();
        reference
            .define_rule(
                "step",
                atom("path", &["a", "c"]),
                [atom("path", &["a", "b"]), atom("edge", &["b", "c"])],
            )
            .unwrap();
        reference
            .define_rule(
                "result",
                atom("inferred", &["x", "y"]),
                [
                    atom("member", &["x", "a"]),
                    atom("path", &["a", "b"]),
                    atom("member", &["y", "b"]),
                ],
            )
            .unwrap();
        let reference = reference.fixpoint().unwrap();
        let expected: BTreeMap<_, _> = reference
            .derived
            .iter()
            .filter(|row| row.atom.pred == "inferred" && row.atom.args[0] != row.atom.args[1])
            .map(|row| {
                (
                    (
                        row.atom.args[0].parse::<u32>().unwrap(),
                        row.atom.args[1].parse::<u32>().unwrap(),
                    ),
                    row.ann(),
                )
            })
            .collect();
        model.inferred_era_edges = infer(&model);
        model.era_inference_revision = INFERENCE_REVISION;
        let (actual_graph, actual_anns) = model.annotated_chronology();
        let actual: BTreeMap<_, _> = actual_graph
            .edges
            .into_iter()
            .zip(actual_anns)
            .filter(|((earlier, later), _)| earlier.0 >= 10 && later.0 >= 10)
            .map(|((earlier, later), ann)| ((earlier.0, later.0), ann))
            .collect();
        assert_eq!(actual, expected);
    }
    #[test]
    fn event_edge_joins_independent_paths_and_preserves_defeaters() {
        use dli_core::ann::Status;
        let mut model = WorldModel::default();
        model.era_memberships = vec![
            EraMembership {
                event: TimePoint(10),
                era: TimePoint(0),
                ann: Ann::new(PosBool::pos(Atom(0)), PosBool::pos(Atom(3))),
            },
            EraMembership {
                event: TimePoint(10),
                era: TimePoint(2),
                ann: Ann::supported(PosBool::pos(Atom(1))),
            },
            EraMembership {
                event: TimePoint(11),
                era: TimePoint(1),
                ann: Ann::unconditional(),
            },
        ];
        model.inferred_era_edges = vec![
            EraEdge {
                earlier: TimePoint(0),
                later: TimePoint(1),
                ann: Ann::unconditional(),
            },
            EraEdge {
                earlier: TimePoint(2),
                later: TimePoint(1),
                ann: Ann::unconditional(),
            },
        ];
        model.era_inference_revision = INFERENCE_REVISION;
        model
            .world
            .chronology
            .edges
            .push((TimePoint(10), TimePoint(11)));
        model
            .chronology_anns
            .push(Ann::supported(PosBool::pos(Atom(2))));
        let (graph, anns) = model.annotated_chronology();
        let edges: Vec<_> = graph
            .edges
            .iter()
            .zip(&anns)
            .filter(|(edge, _)| **edge == (TimePoint(10), TimePoint(11)))
            .collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].1.support.monomials().count(), 3);
        for i in 0..3 {
            let mut valuation = dli_core::valuation::Valuation::new();
            for j in 0..3 {
                valuation.set(Atom(j), i == j);
            }
            assert_eq!(
                edges[0].1.status(&valuation, &model.world.theory),
                Status::Certain
            );
        }
        let mut valuation = dli_core::valuation::Valuation::new();
        for i in 0..4 {
            valuation.set(Atom(i), i == 0 || i == 3);
        }
        assert_ne!(
            edges[0].1.status(&valuation, &model.world.theory),
            Status::Certain
        );
        valuation.set(Atom(1), true);
        assert_eq!(
            edges[0].1.status(&valuation, &model.world.theory),
            Status::Certain
        );
    }
}
