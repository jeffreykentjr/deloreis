//! Attestations: recorded model answers and their staleness store.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A recorded model answer to a question, with everything needed to decide
/// whether it is still current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attestation {
    /// The question posed.
    pub question: String,
    /// The recorded answer.
    pub answer: String,
    /// The model that produced it.
    pub model: String,
    /// The prompt sent.
    pub prompt: String,
    /// The generation parameters (name to rendered value).
    pub params: BTreeMap<String, String>,
    /// The policy under which the attestation was taken.
    pub policy: String,
    /// A prior attestation this one supersedes, if any.
    pub supersedes: Option<String>,
    /// The sweep fingerprint the attestation was valid under.
    pub sweep: String,
}

/// The store of recorded attestations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestationStore {
    /// Every recorded attestation.
    pub attestations: Vec<Attestation>,
}
