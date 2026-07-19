//! Queries and their answers.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::term::AtomExpr;
use dli_core::ann::Status;
use dli_core::verdict::Verdict;

/// A query: a goal atom, possibly with variables to bind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Query {
    /// The goal to solve.
    pub goal: AtomExpr,
}

/// One answer row: the matched ground atom, its variable bindings, the verdict
/// (support and grade) for that binding, and the atom's status under the
/// valuation the query was evaluated in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryRow {
    /// The matched ground atom, rendered `pred(a, b)`.
    pub atom: String,
    /// Variable name to bound-term rendering.
    pub bindings: BTreeMap<String, String>,
    /// The support and grade for this binding.
    pub verdict: Verdict,
    /// The atom's standing under the ambient valuation.
    pub status: Status,
}

/// A full query answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Answer {
    /// The answer rows.
    pub rows: Vec<QueryRow>,
}
