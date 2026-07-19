//! Documentation attached to declarations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::span::Span;

/// A documentation comment attached to a declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Doc {
    /// The prose body of the comment, with markers stripped.
    pub text: String,
    /// Where the comment appears in source.
    pub span: Span,
}
