#![allow(clippy::unwrap_used)]
//! Schema-description stability guard for the `Delta` and `LedgerReport`
//! types. Schemars derives these descriptions from the types' doc comments.

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
