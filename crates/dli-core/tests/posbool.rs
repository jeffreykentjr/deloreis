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

fn arb_valuation() -> impl Strategy<Value = Valuation> {
    prop::collection::vec((0u32..8, any::<bool>()), 0..8).prop_map(|pairs| {
        let mut valuation = Valuation::new();
        for (id, value) in pairs {
            valuation.set(Atom(id), value);
        }
        valuation
    })
}

proptest! {
    /// The wire form round-trips up to canonicalization: serialize then
    /// deserialize yields the absorptive antichain of the original formula.
    #[test]
    fn posbool_serde_round_trips(formula in arb_posbool()) {
        let json = serde_json::to_string(&formula).unwrap();
        let back: PosBool = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(formula.absorb(), back);
    }
}

#[test]
fn deserialize_drops_contradictory_monomials() {
    // A wire monomial holding both rails of an atom is identically false and
    // must not survive into the in-memory form.
    let parsed: PosBool = serde_json::from_str(r#"{"monomials":[["+0","-0"]]}"#).unwrap();
    assert!(parsed.is_false());
}

#[test]
fn deserialize_absorbs_subsumed_monomials() {
    // A wire form carrying a subsumed monomial canonicalizes on the way in, so
    // `==`-based change detection cannot be poisoned by non-minimal input.
    let parsed: PosBool = serde_json::from_str(r#"{"monomials":[["+0"],["+0","+1"]]}"#).unwrap();
    assert_eq!(parsed, PosBool::pos(Atom(0)));
}

proptest! {
    #[test]
    fn or_is_commutative(a in arb_posbool(), b in arb_posbool()) {
        prop_assert_eq!(a.or_(&b), b.or_(&a));
    }
}

proptest! {
    #[test]
    fn and_is_commutative(a in arb_posbool(), b in arb_posbool()) {
        prop_assert_eq!(a.and_(&b), b.and_(&a));
    }
}

proptest! {
    #[test]
    fn and_absorbs_or(a in arb_posbool(), b in arb_posbool()) {
        // Absorption: a ∨ (a ∧ b) == a.
        prop_assert_eq!(a.or_(&a.and_(&b)).absorb(), a.absorb());
    }
}

proptest! {
    #[test]
    fn eval_is_a_homomorphism(a in arb_posbool(), b in arb_posbool(), nu in arb_valuation()) {
        // Evaluation distributes over both connectives under any valuation.
        prop_assert_eq!(
            a.or_(&b).eval_under(&nu),
            a.eval_under(&nu) || b.eval_under(&nu)
        );
        prop_assert_eq!(
            a.and_(&b).eval_under(&nu),
            a.eval_under(&nu) && b.eval_under(&nu)
        );
    }
}

proptest! {
    /// Absorption is eval-preserving under every valuation: dropping subsumed
    /// and contradictory monomials never changes the formula's meaning.
    #[test]
    fn absorb_preserves_eval(a in arb_posbool(), nu in arb_valuation()) {
        prop_assert_eq!(a.eval_under(&nu), a.absorb().eval_under(&nu));
    }
}

proptest! {
    /// Absorption is idempotent and yields a minimal antichain: no surviving
    /// monomial is contradictory or subsumed by another.
    #[test]
    fn absorb_is_a_minimal_antichain(a in arb_posbool()) {
        let reduced = a.absorb();
        prop_assert_eq!(&reduced, &reduced.absorb());
        let monomials: Vec<&Monomial> = reduced.monomials().collect();
        for (i, m) in monomials.iter().enumerate() {
            prop_assert!(!m.is_contradictory());
            for (j, other) in monomials.iter().enumerate() {
                if i != j {
                    prop_assert!(!other.subsumes(m));
                }
            }
        }
    }
}

proptest! {
    /// Adding support never shrinks what holds: `or_` is monotone, so if `a`
    /// evaluates true so does `a ∨ b` (the certain set only grows with facts).
    #[test]
    fn or_is_monotone(a in arb_posbool(), b in arb_posbool(), nu in arb_valuation()) {
        if a.eval_under(&nu) {
            prop_assert!(a.or_(&b).eval_under(&nu));
        }
        if a.is_certain_under(&nu) {
            prop_assert!(a.or_(&b).is_certain_under(&nu));
        }
    }
}

proptest! {
    /// The De Morgan complement agrees with pointwise boolean negation under
    /// every total (open-goes-false) reading of the valuation.
    #[test]
    fn negate_is_complement(a in arb_posbool(), nu in arb_valuation()) {
        prop_assert_eq!(a.negate().eval_under(&nu), !a.eval_under(&nu));
    }
}
