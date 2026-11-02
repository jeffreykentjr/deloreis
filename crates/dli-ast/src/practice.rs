//! Typed interior of a `practice` declaration body (LS-9 expert-tier structuring).
//!
//! Before LS-9 a `practice` carried `body: Vec<Clause>` — whitespace-normalized
//! raw text that `dli-ledger` re-parsed at compile time, well past the load
//! boundary, through `dli_syntax::parse_clause_atom`. The front end now
//! structures the body at parse time into the typed [`PracticeDef`], so the
//! ledger reads typed AST instead of calling the parser.
//!
//! The interior grammar is nine keyword-fronted forms (`use`, `kind`, `role`,
//! `resource`, `rule`, `condition`, `error`, `effect`, `cost`); the single
//! producer of the typed structure is `dli_syntax::structure_practice_body`,
//! which lives behind the front end. Downstream crates (`dli-ledger`,
//! `dli-sim`, `dli-world`) are readers.
//!
//! Retaining the normalized clause text alongside the typed [`PracticeDef`] on
//! [`PracticeBody`] keeps the canonical surface exact: the formatter renders the
//! text verbatim, so parse → format is byte-stable (charter koan 7, DL-0003).
//! The typed [`PracticeDef`] is *lossy* against the surface (a role's sort is
//! read as its first word; premise/law text is normalized), so it cannot be the
//! sole interior — the raw clauses are the round-trip witness.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::annot::ProvenanceMarker;
use crate::clause::Clause;
use crate::ident::Ident;
use crate::span::Span;

/// The seven structural error classes the checker knows.
///
/// Every diegetic failure a practice declares is a *refinement* of one of these
/// (`docs/spec/04-practices.md` §2). The classes are the closed target set that
/// well-formedness obligation 1 checks against: an `error` refining anything
/// else does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum StructuralClass {
    /// L1 — unknown-form: the casting names a form the practice does not declare.
    L1,
    /// L2 — premise-not-available: a required premise is absent or contradicted.
    L2,
    /// L3 — side-condition: a declared condition fails (timing, consent, scope).
    L3,
    /// L4 — bad-instantiation: a form is instantiated against forbidden sorts.
    L4,
    /// L5 — invalid-chain: a step joins premises the law does not license.
    L5,
    /// L6 — resource-violation: a spend breaks the resource's algebra.
    L6,
    /// L7 — close-mismatch: the close does not follow from the accounted ledger.
    L7,
}

impl StructuralClass {
    /// Parse a refinement token (`L1`..`L7`, case-insensitive) into a class.
    ///
    /// Anything else — a bare word, an out-of-range `L8`, a lone `L` — is
    /// [`None`], which well-formedness surfaces as an error refining nothing.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token.to_ascii_uppercase().as_str() {
            "L1" => Some(Self::L1),
            "L2" => Some(Self::L2),
            "L3" => Some(Self::L3),
            "L4" => Some(Self::L4),
            "L5" => Some(Self::L5),
            "L6" => Some(Self::L6),
            "L7" => Some(Self::L7),
            _ => None,
        }
    }

    /// The structural meaning of this class.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::L1 => "unknown-form",
            Self::L2 => "premise-not-available",
            Self::L3 => "side-condition",
            Self::L4 => "bad-instantiation",
            Self::L5 => "invalid-chain",
            Self::L6 => "resource-violation",
            Self::L7 => "close-mismatch",
        }
    }
}

/// The conservation law a resource obeys — the closed algebra, as a tag.
///
/// This enum is closed on purpose (spec §6, second well-formedness obligation):
/// a resource whose kind is not one of these members is a compile error, because
/// there is no open resource vocabulary. The law's *parameters* — an affine
/// cap, a graded order — are not stored here; they ride on the resource
/// declaration and are passed to the semantics functions explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ResourceKind {
    /// `linear` (`conserved: false`) or `linear conserved` (`conserved: true`).
    Linear {
        /// Whether the multiset must net to zero across the close (no mint, no
        /// burn), on top of the never-negative discipline every linear resource
        /// keeps.
        conserved: bool,
    },
    /// `affine bounded <cap>` — spent at most up to a declared capacity.
    AffineBounded,
    /// `graded monotone <order>` — an ordered grade that only ever wears in one
    /// declared direction.
    GradedMonotone,
}

