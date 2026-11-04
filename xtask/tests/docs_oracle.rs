#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The committed examples must parse as real De Lore Is documents.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(manifest).parent().unwrap().to_path_buf()
}

#[test]
fn parser_accepts_committed_examples() {
    let examples = repo_root().join("examples");
    let mut saw_example = false;
    for entry in walkdir::WalkDir::new(&examples)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path().to_path_buf();
        if path.extension().and_then(|ext| ext.to_str()) != Some("dli") {
            continue;
        }
        saw_example = true;
        let content = std::fs::read_to_string(&path).unwrap();
        let (_, diagnostics) = dli_syntax::parse_document(&content);
        let errors: Vec<_> = diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.class.is_error())
            .collect();
        assert!(
            errors.is_empty(),
            "parser rejected {}: {errors:?}",
            path.display()
        );
    }
    assert!(saw_example, "expected at least one example .dli file");
}
