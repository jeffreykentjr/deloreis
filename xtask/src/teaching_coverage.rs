//! The teaching-coverage gate: keeps the agent-facing authoring surface honest
//! against the shipped wire.
//!
//! WS-5's finding was that two inventories disagreed by nine operations: the
//! wire (`Op::ALL`) and what `SKILL.md` / the cards / spec §11 actually teach.
//! No gate lane checked that gap, so it widened silently release after release
//! (DL-0053: "a stale map is worse than none, because it is trusted"). This
//! lane is the structural fix — it lands *before* any op-adding PR so those
//! PRs' own doc updates are enforced rather than merely checklisted.
//!
//! Modeled on two house patterns: `retired_vocabulary.rs`'s scan-teaching-
//! surfaces-and-report shape, and `finding_families.rs`'s discipline of
//! checking authored text against a live source of truth (`Op::ALL` and
//! `Op::summary()`) rather than a second hand-maintained
//! table. Four checks, all against the same source of truth:
//!
//! (a) every `author.*` op's wire name is named in at least one author-facing
//!     teaching surface (`SKILL.md`, `docs/cards/*.md`,
//!     `docs/spec/11-agent-authoring.md`), and every op's wire name is named in
//!     the generated `docs/reference-generated/ops.md`;
//! (b) the tool-count lines this PR wrote into `SKILL.md` and spec §11 agree
//!     with `Op::ALL`'s actual counts;
//! (c) best-effort: every fenced ` ```json ` example frame carrying a `"dli"`
//!     version field negotiates a version in `ACCEPTED_CONTRACT_VERSIONS`;
//! (d) a documented allowlist for deliberate, principled omissions.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use walkdir::WalkDir;

use dli_serve::{ACCEPTED_CONTRACT_VERSIONS, Op};

/// The author-facing teaching surfaces every `author.*` op's wire name must
/// appear in at least one of (check (a)). Repo-relative; `docs/cards` expands
/// to every file in that directory.
const AUTHOR_TEACHING_FILES: &[&str] = &["SKILL.md", "docs/spec/11-agent-authoring.md"];
const AUTHOR_TEACHING_DIRS: &[&str] = &["docs/cards"];

/// The generated reference page check (a) also requires every op — `author.*`
/// and `dli.*` alike — to be named in.
const OPS_REFERENCE_PAGE: &str = "docs/reference-generated/ops.md";

/// Doc directories excluded from the fenced-`"dli"`-version scan (check (c)):
/// design notes are settled historical reasoning, not a live teaching surface,
/// and are excluded from `retired-vocabulary` on the same principle.
const FENCE_SCAN_EXCLUDED_DIRS: &[&str] = &["docs/design-notes"];

/// `author.*` ops deliberately not required to appear in an author-facing
/// teaching surface, each with its justification. Empty today — PR-0a's
/// currency pass brought every author op into `SKILL.md` and/or spec §11 — but
/// the mechanism stays so a future principled exception is a documented one
/// line, not a silent gate weakening.
const AUTHOR_OP_ALLOWLIST: &[(&str, &str)] = &[
    // (none yet)
];

/// Run the teaching-coverage lane.
pub fn run_teaching_coverage(root: &Path) -> Result<()> {
    let mut problems: Vec<String> = Vec::new();

    problems.extend(check_author_ops_are_taught(root)?);
    problems.extend(check_ops_reference_page_names_every_op(root)?);
    problems.extend(check_stated_tool_counts(root)?);
    problems.extend(check_fenced_dli_versions(root));

    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("teaching-coverage: {problem}");
        }
        bail!(
            "teaching-coverage: {} problem(s); the agent-facing teaching surface has drifted \
             from the wire (Op::ALL) — update SKILL.md / docs/cards / docs/spec/11-agent-authoring.md",
            problems.len()
        );
    }

    println!(
        "teaching-coverage: OK — every op is taught, the core profile and stated count agree, \
         fenced dli versions are accepted"
    );
    Ok(())
}