impl ResourceKind {
    /// The surface phrase this kind renders as (without the affine cap or the
    /// graded order, which live on the declaration).
    #[must_use]
    pub const fn phrase(&self) -> &'static str {
        match self {
            Self::Linear { conserved: false } => "linear",
            Self::Linear { conserved: true } => "linear conserved",
            Self::AffineBounded => "affine bounded",
            Self::GradedMonotone => "graded monotone",
        }
    }
}

/// The single declared direction a [`ResourceKind::GradedMonotone`] grade moves.
///
/// A graded order is written `whole > worn > husk`; the `>` reads as "wears
/// down to", so the default and the series-canonical direction is [`Decays`]
/// (`docs/spec/04-practices.md` §4 — "nothing is spent, knowing only *wears*").
/// The opposite is admitted so the algebra is symmetric: a grade that may only
/// climb its order is [`Grows`].
///
/// [`Decays`]: GradeDirection::Decays
/// [`Grows`]: GradeDirection::Grows
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum GradeDirection {
    /// The grade only ever moves *down* its order (negative deltas): it wears.
    Decays,
    /// The grade only ever moves *up* its order (positive deltas): it ripens.
    Grows,
}

/// A declared finite sort: `sort figure = the-tide | the-weir | …`, or a sort
/// drawn wholesale from a consumed layer: `sort binder from lexicon.loan-words`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SortDef {
    /// The sort name.
    pub name: Ident,
    /// The enumerated members, when written with `= a | b | …`.
    pub variants: Vec<Ident>,
    /// The layer the sort is drawn from, when written with `from <source>`.
    pub source: Option<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// A cast role: `role reader : entity requires standing(reader)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoleDef {
    /// The role name.
    pub name: Ident,
    /// The sort a filler of this role must belong to.
    pub sort: Ident,
    /// The standing conditions the role requires, verbatim.
    pub requires: Option<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// A declared resource and the closed-algebra law it obeys:
/// `resource acquaintance(name) : graded monotone whole > worn > husk`.
///
/// The [`kind`](ResourceDecl::kind) is the closed-algebra tag; it is [`None`]
/// exactly when the declared law is not a member of the algebra, which
/// well-formedness surfaces as a compile error. The law's parameters — an affine
/// [`cap`](ResourceDecl::cap), a graded [`order`](ResourceDecl::order) and
/// [`direction`](ResourceDecl::direction) — ride here alongside the tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceDecl {
    /// The resource name.
    pub name: Ident,
    /// The parameter sort, when written `name(arg)` (e.g. `acquaintance(name)`).
    pub arg: Option<Ident>,
    /// The closed-algebra law, or [`None`] if the declared law is outside it.
    pub kind: Option<ResourceKind>,
    /// The affine capacity expression, verbatim, for `affine bounded <cap>`.
    pub cap: Option<String>,
    /// The graded order, most-whole first, for `graded monotone <order>`.
    pub order: Vec<Ident>,
    /// The direction a graded resource wears (`>` reads as decay).
    pub direction: GradeDirection,
    /// The law phrase as written, kept for the error message when `kind` is
    /// [`None`].
    pub law_text: String,
    /// Source span of the declaration.
    pub span: Span,
}

/// A law of the practice: `rule lay(figure, leaf) requires phase(moon-phase) = leaf`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDef {
    /// The form the rule concludes (the head predicate).
    pub form: Ident,
    /// The head argument sorts/variables, in order.
    pub args: Vec<Ident>,
    /// The premise body after `requires`, verbatim.
    pub requires: Option<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// A side condition: `condition seated-reading requires phase(moon-phase) within declared-span`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConditionDef {
    /// The condition name.
    pub name: Ident,
    /// The predicate body after `requires`, verbatim.
    pub requires: String,
    /// Source span of the declaration.
    pub span: Span,
}

