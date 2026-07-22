//! The docs-side world-token lint: docs, the skill, and the shipped examples must
//! not name a `TPtH` world proper noun outside a legitimate worldbook cross-link.
//!
//! The engine boundary guard ([`crate::boundary`]) keeps `crates/**/src` `TPtH`-free,
//! but nothing guarded `docs/`, `SKILL.md`, or `examples/` — so a `.dli` teaching
//! block, a spec's prose, or a shipped example could quietly re-teach the generic
//! engine on one private world (`saltmarket`, `mara`, `the-shallows`, …). This lint
//! scans those surfaces for the same `WORLD_TOKENS` set the boundary guard uses
//! and fails the gate on any hit.
//!
//! Two allowlists carve out the legitimate `TPtH` mentions:
//! - **Whole paths** (`ALLOWED_PREFIXES`, `ALLOWED_FILES`) — the tpth worldbook,
//!   the tpth design-notes, and the plan/decision/migration docs that legitimately
//!   name the world are skipped entirely.
//! - **Worldbook-footer paragraphs** — a doc paragraph carrying a `worlds/tpth`
//!   cross-link is allowed to name the world it points at. The mandated spec footers
//!   describe the linked world across several lines and close with the link, so the
//!   WHOLE paragraph is exempt, not merely the line bearing the URL.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use walkdir::WalkDir;

use crate::boundary::{WORLD_TOKENS, contains_word};

/// Repo-relative path prefixes (forward slashes) whose every file legitimately
/// names `TPtH`: the worldbook renderings and the world tree itself.
const ALLOWED_PREFIXES: &[&str] = &["docs/worlds/tpth/", "worlds/tpth/"];

/// Exact repo-relative files (forward slashes) that legitimately name `TPtH`: the
/// decision ledger, the plan, the migration guide, the surface-naming record, and
/// the tpth-scoped design-notes (separation, splits, the turning/craft extractions,
/// the vendored corpus). `prior-art-licenses.md` cites `TPtH` canon file *paths*
/// (`canon/world/{…,the-turning}.md`) as prior-art evidence, not as engine teaching.
const ALLOWED_FILES: &[&str] = &[
    "docs/decisions.md",
    "docs/PLAN.md",
    "docs/integration/migration.md",
    "docs/surface-v1-naming.md",
    "docs/design-notes/tpth-separation-plan.md",
    "docs/design-notes/docs-split.md",
    "docs/design-notes/sim-genericization.md",
    "docs/design-notes/agent-authoring-validation.md",
    "docs/design-notes/tpth-turning-extraction.md",
    "docs/design-notes/craft-separation-design.md",
    "docs/design-notes/vendored-corpus.md",
    "docs/design-notes/prior-art-licenses.md",
];

/// The marker that makes a paragraph a worldbook cross-link: a `docs/worlds/tpth/`
/// link. Matched lowercase against a lowercased line.
const WORLDBOOK_LINK: &str = "worlds/tpth";

/// One leak: a forbidden world token named on a non-exempt line of a scanned doc.
struct Leak {
    file: String,
    line_no: usize,
    tokens: Vec<&'static str>,
    text: String,
}

/// Scan the docs, the skill, and the examples and fail if any world token appears
/// outside an allowlisted path or a worldbook-footer paragraph.
pub fn run_docs_tokens(root: &Path) -> Result<()> {
    let files = scan_targets(root);
    if files.is_empty() {
        bail!(
            "docs-tokens: no docs/skill/example surfaces found under {}",
            root.display()
        );
    }

    let scanned = files.len();
    let mut allowlisted = 0usize;
    let mut leaks: Vec<Leak> = Vec::new();

    for path in &files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if is_allowed_path(&rel) {
            allowlisted += 1;
            continue;
        }
        // A non-UTF-8 asset under examples/ is not a teaching surface; skip it.
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        scan_file(&rel, &text, &mut leaks);
    }

    if leaks.is_empty() {
        println!(
            "docs-tokens: OK — no TPtH world tokens in docs/skill/examples \
             ({scanned} files scanned, {allowlisted} allowlisted path(s))"
        );
        return Ok(());
    }

    for leak in &leaks {
        let names = leak
            .tokens
            .iter()
            .map(|token| format!("`{token}`"))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!(
            "docs-tokens: LEAK {}:{}: world token {names} in a teaching surface\n    {}",
            leak.file,
            leak.line_no,
            leak.text.trim()
        );
    }
    eprintln!(
        "docs-tokens: extract to worlds/tpth or genericize (swap to the camelot \
         teaching world); allowlist a legit worldbook cross-link paragraph (one that \
         links docs/worlds/tpth/…) or a TPtH-owned doc path in docs_tokens.rs."
    );
    bail!(
        "docs-tokens: {} world-token leak(s) in docs/skill/examples",
        leaks.len()
    );
}

