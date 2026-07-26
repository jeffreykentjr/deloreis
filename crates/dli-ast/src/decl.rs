//! Declarations and the document root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::annot::Annotation;
use crate::clause::Clause;
use crate::ident::Ident;
use crate::span::Span;
use crate::term::{AtomExpr, Term};

/// A parsed source document: an optional scope header plus its declarations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    /// The scope header, if the document opens with one.
    pub scope: Option<ScopeHeader>,
    /// The declarations in source order.
    pub decls: Vec<Decl>,
}

/// A `scope <name>` header naming what the document's claims are about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeHeader {
    /// The scope name.
    pub name: Ident,
    /// Source span of the header.
    pub span: Span,
}

/// A top-level declaration.
///
/// The gentle subset — [`Entity`](Decl::Entity), [`Attr`](Decl::Attr),
/// [`Fact`](Decl::Fact), [`Question`](Decl::Question), [`Before`](Decl::Before),
/// [`Hypothesis`](Decl::Hypothesis) — is fully typed. The expert tier keeps a
/// typed header and a captured [`Clause`] body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Decl {
    /// `era|place|person|thing|event <name> ["label"] …` — a world entity.
    Entity(EntityDecl),
    /// `<subject>.<attr> = <value>` — a slot fact about an entity.
    Attr(AttrDecl),
    /// `<pred>(<args>)` or `fact <subj> <pred> <obj>` — a ground relation.
    Fact(FactDecl),
    /// `question <name> "<prompt>"` with its options.
    Question(QuestionDecl),
    /// `before <earlier> -> <later>` — a timeline ordering.
    Before(BeforeDecl),
    /// `hypothesis <name> "<statement>"` — a named speculative premise.
    Hypothesis(HypothesisDecl),
    /// `use <target>` — import another scope's surface.
    Use(UseDecl),
    /// `kind <name>` — declare an entity kind.
    Kind(KindDecl),
    /// `rule <name> [over <domain>] …` — a checker rule (world, order, or season).
    Rule(RuleDecl),
    /// `state <name> = <value> …` — a time-varying property.
    State(StateDecl),
    /// `cycle <name>(<modulus>) = <spokes>` — a cyclic time structure.
    Cycle(CycleDecl),
    /// `fork <question>` — an open creative fork.
    Fork(ForkDecl),
    /// `practice <name> { … }` — a resource-tracked practice.
    Practice(PracticeDecl),
    /// `dialect <name> { … }` — a diction dialect.
    Dialect(DialectDecl),
    /// `word <name>` — a coined term.
    Word(WordDecl),
    /// `name-part <name>` — a naming-layer name-part.
    NamePart(NamePartDecl),
    /// `draft <name>` — a draft payload binding.
    Draft(DraftDecl),
    /// `attest <question>` — an attestation request.
    Attest(AttestDecl),
    /// `export <profile>` — an export directive.
    Export(ExportDecl),
    /// `name <name> "<label>" …` — a composed proper name.
    Name(NameDecl),
    /// `term <name> "<label>"` — a coined lexicon term.
    Term(TermDecl),
    /// `banned <word> <d-id> "<reason>"` — a banned in-world word.
    Banned(BannedDecl),
    /// `narration <role> <era>` — a narrative-vantage declaration.
    Narration(NarrationDecl),
    /// `variant <path>` — a lore variant of an artifact.
    Variant(VariantDecl),
    /// `ratify <d-id> { … }` — a hypothesis-bundle ratification.
    Ratify(RatifyDecl),
    /// `principle <name> …` — a craft principle.
    Principle(PrincipleDecl),
    /// `proof <name> … { … }` — a casting proof block.
    Proof(ProofDecl),
}

/// The domain a `rule` is enforced over, routing it to the right checker engine.
///
/// A rule with no domain is a world (derivation) rule; `over order` routes to the
/// episode-ordering engine, `over season` to the season-ledger engine. The three
/// forms unify the surface under one `rule` keyword (the domain is stated, not
/// keyword-encoded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleDomain {
    /// `over order` — an episode-ordering constraint.
    Order,
    /// `over season` — a season-ledger (craft) rule.
    Season,
}

