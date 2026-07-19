//! Claim grades: the monotone strength-of-knowledge order.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How strongly a claim is known, weakest to strongest.
///
/// The derived `Ord` is the semantics: `Asserted < Attested < Tested <
/// Checked`. A claim's grade can only rise as evidence accumulates.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    /// Stated in canon, not yet checked against anything.
    Asserted,
    /// Backed by a recorded model attestation.
    Attested,
    /// Exercised by a passing test or oracle.
    Tested,
    /// Machine-checked against the world model.
    Checked,
}
