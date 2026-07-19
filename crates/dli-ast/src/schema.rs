//! JSON Schema emission for the AST.

use crate::decl::Document;

/// The JSON Schema for the [`Document`] root.
///
/// This is real: `xtask schema` emits it to `schemas/ast/document.schema.json`
/// and the gate diffs the committed copy against a fresh render.
#[must_use]
pub fn ast_schema() -> schemars::Schema {
    schemars::schema_for!(Document)
}
