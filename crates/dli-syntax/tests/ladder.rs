#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The graded error ladder renders every diagnostic class in the domain, and
//! the reference cards keep their contract: the gentle card teaches only the
//! gentle subset, and every `dli` example on either card parses clean.
//!
//! The snapshots pin the ladder rendering seam the terminal front end consumes;
//! the card checks are the in-crate mirror of the repo-wide docs-oracle.

use std::path::PathBuf;

use dli_syntax::diag::DiagClass;
use dli_syntax::{ladder, parse_document};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Parse `source`, keep only the diagnostics of `class`, and render their
/// graded ladders — a class-focused snapshot of how one fault reads.
fn render_class(source: &str, class: DiagClass) -> String {
    let (_doc, diags) = parse_document(source);
    let picked: Vec<_> = diags.into_iter().filter(|d| d.class == class).collect();
    assert!(!picked.is_empty(), "expected a {class:?} diagnostic");
    ladder::render(source, &picked)
}

#[test]
fn ladder_unknown_keyword() {
    let out = render_class("zorble frobnicate\n", DiagClass::UnknownKeyword);
    insta::assert_snapshot!("ladder_unknown_keyword", out);
}

#[test]
fn ladder_origin_required() {
    let out = render_class(
        "place weir \"The Weir\" ratified\n",
        DiagClass::OriginRequired,
    );
    insta::assert_snapshot!("ladder_origin_required", out);
}

#[test]
fn ladder_origin_missing_is_a_warning() {
    // A warning stops at rung 2 — the header reads `warning[…]`, no `rule` line.
    let out = render_class("place weir\n", DiagClass::OriginMissing);
    assert!(out.contains("warning["));
    assert!(!out.contains("= rule:"));
    insta::assert_snapshot!("ladder_origin_missing", out);
}

#[test]
fn ladder_conflicting_markers() {
    let out = render_class(
        "place weir \"W\" proposed ratified d-1\n",
        DiagClass::ConflictingMarkers,
    );
    insta::assert_snapshot!("ladder_conflicting_markers", out);
}

#[test]
fn ladder_curly_quote() {
    let out = render_class(
        "place weir \u{201C}The Weir\u{201D} proposed\n",
        DiagClass::CurlyQuote,
    );
    insta::assert_snapshot!("ladder_curly_quote", out);
}

#[test]
fn ladder_stray_option() {
    let out = render_class("option lonely\n", DiagClass::StrayOption);
    insta::assert_snapshot!("ladder_stray_option", out);
}

#[test]
fn ladder_unterminated_string() {
    let out = render_class(
        "place weir \"The Weir proposed\nplace other \"O\" proposed\n",
        DiagClass::UnterminatedString,
    );
    insta::assert_snapshot!("ladder_unterminated_string", out);
}

#[test]
fn ladder_unterminated_block() {
    // Brace recovery: an unclosed `{` is diagnosed, not silently swallowed.
    let out = render_class(
        "practice ward-form {\n  step s1 instantiate\n",
        DiagClass::UnterminatedBlock,
    );
    insta::assert_snapshot!("ladder_unterminated_block", out);
}

#[test]
fn ladder_bad_continuation() {
    // An indented line that is not a slot fact — a Malformed continuation.
    let out = render_class(
        "person nara \"Nara\" decided d-1\n  nara is a fisher\n",
        DiagClass::Malformed,
    );
    insta::assert_snapshot!("ladder_bad_continuation", out);
}

#[test]
fn ladder_before_chain() {
    // An ordering with no target — the era/before-chain Malformed shape.
    let out = render_class("before founding\n", DiagClass::Malformed);
    insta::assert_snapshot!("ladder_before_chain", out);
}

#[test]
fn ladder_stacks_error_and_warning() {
    // The full, unfiltered view: a hard error and an advisory warning together,
    // proving severity-aware headers and rungs-1-2-for-warnings in one render.
    let source = "place weir \"The Weir proposed\nplace other \"O\" proposed\n";
    let (_doc, diags) = parse_document(source);
    insta::assert_snapshot!("ladder_stacked_findings", ladder::render(source, &diags));
}

// --- the reference cards keep their contract ------------------------------

/// The gentle words a card line may open with, plus the provenance markers.
const GENTLE_OPENERS: &[&str] = &[
    "scope",
    "era",
    "event",
    "before",
    "place",
    "person",
    "thing",
    "adjacent",
    "question",
    "option",
    "proposed",
    "decided",
    "ratified",
    "provisional",
];

/// Expert-tier words distinctive enough that they never occur in innocent
/// gentle-card prose, so their mere presence is a contract breach.
///
/// The surface-v1 pass renamed `fluent`→`state` and `lexeme`→`word` into plain
/// English words that DO occur in gentle prose, so they are deliberately not
/// listed here — a keyword table cannot police them by spelling alone.
const FORBIDDEN_EXPERT_WORDS: &[&str] = &[
    "practice",
    "proof",
    "dialect",
    "cycle",
    "fork",
    "name-part",
    "ratify",
    "narration",
    "hypothesis",
    "hyp",
];

fn read_card(name: &str) -> String {
    let path = repo_root().join("docs/cards").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn dli_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
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

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
}

fn opening_token(line: &str) -> &str {
    let trimmed = line.trim_start();
    let end = trimmed
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(trimmed.len());
    &trimmed[..end]
}

fn is_relation_form(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(paren) = trimmed.find('(') else {
        return false;
    };
    let head = &trimmed[..paren];
    !head.is_empty()
        && head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[test]
fn gentle_card_holds_no_expert_word() {
    let card = read_card("gentle-card.md");
    let hits: Vec<String> = words(&card)
        .filter(|w| FORBIDDEN_EXPERT_WORDS.contains(&w.as_str()))
        .collect();
    assert!(
        hits.is_empty(),
        "gentle card names expert-tier words: {hits:?}"
    );
}

#[test]
fn gentle_card_teaches_only_the_gentle_grammar() {
    // Every non-blank, non-comment, non-indented line in a gentle-card `dli`
    // block must open with a gentle word, be a named relation, or parse as a
    // standalone slot fact. An expert
    // keyword opening a line would be teaching expert grammar on the first card.
    let card = read_card("gentle-card.md");
    for block in dli_blocks(&card) {
        for line in block.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if line.starts_with(|c: char| c.is_whitespace()) {
                continue; // an indented slot fact or option
            }
            let token = opening_token(line);
            assert!(
                GENTLE_OPENERS.contains(&token)
                    || is_relation_form(line)
                    || matches!(
                        parse_document(line).0.decls.as_slice(),
                        [dli_ast::decl::Decl::Attr(_)]
                    ),
                "gentle card line opens with non-gentle token {token:?}: {line:?}"
            );
        }
    }
}

#[test]
fn every_card_example_parses_clean() {
    // The in-crate mirror of the docs-oracle: every `dli` block on all three
    // cards parses with no error-severity diagnostic (advisory warnings are OK).
    let mut saw = false;
    for name in ["gentle-card.md", "expert-card.md", "probe-protocol.md"] {
        for (index, block) in dli_blocks(&read_card(name)).into_iter().enumerate() {
            saw = true;
            let (_doc, diags) = parse_document(&block);
            let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
            assert!(errs.is_empty(), "{name} block #{}: {errs:?}", index + 1);
        }
    }
    assert!(saw, "expected at least one dli block across the cards");
}
