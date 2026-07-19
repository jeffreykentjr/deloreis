//! Declarations and the document root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::annot::Annotation;
use crate::ident::Ident;
use crate::span::Span;
use crate::term::AtomExpr;

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
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Decl {
    /// `use <target>` — import another pack's surface.
    Use(UseDecl),
    /// `sort <name>` — declare an entity sort.
    Sort(SortDecl),
    /// `fact <atom>` — assert a ground fact.
    Fact(FactDecl),
    /// `rule <name>: <head> :- <body>` — a derivation rule.
    Rule(RuleDecl),
    /// `fluent <name>` — a time-varying property.
    Fluent(FluentDecl),
    /// `wheel <name>` — a cyclic time structure.
    Wheel(WheelDecl),
    /// `fork <question>` — an open creative fork.
    Fork(ForkDecl),
    /// `practice <name>` — a resource-tracked practice.
    Practice(PracticeDecl),
    /// `register <name>` — a diction register.
    Register(RegisterDecl),
    /// `lexeme <name>` — a coined term.
    Lexeme(LexemeDecl),
    /// `craft <name>` — a craft rule.
    Craft(CraftDecl),
    /// `draft <name>` — a draft payload binding.
    Draft(DraftDecl),
    /// `attest <question>` — an attestation request.
    Attest(AttestDecl),
    /// `export <profile>` — an export directive.
    Export(ExportDecl),
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

/// `fact <atom>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FactDecl {
    /// The asserted atom.
    pub atom: AtomExpr,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `rule <name>: <head> :- <body>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleDecl {
    /// The rule name.
    pub name: Ident,
    /// The head atom the rule derives.
    pub head: AtomExpr,
    /// The body atoms (conjunctive premises).
    pub body: Vec<AtomExpr>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `fluent <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FluentDecl {
    /// The fluent name.
    pub name: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `wheel <name>` with its ordered spokes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WheelDecl {
    /// The wheel name.
    pub name: Ident,
    /// The ordered spoke names around the wheel.
    pub spokes: Vec<Ident>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `fork <question>` with its options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkDecl {
    /// The open question.
    pub question: Ident,
    /// The candidate options.
    pub options: Vec<Ident>,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `practice <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PracticeDecl {
    /// The practice name.
    pub name: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `register <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegisterDecl {
    /// The register name.
    pub name: Ident,
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

/// `craft <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CraftDecl {
    /// The craft rule name.
    pub name: Ident,
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}

/// `draft <name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftDecl {
    /// The draft name.
    pub name: Ident,
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
    /// Provenance annotation, if any.
    pub provenance: Option<Annotation>,
}
