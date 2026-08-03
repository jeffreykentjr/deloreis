//! Deloreis Lore Interchange abstract syntax tree.
//!
//! The AST is the normative surface of the language: the concrete grammar is a
//! detail that lowers into these types. Every node carries a [`span::Span`] so
//! diagnostics can point back into source, and lore-bearing declarations carry
//! optional [`annot::Annotation`] provenance (citations and hypothesis
//! conditions).

pub mod annot;
pub mod canonical;
pub mod clause;
pub mod decl;
pub mod doc;
pub mod ident;
pub mod literal;
pub mod proof;
pub mod rule;
pub mod schema;
pub mod span;
pub mod term;

pub use annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
pub use clause::Clause;
pub use decl::{
    AttestDecl, AttrDecl, BannedDecl, BeforeDecl, CycleDecl, Decl, DialectDecl, Document,
    DraftDecl, EntityDecl, EntityKind, ExportDecl, FactDecl, ForkDecl, HypothesisDecl, KindDecl,
    NameDecl, NamePartDecl, NarrationDecl, OptionClause, PracticeDecl, PrincipleDecl, ProofDecl,
    QuestionDecl, RatifyDecl, RuleDecl, RuleDomain, ScopeHeader, StateDecl, TermDecl, TimeClause,
    UseDecl, VariantDecl, WordDecl,
};
pub use doc::Doc;
pub use ident::Ident;
pub use literal::BodyLiteral;
pub use proof::{
    Binding, MalformedOp, PhaseEq, ProofAssume, ProofBody, ProofClause, ProofClose, ProofOp,
    ProofPremise, ProofResource, ProofStep, ResourceKeyword,
};
pub use rule::{Datalog, RuleBody, RuleClause};
pub use span::{FileId, Span};
pub use term::{AtomExpr, Term, TimeRef};