/// Every `author.*` op in [`Op::ALL`] must be named, by exact wire string, in
/// at least one of `AUTHOR_TEACHING_FILES` / `AUTHOR_TEACHING_DIRS`, unless
/// allowlisted. `dli.*` ops are exempt from this check by construction — the
/// authoring manual states outright (`SKILL.md` §2: "you never touch a
/// `dli.*` batch op ... to author *or* self-check") that the batch family is
/// not part of the tool surface an author reaches for; they are still
/// required in the generated reference page (`check_ops_reference_page_names_every_op`).
fn check_author_ops_are_taught(root: &Path) -> Result<Vec<String>> {
    let files = author_teaching_files(root);
    let mut combined = String::new();
    for path in &files {
        combined.push_str(&std::fs::read_to_string(path)?);
        combined.push('\n');
    }

    let allowlist: std::collections::BTreeMap<&str, &str> =
        AUTHOR_OP_ALLOWLIST.iter().copied().collect();

    let mut problems = Vec::new();
    for &op in &Op::ALL {
        let wire = wire_name(op);
        if !wire.starts_with("author.") {
            continue;
        }
        if allowlist.contains_key(wire.as_str()) {
            continue;
        }
        if !mentions_token(&combined, &wire) {
            problems.push(format!(
                "`{wire}` is not named in any of {AUTHOR_TEACHING_FILES:?} or {AUTHOR_TEACHING_DIRS:?} \
                 (add it, or add a justified entry to AUTHOR_OP_ALLOWLIST)"
            ));
        }
    }
    Ok(problems)
}

/// Every op — `author.*` and `dli.*` alike — must be named in the generated
/// `ops.md`. This is trivially true by construction (docgen renders it
/// straight from `Op::ALL`), so this catches a docgen regression, not a doc
/// author's oversight.
fn check_ops_reference_page_names_every_op(root: &Path) -> Result<Vec<String>> {
    let path = root.join(OPS_REFERENCE_PAGE);
    let text = std::fs::read_to_string(&path)?;
    let mut problems = Vec::new();
    for &op in &Op::ALL {
        let wire = wire_name(op);
        if !mentions_token(&text, &wire) {
            problems.push(format!(
                "`{wire}` is not named in {OPS_REFERENCE_PAGE} — regenerate with `cargo run -p xtask --features syntax -- docgen`"
            ));
        }
    }
    Ok(problems)
}

/// Whether `token` occurs in `haystack` as a whole wire-name mention, not
/// merely as a substring of a longer sibling wire name. Wire names are built
/// from `[A-Za-z0-9._-]`, so e.g. `author.add_name` is a literal substring of
/// `author.add_name_part`, and `author.open` of `author.open_question`; a raw
/// `str::contains` would let the longer sibling's mention silently satisfy
/// the shorter op's coverage requirement (red-team rt-0a, SEV-C). A match
/// counts only when the character immediately before and after it — treating
/// the start/end of `haystack` as a boundary — is outside that set.
fn mentions_token(haystack: &str, token: &str) -> bool {
    fn is_wire_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
    }

    let mut search_from = 0;
    while let Some(offset) = haystack
        .get(search_from..)
        .and_then(|rest| rest.find(token))
    {
        let match_start = search_from + offset;
        let match_end = match_start + token.len();
        let before_is_boundary = haystack[..match_start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_wire_char(c));
        let after_is_boundary = haystack[match_end..]
            .chars()
            .next()
            .is_none_or(|c| !is_wire_char(c));
        if before_is_boundary && after_is_boundary {
            return true;
        }
        // Advance past this (rejected) match's start by one byte and keep
        // scanning — a token is ASCII, so `+1` always lands on a boundary.
        search_from = match_start + 1;
    }
    false
}

