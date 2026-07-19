#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Replayable judgments: byte-stable serialization, digest stability, and
//! faithful (or detectably unfaithful) replay.

use std::collections::{BTreeMap, BTreeSet};

use dli_core::atom::{Atom, Lit, Rail};
use dli_core::derivation::{GradeBasis, SupportBasis};
use dli_core::judgment::Judgment;
use dli_core::posbool::{Monomial, PosBool};
use dli_core::{ClaimId, Derivation, Grade};

fn pos(atom: u32) -> PosBool {
    PosBool(BTreeSet::from([Monomial(BTreeSet::from([Lit {
        atom: Atom(atom),
        rail: Rail::Pos,
    }]))]))
}

/// An axiom-based judgment: its support rebuild is a direct lookup that never
/// invokes `PosBool::and_`, keeping this helper independent of the DNF meet.
fn axiom_judgment() -> (Judgment, GradeBasis, SupportBasis) {
    let claim = ClaimId(1);
    let grades = GradeBasis {
        claims: BTreeMap::from([(claim, Grade::Checked)]),
        rules: BTreeMap::new(),
    };
    let support = SupportBasis {
        claims: BTreeMap::from([(claim, pos(113))]),
        rules: BTreeMap::new(),
    };
    let j = Judgment::derive(claim, Derivation::Axiom { claim }, &grades, &support);
    (j, grades, support)
}

#[test]
fn serialization_is_byte_stable() {
    let (j, _, _) = axiom_judgment();
    let bytes1 = serde_json::to_vec(&j).unwrap();
    let back: Judgment = serde_json::from_slice(&bytes1).unwrap();
    let bytes2 = serde_json::to_vec(&back).unwrap();
    assert_eq!(
        bytes1, bytes2,
        "serialize -> deserialize -> serialize is stable"
    );
    assert_eq!(j, back);
}

#[test]
fn replays_faithfully_against_the_same_bases() {
    let (j, grades, support) = axiom_judgment();
    let replay = j.replay(&grades, &support);
    assert!(replay.is_faithful());
    assert!(replay.verdict_matches);
    assert!(replay.digest_matches);
    assert_eq!(replay.verdict, j.verdict);
    assert_eq!(replay.digest, j.inputs_digest);
}

#[test]
fn digest_survives_a_round_trip() {
    let (j, grades, support) = axiom_judgment();
    let json = serde_json::to_string(&j).unwrap();
    let back: Judgment = serde_json::from_str(&json).unwrap();
    // The digest recomputed from the round-tripped record reproduces exactly.
    let replay = back.replay(&grades, &support);
    assert_eq!(replay.digest, j.inputs_digest);
    assert_eq!(back.inputs_digest, j.inputs_digest);
}

#[test]
fn digest_is_prefixed_and_hex() {
    let (j, _, _) = axiom_judgment();
    let Judgment { inputs_digest, .. } = &j;
    assert!(inputs_digest.0.starts_with("sha256:"));
    let hex = &inputs_digest.0["sha256:".len()..];
    assert_eq!(hex.len(), 64);
    assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn replay_against_different_bases_is_a_finding() {
    let (j, grades, _) = axiom_judgment();
    // A different support environment: the digest must diverge, and it is a
    // finding, never a silent difference (koan 4).
    let other = SupportBasis {
        claims: BTreeMap::from([(ClaimId(1), PosBool::truth())]),
        rules: BTreeMap::new(),
    };
    let replay = j.replay(&grades, &other);
    assert!(!replay.digest_matches);
    assert!(!replay.is_faithful());
    // Channel two (grade) is unaffected — it never reads support — while
    // channel one (support) moved.
    assert_eq!(replay.verdict.grade, j.verdict.grade);
    assert_ne!(replay.verdict.support, j.verdict.support);
}

// A judgment over a Step derivation exercises support_of's and_/absorb.
#[test]
fn step_judgment_replays_faithfully() {
    let claim = ClaimId(1);
    let grades = GradeBasis {
        claims: BTreeMap::from([(claim, Grade::Checked)]),
        rules: BTreeMap::from([(dli_core::RuleId(9), Grade::Checked)]),
    };
    let support = SupportBasis {
        claims: BTreeMap::from([(claim, pos(2))]),
        rules: BTreeMap::from([(dli_core::RuleId(9), pos(1))]),
    };
    let derivation = Derivation::Step {
        rule: dli_core::RuleId(9),
        from: vec![Derivation::Axiom { claim }],
    };
    let j = Judgment::derive(ClaimId(50), derivation, &grades, &support);
    assert!(j.replay(&grades, &support).is_faithful());
}
