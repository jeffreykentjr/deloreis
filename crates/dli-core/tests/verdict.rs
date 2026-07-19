#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The two-channel verdict: rollup, the min-grade filter, and channel
//! independence.

use std::collections::BTreeSet;

use dli_core::atom::{Atom, Lit, Rail};
use dli_core::posbool::{Monomial, PosBool};
use dli_core::verdict::verdict_of_procedure;
use dli_core::{Grade, Verdict};

fn cited(atom: u32) -> PosBool {
    PosBool(BTreeSet::from([Monomial(BTreeSet::from([Lit {
        atom: Atom(atom),
        rail: Rail::Pos,
    }]))]))
}

#[test]
fn empty_procedure_is_asserted() {
    assert_eq!(verdict_of_procedure(std::iter::empty()), Grade::Asserted);
}

#[test]
fn procedure_rolls_up_to_the_weakest_step() {
    assert_eq!(
        verdict_of_procedure([Grade::Checked, Grade::Tested, Grade::Checked]),
        Grade::Tested
    );
    assert_eq!(
        verdict_of_procedure([Grade::Checked, Grade::Checked]),
        Grade::Checked
    );
}

#[test]
fn channels_are_independent() {
    let support = cited(7);

    // Same weak-authority citation in channel one; an all-checked procedure in
    // channel two. The grade does not collapse to match what it rests on.
    let checked = Verdict::of_procedure(support.clone(), [Grade::Checked, Grade::Checked]);
    assert_eq!(checked.grade, Grade::Checked);
    assert_eq!(checked.support, support);

    // Weaken only the procedure: the grade moves, the support does not.
    let attested = Verdict::of_procedure(support.clone(), [Grade::Checked, Grade::Attested]);
    assert_eq!(attested.grade, Grade::Attested);
    assert_eq!(attested.support, support);
}

#[test]
fn min_grade_filter() {
    let v = Verdict::new(PosBool::truth(), Grade::Tested);
    assert!(v.meets_min_grade(Grade::Asserted));
    assert!(v.meets_min_grade(Grade::Attested));
    assert!(v.meets_min_grade(Grade::Tested));
    assert!(!v.meets_min_grade(Grade::Checked));
}

#[test]
fn verdict_serde_round_trips() {
    let v = Verdict::of_procedure(cited(3), [Grade::Checked, Grade::Tested]);
    let json = serde_json::to_string(&v).unwrap();
    let back: Verdict = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn chain_combines_both_channels() {
    let a = Verdict::new(cited(0), Grade::Checked);
    let b = Verdict::new(cited(1), Grade::Tested);
    let chained = a.chain(&b);
    // Channel two: the weakest link of the two-step chain.
    assert_eq!(chained.grade, Grade::Tested);
    // Channel one: the conjunction of what each step rests on.
    assert_eq!(chained.support, cited(0).and_(&cited(1)));
    // Grade rollup is order-insensitive; support conjunction is symmetric.
    assert_eq!(b.chain(&a).grade, Grade::Tested);
    assert_eq!(b.chain(&a).support, chained.support);
}

#[test]
fn chain_fold_matches_procedure_rollup() {
    // Folding a procedure verdict-by-verdict reproduces verdict_of_procedure on
    // the grade channel while the support accumulates.
    let steps = [Grade::Checked, Grade::Tested, Grade::Checked];
    let folded = steps
        .into_iter()
        .map(|g| Verdict::new(PosBool::truth(), g))
        .reduce(|acc, v| acc.chain(&v))
        .unwrap();
    assert_eq!(folded.grade, verdict_of_procedure(steps));
    assert_eq!(folded.grade, Grade::Tested);
    assert_eq!(folded.support, PosBool::truth());
}
