//! Typed interior of a `proof` declaration body (LS-9 expert-tier structuring).
//!
//! Before LS-9 a `proof` carried `body: Vec<Clause>` — whitespace-normalized raw
//! text that `dli-ledger`'s proof checker and `dli-world`'s payload shadow-match
//! re-parsed at check time, well past the load boundary. The front end now
//! structures the body at parse time: each interior line becomes a typed
//! [`ProofOp`], with the parse-dependent pieces (the premise atom split from its
//! modifiers, the atom pre-parsed into an [`AtomExpr`], the phase-comparison,
//! the instantiate bindings, the custom-step arguments, the close claim/verdict)
//! computed once by the single producer, `dli_syntax::structure_proof_body`.
//!
//! Retaining the normalized [`Clause`] text alongside the typed [`ProofOp`]
//! keeps the canonical surface exact: the formatter renders the text verbatim,
//! so parse → format is byte-stable (charter koan 7, DL-0003). Downstream crates
//! (`dli-ledger`, `dli-world`) read the typed interior without re-entering the
//! front end; a recognized keyword whose arguments do not form a valid operation
//! is captured as [`ProofOp::Malformed`] (kept, never dropped — koan 4) so the
//! structural validator can refuse it in place of a format-and-reparse round-trip.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::clause::Clause;
use crate::term::AtomExpr;

/// The structured body of a [`ProofDecl`](crate::decl::ProofDecl): its interior
/// lines, in source order, each pairing the retained clause text with its typed
/// operation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProofBody {
    /// The interior lines, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ProofClause>,
}

impl ProofBody {
    /// Whether the body carries no interior lines.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The retained clause texts, in source order — for consumers (the formatter,
    /// citation harvesting) that read the surface form rather than the typed op.
    pub fn clauses(&self) -> impl Iterator<Item = &Clause> {
        self.items.iter().map(|item| &item.clause)
    }
}

/// One interior line of a proof body: its normalized text and, when the line
/// opens with a recognized proof keyword, its typed operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofClause {
    /// The whitespace-normalized clause text and its span. Retained so the
    /// formatter round-trips exactly and text-shaped consumers read it directly.
    pub clause: Clause,
    /// The typed operation, present when the line opens with a proof keyword
    /// (`premise`, `assume`, `step`, `spend`, `release`, `reveal`,
    /// `redistribute`, `close`). `None` for a line that opens with no proof
    /// keyword — kept for round-trip, refused by the structural validator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<ProofOp>,
}

/// A typed proof-block operation. Externally tagged (serde default): the
/// data-carrying variants wrap distinct structs, so external tagging keeps each
/// operation a single-key object without the internal-tag merge constraints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProofOp {
    /// `premise <label> <atom> [evidence…]` — an atom the casting rests on.
    Premise(ProofPremise),
    /// `assume <label> <atom>` — a fork-local supposition (always `asserted`).
    Assume(ProofAssume),
    /// `step <label> <operation> …` — an instantiate, chain, or custom step.
    Step(ProofStep),
    /// `spend|release|reveal|redistribute <label>` — a resource-ledger operation.
    Resource(ProofResource),
    /// `close <atom> [expect <verdict>]` — the casting's conclusion.
    Close(ProofClose),
    /// A recognized proof keyword whose arguments do not form a valid operation.
    /// The parser's strict per-line check rejected it; it is kept so the ledger
    /// run still sees the keyword and the structural validator can refuse it.
    Malformed(MalformedOp),
}

impl ProofOp {
    /// The proof keyword this operation renders/reassembles under — the discriminant
    /// the ledger run and close-layout accounting key on.
    #[must_use]
    pub fn keyword(&self) -> &str {
        match self {
            Self::Premise(_) => "premise",
            Self::Assume(_) => "assume",
            Self::Step(_) => "step",
            Self::Resource(r) => r.keyword.keyword(),
            Self::Close(_) => "close",
            Self::Malformed(m) => &m.keyword,
        }
    }
}

/// A `premise` line: an atom, its provenance/grade modifiers, and the parse the
/// checker classifies it by (a resource draw, a timing claim, or a world fact).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofPremise {
    /// The premise label (`p1`).
    pub label: String,
    /// The premise atom, as written (before any provenance modifier).
    pub atom: String,
    /// The provenance/grade modifier tokens after the atom, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<String>,
    /// The atom pre-parsed into a single [`AtomExpr`], when it is atom-shaped.
    /// `None` for a bare nullary claim (`a-fact`) or a phase comparison — the
    /// checker classifies off this and [`Self::phase_eq`], never re-parsing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atom_expr: Option<AtomExpr>,
    /// The `phase(<cycle>) = <spoke>` comparison this atom decodes to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_eq: Option<PhaseEq>,
}

/// An `assume` line: a fork-local supposition. Its atom is authorial ground
/// (never classified against the scene), so it carries only its label and atom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofAssume {
    /// The assumption label.
    pub label: String,
    /// The assumed atom, as written.
    pub atom: String,
}

/// A `phase(<cycle>) = <spoke>` timing comparison decoded from a premise atom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PhaseEq {
    /// The cycle named.
    pub cycle: String,
    /// The spoke (phase) the casting requires.
    pub spoke: String,
}

/// A `step` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProofStep {
    /// `step <label> instantiate <form> with <name> = <value>, …`.
    Instantiate {
        /// The step label.
        label: String,
        /// The form being instantiated.
        form: String,
        /// The parameter bindings.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        bindings: Vec<Binding>,
    },
    /// `step <label> chain from <labels> by <law> conclusion <atom>`.
    Chain {
        /// The step label.
        label: String,
        /// The prior labels joined by the chain.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        joins: Vec<String>,
        /// The law applied (the word after `by`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        law: Option<String>,
        /// The concluded atom (the text after `conclusion`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        conclusion: Option<String>,
    },
    /// `step <label> <declared-rule> [<value>, …]` — a practice-declared custom step.
    Custom {
        /// The step label.
        label: String,
        /// The custom operation (a practice-declared rule form).
        operation: String,
        /// The positional argument values.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
}

/// One `<name> = <value>` instantiate binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Binding {
    /// The parameter name.
    pub name: String,
    /// The bound value (a proof term value).
    pub value: String,
}

/// A resource-ledger operation naming a resource premise label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofResource {
    /// Which resource operation this is.
    pub keyword: ResourceKeyword,
    /// The resource premise label it operates on.
    pub label: String,
}

/// The four resource-ledger operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKeyword {
    /// `spend <label>` — draw a resource unit against a premise.
    Spend,
    /// `release <label>` — return a conserved unit to the multiset.
    Release,
    /// `reveal <label>` — surface a conserved unit already implied.
    Reveal,
    /// `redistribute <label>` — move a conserved unit between accounts.
    Redistribute,
}

impl ResourceKeyword {
    /// The surface keyword this resource operation renders as.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Spend => "spend",
            Self::Release => "release",
            Self::Reveal => "reveal",
            Self::Redistribute => "redistribute",
        }
    }
}

/// A `close <atom> [expect <verdict>]` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofClose {
    /// The close claim atom, as written.
    pub atom: String,
    /// The verdict token(s) after `expect`, if the close carried an expectation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
}

/// A recognized proof keyword whose arguments failed the strict per-line check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MalformedOp {
    /// The proof keyword the malformed line opened with.
    pub keyword: String,
}
