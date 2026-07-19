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
pub mod schema;
pub mod span;
pub mod term;

pub use annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
pub use clause::Clause;
pub use decl::{
    AttestDecl, AttrDecl, BannedDecl, BeforeDecl, CraftDecl, Decl, Document, DraftDecl, EntityDecl,
    EntityKind, ExportDecl, FactDecl, FluentDecl, ForkDecl, HypothesisDecl, LedgerDecl, LexemeDecl,
    MorphemeDecl, NameDecl, NarrationDecl, OptionClause, OrderRuleDecl, PackHeader, PracticeDecl,
    PrincipleDecl, QuestionDecl, RatifyDecl, RegisterDecl, RuleDecl, SortDecl, TermDecl,
    TimeClause, UseDecl, VariantDecl, WheelDecl,
};
pub use doc::Doc;
pub use ident::Ident;
pub use span::{FileId, Span};
pub use term::{AtomExpr, Term, TimeRef};
