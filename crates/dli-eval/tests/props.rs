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
