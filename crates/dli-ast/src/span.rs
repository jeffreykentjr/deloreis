//! Source locations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Identifier for a source file within a compilation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct FileId(pub u32);

/// A half-open byte range within a single source file.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct Span {
    /// The file this span points into.
    pub file: FileId,
    /// Start byte offset (inclusive).
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

impl Span {
    /// A synthetic span used for nodes with no source location.
    #[must_use]
    pub const fn synthetic() -> Self {
        Self {
            file: FileId(0),
            start: 0,
            end: 0,
        }
    }
}