/// The surfaces this lint owns: `SKILL.md`, every `.md` under `docs/`, and every
/// file under `examples/`. Sorted and de-duplicated for a stable report order.
fn scan_targets(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    let skill = root.join("SKILL.md");
    if skill.is_file() {
        files.push(skill);
    }

    let docs = root.join("docs");
    if docs.is_dir() {
        for entry in WalkDir::new(&docs).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                files.push(path.to_path_buf());
            }
        }
    }

    let examples = root.join("examples");
    if examples.is_dir() {
        for entry in WalkDir::new(&examples).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                files.push(path.to_path_buf());
            }
        }
    }

    files.sort();
    files.dedup();
    files
}

/// A path allowlisted wholesale: an exact `TPtH`-owned file, or anything under a
/// worldbook/world-tree prefix.
fn is_allowed_path(rel: &str) -> bool {
    ALLOWED_FILES.contains(&rel)
        || ALLOWED_PREFIXES
            .iter()
            .any(|prefix| rel.starts_with(prefix))
}

/// Scan one doc, appending any world-token leak on a line that is neither inside a
/// worldbook-footer paragraph nor otherwise exempt.
fn scan_file(rel: &str, text: &str, leaks: &mut Vec<Leak>) {
    let lines: Vec<&str> = text.lines().collect();
    let exempt = footer_exempt_lines(&lines);

    for (index, line) in lines.iter().enumerate() {
        if exempt[index] {
            continue;
        }
        let lower = line.to_lowercase();
        let hits: Vec<&'static str> = WORLD_TOKENS
            .iter()
            .copied()
            .filter(|token| contains_word(&lower, token))
            .collect();
        if !hits.is_empty() {
            leaks.push(Leak {
                file: rel.to_owned(),
                line_no: index + 1,
                tokens: hits,
                text: (*line).to_owned(),
            });
        }
    }
}

/// True when the line carries a real markdown link whose TARGET names the tpth
/// worldbook — `](…worlds/tpth…)`. A bare `worlds/tpth` mention in prose does NOT
/// count: otherwise a paragraph could launder a private lexicon by merely naming
/// the path once. The exemption requires an actual cross-link.
fn has_worldbook_link(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.match_indices("](").any(|(i, _)| {
        let after = &lower[i + 2..];
        match after.find(')') {
            Some(end) => after[..end].contains(WORLDBOOK_LINK),
            None => false,
        }
    })
}

/// Mark every line that belongs to a worldbook-footer paragraph. A paragraph is a
/// maximal run of consecutive non-blank lines; if any of its lines carries a real
/// `](…worlds/tpth…)` cross-link, the whole paragraph is a legitimate worldbook
/// pointer and every line in it is exempt.
fn footer_exempt_lines(lines: &[&str]) -> Vec<bool> {
    let mut exempt = vec![false; lines.len()];
    let mut start = 0;

    for boundary in 0..=lines.len() {
        let at_break = boundary == lines.len() || lines[boundary].trim().is_empty();
        if !at_break {
            continue;
        }
        if start < boundary {
            let links_worldbook = lines[start..boundary]
                .iter()
                .any(|line| has_worldbook_link(line));
            if links_worldbook {
                for slot in &mut exempt[start..boundary] {
                    *slot = true;
                }
            }
        }
        start = boundary + 1;
    }

    exempt
}

#[cfg(test)]
mod tests {
    use super::{footer_exempt_lines, has_worldbook_link};

    #[test]
    fn worldbook_link_requires_a_real_link_not_prose() {
        assert!(has_worldbook_link(
            "[the TPtH worldbook](../worlds/tpth/concordance.md)."
        ));
        // R-M1 F1: a bare prose mention of the path must NOT count as a link.
        assert!(!has_worldbook_link(
            "Compare the notes under worlds/tpth someday."
        ));
        // A link to something else does not launder a prose mention either.
        assert!(!has_worldbook_link(
            "[see here](../elsewhere.md) worlds/tpth"
        ));
    }

    #[test]
    fn prose_paragraph_naming_the_path_is_not_footer_exempt() {
        // The F1 laundering attempt: tokens in a paragraph that only *mentions*
        // worlds/tpth (no real cross-link) must stay non-exempt.
        let lines = vec![
            "The engine works like this: saltmarket debt and tide-bell phases.",
            "(Compare the notes under worlds/tpth someday.)",
        ];
        assert!(footer_exempt_lines(&lines).iter().all(|&e| !e));
    }

    #[test]
    fn a_real_worldbook_footer_paragraph_is_exempt() {
        let lines = vec![
            "**Worldbook.** The Concordance and its debt live in",
            "[the TPtH worldbook](../worlds/tpth/concordance.md).",
        ];
        assert!(footer_exempt_lines(&lines).iter().all(|&e| e));
    }
}
