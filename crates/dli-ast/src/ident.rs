//! Identifiers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::span::Span;

/// A named identifier with its source span.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct Ident {
    /// The identifier text as written.
    pub text: String,
    /// Where the identifier appears in source.
    pub span: Span,
}

impl Ident {
    /// Construct an identifier at a synthetic span.
    #[must_use]
    pub fn synthetic(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            span: Span::synthetic(),
        }
    }
}
