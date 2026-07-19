//! Provenance annotations: citations and hypothesis conditions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ident::Ident;
use crate::span::Span;

/// A reference to a decision-ledger entry, e.g. `D-0042`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CitationRef {
    /// The cited decision identifier.
    pub id: Ident,
}

/// A boolean condition over named hypotheses that gates a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CondExpr {
    /// The named hypothesis holds.
    Hyp(Ident),
    /// Negation.
    Not(Box<CondExpr>),
    /// Conjunction of sub-conditions.
    And(Vec<CondExpr>),
    /// Disjunction of sub-conditions.
    Or(Vec<CondExpr>),
}

/// Provenance attached to a lore-bearing declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    /// Supporting citations.
    pub citations: Vec<CitationRef>,
    /// Optional hypothesis condition under which the claim is asserted.
    pub condition: Option<CondExpr>,
    /// Where the annotation appears in source.
    pub span: Span,
}
