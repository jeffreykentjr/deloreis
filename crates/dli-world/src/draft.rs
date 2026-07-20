//! Draft payloads submitted for fact-checking.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::term::AtomExpr;
use dli_core::grade::Grade;
use dli_ledger::Delta;

/// Draft metadata (the `payload/v1` header).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftMeta {
    /// The draft title.
    pub title: String,
    /// The scope this draft targets.
    pub scope: String,
    /// The payload schema version.
    pub version: u32,
}

/// A draft payload (`payload/v1`): everything a draft asserts and spends.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftPayload {
    /// Draft metadata.
    pub meta: DraftMeta,
    /// The claims the draft asserts.
    pub claims: Vec<AtomExpr>,
    /// Resource deltas the draft records.
    pub deltas: Vec<Delta>,
    /// Scene identifiers.
    pub scenes: Vec<String>,
    /// Beat identifiers.
    pub beats: Vec<String>,
    /// Promise-ledger entries opened or closed.
    pub promises: Vec<String>,
    /// Proof references (the in-practice proof blocks the draft closes).
    pub proofs: Vec<String>,
    /// Proper names introduced (each must pass the naming oracle).
    pub names: Vec<String>,
    /// Coined lexicon terms introduced.
    pub lexicon: Vec<String>,
}

/// The outcome of fact-checking one draft claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftFinding {
    /// The claim, rendered.
    pub claim: String,
    /// The grade at which it is supported.
    pub grade: Grade,
    /// A human-readable classification message.
    pub message: String,
}
