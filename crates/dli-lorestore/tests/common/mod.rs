#![allow(dead_code)]

//! Shared helpers for LS-2 lorestore oracle tests.

use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_lorestore::ClaimId;
use dli_lorestore::FactObject;

/// Build a simple `FactObject` for testing with a positive support polynomial.
pub fn test_fact(
    claim_label: &str,
    pred: &str,
    args: &[&str],
    support: PosBool,
    grade: Grade,
) -> FactObject {
    FactObject::new(
        ClaimId::from_label(claim_label),
        dli_eval::world::GroundAtom {
            pred: pred.to_owned(),
            args: args.iter().map(ToString::to_string).collect(),
        },
        support,
        PosBool::FALSE,
        Vec::new(),
        grade,
    )
}
