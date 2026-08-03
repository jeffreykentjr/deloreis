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
pub use format::{format_document, format_source};
pub use lexer::{Token, TokenKind, lex};
pub use parser::{
    LINE_OPEN_KEYWORDS, line_open_keywords, parse_clause_atom, parse_document,
    parse_proof_arguments, parse_proof_bindings, parse_proof_close_parts, parse_rule_clause,
    structure_dialect_body, structure_name_body, structure_proof_body,
    structure_ratify_body, structure_rule_body,
};
pub use probe::{KEYWORDS, ProbeError, probe_line, probe_source};

// `validate_proof_decl` — the format-and-reparse round-trip that revalidated a
// proof AST through the canonical surface — was retired by LS-9: the proof
// interior is now typed at parse time ([`parser::structure_proof_body`]), and a
// malformed operation is captured as `dli_ast::proof::ProofOp::Malformed`. The
// checker refuses those structurally (`dli_ledger::ledger_check`), so no second
// proof grammar and no reparse are maintained downstream (roadmap LS-9,
// "a semantic check implemented as syntax — retire it").
