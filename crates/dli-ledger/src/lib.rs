//! Deloreis Lore Interchange resource ledger.
//!
//! A practice spends and yields resources; each resource obeys a
//! [`ResourceKind`] law (linear/conserved, affine-bounded, or graded-monotone).
//! [`check_ledger`] replays a sequence of [`LedgerEntry`] deltas against the
//! practices in a world and reports every law it violates.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::ident::Ident;
use dli_core::grade::Grade;
use dli_eval::world::World;

/// The conservation law a resource obeys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ResourceKind {
    /// A linear resource; `conserved` requires the net delta to be zero.
    Linear {
        /// Whether the resource must net to zero across the ledger.
        conserved: bool,
    },
    /// An affine resource bounded below at zero (may be spent, not created).
    AffineBounded,
    /// A resource that may only increase (a graded, monotone quantity).
    GradedMonotone,
}

/// A declared practice and the resource law it is checked against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PracticeDef {
    /// The practice name.
    pub name: Ident,
    /// The resource law this practice's deltas obey.
    pub resource: ResourceKind,
}

/// A single resource change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Delta {
    /// The resource affected.
    pub resource: Ident,
    /// The signed change amount.
    pub amount: i64,
}

/// One ledger entry: a practice invocation and the deltas it applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerEntry {
    /// The practice invoked.
    pub practice: Ident,
    /// The resource changes it applied.
    pub deltas: Vec<Delta>,
}

/// A single detected law violation.
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

/// Replay a ledger against a world's practices and report law violations.
pub fn check_ledger(
    _world: &World,
    _practices: &[PracticeDef],
    _entries: &[LedgerEntry],
) -> LedgerReport {
    todo!("LEDGER-1: replay deltas per resource and flag law violations")
}
