//! Verdicts: a support formula paired with a grade.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::grade::Grade;
use crate::posbool::PosBool;

/// The outcome of asking about a claim: under which hypotheses it holds, and
/// how strongly it is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    /// The hypotheses under which the claim holds.
    pub support: PosBool,
    /// The strength of knowledge.
    pub grade: Grade,
}
