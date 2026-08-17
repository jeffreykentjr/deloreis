//! Declarations and the document root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::annot::Annotation;
use crate::clause::Clause;
use crate::ident::Ident;
use crate::language::{DialectBody, NameBody, RatifyBody};
use crate::practice::PracticeBody;
use crate::proof::ProofBody;
use crate::rule::RuleBody;
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
    /// `same-as <left> = <right>` — an identity claim: `left` and `right` name
    /// the same bearer.
    SameAs(SameAsDecl),
    /// A predicate identity claim: `left` and `right` name the same property.
    ///
    /// This is structured carriage for the semantic authoring door. It has no
    /// parser or formatter representation; callers construct it through the
    /// typed wire surface and the evaluator records it as a defeasible claim.
    PredicateSameAs(PredicateSameAsDecl),
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
    /// `functional <pred>` — declare a predicate single-valued (a functional
    /// dependency): each subject may carry at most one certain value for it.
    Functional(FunctionalDecl),
    /// `inertial <pred>` — declare a predicate's fluent values persistent: a
    /// value asserted `since t` holds forward until a later, distinct, live
    /// value defeats it (contract 1.8.0).
    Inertial(InertialDecl),
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

/// `same-as <left> = <right>`.
///
/// An exact structural mirror of [`BeforeDecl`]: two entity names and an
/// optional provenance annotation. Unlike `before`, the relation is undirected
/// — `left`/`right` name the two sides of a claim, not an earlier/later pair —
/// but the shape is kept parallel because the two declarations are carried,
/// gated, and lowered through the identical pipeline stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SameAsDecl {
    /// One side of the identity claim.
    pub left: Ident,
    /// The other side of the identity claim.
    pub right: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// A predicate identity claim carried by the typed authoring surface.
///
/// The declaration is deliberately a structural mirror of [`SameAsDecl`], but
/// it lives in its own enum variant and evaluator channel. Predicate identity
/// is never inferred from names, values, or data shape: this declaration is
/// the only input that can assert it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PredicateSameAsDecl {
    /// One predicate name in the identity claim.
    pub left: Ident,
    /// The other predicate name in the identity claim.
    pub right: Ident,
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

/// `rule <name> [over <domain>] …` — header plus its typed body (LS-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDecl {
    /// The rule name.
    pub name: Ident,
    /// The domain the rule is enforced over: `None` is a world rule, `over order`
    /// an episode-ordering constraint, `over season` a season-ledger rule.
    pub domain: Option<RuleDomain>,
    /// Header tokens after the name (and any `over <domain>`), before the body
    /// (e.g. `severity error`, or the inline `head : body` datalog tokens).
    pub head: Vec<String>,
    /// The typed body: world-rule datalog pre-parsed into
    /// [`Datalog`](crate::rule::Datalog); order/season clauses retained as text
    /// for their domain engines (LS-9, `docs/design-notes/lorestore-roadmap.md`).
    #[serde(default, skip_serializing_if = "RuleBody::is_empty")]
    pub body: RuleBody,
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

