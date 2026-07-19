//! The graded error ladder.
//!
//! Errors are lore (charter koan 9): a diagnostic is a world-record artifact,
//! and the ladder is how it teaches rather than merely rejects. Every
//! [`DiagClass`] is organized into three rungs a writer
//! can climb:
//!
//! 1. **what happened**, in the world's terms — a missing ruling, a season with
//!    no first, a block left open — never compiler idiom;
//! 2. **the smallest repair** — a *typed* [`Repair`] naming the token or line to
//!    change, in the same voice as the loader's structured repair data;
//! 3. **the deeper rule it protects** — one sentence citing the charter koan and
//!    the spec section the fault runs against.
//!
//! [`render`] draws these under the wave-1 caret block via
//! [`crate::diag::render_block`], so the whole ladder is deterministic ASCII and
//! snapshot-testable. Error-severity classes show all three rungs; advisory
//! warnings (koan 5's proposed-first authoring) show rungs 1–2 — the protected
//! rule is still recorded here, but an advisory informs, it does not lecture.
//!
//! This module is the seam the terminal front end consumes: [`render`] for the
//! rendered ladder, [`ladder`] for the structured rungs behind it.

use crate::diag::{DiagClass, Diagnostic};

/// A charter koan — the deepest rule a rung-3 anchor points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Koan {
    /// The koan's number in the charter (1–12).
    pub n: u8,
    /// Its short name, e.g. `"claims come from somewhere"`.
    pub name: &'static str,
}

/// The deeper rule a diagnostic protects: one sentence, a koan, a spec section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// The rule stated in one sentence, in the world's terms.
    pub statement: &'static str,
    /// The charter koan the rule descends from.
    pub koan: Koan,
    /// The spec section that treats it, e.g. `"01-syntax.md §2"`.
    pub spec: &'static str,
}

impl Rule {
    /// The parenthetical anchor rendered after the rung-3 sentence, e.g.
    /// `"koan 4 — nothing silently dropped; 01-syntax.md §2"`.
    #[must_use]
    pub fn anchor(&self) -> String {
        format!("koan {} — {}; {}", self.koan.n, self.koan.name, self.spec)
    }
}

/// The smallest repair for a fault, as a typed value rather than free prose.
///
/// Each variant names *what* to change, mirroring the loader's structured repair
/// data so a front end (or an agent reading the finding) can act on it, not just
/// read it. [`Repair::instruction`] renders the one-line domain-spoken form the
/// ladder shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repair {
    /// Add a closing delimiter that was never written (a `"` or a `}`).
    Close {
        /// The delimiter to add.
        delimiter: &'static str,
    },
    /// Swap one token for another (a curly quote for a straight one).
    Replace {
        /// The token to remove.
        from: &'static str,
        /// The token to write in its place.
        to: &'static str,
    },
    /// Drop a stray token (a duplicate origin marker).
    Remove {
        /// The token to drop.
        token: &'static str,
    },
    /// Give the claim an origin — cite a decision, or mark it `proposed`.
    Origin,
    /// Move a declaration under the owner it belongs to (an option under a
    /// question).
    Reparent {
        /// The owner to place it under.
        under: &'static str,
    },
    /// Recast the line into a named shape; the phrase *is* the instruction.
    Recast {
        /// The shape to recast into.
        into: &'static str,
    },
}

impl Repair {
    /// The domain-spoken, one-line repair instruction shown as rung 2.
    #[must_use]
    pub fn instruction(&self) -> String {
        match self {
            Self::Close { delimiter } => format!("add the closing `{delimiter}`"),
            Self::Replace { from, to } => format!("replace {from} with {to}"),
            Self::Remove { token } => format!("drop {token}"),
            Self::Origin => {
                "cite a decision (`decided d-nnnn`), or mark the claim `proposed`".to_owned()
            }
            Self::Reparent { under } => format!("put it under {under} on the line above"),
            Self::Recast { into } => (*into).to_owned(),
        }
    }
}

/// The full ladder for one diagnostic class: the three rungs, keyed by class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ladder {
    /// The class this ladder speaks for.
    pub class: DiagClass,
    /// Rung 1 — what happened, in the world's terms.
    pub what: &'static str,
    /// Rung 2 — the smallest, typed repair.
    pub repair: Repair,
    /// Rung 3 — the deeper rule it protects. Recorded for every class; only
    /// rendered for error-severity ones (see the module docs).
    pub rule: Rule,
}