/// Check the default MCP profile is taught and the specification's stated
/// operation count agrees with `Op::ALL`.
fn check_stated_tool_counts(root: &Path) -> Result<Vec<String>> {
    let total_op_count = Op::ALL.len();

    let mut problems = Vec::new();

    let skill_path = root.join("SKILL.md");
    let skill = std::fs::read_to_string(&skill_path)?;
    problems.extend(check_core_profile(&skill)?);

    let spec_path = root.join("docs/spec/11-agent-authoring.md");
    let spec = std::fs::read_to_string(&spec_path)?;
    check_line(
        &mut problems,
        "docs/spec/11-agent-authoring.md",
        &spec,
        &format!(
            "The implemented `2.0.0` surface enforces that rule for all {total_op_count} operations."
        ),
    );

    Ok(problems)
}

/// Compare the manual with the actual default MCP advertisement, not a
/// hard-coded count or wording in the prose.
fn check_core_profile(skill: &str) -> Result<Vec<String>> {
    let mut sessions = dli_serve::session::Sessions::new();
    let mut author = dli_serve::author::store::Store::default();
    let reply = dli_serve::mcp::handle_rpc(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        &mut sessions,
        &mut author,
    )
    .ok_or_else(|| anyhow::anyhow!("core MCP profile did not respond"))?;
    let response: serde_json::Value = serde_json::from_str(&reply)?;
    let tools = response["result"]["tools"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("core MCP profile has no tool list"))?;
    if tools.is_empty() {
        anyhow::bail!("core MCP profile is empty");
    }
    let mut advertised = std::collections::BTreeSet::new();
    for tool in tools {
        let name = tool["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("core MCP tool lacks a name"))?;
        if !advertised.insert(name.to_owned()) {
            anyhow::bail!("duplicate core MCP tool `{name}`");
        }
    }
    let section = skill
        .split_once("## Core tools\n")
        .ok_or_else(|| anyhow::anyhow!("SKILL.md lacks the core tool classification"))?
        .1;
    let mut listing = String::new();
    let mut started = false;
    let mut list_started = false;
    for line in section.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with("- ") {
            list_started = true;
        } else if (list_started && !line.starts_with("  "))
            || (started && !list_started && line.starts_with("## "))
        {
            break;
        }
        started = true;
        listing.push_str(line);
        listing.push('\n');
    }
    let documented: std::collections::BTreeSet<String> = listing
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|name| name.starts_with("author.") || name.starts_with("dli."))
        .map(str::to_owned)
        .collect();
    let mut problems = Vec::new();
    for missing in advertised.difference(&documented) {
        problems.push(format!(
            "SKILL.md: default MCP tool `{missing}` is missing from the core list"
        ));
    }
    for extra in documented.difference(&advertised) {
        problems.push(format!(
            "SKILL.md: `{extra}` is listed as core but is not advertised"
        ));
    }
    Ok(problems)
}

fn check_line(problems: &mut Vec<String>, file: &str, text: &str, expected_substring: &str) {
    if !text.contains(expected_substring) {
        problems.push(format!(
            "{file}: expected the exact line `{expected_substring}` (the stated tool \
             count has drifted from Op::ALL, or the wording moved without updating \
             xtask/src/teaching_coverage.rs)"
        ));
    }
}

/// Check (c), best-effort: scan every doc (outside `FENCE_SCAN_EXCLUDED_DIRS`)
/// for fenced ` ```json ` blocks (bare, `transcript`-tagged, or schema-tagged —
/// any block whose fence's first word is `json`), and inside each, every
/// `"dli": "<version>"` value must be a member of `ACCEPTED_CONTRACT_VERSIONS`.
/// This is a text scan, not a JSON parse — the docs' frames are sometimes
/// elided sketches (`…`, `→`/`←` markers) that are not valid JSON on their own,
/// so a real parser would need to special-case them anyway. Per-op version
/// floors are NOT checked here (deliberately left as a follow-up — see the
/// module doc); a fragile per-op parser is not worth blocking this PR on.
fn check_fenced_dli_versions(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for path in all_doc_files(root) {
        let rel = repo_relative(root, &path);
        if FENCE_SCAN_EXCLUDED_DIRS
            .iter()
            .any(|dir| is_within_dir(&rel, dir))
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for fence in extract_json_fences(&rel, &text) {
            for version in dli_versions_in(&fence.body) {
                if !ACCEPTED_CONTRACT_VERSIONS.contains(&version.as_str()) {
                    problems.push(format!(
                        "{}: fenced example negotiates `\"dli\": \"{version}\"`, which is not in \
                         ACCEPTED_CONTRACT_VERSIONS",
                        fence.label
                    ));
                }
            }
        }
    }
    problems
}

