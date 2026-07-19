//! The gentle-subset probe.
//!
//! A line is accepted when it is blank, a `#` comment, an indented continuation,
//! opens with a known keyword, or is a relation fact of the form `pred(args)`.
//! Everything else is rejected. This is deliberately not a parser: it is a
//! coarse gate that lets the docs-oracle verify example snippets stay within the
//! subset the eventual grammar will cover.

use miette::Diagnostic;
use thiserror::Error;

/// The reserved keywords a line may open with.
pub const KEYWORDS: &[&str] = &[
    "pack",
    "use",
    "sort",
    "fact",
    "rule",
    "fluent",
    "wheel",
    "when",
    "fork",
    "practice",
    "ledger",
    "register",
    "name",
    "lexeme",
    "craft",
    "draft",
    "attest",
    "export",
    "cite",
    "hyp",
    "era",
    "event",
    "before",
    "place",
    "person",
    "thing",
    "question",
    "option",
    "hypothesis",
    "variant",
    "morpheme",
    "term",
    "banned",
    "premise",
    "step",
    "spend",
    "close",
    "principle",
    "order-rule",
    "adjacent",
    "narration",
    "ratify",
    "decided",
    "proposed",
    "ratified",
    "provisional",
];

/// A line the probe could not place within the gentle subset.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
#[error("line {line}: unrecognized leading token '{token}'")]
#[diagnostic(
    code(dli::syntax::probe),
    help("open the line with a known keyword or a relation of the form pred(args)")
)]
pub struct ProbeError {
    /// The 1-based line number (0 when produced by [`probe_line`] alone).
    pub line: usize,
    /// The offending leading token.
    pub token: String,
}

fn first_token(line: &str) -> &str {
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
    let mut chars = head.chars();
    let starts_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    starts_ok
        && head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Probe a single line. Accepts the gentle subset; the returned error carries
/// line 0 because a bare line has no position — [`probe_source`] fills it in.
pub fn probe_line(line: &str) -> Result<(), ProbeError> {
    if line.trim().is_empty() {
        return Ok(());
    }
    if line.trim_start().starts_with('#') {
        return Ok(());
    }
    if line.starts_with(|c: char| c.is_whitespace()) {
        return Ok(());
    }
    let token = first_token(line);
    if KEYWORDS.contains(&token) || is_relation_form(line) {
        return Ok(());
    }
    Err(ProbeError {
        line: 0,
        token: token.to_owned(),
    })
}

/// Probe every line of a source string, returning one error per rejected line
/// with its 1-based line number filled in.
pub fn probe_source(source: &str) -> Vec<ProbeError> {
    let mut errors = Vec::new();
    for (index, line) in source.lines().enumerate() {
        if let Err(mut error) = probe_line(line) {
            error.line = index + 1;
            errors.push(error);
        }
    }
    errors
}
