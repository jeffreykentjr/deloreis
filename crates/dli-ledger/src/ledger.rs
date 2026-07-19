//! The ledger-checking seam (PRAC-2).
//!
//! A **ledger** is one casting written out for the checker: its premises, its
//! steps, its spends, and its close, run `in <practice>`
//! (`docs/spec/04-practices.md` §5). [`check_ledger`] will replay the recorded
//! [`Delta`]s against each named practice's resource law and match the close
//! against its declared expectation.
//!
//! Only the seam types are frozen here; the replay itself lands in PRAC-2
//! (wave 3), which consumes the [`PracticeDef`]
//! and the resource semantics this crate already exposes. When it lands it will
//! project the world as of the scene (certain ∩ known-by-speaker ∩ present) to
//! select premises; that world handle is deliberately absent from the stub so
//! this crate stays free of the evaluator until the checker needs it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::ident::Ident;
use dli_core::grade::Grade;

use crate::practice::PracticeDef;
use crate::resource::Delta;

/// One ledger entry: a practice invocation and the deltas it applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerEntry {
    /// The practice invoked.
    pub practice: Ident,
    /// The resource changes it applied.
    pub deltas: Vec<Delta>,
}

/// A single detected law violation.
//
// The one-line description is embedded verbatim in the committed `check-ledger`
// contract schema; keep it byte-identical so `xtask schema --check` stays green.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerViolation {
    /// The index of the offending entry.
    pub entry: usize,
    /// A human-readable description.
    pub message: String,
    /// The grade at which the violation is asserted.
    pub grade: Grade,
}

/// The result of checking a ledger: the violations found, if any.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerReport {
    /// Every law violation detected, in entry order.
    pub violations: Vec<LedgerViolation>,
}

/// Replay a ledger against a set of practices and report law violations.
///
/// PRAC-2 implements the replay: for each entry, look up its practice, and run
/// the recorded deltas against every resource's [`ResourceKind`] law using the
/// semantics in [`crate::resource`], lifting each [`ResourceViolation`] into a
/// graded [`LedgerViolation`].
///
/// [`ResourceKind`]: crate::resource::ResourceKind
/// [`ResourceViolation`]: crate::resource::ResourceViolation
pub fn check_ledger(_practices: &[PracticeDef], _entries: &[LedgerEntry]) -> LedgerReport {
    todo!("PRAC-2: replay deltas per resource and flag law violations")
}
