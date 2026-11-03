#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Golden coverage of parsed AST documents.

/// The generic `camelot` example, embedded at compile time so the golden is a
/// single source of truth with `examples/camelot.dli` and needs no runtime CWD.
const CAMELOT_DLI: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/camelot.dli"
));

#[test]
fn golden_document_matches_committed_snapshot() {
    // SURF-1 has landed: a concrete-syntax document lowers deterministically into
    // these AST types. Parse the shipped generic `camelot` world and assert the
    // lowered `Document` matches the committed golden AST snapshot. Advisory
    // warnings (origin-missing and the like) are legal surface, never a parse
    // failure — only error-class diagnostics would indicate a broken lowering.
    let (document, diagnostics) = dli_syntax::parse_document(CAMELOT_DLI);
    let errors: Vec<&dli_syntax::diag::Diagnostic> =
        diagnostics.iter().filter(|d| d.class.is_error()).collect();
    assert!(errors.is_empty(), "unexpected parse errors: {errors:?}");

    // The committed `.snap` fixture IS the golden AST: a change in the parser, the
    // AST types, or the example that alters the lowering shows up as a snapshot
    // diff and fails the gate. Regenerate deliberately with `cargo insta review`.
    insta::assert_json_snapshot!(document);
}
