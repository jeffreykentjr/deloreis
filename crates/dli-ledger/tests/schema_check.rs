#![allow(clippy::unwrap_used)]
//! Schema-stability guard: the `Delta` and `LedgerReport` types are embedded in
//! the committed `check-draft` and `check-proof` contract schemas, whose
//! descriptions schemars bakes from these types' doc comments. This test fails
//! the moment one drifts, before `xtask schema --check` ever runs — so an
//! innocent doc-comment edit cannot silently break the fantasy-worker contract.

use dli_ledger::{Delta, LedgerReport};

#[test]
fn schema_descriptions_match_committed_contract() {
    let delta = serde_json::to_value(schemars::schema_for!(Delta)).unwrap();
    assert_eq!(delta["description"], "A single resource change.");
    let props = &delta["properties"];
    assert_eq!(props["resource"]["description"], "The resource affected.");
    assert_eq!(props["amount"]["description"], "The signed change amount.");

    let report = serde_json::to_value(schemars::schema_for!(LedgerReport)).unwrap();
    assert_eq!(
        report["description"],
        "The result of checking a ledger: the violations found, if any."
    );
    assert_eq!(
        report["properties"]["violations"]["description"],
        "Every law violation detected, in entry order."
    );
    let viol = &report["$defs"]["LedgerViolation"];
    assert_eq!(viol["description"], "A single detected law violation.");
    assert_eq!(
        viol["properties"]["entry"]["description"],
        "The index of the offending entry."
    );
    assert_eq!(
        viol["properties"]["message"]["description"],
        "A human-readable description."
    );
    assert_eq!(
        viol["properties"]["grade"]["description"],
        "The grade at which the violation is asserted."
    );
}