/// One fenced ` ```json ` block: its origin label (`path:line` of the opening
/// fence) and body text.
struct JsonFence {
    label: String,
    body: String,
}

/// Extract every fenced block whose info string's first word is `json` (bare,
/// `transcript`, or a schema tag — the check here does not care which).
/// Mirrors `docs_oracle::extract_fences`'s line-scan algorithm at the narrower
/// scope this lane needs.
fn extract_json_fences(origin: &str, text: &str) -> Vec<JsonFence> {
    let mut fences = Vec::new();
    let mut body = String::new();
    let mut open: Option<(usize, bool)> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some((start, is_json)) = open.take() {
            if trimmed.starts_with("```") {
                if is_json {
                    fences.push(JsonFence {
                        label: format!("{origin}:{start}"),
                        body: std::mem::take(&mut body),
                    });
                }
                body.clear();
            } else {
                body.push_str(line);
                body.push('\n');
                open = Some((start, is_json));
            }
        } else if let Some(info) = trimmed.strip_prefix("```") {
            let lang = info.split_whitespace().next().unwrap_or_default();
            open = Some((index + 1, lang == "json"));
        }
    }
    fences
}

/// Pull every `"dli": "<version>"` value out of a fenced block's body.
fn dli_versions_in(body: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for line in body.lines() {
        let mut rest = line;
        while let Some(pos) = rest.find("\"dli\"") {
            rest = &rest[pos + 5..];
            let Some(colon) = rest.find(':') else { break };
            rest = &rest[colon + 1..];
            let Some(open_quote) = rest.find('"') else {
                break;
            };
            rest = &rest[open_quote + 1..];
            let Some(close_quote) = rest.find('"') else {
                break;
            };
            hits.push(rest[..close_quote].to_owned());
            rest = &rest[close_quote + 1..];
        }
    }
    hits
}

/// The wire identifier for an op — its serde rename, the contract's name.
fn wire_name(op: Op) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{op:?}"))
}

fn repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Whether repo-relative path `rel` is `dir` itself or lives under it — a
/// proper path-segment check, not `starts_with(dir)` (which would also match
/// a sibling directory whose name merely has `dir` as a string prefix, e.g.
/// `docs/design-notes-sibling/`; red-team rt-0a, SEV-C).
fn is_within_dir(rel: &str, dir: &str) -> bool {
    rel == dir || rel.starts_with(&format!("{dir}/"))
}

/// `AUTHOR_TEACHING_FILES` plus every file under each of `AUTHOR_TEACHING_DIRS`.
fn author_teaching_files(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = AUTHOR_TEACHING_FILES
        .iter()
        .map(|file| root.join(file))
        .collect();
    for dir in AUTHOR_TEACHING_DIRS {
        let dir = root.join(dir);
        if !dir.is_dir() {
            continue;
        }
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                files.push(path.to_owned());
            }
        }
    }
    files.sort();
    files
}

