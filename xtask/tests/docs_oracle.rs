#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The committed examples must stay inside the gentle probe subset.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(manifest).parent().unwrap().to_path_buf()
}

#[test]
fn probe_accepts_committed_examples() {
    let examples = repo_root().join("examples");
    let mut saw_example = false;
    for entry in std::fs::read_dir(&examples).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("dli") {
            continue;
        }
        saw_example = true;
        let content = std::fs::read_to_string(&path).unwrap();
        let errors = dli_syntax::probe_source(&content);
        assert!(
            errors.is_empty(),
            "probe rejected {}: {errors:?}",
            path.display()
        );
    }
    assert!(saw_example, "expected at least one example .dli file");
}
