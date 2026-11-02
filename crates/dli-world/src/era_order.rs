//! Symbolic event ordering derived from era memberships.
//!
//! The worklist uses the evaluator's conditioned-support algebra, retaining
//! alternative paths and defeaters without a general datalog cross-product.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use dli_core::ann::Ann;
use dli_eval::world::{conjoin_annotations, disjoin_annotations};
use dli_time::chronology::TimePoint;

use crate::model::{EraEdge, WorldModel};

pub(crate) fn infer(model: &WorldModel) -> Vec<EraEdge> {
    if model.era_memberships.is_empty() {
        return Vec::new();
    }
    let mut adjacency: BTreeMap<u32, Vec<(u32, Ann)>> = BTreeMap::new();
    for (index, &(earlier, later)) in model.world.chronology.edges.iter().enumerate() {
        let ann = model
            .chronology_anns
            .get(index)
            .cloned()
            .unwrap_or_else(Ann::unconditional);
        adjacency.entry(earlier.0).or_default().push((later.0, ann));
    }
    let sources: BTreeSet<_> = model
        .era_memberships
        .iter()
        .map(|entry| entry.era.0)
        .collect();
    let mut paths = BTreeMap::new();
    for source in sources {
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
        paths.insert(source, reached);
    }
    let mut inferred = Vec::new();
    for (&source, targets) in &paths {
        for (&target, ann) in targets {
            if source != target && paths.contains_key(&target) {
                inferred.push(EraEdge {
                    earlier: TimePoint(source),
                    later: TimePoint(target),
                    ann: ann.clone(),
                });
            }
        }
    }
    inferred
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
