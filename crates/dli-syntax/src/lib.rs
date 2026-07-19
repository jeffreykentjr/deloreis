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
pub mod probe;

pub use diag::{DiagClass, Diagnostic, render};
pub use format::format_document;
pub use lexer::{Token, TokenKind, lex};
pub use parser::{parse_clause_atom, parse_document};
pub use probe::{KEYWORDS, ProbeError, probe_line, probe_source};
