#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Verify that every token in `dli-world`'s `FORBIDDEN_WORLD_TOKENS` is a subset
//! of the canonical `boundary::WORLD_TOKENS`. This replaces the hand-copied
//! `CANONICAL_WORLD_TOKENS` list that lived inside `load.rs` — same invariant,
//! no duplication.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(manifest).parent().unwrap().to_path_buf()
}

/// Extract quoted string entries from the `FORBIDDEN_WORLD_TOKENS` const array
/// in the given source text. This is the same grep-and-parse-source pattern
/// `finding_families.rs` uses for code scraping.
fn extract_forbidden_tokens(source: &str) -> Vec<String> {
    let mut in_const = false;
    let mut tokens = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.contains("FORBIDDEN_WORLD_TOKENS") && trimmed.contains("&[") {
            in_const = true;
            // The opening line may also contain entries.
        }
        if in_const {
            // Extract all quoted strings on this line.
            let mut rest = trimmed;
            while let Some(start) = rest.find('"') {
                let after = &rest[start + 1..];
                if let Some(end) = after.find('"') {
                    let token = &after[..end];
                    if !token.is_empty() {
                        tokens.push(token.to_owned());
                    }
                    rest = &after[end + 1..];
                } else {
                    break;
                }
            }
            if trimmed.contains("];") {
                break;
            }
        }
    }
    tokens
}

#[test]
fn forbidden_world_tokens_are_subset_of_boundary() {
    let root = repo_root();
    let source = std::fs::read_to_string(root.join("crates/dli-world/src/craft/load.rs"))
        .expect("reading craft/load.rs");

    let forbidden = extract_forbidden_tokens(&source);
    assert!(
        !forbidden.is_empty(),
        "failed to extract any tokens from FORBIDDEN_WORLD_TOKENS in load.rs"
    );

    for token in &forbidden {
        assert!(
            xtask::boundary::WORLD_TOKENS.contains(&token.as_str()),
            "FORBIDDEN_WORLD_TOKENS entry `{token}` (from crates/dli-world/src/craft/load.rs) \
             is not in boundary::WORLD_TOKENS (xtask/src/boundary.rs) — add it there or \
             remove it from the craft lint"
        );
    }
}