/// A declared diegetic failure, refining a structural class:
/// `error wrong-phase refines L3 side-condition [timing] over cycle moon-phase`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorClassDef {
    /// The failure name (the drama the scene voices).
    pub name: Ident,
    /// The token written after `refines` (`L3`, or whatever malformed thing).
    pub refines_token: String,
    /// The structural class, or [`None`] if `refines_token` names none.
    pub class: Option<StructuralClass>,
    /// The discriminant — the sub-case detail after the class token, verbatim.
    pub discriminant: String,
    /// The on-mic signal a performer voices when it fires (`signals <x>`).
    pub signal: Option<String>,
    /// The resource an `L6` failure spends against (`… on <resource>`).
    pub on_resource: Option<Ident>,
    /// The cycles a timing failure is seated on (`… over cycle <w1>, <w2>`).
    pub over_cycles: Vec<Ident>,
    /// Source span of the declaration.
    pub span: Span,
}

/// What a casting does: `effect invoke degrades acquaintance by 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EffectDef {
    /// The form whose casting carries the effect.
    pub form: Ident,
    /// The effect body, verbatim.
    pub detail: String,
    /// Source span of the declaration.
    pub span: Span,
}

/// A compiled practice: the workbench's typed view of one declared law-set.
///
/// Everything that distinguishes one practice from another lives here as data
/// (charter koan 10); the generic checker and evaluator read this and nothing
/// else. The cycles that were in scope at compile time are recorded on
/// [`cycles_in_scope`](PracticeDef::cycles_in_scope) so reference resolution
/// needs no further environment; the front end produces the structure with an
/// empty `cycles_in_scope`, and `dli-ledger` overlays the document's cycles when
/// it compiles the practice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PracticeDef {
    /// The practice name.
    pub name: Ident,
    /// The provenance marker the declaration carried, if any.
    pub marker: Option<ProvenanceMarker>,
    /// The layers this practice consumes (`use language.registers`).
    pub uses: Vec<String>,
    /// The declared finite sorts.
    pub sorts: Vec<SortDef>,
    /// The cast roles.
    pub roles: Vec<RoleDef>,
    /// The resources and their algebra laws.
    pub resources: Vec<ResourceDecl>,
    /// The laws of the practice.
    pub rules: Vec<RuleDef>,
    /// The side conditions.
    pub conditions: Vec<ConditionDef>,
    /// The declared failures.
    pub errors: Vec<ErrorClassDef>,
    /// The casting effects.
    pub effects: Vec<EffectDef>,
    /// The currency vocabulary a casting may spend in.
    pub costs: Vec<Ident>,
    /// The cycles in scope at compile time (those the document declares), against
    /// which cycle references are resolved. The engine seats no ambient cycles —
    /// a world's cyclic vocabulary is declared, never baked in (DL-0017). The
    /// front end leaves this empty; `dli-ledger` fills it at compile time.
    pub cycles_in_scope: BTreeSet<String>,
    /// Source span of the declaration.
    pub span: Span,
}

/// The typed interior of a [`PracticeDecl`](crate::decl::PracticeDecl) body
/// (LS-9): the retained raw clauses plus the structured [`PracticeDef`].
///
/// The parser is the single producer (via
/// `dli_syntax::structure_practice_body`); `dli-ledger` reads
/// [`def`](PracticeBody::def) without re-entering the front end. The raw
/// [`clauses`](PracticeBody::clauses) are kept so the formatter round-trips the
/// surface byte-for-byte — the typed `def` is lossy against the written text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PracticeBody {
    /// The whitespace-normalized interior clauses (one per physical line), in
    /// source order. Retained verbatim so the formatter reproduces the surface.
    pub clauses: Vec<Clause>,
    /// The structured practice, produced by the front end from `clauses`.
    ///
    /// Boxed to keep the [`Decl`](crate::decl::Decl) enum small (the many-field
    /// `PracticeDef` would otherwise dominate every declaration's size); the box
    /// is transparent to serde and the JSON Schema, so the stored carriage bytes
    /// are unchanged.
    pub def: Box<PracticeDef>,
}
