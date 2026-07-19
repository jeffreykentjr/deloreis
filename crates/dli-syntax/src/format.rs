//! Formatter (SURF-1).

use dli_ast::decl::Document;

/// Render a document back to canonical concrete syntax. Must round-trip with
/// [`crate::parser::parse`] once both are real.
pub fn format(_document: &Document) -> String {
    todo!("SURF-1: pretty-printer that round-trips with the parser")
}