/// `SKILL.md` plus every `docs/**/*.md` file — the same universe
/// `retired_vocabulary`'s scan uses — for the fenced-version scan.
fn all_doc_files(root: &Path) -> Vec<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// rt-0a repro: a teaching text that mentions only the longer sibling
    /// wire name must NOT count as coverage for the shorter one. Covers all
    /// three collisions the finding named: `add_name`/`add_name_part`,
    /// `open`/`open_question`, and `dli.check`/`dli.check-proof`.
    #[test]
    fn mentions_token_does_not_bleed_across_wire_name_prefixes() {
        assert!(!mentions_token(
            "call `author.add_name_part` to declare a morpheme",
            "author.add_name"
        ));
        assert!(!mentions_token(
            "`author.open_question` files a question",
            "author.open"
        ));
        assert!(!mentions_token(
            "`dli.check-proof` checks a proof block",
            "dli.check"
        ));
    }

    /// The same helper must still recognize a genuine standalone mention,
    /// including one immediately followed by punctuation rather than
    /// whitespace (backtick, parenthesis, end of string).
    #[test]
    fn core_profile_requires_exact_classification() {
        let skill = include_str!("../../SKILL.md");
        let without = skill.replacen("inspection: `author.open`,", "inspection:", 1);
        assert!(
            without.contains("`author.open`"),
            "the loop still mentions it"
        );
        let missing = check_core_profile(&without).expect("core profile is available");
        assert!(
            missing
                .iter()
                .any(|problem| problem.contains("author.open"))
        );
        let extra = skill.replacen(
            "- Acceptance:",
            "- Extra: `author.branch`.\n- Acceptance:",
            1,
        );
        let extra = check_core_profile(&extra).expect("core profile is available");
        assert!(
            extra
                .iter()
                .any(|problem| problem.contains("author.branch"))
        );
    }

    #[test]
    fn mentions_token_still_matches_a_real_standalone_mention() {
        assert!(mentions_token(
            "call `author.add_name` to compose one",
            "author.add_name"
        ));
        assert!(mentions_token("(`author.open`)", "author.open"));
        assert!(mentions_token("author.open", "author.open"));
        // A real mention alongside the longer sibling in the same text.
        assert!(mentions_token(
            "`author.add_name` composes; `author.add_name_part` declares.",
            "author.add_name"
        ));
    }

    #[test]
    fn is_within_dir_does_not_match_sibling_directories() {
        assert!(is_within_dir("docs/design-notes", "docs/design-notes"));
        assert!(is_within_dir(
            "docs/design-notes/foo.md",
            "docs/design-notes"
        ));
        assert!(!is_within_dir(
            "docs/design-notes-sibling/foo.md",
            "docs/design-notes"
        ));
    }

    /// The lane must still pass against the actually-committed docs — the
    /// boundary-aware fix must not turn real, correct mentions into false
    /// negatives.
    #[test]
    fn full_lane_passes_against_the_committed_docs() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask has a parent directory")
            .to_path_buf();
        if let Err(error) = run_teaching_coverage(&root) {
            panic!("teaching-coverage regressed against the committed docs: {error}");
        }
    }

    #[test]
    fn extract_json_fences_finds_only_json_blocks() {
        let text = "```text\nnot json\n```\n\n```json transcript\n{\"dli\":\"1.6.0\"}\n```\n";
        let fences = extract_json_fences("f.md", text);
        assert_eq!(fences.len(), 1);
        assert!(fences[0].body.contains("1.6.0"));
    }

    #[test]
    fn dli_versions_in_finds_every_occurrence_on_a_line() {
        let body = "{\"dli\": \"1.0.0\", \"nested\": {\"dli\": \"1.6.0\"}}\n";
        let versions = dli_versions_in(body);
        assert_eq!(versions, vec!["1.0.0".to_owned(), "1.6.0".to_owned()]);
    }

    #[test]
    fn author_op_allowlist_entries_name_real_author_ops() {
        for (wire, _reason) in AUTHOR_OP_ALLOWLIST {
            assert!(
                Op::ALL.iter().any(|op| wire_name(*op) == *wire),
                "allowlisted `{wire}` is not a real op wire name"
            );
        }
    }
}
