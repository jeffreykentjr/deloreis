#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Property coverage for the annotated evaluator: valuation commutes with the
//! fixpoint, supports stay antichain-minimal, and derivation is monotone.

use std::collections::BTreeSet;

use dli_ast::ident::Ident;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Ann;
use dli_core::atom::Atom;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;

use dli_eval::Query;
use dli_eval::world::{GroundAtom, World};

use proptest::prelude::*;

fn node(id: u32) -> String {
    format!("n{id}")
}

fn var(name: &str) -> Term {
    Term::Var(Ident::synthetic(name))
}

fn pattern(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: Ident::synthetic(pred),
        args: args.iter().map(|a| var(a)).collect(),
    }
}

/// A reachability world whose every edge is gated on its own event variable, so
/// derived supports are non-trivial polynomials over those variables.
fn conditional_reach_world(edges: &[(u32, u32)]) -> World {
    let mut world = World::new();
    for (i, &(a, b)) in edges.iter().enumerate() {
        let var = Atom(u32::try_from(i).unwrap());
        world.assert_fact(
            GroundAtom {
                pred: "adjacent".to_owned(),
                args: vec![node(a), node(b)],
            },
            Ann::supported(PosBool::pos(var)),
        );
    }
    world
        .define_rule(
            "reach-base",
            pattern("reachable", &["X", "Z"]),
            vec![pattern("adjacent", &["X", "Z"])],
        )
        .unwrap();
    world
        .define_rule(
            "reach-step",
            pattern("reachable", &["X", "Z"]),
            vec![
                pattern("adjacent", &["X", "Y"]),
                pattern("reachable", &["Y", "Z"]),
            ],
        )
        .unwrap();
    world.fixpoint().unwrap()
}

fn arb_edges() -> impl Strategy<Value = Vec<(u32, u32)>> {
    prop::collection::vec((0u32..4, 0u32..4), 0..6)
}

fn arb_valuation(vars: usize) -> impl Strategy<Value = Valuation> {
    prop::collection::vec(any::<bool>(), vars).prop_map(|bits| {
        let mut valuation = Valuation::new();
        for (i, bit) in bits.into_iter().enumerate() {
            valuation.set(Atom(u32::try_from(i).unwrap()), bit);
        }
        valuation
    })
}

proptest! {
    /// Every derived support survives the fixpoint in absorptive form: no
    /// monomial subsumes another and none is contradictory.
    #[test]
    fn supports_stay_antichain_minimal(edges in arb_edges()) {
        let world = conditional_reach_world(&edges);
        for row in &world.derived {
            prop_assert_eq!(&row.support, &row.support.absorb());
        }
    }
}

proptest! {
    /// Attaching a valuation to the world and querying agrees with evaluating
    /// the same valuation on the shared saturation — the fork seam commutes.
    #[test]
    fn valuation_commutes_with_query(edges in arb_edges(), nu in arb_valuation(6)) {
        let world = conditional_reach_world(&edges);
        let goal = Query { goal: pattern("reachable", &["X", "Z"]) };
        let attached = world.with_valuation(nu.clone()).query(&goal).unwrap();
        let evaluated = world.eval_under(&nu, &goal).unwrap();
        prop_assert_eq!(attached.rows, evaluated.rows);
    }
}

proptest! {
    /// Adding an edge never removes a reachable pair: the fixpoint is monotone
    /// in the facts.
    #[test]
    fn derivation_is_monotone(edges in arb_edges(), extra in (0u32..4, 0u32..4)) {
        let base = reachable_pairs(&conditional_reach_world(&edges));
        let mut more = edges.clone();
        more.push(extra);
        let grown = reachable_pairs(&conditional_reach_world(&more));
        prop_assert!(base.is_subset(&grown));
    }
}

