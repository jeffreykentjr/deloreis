//! Syntax diagnostics.

use miette::Diagnostic;
use thiserror::Error;

/// A syntax error produced by the lexer or parser (SURF-1). Modeled as a miette
/// [`Diagnostic`] now so downstream error plumbing is in place before the real
/// front end lands.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(dli::syntax::error))]
pub struct SyntaxError {
    /// A human-readable description of the problem.
    pub message: String,
}
