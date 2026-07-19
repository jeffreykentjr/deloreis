//! Packs: named bundles of source documents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::decl::Document;

/// A named bundle of documents compiled together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Pack {
    /// The pack name.
    pub name: String,
    /// The documents in the pack.
    pub documents: Vec<Document>,
}
