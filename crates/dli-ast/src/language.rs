//! Typed interiors of the language-layer expert declarations `dialect`, `name`,
//! and `ratify` (LS-9 expert-tier structuring, `docs/design-notes/lorestore-roadmap.md`).
//!
//! Before LS-9 these three declarations carried `body: Vec<Clause>` —
//! whitespace-normalized raw text that `dli-world`'s language compiler re-lexed
//! (and, for a `name-part` clause or a provenance tail, re-ran `parse_document`
//! over) at compile time, well past the load boundary. That re-entry linked the
//! parser into `dli-world`'s default build. The front end now structures each
//! body at parse time, so the compiler reads typed AST and never re-enters
//! `dli_syntax`.
//!
//! Each structured clause **retains its normalized [`Clause`] text** alongside
//! the typed payload, exactly as [`RuleClause`](crate::rule::RuleClause) does:
//! the formatter renders the retained text verbatim, so parse → format stays
//! byte-stable (charter koan 7, DL-0003), while the compiler reads the typed
//! payload. The parser is the single producer of the typed structure
//! (`dli_syntax::structure_dialect_body`, `structure_name_body`,
//! `structure_ratify_body`); downstream crates are readers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::annot::Annotation;
use crate::clause::Clause;
use crate::decl::NamePartDecl;

// ── dialect ────────────────────────────────────────────────────────────────

/// The structured body of a [`DialectDecl`](crate::decl::DialectDecl): its
/// interior clauses in source order, each pre-classified into a [`DialectItem`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct DialectBody {
    /// The structured interior clauses, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clauses: Vec<DialectClause>,
}

impl DialectBody {
    /// Whether the body carries no clauses.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty()
    }
}

/// One structured clause of a dialect body: its normalized text (retained for
/// exact round-trip) plus the typed payload the compiler reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DialectClause {
    /// The whitespace-normalized clause text and its span. Retained so the
    /// formatter round-trips exactly.
    pub clause: Clause,
    /// The typed interpretation of the clause.
    pub item: DialectItem,
}

/// The recognized interior forms of a dialect body (the grammar owned by the
/// language compiler, `crates/dli-world/src/language/dialect.rs`).
///
/// Each variant carries exactly the values the pre-LS-9 compiler derived from
/// the clause tokens, so the typed reader reproduces the old string-body result
/// byte-for-byte. A clause whose head keyword is not recognized is
/// [`Other`](DialectItem::Other) — the compiler ignored it before, and ignores
/// it now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DialectItem {
    /// `basis [attested] "<real-world basis>"` — the attestation seam.
    Basis {
        /// Whether the `attested` grade word led the clause tail.
        attested: bool,
        /// The quoted real-world basis, if written.
        text: Option<String>,
    },
    /// `vowels <v> <v> …` — the vowel inventory (lowercased words).
    Vowels(Vec<String>),
    /// `endings <e> <e> …` — characteristic name-endings (lowercased, a leading
    /// `-` stripped).
    Endings(Vec<String>),
    /// `foreign <c> <c> …` — clusters marking a form as not-ours (lowercased).
    Foreign(Vec<String>),
    /// `cluster-max <n>` — the consonant-cluster ceiling, `None` if no token
    /// parsed as a count.
    ClusterMax(Option<usize>),
    /// `sayability-max <n>` — the per-word syllable ceiling, `None` if no token
    /// parsed as a count.
    SayabilityMax(Option<usize>),
    /// `stress <keyword>` — the stress rule keyword (the first tail token), if
    /// present.
    Stress(Option<String>),
    /// `name-kind <kind> = <role>… [ | <role>… ]` — a slot grammar, `None` if
    /// the clause held no well-formed pattern.
    NameKind(Option<NameKindSpec>),
    /// `name-part <surface> …` — a morpheme declaration, typed exactly as the
    /// top-level parser types a `name-part` line.
    NamePart(Box<NamePartDecl>),
    /// A clause whose head keyword the dialect grammar does not recognize.
    Other,
}

/// A `name-kind` slot grammar: the name kind and its admissible slot-role
/// sequences (alternatives split by `|`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NameKindSpec {
    /// The name kind this grammar governs (e.g. `person.given`, `place`).
    pub kind: String,
    /// The admissible slot-role sequences (alternatives).
    pub patterns: Vec<Vec<String>>,
}

// ── name ─────────────────────────────────────────────────────────────────

/// The structured body of a [`NameDecl`](crate::decl::NameDecl): its interior
/// clauses in source order, each pre-classified into a [`NameItem`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct NameBody {
    /// The structured interior clauses, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clauses: Vec<NameClause>,
}

impl NameBody {
    /// Whether the body carries no clauses.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty()
    }
}

/// One structured clause of a name body: its normalized text (retained for exact
/// round-trip) plus the typed payload the compiler reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NameClause {
    /// The whitespace-normalized clause text and its span. Retained so the
    /// formatter round-trips exactly.
    pub clause: Clause,
    /// The typed interpretation of the clause.
    pub item: NameItem,
}

/// The recognized interior forms of a name body (the grammar owned by the
/// language compiler, `crates/dli-world/src/language/name.rs`).
///
/// Classification is by the clause's first token, exactly as the pre-LS-9
/// compiler classified it; the compiler applies the same header-first precedence
/// (a header composition or bearer wins; the first body provenance wins) when it
/// reads these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum NameItem {
    /// `= <role> <morpheme> [ + <role> <morpheme> … ]` — the composition steps
    /// (empty when the clause held no well-formed `role morpheme` step).
    Composition(Vec<CompositionStep>),
    /// `bearer <entity> [<provenance>]` — the entity that bears the name and any
    /// provenance riding the same line.
    Bearer {
        /// The bearer entity, if a token followed `bearer`.
        entity: Option<String>,
        /// Provenance parsed from the line's tail, if any.
        provenance: Option<Annotation>,
    },
    /// A standalone provenance line (`ratified D-1`, …).
    Provenance(Annotation),
    /// A clause the name grammar does not recognize.
    Other,
}

/// One composition step: a slot role filled by a named morpheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompositionStep {
    /// The slot role (e.g. `given-proto`).
    pub role: String,
    /// The morpheme surface filling it (e.g. `en`).
    pub morpheme: String,
}

// ── ratify ─────────────────────────────────────────────────────────────────

/// The structured body of a [`RatifyDecl`](crate::decl::RatifyDecl): the
/// hypothesis valuation lines it carries, plus the decision ids its lines cite.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct RatifyBody {
    /// The valuation clauses, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clauses: Vec<RatifyClause>,
    /// The decision ids cited across the body lines, harvested at parse time so
    /// the citation gate reads them without re-scanning clause text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<String>,
}

impl RatifyBody {
    /// Whether the body carries no clauses and cites nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty() && self.citations.is_empty()
    }
}

/// One clause of a ratify body: its normalized text (retained for exact
/// round-trip) plus the typed hypothesis assignment it makes, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RatifyClause {
    /// The whitespace-normalized clause text and its span. Retained so the
    /// formatter round-trips exactly.
    pub clause: Clause,
    /// The `hyp := true|false` assignment this clause makes, if it is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment: Option<Assignment>,
}

/// A `hyp := true|false` valuation: rule a bundle hypothesis true or false.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Assignment {
    /// The hypothesis name being valued (the first non-operator word).
    pub hyp: String,
    /// The value assigned (the last `true`/`false` word on the line).
    pub value: bool,
}
