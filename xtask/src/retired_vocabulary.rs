//! Guard the active teaching surfaces against the retired authority vocabulary.
//!
//! Historical ledger and design-note records may describe the old behavior. The
//! live docs, cards, skill, and examples must teach the current citation-integrity
//! model instead.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use walkdir::WalkDir;

const RETIRED: &[&str] = &[
    "cited-refused",
    "human-only",
    "sign-off is ratification",
    "only a human ruling",
    "door h",
];

const ALLOWED_FILES: &[&str] = &["docs/decisions.md", "docs/integration/migration.md"];

/// Scan one teaching surface and return `(line, token)` hits.
#[must_use]
pub fn scan_text(text: &str) -> Vec<(usize, String)> {
    let mut hits = text
        .lines()
        .enumerate()
        .flat_map(|(index, line)| {
            let lower = line.to_lowercase();
            RETIRED.iter().filter_map(move |token| {
                lower
                    .find(token)
                    .map(|position| (index + 1, position, (*token).to_owned()))
            })
        })
        .collect::<Vec<_>>();
    hits.sort_by_key(|(line, position, _)| (*line, *position));
    hits.into_iter()
        .map(|(line, _position, token)| (line, token))
        .collect()
}

/// Run the retired-vocabulary lint over docs and `SKILL.md`.
pub fn run_retired_vocabulary(root: &Path) -> Result<()> {
    let files = scan_targets(root);
    if files.is_empty() {
        bail!(
            "retired-vocabulary: no docs or SKILL.md found under {}",
            root.display()
        );
    }

    let mut violations = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if is_allowlisted_file(&rel) {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        for (line, token) in scan_text(&text) {
            if is_allowlisted_line(&rel, line) {
                continue;
            }
            violations.push((rel.clone(), line, token));
        }
    }

    if violations.is_empty() {
        println!(
            "retired-vocabulary: OK — active teaching surfaces contain no retired authority terms"
        );
        return Ok(());
    }

    for (path, line, token) in &violations {
        eprintln!("retired-vocabulary: {path}:{line}: retired term `{token}`");
    }
    bail!(
        "retired-vocabulary: {} retired term occurrence(s); update the active teaching surface",
        violations.len()
    );
}

fn scan_targets(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let skill = root.join("SKILL.md");
    if skill.is_file() {
        files.push(skill);
    }
    let docs = root.join("docs");
    if docs.is_dir() {
        for entry in WalkDir::new(docs).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                files.push(path.to_owned());
            }
        }
    }
    files.sort();
    files
}

fn is_allowlisted_file(rel: &str) -> bool {
    ALLOWED_FILES.contains(&rel) || rel.starts_with("docs/design-notes/")
}

fn is_allowlisted_line(rel: &str, line: usize) -> bool {
    // The exact legacy wire code is intentionally retained in the two-row
    // compatibility table in spec/11; all surrounding teaching prose uses the
    // current citation-integrity vocabulary.
    (rel == "docs/spec/11-agent-authoring.md" && (568..=571).contains(&line))
        // Generated finding references must retain the exact legacy wire code;
        // the surrounding prose is scrubbed and spec/11 carries the teaching table.
        || (rel == "docs/reference-generated/finding-families.md" && line == 273)
}
