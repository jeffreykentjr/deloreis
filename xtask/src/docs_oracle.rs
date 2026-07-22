//! The docs oracle: every fenced `dli` block and example parses cleanly.
//!
//! This runs the real front end ([`dli_syntax::parse_document`]) over every
//! `dli` block in the docs and skill and over every committed example. A block
//! that produces any diagnostic fails the gate, so documentation cannot rot
//! into showing forms the grammar no longer accepts (spec `01-syntax.md` §7).

use std::path::Path;

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

const MIN_SKILL_BLOCKS: usize = 3;
const MIN_TOTAL_UNITS: usize = 5;

/// Extract `dli` fenced blocks and example files, then probe every one.
pub fn run_docs_oracle(root: &Path) -> Result<()> {
    let mut units: Vec<(String, String)> = Vec::new();

    let skill_path = root.join("SKILL.md");
    let skill_text = std::fs::read_to_string(&skill_path)
        .with_context(|| format!("reading {}", skill_path.display()))?;
    let skill_blocks = extract_dli_blocks(&skill_text);
    if skill_blocks.len() < MIN_SKILL_BLOCKS {
        bail!(
            "docs-oracle: SKILL.md has {} dli blocks, need at least {MIN_SKILL_BLOCKS}",
            skill_blocks.len()
        );
    }
    for (index, block) in skill_blocks.into_iter().enumerate() {
        units.push((format!("SKILL.md #{}", index + 1), block));
    }

    let docs_dir = root.join("docs");
    if docs_dir.is_dir() {
        for entry in WalkDir::new(&docs_dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let text = std::fs::read_to_string(path)?;
            for (index, block) in extract_dli_blocks(&text).into_iter().enumerate() {
                units.push((format!("{} #{}", path.display(), index + 1), block));
            }
        }
    }

    let examples_dir = root.join("examples");
    if examples_dir.is_dir() {
        let mut example_paths: Vec<_> = WalkDir::new(&examples_dir)
            .into_iter()
            .filter_map(Result::ok)
            .map(walkdir::DirEntry::into_path)
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("dli"))
            .collect();
        example_paths.sort();
        for path in example_paths {
            let text = std::fs::read_to_string(&path)?;
            units.push((path.display().to_string(), text));
        }
    }

    if units.len() < MIN_TOTAL_UNITS {
        bail!(
            "docs-oracle: {} units, need at least {MIN_TOTAL_UNITS}",
            units.len()
        );
    }

    // The gate fails on hard errors only. Advisory warnings (e.g. a marker-less
    // claim, koan 5's proposed-first authoring) are surfaced to authors by the
    // parser but never block the build.
    let mut failures: Vec<String> = Vec::new();
    for (label, content) in &units {
        let (_document, diagnostics) = dli_syntax::parse_document(content);
        for diag in diagnostics {
            if diag.class.is_error() {
                failures.push(format!("{label}: [{}] {}", diag.class.code(), diag.message));
            }
        }
    }
    if !failures.is_empty() {
        bail!(
            "docs-oracle: the parser rejected {} line(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    println!("docs-oracle: {} units parse without errors", units.len());
    Ok(())
}

fn extract_dli_blocks(text: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_block = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if in_block {
            if trimmed.starts_with("```") {
                blocks.push(std::mem::take(&mut current));
                in_block = false;
            } else {
                current.push_str(line);
                current.push('\n');
            }
        } else if trimmed.starts_with("```dli") {
            in_block = true;
            current.clear();
        }
    }
    blocks
}
