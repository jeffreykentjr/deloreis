//! Deloreis Lore Interchange concrete syntax: the lexer, the recursive-descent
//! parser, the canonical formatter, and the domain-spoken diagnostics.
//!
//! The two frozen front-end seams are [`parse_document`] (surface text → the
//! normative [`dli_ast::Document`] plus diagnostics) and [`format_document`]
//! (an AST back to the one canonical surface form). They round-trip: parsing
//! formatted output yields the same document modulo source spans, and the
//! formatter is idempotent (charter koan 7; DL-0003).
//!
//! [`probe`] is the pre-parser relic — a coarse gentle-subset gate the
//! docs-oracle used before a real parser existed. It is kept only because a
//! sibling gate still calls [`probe::probe_source`]; the real oracle now runs
//! [`parse_document`].

pub mod diag;
pub mod format;
pub mod ladder;
pub mod lexer;
pub mod parser;
pub mod practice;
pub mod probe;

pub use diag::{DiagClass, Diagnostic, render};
pub use format::{format_document, format_source};
pub use lexer::{Token, TokenKind, lex};
pub use parser::{
    LINE_OPEN_KEYWORDS, line_open_keywords, parse_clause_atom, parse_document,
    parse_proof_arguments, parse_proof_bindings, parse_proof_close_parts, parse_rule_clause,
    structure_rule_body,
};
pub use practice::structure_practice_body;
pub use probe::{KEYWORDS, ProbeError, probe_line, probe_source};

/// Revalidate a proof AST through the canonical surface and real parser.
///
/// Parser-backed callers already crossed this boundary. Direct AST callers use
/// this replay to prove that captured proof clauses still form accepted syntax;
/// no second proof grammar is maintained by downstream checker crates.
#[must_use]
pub fn validate_proof_decl(proof: &dli_ast::decl::ProofDecl) -> Vec<Diagnostic> {
    let document = dli_ast::decl::Document {
        scope: None,
        decls: vec![dli_ast::decl::Decl::Proof(proof.clone())],
    };
    let surface = format_document(&document);
    parse_document(&surface).1
}
