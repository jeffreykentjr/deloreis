#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Property coverage for the support lattice.

use dli_core::atom::{Atom, Lit, Rail};
use dli_core::posbool::{Monomial, PosBool};
use dli_core::valuation::Valuation;
use proptest::prelude::*;

fn arb_lit() -> impl Strategy<Value = Lit> {
    (0u32..8, prop_oneof![Just(Rail::Pos), Just(Rail::Neg)]).prop_map(|(id, rail)| Lit {
        atom: Atom(id),
        rail,
    })
}

fn arb_monomial() -> impl Strategy<Value = Monomial> {
    prop::collection::btree_set(arb_lit(), 0..4).prop_map(Monomial)
}

fn arb_posbool() -> impl Strategy<Value = PosBool> {
    prop::collection::btree_set(arb_monomial(), 0..4).prop_map(PosBool)
}

proptest! {
    /// Smoke: the real wire form round-trips for any small formula.
    #[test]
    fn posbool_serde_round_trips(formula in arb_posbool()) {
        let json = serde_json::to_string(&formula).unwrap();
        let back: PosBool = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(formula, back);
    }
}

proptest! {
    #[test]
    #[ignore = "enabled at CORE-2"]
    fn or_is_commutative(a in arb_posbool(), b in arb_posbool()) {
        prop_assert_eq!(a.or_(&b), b.or_(&a));
    }
}

proptest! {
    #[test]
    #[ignore = "enabled at CORE-2"]
    fn and_absorbs_or(a in arb_posbool(), b in arb_posbool()) {
        // Absorption: a ∨ (a ∧ b) == a.
        prop_assert_eq!(a.or_(&a.and_(&b)).absorb(), a.absorb());
    }
}

proptest! {
    #[test]
    #[ignore = "enabled at CORE-2"]
    fn eval_is_a_homomorphism(a in arb_posbool(), b in arb_posbool()) {
        let valuation = Valuation::new();
        prop_assert_eq!(
            a.or_(&b).eval_under(&valuation),
            a.eval_under(&valuation) || b.eval_under(&valuation)
        );
    }
}
