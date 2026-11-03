//! Syntax diagnostics.
//!
//! Errors are lore (charter koan 9): a diagnostic speaks the worldbuilder's
//! domain, never compiler idiom. Each carries a stable machine-readable
//! [`DiagClass`] *and* a domain-spoken message — the two-channel error model,
//! because the errors are an API (DL-0008). Rendering runs through a
//! deterministic ASCII miette handler so the messages can be snapshot-tested.

use dli_ast::span::Span;
use miette::{Diagnostic as MietteDiagnostic, LabeledSpan, Severity};
use thiserror::Error;

/// The stable, machine-readable class of a syntax diagnostic.
///
/// Downstream tooling keys off the class; the [`Diagnostic::message`] is for the
/// author. The class is stable across message rewordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagClass {
    /// The line opened with a word that is not a De Lore Is keyword.
    UnknownKeyword,
    /// The line opened with a keyword that was renamed by the surface-v1 pass
    /// (e.g. `fluent` → `state`); the old spelling is a killed form.
    RenamedKeyword,
    /// A retired surface form with no direct spelling replacement was used.
    KilledForm,
    /// A ledger marker (`decided`/`ratified`/`provisional`) with no decision
    /// behind it.
    OriginRequired,
    /// A claim-bearing declaration carried no provenance word at all. Advisory
    /// (koan 1: the surface should *say* where a claim comes from), but the
    /// declaration still parses — the loader stays the fail-closed authority.
    OriginMissing,
    /// Two provenance markers on one line (e.g. `proposed ratified d-1`).
    ConflictingMarkers,
    /// A curly / smart quote appeared where a straight-quoted string belongs.
    CurlyQuote,
    /// An `option` appeared with no `question` or `fork` to belong to.
    StrayOption,
    /// A `{` block was never closed.
    UnterminatedBlock,
    /// A string literal was never closed.
    UnterminatedString,
    /// A record could not be read as any known declaration.
    Malformed,
}

impl DiagClass {
    /// The stable dotted code string for this class.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnknownKeyword => "dli::syntax::unknown-keyword",
            Self::RenamedKeyword => "dli::syntax::renamed-keyword",
            Self::KilledForm => "dli::syntax::killed-form",
            Self::OriginRequired => "dli::syntax::origin-required",
            Self::OriginMissing => "dli::syntax::origin-missing",
            Self::ConflictingMarkers => "dli::syntax::conflicting-markers",
            Self::CurlyQuote => "dli::syntax::curly-quote",
            Self::StrayOption => "dli::syntax::stray-option",
            Self::UnterminatedBlock => "dli::syntax::unterminated-block",
            Self::UnterminatedString => "dli::syntax::unterminated-string",
            Self::Malformed => "dli::syntax::malformed",
        }
    }

    /// A short label placed under the offending span in rendered output.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::UnknownKeyword => "not a keyword",
            Self::RenamedKeyword => "renamed keyword",
            Self::KilledForm => "retired form",
            Self::OriginRequired => "needs a decision",
            Self::OriginMissing => "no origin — is this proposed?",
            Self::ConflictingMarkers => "second marker",
            Self::CurlyQuote => "curly quote",
            Self::StrayOption => "no question here",
            Self::UnterminatedBlock => "unclosed block",
            Self::UnterminatedString => "unclosed string",
            Self::Malformed => "cannot read this",
        }
    }

    /// Whether this class is an error (gating) rather than an advisory warning.
    ///
    /// Only [`OriginMissing`](Self::OriginMissing) is advisory: incompleteness is
    /// a first-class state (koan 5), so a marker-less claim is flagged but never
    /// blocks the gate. Everything else is a hard error.
    #[must_use]
    pub const fn is_error(self) -> bool {
        !matches!(self, Self::OriginMissing)
    }
}

/// A syntax diagnostic: a stable class, a domain-spoken message, and a span.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct Diagnostic {
    /// The stable machine-readable class.
    pub class: DiagClass,
    /// The domain-spoken message shown to the author.
    pub message: String,
    /// Where the problem is, in source bytes.
    pub span: Span,
}

