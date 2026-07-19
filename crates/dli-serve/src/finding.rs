//! Findings and provenance shared across op results.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::grade::Grade;

/// A finding's severity. Orthogonal to grade (evidence strength): severity gates
/// `author.materialize` — an `error` blocks it, a `warn` passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Blocks `author.materialize`.
    Error,
    /// Advisory; does not block materialization.
    Warn,
}

/// A single finding attached to a result: a coded, graded observation with an
/// optional machine-applicable repair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// The severity, which gates materialization.
    pub severity: Severity,
    /// A stable finding code.
    pub code: String,
    /// A human-readable message.
    pub message: String,
    /// The grade at which the finding is asserted.
    pub grade: Grade,
    /// An optional structured repair suggestion.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub repair: Option<serde_json::Value>,
    /// Supporting citations.
    #[serde(default)]
    pub citations: Vec<String>,
}

/// Provenance supplied when adding authored material. Internally tagged on
/// `kind`; origin tracking is load-bearing for Door T (nothing an agent supplies
/// mints authority).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Provenance {
    /// Cited to one or more decisions: `{"kind":"cited","decisions":["D-0104"]}`.
    Cited {
        /// The cited decision identifiers in the consuming repo's ledger.
        decisions: Vec<String>,
    },
    /// Proposed (not yet ratified): `{"kind":"proposed","origin":"session-7c9"}`.
    Proposed {
        /// The origin token of the proposal (e.g. an agent session id).
        origin: String,
    },
    /// Conditional on a hypothesis: `{"kind":"hypothesis","hyp":"coast-runs-north"}`.
    Hypothesis {
        /// The hypothesis the material is conditioned on.
        hyp: String,
    },
}