/// The set of reachable `(from, to)` pairs derived in a world.
fn reachable_pairs(world: &World) -> BTreeSet<(String, String)> {
    world
        .derived
        .iter()
        .filter(|row| row.atom.pred == "reachable")
        .map(|row| (row.atom.args[0].clone(), row.atom.args[1].clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// TQ-N3: Negation property coverage — valuation-neutral poison commutation
// ---------------------------------------------------------------------------
//
// The property: for a world with stratified negation, `eval_under(ν)` on the
// symbolic fixpoint agrees with a fresh fixpoint computed after setting ν as
// the ambient valuation. This is the commutation invariant that the
// valuation-neutral poison semantics (DL-0024) guarantee: the fixpoint does
// not filter poison by the active valuation, so the derived supports and
// defeaters are identical regardless of which ν was active during saturation.
// A regression that makes negative_poison valuation-dependent would break this.

use dli_ast::literal::BodyLiteral;
use dli_core::ann::Status;

/// Build a stratified world with negation: `base(x)` facts on event variables,
/// `obs(x)` facts on event variables, and a rule:
///   `derived(X) :- base(X), not obs(X)`
///
/// Stratification is guaranteed: `obs` is purely extensional (stratum 0),
/// `base` is extensional (stratum 0), `derived` depends positively on `base`
/// and negatively on `obs` — it sits in stratum 1. The negated predicate is
/// always in a strictly earlier stratum.
fn negated_two_predicate_world(
    base_count: usize,
    obs_count: usize,
    num_vars: usize,
) -> impl Strategy<Value = World> {
    // Each base fact `base(nI)` is gated on variable I (mod num_vars).
    // Each obs fact `obs(nI)` is gated on variable (base_count + I) (mod num_vars).
    let base_count = base_count.min(8);
    let obs_count = obs_count.min(8);
    let _num_vars = num_vars.clamp(1, 12);

    // We generate which base and obs facts to include, and which variables gate them.
    let base_flags = prop::collection::vec(any::<bool>(), base_count);
    let obs_flags = prop::collection::vec(any::<bool>(), obs_count);

    (base_flags, obs_flags).prop_map(move |(bf, of)| {
        let mut world = World::new();
        // Assert base facts gated on distinct variables.
        for (i, &present) in bf.iter().enumerate() {
            if present {
                let var = Atom(u32::try_from(i).unwrap());
                world.assert_fact(
                    GroundAtom {
                        pred: "base".to_owned(),
                        args: vec![node(u32::try_from(i).unwrap())],
                    },
                    Ann::supported(PosBool::pos(var)),
                );
            }
        }
        // Assert obs facts gated on separate variables (offset by base_count).
        for (i, &present) in of.iter().enumerate() {
            if present {
                let var = Atom(u32::try_from(base_count + i).unwrap());
                world.assert_fact(
                    GroundAtom {
                        pred: "obs".to_owned(),
                        args: vec![node(u32::try_from(i).unwrap())],
                    },
                    Ann::supported(PosBool::pos(var)),
                );
            }
        }
        // Rule: derived(X) :- base(X), not obs(X)
        // This is stratifiable: obs is purely extensional.
        world
            .define_rule(
                "neg-derive",
                pattern("derived", &["X"]),
                vec![
                    BodyLiteral::pos(pattern("base", &["X"])),
                    BodyLiteral::not(pattern("obs", &["X"])),
                ],
            )
            .unwrap();
        world.fixpoint().unwrap()
    })
}

/// Generate a valuation over a given variable range.
fn arb_valuation_n(num_vars: usize) -> impl Strategy<Value = Valuation> {
    prop::collection::vec(prop::option::of(any::<bool>()), num_vars).prop_map(|bits| {
        let mut valuation = Valuation::new();
        for (i, bit) in bits.into_iter().enumerate() {
            if let Some(v) = bit {
                valuation.set(Atom(u32::try_from(i).unwrap()), v);
            }
        }
        valuation
    })
}

/// For a derived atom, compute its status under a valuation using the world's
/// theory.
fn atom_status(world: &World, atom: &GroundAtom, nu: &Valuation) -> Option<Status> {
    world
        .derived
        .iter()
        .find(|d| &d.atom == atom)
        .map(|d| d.ann().status(nu, &world.theory))
}

proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]

    /// TQ-N3: Valuation-neutral negation commutation.
    ///
    /// For a world with stratified negation, the fixpoint is
    /// valuation-independent: computing it under an empty valuation and then
    /// evaluating under ν gives the same statuses as computing it with ν already
    /// set as the ambient valuation. This catches the 74c3e74-class regression
    /// where `negative_poison` improperly filtered by the active valuation.
    #[test]
    fn negation_commutes_with_valuation(
        world in negated_two_predicate_world(5, 5, 10),
        nu in arb_valuation_n(10),
    ) {
        // world was fixpointed with the default (empty) valuation.
        // Build the same program with ν set before fixpoint.
        let mut pre_valued = World::new();
        pre_valued.valuation = nu.clone();
        // Copy facts and rules from the (pre-fixpoint) structure. The fixpointed
        // world still carries the original facts and rules.
        for fact in &world.facts {
            pre_valued.facts.push(fact.clone());
        }
        for rule in &world.rules {
            pre_valued.rules.push(rule.clone());
        }
        pre_valued.theory = world.theory.clone();
        pre_valued.interner = world.interner.clone();

        let pre_valued_sat = pre_valued.fixpoint().unwrap();

        // For every derived atom in the symbolic world, the status under ν must
        // agree between the two fixpoints.
        let all_atoms: BTreeSet<&GroundAtom> = world
            .derived
            .iter()
            .map(|d| &d.atom)
            .chain(pre_valued_sat.derived.iter().map(|d| &d.atom))
            .collect();

        for atom in all_atoms {
            let status_symbolic = atom_status(&world, atom, &nu);
            let status_recomputed = atom_status(&pre_valued_sat, atom, &nu);
            prop_assert_eq!(
                status_symbolic,
                status_recomputed,
                "Status mismatch for {} under valuation {:?}",
                atom.render(),
                nu,
            );
        }
    }
}

proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]

    /// TQ-N3 (extended): A reachability world with negation gates. The rule
    ///   blocked(X,Y) :- adjacent(X,Y), not reachable(Y,Z)
    /// is in a higher stratum than `reachable` (which depends only on
    /// `adjacent`). The commutation property holds: eval_under(ν) on the
    /// precomputed symbolic saturation agrees with a fresh fixpoint under ν.
    #[test]
    fn negated_reach_commutes_with_valuation(
        edges in arb_edges(),
        extra_obs in prop::collection::vec((0u32..4, 0u32..4, any::<bool>()), 0..4),
        nu in arb_valuation(6),
    ) {
        let _ = &extra_obs; // used to widen the search space via shrinking
        // Build reachability world with negation: blocked(X,Y) :- adjacent(X,Y), not reachable(Y,Z)
        let build = |val: Valuation| {
            let mut world = World::new();
            for (i, &(a, b)) in edges.iter().enumerate() {
                let var = Atom(u32::try_from(i).unwrap());
                world.assert_fact(
                    GroundAtom {
                        pred: "adjacent".to_owned(),
                        args: vec![node(a), node(b)],
                    },
                    Ann::supported(PosBool::pos(var)),
                );
            }
            world
                .define_rule(
                    "reach-base",
                    pattern("reachable", &["X", "Z"]),
                    vec![BodyLiteral::pos(pattern("adjacent", &["X", "Z"]))],
                )
                .unwrap();
            world
                .define_rule(
                    "reach-step",
                    pattern("reachable", &["X", "Z"]),
                    vec![
                        BodyLiteral::pos(pattern("adjacent", &["X", "Y"])),
                        BodyLiteral::pos(pattern("reachable", &["Y", "Z"])),
                    ],
                )
                .unwrap();
            // blocked(X,Y) :- adjacent(X,Y), not reachable(Y,Z)
            // This is stratifiable: reachable depends only on adjacent (stratum 0→1),
            // blocked depends negatively on reachable (stratum 2).
            // Range restriction: X and Y bound by adjacent(X,Y); Z bound by... wait,
            // Z appears only in the negated literal. We need Z in a positive atom too.
            // Fix: blocked(X,Y,Z) :- adjacent(X,Y), adjacent(Y,Z), not reachable(Y,Z)
            world
                .define_rule(
                    "blocked",
                    pattern("blocked", &["X", "Y", "Z"]),
                    vec![
                        BodyLiteral::pos(pattern("adjacent", &["X", "Y"])),
                        BodyLiteral::pos(pattern("adjacent", &["Y", "Z"])),
                        BodyLiteral::not(pattern("reachable", &["Y", "Z"])),
                    ],
                )
                .unwrap();
            world.valuation = val;
            world.fixpoint().unwrap()
        };

        let world_empty = build(Valuation::new());
        let world_nu = build(nu.clone());

        // Collect all atoms from both fixpoints.
        let all_atoms: BTreeSet<&GroundAtom> = world_empty
            .derived
            .iter()
            .map(|d| &d.atom)
            .chain(world_nu.derived.iter().map(|d| &d.atom))
            .collect();

        for atom in all_atoms {
            let status_from_empty = atom_status(&world_empty, atom, &nu);
            let status_from_nu = atom_status(&world_nu, atom, &nu);
            prop_assert_eq!(
                status_from_empty,
                status_from_nu,
                "Negated-reach commutation failed for {} under {:?}",
                atom.render(),
                nu,
            );
        }
    }
}