/// The ladder for a diagnostic class.
///
/// Total over [`DiagClass`]: every class has all three
/// rungs, so a new class cannot be added without also giving it its lore.
///
/// ```
/// use dli_syntax::diag::DiagClass;
/// use dli_syntax::ladder::ladder;
///
/// let l = ladder(DiagClass::OriginRequired);
/// assert_eq!(l.rule.koan.n, 1); // claims come from somewhere
/// assert!(l.repair.instruction().contains("proposed"));
/// ```
// The match is the per-class lore table — one arm per DiagClass — so its length
// is inherent, not accidental (same shape as the parser's keyword dispatch).
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn ladder(class: DiagClass) -> Ladder {
    let (what, repair, rule) = match class {
        DiagClass::UnknownKeyword => (
            "the line opens with a word the world has no keyword for, so it reads as no \
             declaration at all",
            Repair::Recast {
                into: "open with a world word — era, place, person, thing, event, fact, \
                       question, … — or a named relation `pred(a, b)`",
            },
            Rule {
                statement: "the keyword vocabulary is closed by design — the worldbuilder's own \
                            artifact names, no synonyms and no user-defined forms — so an \
                            unknown opener is a fault surfaced, never a silent extension",
                koan: Koan {
                    n: 7,
                    name: "one way to write it",
                },
                spec: "01-syntax.md §2",
            },
        ),
        DiagClass::OriginRequired => (
            "a ruling marker stands here (`ratified`, `decided`, `provisional`) but names no \
             decision, so the claim is ruled true by nothing",
            Repair::Origin,
            Rule {
                statement: "a claim's authority is its origin — a status with nothing behind it \
                            is a claim from nowhere, which the grammar has no slot to hold",
                koan: Koan {
                    n: 1,
                    name: "claims come from somewhere",
                },
                spec: "01-syntax.md §1",
            },
        ),
        DiagClass::OriginMissing => (
            "this claim ends with no origin word, so the surface does not yet say where it \
             comes from",
            Repair::Origin,
            Rule {
                statement: "claims come from somewhere; an unmarked claim is kept as a sketch, \
                            out of canon until it names an origin",
                koan: Koan {
                    n: 1,
                    name: "claims come from somewhere",
                },
                spec: "01-syntax.md §4",
            },
        ),
        DiagClass::ConflictingMarkers => (
            "two origin markers sit on one claim — its standing was already set by the first, \
             and a second would make it ambiguous",
            Repair::Remove {
                token: "the second marker",
            },
            Rule {
                statement: "a claim carries exactly one origin; the surface refuses a second \
                            rather than silently choosing between them",
                koan: Koan {
                    n: 1,
                    name: "claims come from somewhere",
                },
                spec: "01-syntax.md §1",
            },
        ),
        DiagClass::CurlyQuote => (
            "a curly (smart) quote appears where a label's straight-quoted text belongs — the \
             artifact an editor leaves when it auto-replaces quotes",
            Repair::Replace {
                from: "the curly quote",
                to: "a straight `\"`",
            },
            Rule {
                statement: "there is one way to write a label — straight quotes — so the \
                            canonical surface never turns on a character an editor might \
                            substitute unseen",
                koan: Koan {
                    n: 7,
                    name: "one way to write it",
                },
                spec: "01-syntax.md §1",
            },
        ),
        DiagClass::StrayOption => (
            "an `option` stands here with no `question` or `fork` open above it, so it is an \
             arm of no choice",
            Repair::Reparent {
                under: "a `question q-nnnn \"…\"` (or a `fork`)",
            },
            Rule {
                statement: "an option is one arm of an undecided fork; without its question \
                            there is no debt for it to belong to, and a dangling option is a \
                            choice about nothing",
                koan: Koan {
                    n: 5,
                    name: "an open question is a debt",
                },
                spec: "01-syntax.md §3",
            },
        ),
        DiagClass::UnterminatedBlock => (
            "a braced block was opened with `{` and never closed, so it runs to the end of the \
             file",
            Repair::Close { delimiter: "}" },
            Rule {
                statement: "an unclosed block would swallow every following record into one \
                            runaway body; the boundary is required so nothing after it is lost \
                            unseen",
                koan: Koan {
                    n: 4,
                    name: "nothing is silently dropped",
                },
                spec: "01-syntax.md §2",
            },
        ),
        DiagClass::UnterminatedString => (
            "a label's quote was opened and never closed, so the words after it are read as \
             part of the label",
            Repair::Close { delimiter: "\"" },
            Rule {
                statement: "a run-on label would swallow the claims after it; closing the quote \
                            keeps each claim on its own line, where its blame and origin live",
                koan: Koan {
                    n: 4,
                    name: "nothing is silently dropped",
                },
                spec: "01-syntax.md §2",
            },
        ),
        DiagClass::Malformed => (
            "this record opens like a declaration but does not match the shape its keyword takes",
            Repair::Recast {
                into: "recast the line into the shape the message names",
            },
            Rule {
                statement: "each keyword has exactly one shape; a record that does not fit is \
                            surfaced with the shape it should take, never guessed at and never \
                            dropped",
                koan: Koan {
                    n: 4,
                    name: "nothing is silently dropped",
                },
                spec: "01-syntax.md §4",
            },
        ),
    };
    Ladder {
        class,
        what,
        repair,
        rule,
    }
}

/// The rung lines for a class: `what` and `fix` always; `rule` only for
/// error-severity classes (warnings stop at rung 2).
fn rungs(class: DiagClass) -> Vec<String> {
    let l = ladder(class);
    let mut notes = vec![
        format!("what: {}", l.what),
        format!("fix:  {}", l.repair.instruction()),
    ];
    if class.is_error() {
        notes.push(format!("rule: {}  ({})", l.rule.statement, l.rule.anchor()));
    }
    notes
}

/// Render diagnostics with their graded ladders into deterministic ASCII text.
///
/// Each diagnostic's caret block ([`crate::diag::render_block`]) is followed by
/// its rungs. This is the frozen rendering seam the terminal front end consumes.
///
/// ```
/// use dli_syntax::diag::{DiagClass, Diagnostic};
/// use dli_syntax::ladder::render;
/// use dli_ast::span::{FileId, Span};
///
/// let source = "place weir\n";
/// let diag = Diagnostic::new(
///     DiagClass::OriginMissing,
///     "this claim has no origin — end it with `proposed`.",
///     Span { file: FileId(0), start: 6, end: 10 },
/// );
/// let out = render(source, std::slice::from_ref(&diag));
/// assert!(out.contains("warning[")); // advisory severity in the header
/// assert!(out.contains("= what:"));
/// assert!(out.contains("= fix:"));
/// assert!(!out.contains("= rule:")); // a warning stops at rung 2
/// ```
#[must_use]
pub fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
    let mut out = diagnostics
        .iter()
        .map(|diag| crate::diag::render_block(source, diag, &rungs(diag.class)))
        .collect::<Vec<_>>()
        .join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}