impl Diagnostic {
    /// Construct a diagnostic in the given class over the given span.
    #[must_use]
    pub fn new(class: DiagClass, message: impl Into<String>, span: Span) -> Self {
        Self {
            class,
            message: message.into(),
            span,
        }
    }
}

impl MietteDiagnostic for Diagnostic {
    fn code(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        Some(Box::new(self.class.code()))
    }

    fn severity(&self) -> Option<Severity> {
        Some(if self.class.is_error() {
            Severity::Error
        } else {
            Severity::Warning
        })
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let start = self.span.start as usize;
        let len = self.span.end.saturating_sub(self.span.start) as usize;
        let label = LabeledSpan::at(start..start + len, self.class.label());
        Some(Box::new(std::iter::once(label)))
    }
}

/// Render one diagnostic's caret block, optionally followed by aligned notes.
///
/// This is the deterministic ASCII primitive both [`render`] (the terse caret
/// report) and the graded error ladder ([`crate::ladder::render`]) build on. A
/// hand-rolled report — no color, no Unicode drawing — so the output is stable
/// across terminals and platforms and can be pinned with insta snapshots.
///
/// The header reads `error[…]` or `warning[…]` per the class's severity. When
/// `notes` is non-empty each entry renders as a `= {note}` line under a
/// separator, gutter-aligned rustc/miette-style; the ladder passes its rungs
/// here. With `notes` empty the output is exactly the caret block.
#[must_use]
pub fn render_block(source: &str, diag: &Diagnostic, notes: &[String]) -> String {
    let (line_no, byte_col, line_text) = locate(source, diag.span.start as usize);
    let col = line_text
        .get(..byte_col)
        .map_or(0, |prefix| prefix.chars().count());
    let span_len = usize::try_from(diag.span.end.saturating_sub(diag.span.start)).unwrap_or(0);
    let caret_len = line_text
        .get(byte_col..byte_col.saturating_add(span_len).min(line_text.len()))
        .map_or(1, |marked| marked.chars().count().max(1));
    let gutter = line_no.to_string();
    let pad = " ".repeat(gutter.len());
    let severity = if diag.class.is_error() {
        "error"
    } else {
        "warning"
    };

    let mut lines: Vec<String> = vec![
        format!("{severity}[{}]: {}", diag.class.code(), diag.message),
        format!("{pad} --> line {line_no}:{}", col + 1),
        format!("{pad} |"),
        format!("{gutter} | {line_text}"),
        format!(
            "{pad} | {}{} {}",
            " ".repeat(col),
            "^".repeat(caret_len),
            diag.class.label()
        ),
    ];
    if !notes.is_empty() {
        lines.push(format!("{pad} |"));
        for note in notes {
            lines.push(format!("{pad} = {note}"));
        }
    }
    lines.join("\n")
}

/// Render diagnostics against their source into deterministic ASCII text.
///
/// The terse report: each diagnostic's caret block, no ladder rungs. For the
/// graded ladder (what a writer meets day to day, koan 9) use
/// [`crate::ladder::render`].
///
/// ```
/// use dli_syntax::diag::{DiagClass, Diagnostic, render};
/// use dli_ast::span::{FileId, Span};
///
/// let source = "zorble frobnicate\n";
/// let diag = Diagnostic::new(
///     DiagClass::UnknownKeyword,
///     "`zorble` is not a De Lore Is keyword.",
///     Span { file: FileId(0), start: 0, end: 6 },
/// );
/// let rendered = render(source, std::slice::from_ref(&diag));
/// assert!(rendered.contains("zorble"));
/// assert!(rendered.contains("unknown-keyword"));
/// ```
#[must_use]
pub fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
    let mut out = diagnostics
        .iter()
        .map(|diag| render_block(source, diag, &[]))
        .collect::<Vec<_>>()
        .join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// Resolve a byte offset to a 1-based line number, a 0-based column, and the
/// text of that line.
fn locate(source: &str, offset: usize) -> (usize, usize, &str) {
    let mut line_start = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let line_end = line_start + line.len();
        if offset < line_end || line_end == source.len() {
            let text = line.strip_suffix('\n').unwrap_or(line);
            let col = offset.saturating_sub(line_start).min(text.len());
            return (index + 1, col, text);
        }
        line_start = line_end;
    }
    (1, 0, "")
}
