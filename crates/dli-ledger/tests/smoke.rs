#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the resource ledger.

use dli_ledger::ResourceKind;

#[test]
fn resource_kind_round_trips() {
    for kind in [
        ResourceKind::Linear { conserved: true },
        ResourceKind::Linear { conserved: false },
        ResourceKind::AffineBounded,
        ResourceKind::GradedMonotone,
    ] {
        let json = serde_json::to_string(&kind).unwrap();
        let back: ResourceKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, back);
    }
}

#[test]
#[ignore = "enabled at PRAC-2"]
fn golden_turning_ledger_report_matches() {
    // The turning-practice ledger fixture becomes a real assertion once
    // check_ledger replays deltas.
    todo!("PRAC-2: golden check_ledger over the turning practice")
}
