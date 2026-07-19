//! Lexer (SURF-1).

use dli_ast::span::{FileId, Span};

/// A lexical token with its source span. Token kinds are filled in at SURF-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The raw lexeme text.
    pub text: String,
    /// The token's source span.
    pub span: Span,
}

/// Tokenize source into a token stream.
pub fn lex(_source: &str, _file: FileId) -> Vec<Token> {
    todo!("SURF-1: real lexer over the concrete grammar")
}
