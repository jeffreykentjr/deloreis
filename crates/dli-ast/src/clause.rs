//! Captured body clauses for keyword-fronted blocks.
//!
//! The gentle subset lowers into fully typed declarations. The expert tier —
//! practice blocks, register inventories, ledgers, rule bodies — is far richer,
//! and its interior clauses are captured here as normalized text with a span
//! rather than fully structured. This keeps the AST small while losing nothing:
//! each clause round-trips exactly, and the lanes that own those constructs
//! structure their own interiors on top of this record. The text is the clause
//! with runs of whitespace collapsed to single spaces and the ends trimmed, so
//! the formatter's rendering is idempotent.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::span::Span;

/// One normalized interior line of a keyword-fronted block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Clause {
    /// The clause text, whitespace-normalized.
    pub text: String,
    /// Where the clause appears in source.
    pub span: Span,
}
