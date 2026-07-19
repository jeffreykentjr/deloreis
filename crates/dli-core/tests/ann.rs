#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Behavioural coverage for annotations: status as evaluation, not storage.

use dli_core::ann::{Ann, Status, Theory};
use dli_core::atom::{Atom, Lit};
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;

fn ruled(atoms: &[(u32, bool)]) -> Valuation {
    let mut valuation = Valuation::new();
    for &(id, value) in atoms {
        valuation.set(Atom(id), value);
    }
    valuation
}

#[test]
fn unconditional_support_is_certain() {
    let ann = Ann::unconditional();
    assert_eq!(
        ann.status(&Valuation::new(), &Theory::new()),
        Status::Certain
    );
}

#[test]
fn open_option_is_possible_then_certain_then_dead() {
    let ann = Ann::supported(PosBool::pos(Atom(0)));
    let theory = Theory::new();
    // Open: a shadow.
    assert_eq!(ann.status(&Valuation::new(), &theory), Status::Possible);
    // Ruled true: certain — same polynomial, new valuation.
    assert_eq!(ann.status(&ruled(&[(0, true)]), &theory), Status::Certain);
    // Ruled false: dead — no admissible world supports it.
    assert_eq!(ann.status(&ruled(&[(0, false)]), &theory), Status::Dead);
}

#[test]
fn disjunctive_certainty_does_not_count() {
    // Supported by *either* option of a question; neither ruled.
    let ann = Ann::supported(PosBool::pos(Atom(0)).or_(&PosBool::pos(Atom(1))));
    let mut theory = Theory::new();
    theory.add_exclusive([Atom(0), Atom(1)]);
    // Both open: assertable under some resolution, but not on resolved events
    // alone — still merely possible.
    assert_eq!(ann.status(&Valuation::new(), &theory), Status::Possible);
}

#[test]
fn exclusivity_kills_a_two_option_monomial() {
    // A single monomial resting on both options of one exclusivity family.
    let ann = Ann::supported(PosBool::conjunction([Lit::pos(Atom(0)), Lit::pos(Atom(1))]));
    let mut theory = Theory::new();
    theory.add_exclusive([Atom(0), Atom(1)]);
    assert_eq!(ann.status(&Valuation::new(), &theory), Status::Dead);
}

#[test]
fn empty_support_fails_closed() {
    // P⁺ = 0: not certain, not even possible — excluded from every layer.
    let ann = Ann::supported(PosBool::FALSE);
    assert_eq!(
        ann.status(&ruled(&[(0, true)]), &Theory::new()),
        Status::Dead
    );
}

#[test]
fn a_fully_ruled_defeater_overrides_certainty() {
    // Certain support, but a resolved defeater: the default is overridden.
    let ann = Ann::new(PosBool::truth(), PosBool::pos(Atom(9)));
    let theory = Theory::new();
    assert_eq!(ann.status(&ruled(&[(9, true)]), &theory), Status::Possible);
    assert!(!ann.at_risk(&ruled(&[(9, true)]), &theory));
}

#[test]
fn a_pending_defeater_leaves_certainty_at_risk() {
    // Certain support, defeater still open: assertable now, risk surfaced.
    let ann = Ann::new(PosBool::truth(), PosBool::pos(Atom(9)));
    let theory = Theory::new();
    assert_eq!(ann.status(&Valuation::new(), &theory), Status::Certain);
    assert!(ann.at_risk(&Valuation::new(), &theory));
}
