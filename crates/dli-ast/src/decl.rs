//! Declarations and the document root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::annot::Annotation;
use crate::clause::Clause;
use crate::ident::Ident;
use crate::span::Span;
use crate::term::{AtomExpr, Term};

/// A parsed source document: an optional pack header plus its declarations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    /// The pack header, if the document opens with one.
    pub pack: Option<PackHeader>,
    /// The declarations in source order.
    pub decls: Vec<Decl>,
}

/// A `pack <name>` header naming the document's pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackHeader {
    /// The pack name.
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
    /// `use <target>` — import another pack's surface.
    Use(UseDecl),
    /// `sort <name>` — declare an entity sort.
    Sort(SortDecl),
    /// `rule <name> …` — a derivation rule.
    Rule(RuleDecl),
    /// `fluent <name> = <value> …` — a time-varying property.
    Fluent(FluentDecl),
    /// `wheel <name>(<modulus>) = <spokes>` — a cyclic time structure.
    Wheel(WheelDecl),
    /// `fork <question>` — an open creative fork.
    Fork(ForkDecl),
    /// `practice <name> { … }` — a resource-tracked practice.
    Practice(PracticeDecl),
    /// `register <name> { … }` — a diction register.
    Register(RegisterDecl),
    /// `lexeme <name>` — a coined term.
    Lexeme(LexemeDecl),
    /// `morpheme <name>` — a naming-layer morpheme.
    Morpheme(MorphemeDecl),
    /// `craft <name>` — a craft rule.
    Craft(CraftDecl),
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
    /// `order-rule <name> …` — an episode-ordering constraint.
    OrderRule(OrderRuleDecl),
    /// `principle <name> …` — a craft principle.
    Principle(PrincipleDecl),
    /// `ledger <name> … { … }` — a casting proof ledger.
    Ledger(LedgerDecl),
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

/// `option <name> [implies <atom>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OptionClause {
    /// The option name.
    pub name: Ident,
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
    /// The imported pack name.
    pub target: Ident,
    /// Source span of the declaration.
    pub span: Span,
}

/// `sort <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SortDecl {
    /// The sort name.
    pub name: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `rule <name> …` — header plus captured body clauses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDecl {
    /// The rule name.
    pub name: Ident,
    /// Header tokens after the name, before the body (e.g. `severity error`).
    pub head: Vec<String>,
    /// The captured body clauses (`: … requires …` or `:- …`).
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `fluent <name> [= <value>] [time]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FluentDecl {
    /// The fluent name (may be dotted, e.g. `gwern.water-status`).
    pub name: Ident,
    /// The value the fluent takes, if written.
    pub value: Option<Term>,
    /// A temporal clause, if written.
    pub time: Option<TimeClause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `wheel <name>(<modulus>) [= <spokes>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WheelDecl {
    /// The wheel name.
    pub name: Ident,
    /// The wheel modulus, if written (`wheel tide(4)`).
    pub modulus: Option<i64>,
    /// The ordered spoke names around the wheel.
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

/// `practice <name> { … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PracticeDecl {
    /// The practice name.
    pub name: Ident,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `register <name> ["label"] { … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegisterDecl {
    /// The register name.
    pub name: Ident,
    /// The display label, if written.
    pub label: Option<String>,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `lexeme <name>` with its morphemes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LexemeDecl {
    /// The lexeme name.
    pub name: Ident,
    /// The morphemes composing the lexeme.
    pub morphemes: Vec<Ident>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `morpheme <name> ["gloss"] …`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MorphemeDecl {
    /// The morpheme name.
    pub name: Ident,
    /// The gloss, if written.
    pub gloss: Option<String>,
    /// Any remaining header tokens (`role given-proto`).
    pub head: Vec<String>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `craft <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CraftDecl {
    /// The craft rule name.
    pub name: Ident,
    /// The captured body clauses.
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `draft <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftDecl {
    /// The draft name.
    pub name: Ident,
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

/// `export <profile>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportDecl {
    /// The export profile name.
    pub profile: Ident,
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

/// `order-rule <name> …`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrderRuleDecl {
    /// The order-rule name.
    pub name: Ident,
    /// The captured body clauses (`: … requires …`).
    pub body: Vec<Clause>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
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

/// `ledger <name> … { … }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerDecl {
    /// The ledger name.
    pub name: Ident,
    /// Remaining header tokens (`speaker protagonist in spoken-proof`).
    pub head: Vec<String>,
    /// The captured body clauses (`premise …`, `step …`, `spend …`, `close …`).
    pub body: Vec<Clause>,
}
