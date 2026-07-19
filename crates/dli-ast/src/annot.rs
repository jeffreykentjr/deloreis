//! Provenance annotations: citations and hypothesis conditions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ident::Ident;
use crate::span::Span;

/// The leading provenance word a claim carries.
///
/// A claim's *standing* is not stored (DL-0003: there is no status slot); it is
/// derived from this marker together with the citations behind it. The marker
/// records only where the claim came from — an agent's proposal or a ledger
/// ruling — and which register of ruling it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProvenanceMarker {
    /// `proposed` — an agent's claim, awaiting a ruling. Carries no citation.
    Proposed,
    /// `decided <d-id>` — grounded in a recorded decision.
    Decided,
    /// `ratified <d-id>` — a ratified ruling.
    Ratified,
    /// `provisional <d-id>` — a provisional ruling, open to revision.
    Provisional,
}

impl ProvenanceMarker {
    /// The surface keyword this marker renders as.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Decided => "decided",
            Self::Ratified => "ratified",
            Self::Provisional => "provisional",
        }
    }

    /// Whether this marker requires at least one citation to be well-formed.
    ///
    /// `proposed` stands on its own (koan 5); every ledger marker needs an
    /// origin (koan 1: a bare `ratified` with nowhere to come from cannot parse).
    #[must_use]
    pub const fn requires_citation(self) -> bool {
        !matches!(self, Self::Proposed)
    }
}

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
    /// The leading provenance word.
    pub marker: ProvenanceMarker,
    /// Supporting citations.
    pub citations: Vec<CitationRef>,
    /// Optional hypothesis condition under which the claim is asserted
    /// (`assumes <hyp>`).
    pub condition: Option<CondExpr>,
    /// Further provenance tokens carried verbatim — an agent id, a session tag,
    /// a scene cue. Retained so nothing on the line is dropped (koan 4).
    pub tags: Vec<String>,
    /// Where the annotation appears in source.
    pub span: Span,
}
