//! Canonical serialization of the AST.

use crate::decl::Document;

/// Serialize a document to canonical JSON bytes.
///
/// This is real and works today: it is the byte image the fixture registry and
/// the wire contract hash. Serialization of a plain derive tree is infallible in
/// practice; a failure here indicates a corrupt in-memory document.
#[must_use]
pub fn to_canonical_json(doc: &Document) -> Vec<u8> {
    serde_json::to_vec(doc).expect("Document is always serializable to JSON")
}
