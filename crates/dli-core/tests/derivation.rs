#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The derivation forest: grade rollup over the procedure chain and support
//! rebuilt from the forest.

use std::collections::{BTreeMap, BTreeSet};

use dli_core::atom::{Atom, Lit, Rail};
use dli_core::derivation::{GradeBasis, SupportBasis};
use dli_core::posbool::{Monomial, PosBool};
use dli_core::{ClaimId, Derivation, Grade, RuleId};

fn axiom(claim: u32) -> Derivation {
    Derivation::Axiom {
        claim: ClaimId(claim),
    }
}

fn pos(atom: u32) -> PosBool {
    PosBool(BTreeSet::from([Monomial(BTreeSet::from([Lit {
        atom: Atom(atom),
        rail: Rail::Pos,
    }]))]))
}

#[test]
fn grade_of_axiom_is_its_base() {
    let grades = GradeBasis {
        claims: BTreeMap::from([(ClaimId(1), Grade::Checked)]),
        rules: BTreeMap::new(),
    };
    assert_eq!(axiom(1).grade_of(&grades), Grade::Checked);
    // An unrecorded claim is asserted — fail-closed.
    assert_eq!(axiom(2).grade_of(&grades), Grade::Asserted);
}

#[test]
fn grade_of_step_mins_over_the_whole_chain() {
    // rule(checked) over premises {checked, attested} -> attested.
    let grades = GradeBasis {
        claims: BTreeMap::from([(ClaimId(1), Grade::Checked), (ClaimId(2), Grade::Attested)]),
        rules: BTreeMap::from([(RuleId(9), Grade::Checked)]),
    };
    let d = Derivation::Step {
        rule: RuleId(9),
        from: vec![axiom(1), axiom(2)],
    };
    assert_eq!(d.grade_of(&grades), Grade::Attested);
}

#[test]
fn grade_of_is_the_weakest_rule_too() {
    // A weak rule drags the step down even when every premise is checked.
    let grades = GradeBasis {
        claims: BTreeMap::from([(ClaimId(1), Grade::Checked), (ClaimId(2), Grade::Checked)]),
        rules: BTreeMap::from([(RuleId(9), Grade::Tested)]),
    };
    let d = Derivation::Step {
        rule: RuleId(9),
        from: vec![axiom(1), axiom(2)],
    };
    assert_eq!(d.grade_of(&grades), Grade::Tested);
}

#[test]
fn grade_of_premiseless_step_is_the_rule() {
    let grades = GradeBasis {
        claims: BTreeMap::new(),
        rules: BTreeMap::from([(RuleId(9), Grade::Tested)]),
    };
    let d = Derivation::Step {
        rule: RuleId(9),
        from: vec![],
    };
    assert_eq!(d.grade_of(&grades), Grade::Tested);
}

#[test]
fn fiat_premises_do_not_collapse_a_checked_proof() {
    // The collapse worry in the derivation layer: the premises are ratified by
    // fiat, but the *procedure* grades are all checked (a lookup is exact), so
    // grade_of stays checked. The fiat lives in support, never in the grade
    // chain (07 §1).
    let grades = GradeBasis {
        claims: BTreeMap::from([
            (ClaimId(113), Grade::Checked),
            (ClaimId(146), Grade::Checked),
        ]),
        rules: BTreeMap::from([(RuleId(1), Grade::Checked)]),
    };
    let proof = Derivation::Step {
        rule: RuleId(1),
        from: vec![axiom(113), axiom(146)],
    };
    assert_eq!(proof.grade_of(&grades), Grade::Checked);
}

#[test]
fn support_of_axiom_is_its_base() {
    let support = SupportBasis {
        claims: BTreeMap::from([(ClaimId(1), pos(113))]),
        rules: BTreeMap::new(),
    };
    assert_eq!(axiom(1).support_of(&support), pos(113));
    // An unrecorded claim is FALSE — fail-closed (P⁺ = 0).
    assert_eq!(axiom(2).support_of(&support), PosBool::FALSE);
}

// support_of over a Step multiplies rule ∧ premises via PosBool::and_/absorb.
#[test]
fn support_of_step_multiplies_rule_and_premises() {
    // rule support {+1} ∧ premise support {+2} = {+1 ∧ +2}.
    let support = SupportBasis {
        claims: BTreeMap::from([(ClaimId(1), pos(2))]),
        rules: BTreeMap::from([(RuleId(9), pos(1))]),
    };
    let d = Derivation::Step {
        rule: RuleId(9),
        from: vec![axiom(1)],
    };
    let expected = PosBool(BTreeSet::from([Monomial(BTreeSet::from([
        Lit {
            atom: Atom(1),
            rail: Rail::Pos,
        },
        Lit {
            atom: Atom(2),
            rail: Rail::Pos,
        },
    ]))]));
    assert_eq!(d.support_of(&support), expected);
}
