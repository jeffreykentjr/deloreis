//! Queries and their answers.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::term::AtomExpr;
use dli_core::verdict::Verdict;

/// A query: a goal atom, possibly with variables to bind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Query {
    /// The goal to solve.
    pub goal: AtomExpr,
}

/// One answer row: variable bindings plus the verdict for that binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryRow {
    /// Variable name to bound-term rendering.
    pub bindings: BTreeMap<String, String>,
    /// The support and grade for this binding.
    pub verdict: Verdict,
}

/// A full query answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Answer {
    /// The answer rows.
    pub rows: Vec<QueryRow>,
}