impl StateDecl {
    /// The ground atom this state lowers to, or `None` when the declaration
    /// names nothing evaluable (an undotted name with no value).
    ///
    /// A state mirrors an attribute so it becomes queryable: a dotted name
    /// `subject.pred` with value `v` lowers to `pred(subject, v)` — the same atom
    /// `subject.pred = v` would produce (issues 47/48, DL-0029). The *interval*
    /// over which it holds rides its fluent timeline and provenance, not the
    /// atom, so the value does not collapse into the timeless attribute layer.
    /// The dotted name is split on its first `.`, matching the subject
    /// resolution the authoring validator already performs.
    ///
    /// - dotted `s.p` with value `v` → `p(s, v)`
    /// - dotted `s.p`, no value → `p(s)` (the bare holding of a unary fluent)
    /// - undotted `n` with value `v` → `n(v)`
    /// - undotted `n`, no value → `None` (nothing to assert; callers record a
    ///   finding rather than dropping it silently — charter koan 4)
    ///
    /// This is the single lowering seam the loader, the evaluator, and the
    /// durable store all share, so their ground atoms cannot drift apart.
    #[must_use]
    pub fn atom(&self) -> Option<AtomExpr> {
        match self.name.text.split_once('.') {
            Some((subject, pred)) => {
                let mut args = vec![Term::Sym(Ident {
                    text: subject.to_owned(),
                    span: self.name.span,
                })];
                if let Some(value) = &self.value {
                    args.push(value.clone());
                }
                Some(AtomExpr {
                    pred: Ident {
                        text: pred.to_owned(),
                        span: self.name.span,
                    },
                    args,
                })
            }
            None => Some(AtomExpr {
                pred: self.name.clone(),
                args: vec![self.value.clone()?],
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicate_identity_is_structured_carriage_only() {
        let declaration = Decl::PredicateSameAs(PredicateSameAsDecl {
            left: Ident::synthetic("current-work"),
            right: Ident::synthetic("current-job"),
            provenance: None,
        });
        let bytes = serde_json::to_vec(&declaration).expect("serializes");
        let round_trip: Decl = serde_json::from_slice(&bytes).expect("deserializes");
        assert_eq!(declaration, round_trip);
        assert!(
            String::from_utf8(bytes)
                .expect("utf8")
                .contains("PredicateSameAs")
        );
    }
}

/// `functional <pred>` — a predicate-level single-valuedness constraint.
///
/// This declares that `pred` is a functional dependency: a subject holds at most
/// one certain value for it, so two distinct certain values are a contradiction
/// (`continuity.fd-violation`). It carries no ground atom — like a `ratify`, it is
/// a carriage-only declaration whose provenance rides its serialized body, never
/// the interner (so the three lowerings stay byte-identical, DL-0071). It lets an
/// author judge a *first-seen* predicate single-valued, which canon precedent
/// alone cannot (issue 49, sub-claim 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FunctionalDecl {
    /// The predicate declared single-valued.
    pub pred: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `inertial <pred>` — a predicate-level fluent-persistence declaration.
///
/// This declares that `pred` is **inertial**: a fluent value asserted `since
/// t` with no authored `until` persists forward until a later, distinct,
/// live value on the same slot defeats it (`docs/spec/03-time.md`, the
/// inertia design note). Per-predicate, mirroring [`FunctionalDecl`]. It
/// carries no ground atom — like `functional`, it is a carriage-only
/// declaration whose provenance rides its serialized body, never the
/// interner, so the three lowerings stay byte-identical. Declaring a
/// predicate inertial is a defeasible claim: it can be cited, and it can be
/// defeated, reverting the slot to plain open-interval projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InertialDecl {
    /// The predicate declared inertial (fluent-persistent).
    pub pred: Ident,
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
    /// The typed body: the retained interior clauses plus the structured
    /// [`PracticeDef`](crate::practice::PracticeDef), pre-parsed by the front end
    /// (LS-9, `docs/design-notes/lorestore-roadmap.md`). `dli-ledger` reads the
    /// typed structure without re-entering `dli_syntax`.
    pub body: PracticeBody,
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
    /// The typed interior: each interior clause pre-classified into a
    /// [`DialectItem`](crate::language::DialectItem) at parse time (LS-9), so the
    /// language compiler reads typed AST instead of re-lexing clause text.
    #[serde(default, skip_serializing_if = "DialectBody::is_empty")]
    pub body: DialectBody,
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
    /// The typed interior: each interior clause (composition, bearer, provenance)
    /// pre-classified into a [`NameItem`](crate::language::NameItem) at parse
    /// time (LS-9), so the name compiler reads typed AST instead of re-lexing
    /// clause text.
    #[serde(default, skip_serializing_if = "NameBody::is_empty")]
    pub body: NameBody,
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
    /// The typed interior: the `h-cymric := true` valuation lines and the
    /// decision ids the body cites, structured at parse time (LS-9), so the
    /// language compiler and the citation gate read typed AST instead of
    /// re-lexing clause text.
    #[serde(default, skip_serializing_if = "RatifyBody::is_empty")]
    pub body: RatifyBody,
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

/// `proof <name> … { … }` — header plus its typed body (LS-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofDecl {
    /// The proof name.
    pub name: Ident,
    /// Remaining header tokens (`speaker protagonist in spoken-proof`).
    pub head: Vec<String>,
    /// The typed body: each interior line pre-parsed into a
    /// [`ProofOp`](crate::proof::ProofOp) by the front end, with the normalized
    /// clause text retained for the formatter (LS-9,
    /// `docs/design-notes/lorestore-roadmap.md`). `dli-ledger` and `dli-world`
    /// read this typed interior without re-entering the parser.
    #[serde(default, skip_serializing_if = "ProofBody::is_empty")]
    pub body: ProofBody,
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