impl RuleDomain {
    /// The surface keyword this domain renders as after `over`.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Order => "order",
            Self::Season => "season",
        }
    }
}

/// The five world-entity kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EntityKind {
    /// `era` — a named span of time.
    Era,
    /// `place` — a location.
    Place,
    /// `person` — a person.
    Person,
    /// `thing` — an artifact or object.
    Thing,
    /// `event` — a point on the timeline.
    Event,
}

impl EntityKind {
    /// The surface keyword this entity kind renders as.
    #[must_use]
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Era => "era",
            Self::Place => "place",
            Self::Person => "person",
            Self::Thing => "thing",
            Self::Event => "event",
        }
    }
}

/// A temporal clause attached to an entity or fluent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TimeClause {
    /// `after <era>` — begins after the named era.
    After(Ident),
    /// `during <era>` — situated within the named era.
    During(Ident),
    /// `since <event>` — holds from the named event forward.
    Since(Ident),
    /// `until <event>` — holds up to the named event.
    Until(Ident),
    /// `< a < b < c` — an era precedence chain rooted at this entity.
    Order(Vec<Ident>),
}

/// `era|place|person|thing|event <name> ["label"] [time] [provenance]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntityDecl {
    /// Which kind of entity this is.
    pub kind: EntityKind,
    /// The entity name.
    pub name: Ident,
    /// The display label, if written.
    pub label: Option<String>,
    /// A temporal clause, if written.
    pub time: Option<TimeClause>,
    /// Indented slot facts written beneath the entity.
    pub slots: Vec<AttrDecl>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `<subject>.<attr> = <value>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttrDecl {
    /// The entity the slot is about.
    pub subject: Ident,
    /// The slot name.
    pub attr: Ident,
    /// The slot value.
    pub value: Term,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `<pred>(<args>)` or `fact <subj> <pred> <obj>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FactDecl {
    /// The asserted atom.
    pub atom: AtomExpr,
    /// Whether the surface used the leading `fact` keyword (triple form).
    pub triple: bool,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `question <name> "<prompt>"` with its options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuestionDecl {
    /// The question name.
    pub name: Ident,
    /// The prompt text, if written.
    pub prompt: Option<String>,
    /// The candidate options.
    pub options: Vec<OptionClause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `option <name> ["label"] [implies <atom>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OptionClause {
    /// The option name.
    pub name: Ident,
    /// The display label, if written.
    pub label: Option<String>,
    /// The atom this option implies, if written.
    pub implies: Option<AtomExpr>,
}

/// `before <earlier> -> <later>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BeforeDecl {
    /// The event that comes first.
    pub earlier: Ident,
    /// The event that comes after.
    pub later: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `hypothesis <name> "<statement>"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HypothesisDecl {
    /// The hypothesis name.
    pub name: Ident,
    /// The statement text, if written.
    pub statement: Option<String>,
}

/// `use <target>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UseDecl {
    /// The imported scope name.
    pub target: Ident,
    /// Source span of the declaration.
    pub span: Span,
}

/// `kind <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KindDecl {
    /// The kind name.
    pub name: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `rule <name> [over <domain>] …` — header plus captured body clauses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDecl {
    /// The rule name.
    pub name: Ident,
    /// The domain the rule is enforced over: `None` is a world rule, `over order`
    /// an episode-ordering constraint, `over season` a season-ledger rule.
    pub domain: Option<RuleDomain>,
    /// Header tokens after the name (and any `over <domain>`), before the body
    /// (e.g. `severity error`).
    pub head: Vec<String>,
    /// The captured body clauses (`: … requires …` or `:- …`).
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `state <name> [= <value>] [time]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateDecl {
    /// The state name (may be dotted, e.g. `gwern.water-status`).
    pub name: Ident,
    /// The value the state takes, if written.
    pub value: Option<Term>,
    /// A temporal clause, if written.
    pub time: Option<TimeClause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `cycle <name>(<modulus>) [= <spokes>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CycleDecl {
    /// The cycle name.
    pub name: Ident,
    /// The cycle modulus, if written (`cycle tide(4)`).
    pub modulus: Option<i64>,
    /// The ordered spoke names around the cycle.
    pub spokes: Vec<Ident>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `fork <question>` with its options and body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkDecl {
    /// The open question.
    pub question: Ident,
    /// The candidate options.
    pub options: Vec<OptionClause>,
    /// Any other captured body clauses (`suppose …`, `add fact …`).
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `practice <name> ["label"] { … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PracticeDecl {
    /// The practice name.
    pub name: Ident,
    /// The display label, if written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `dialect <name> ["label"] { … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DialectDecl {
    /// The dialect name.
    pub name: Ident,
    /// The display label, if written.
    pub label: Option<String>,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `word <name> ["label"] [= <name-parts…>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WordDecl {
    /// The word name.
    pub name: Ident,
    /// The display label, if written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The name-parts composing the word.
    pub name_parts: Vec<Ident>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `name-part <name> ["gloss"] …`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NamePartDecl {
    /// The name-part name.
    pub name: Ident,
    /// The gloss, if written.
    pub gloss: Option<String>,
    /// Any remaining header tokens (`role given-proto`).
    pub head: Vec<String>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `draft <name> ["label"]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftDecl {
    /// The draft name.
    pub name: Ident,
    /// The display label, if written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `attest <question>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestDecl {
    /// The question being attested.
    pub question: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `export <profile> ["label"]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportDecl {
    /// The export profile name.
    pub profile: Ident,
    /// The display label, if written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `name <name> ["label"] …` — a composed proper name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NameDecl {
    /// The name identifier.
    pub name: Ident,
    /// The rendered label, if written.
    pub label: Option<String>,
    /// Remaining header tokens (`person.given in cymric-fringe`).
    pub head: Vec<String>,
    /// The captured body clauses (composition, bearer).
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `term <name> "<label>"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TermDecl {
    /// The term identifier.
    pub name: Ident,
    /// The rendered label, if written.
    pub label: Option<String>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `banned <word> <d-id> "<reason>"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BannedDecl {
    /// The banned word.
    pub word: Ident,
    /// The reason, if written.
    pub reason: Option<String>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `narration <role> <era>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NarrationDecl {
    /// The header tokens (`vantage the-narrators-era`).
    pub head: Vec<String>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `variant <path>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VariantDecl {
    /// The variant path (`turning/keep-stamped`).
    pub path: Ident,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `ratify <d-id> { … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RatifyDecl {
    /// The ratifying decision id.
    pub decision: Ident,
    /// The captured body clauses (`h-cymric := true`).
    pub body: Vec<Clause>,
}

/// `principle <name> …`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PrincipleDecl {
    /// The principle name.
    pub name: Ident,
    /// Remaining header tokens (`family plot mission pass`).
    pub head: Vec<String>,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `proof <name> … { … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofDecl {
    /// The proof name.
    pub name: Ident,
    /// Remaining header tokens (`speaker protagonist in spoken-proof`).
    pub head: Vec<String>,
    /// The captured body clauses (`premise …`, `step …`, `spend …`, `close …`).
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Annotation>,
}

impl ProofDecl {
    /// The role key in a validated proof header.
    #[must_use]
    pub fn role(&self) -> Option<&str> {
        (self.head.len() == 4 && self.head[2] == "in").then(|| self.head[0].as_str())
    }

    /// The entity filling the proof header's role.
    #[must_use]
    pub fn actor(&self) -> Option<&str> {
        (self.head.len() == 4 && self.head[2] == "in").then(|| self.head[1].as_str())
    }

    /// The practice named by a validated proof header.
    #[must_use]
    pub fn practice(&self) -> Option<&str> {
        (self.head.len() == 4 && self.head[2] == "in").then(|| self.head[3].as_str())
    }
}
