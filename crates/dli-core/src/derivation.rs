//! Derivations: proof trees over claims and rules.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A claim identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct ClaimId(pub u32);

/// A rule identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct RuleId(pub u32);

/// A derivation tree witnessing why a claim holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Derivation {
    /// A leaf: the claim is an axiom (asserted directly).
    Axiom {
        /// The asserted claim.
        claim: ClaimId,
    },
    /// An internal node: a rule applied to sub-derivations.
    Step {
        /// The rule applied.
        rule: RuleId,
        /// The premises this step consumed.
        from: Vec<Derivation>,
    },
}
