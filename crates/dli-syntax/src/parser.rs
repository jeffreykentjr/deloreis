//! Parser (SURF-1).

use dli_ast::decl::Document;
use dli_ast::span::FileId;

use crate::diag::SyntaxError;

/// Parse source into the normative AST document.
pub fn parse(_source: &str, _file: FileId) -> Result<Document, SyntaxError> {
    todo!("SURF-1: real parser producing the normative AST")
}
