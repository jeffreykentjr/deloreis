//! Door T render (`AUTH-1`): staged declarations to canonical `.dli` lines.
//!
//! Door T never emits bespoke surface text. It renders staged AST through the
//! one canonical formatter [`dli_syntax::format_document`] — the exact form a
//! Door H author would have written and the gate enforces (spec 11 §5;
//! DL-0003) — so both authoring populations are reviewed through one lens. A
//! synthetic scope-less document is formatted and split into non-empty lines:
//! one declaration renders as a mutation's `rendered`, and the whole staged set
//! renders as `author.diff`.
//!
//! **The rendered diff is the human's only supervision surface, so it is
//! forgery-proof.** Every string a declaration carries is escaped for control
//! characters *before* it reaches the formatter ([`escape_controls`]): a newline
//! smuggled into a provenance origin or a slot value can never split one staged
//! line into two and forge a second declaration the reviewer would read as real
//! (R3/C1). The staged AST keeps the raw bytes (nothing is dropped — koan 4, and
//! a `field.control-char` error finding bars materialization); only the *display*
//! is sanitized.

use dli_ast::annot::{Annotation, CitationRef, CondExpr};
use dli_ast::decl::{
    AttrDecl, BeforeDecl, Decl, Document, EntityDecl, FactDecl, HypothesisDecl, OptionClause,
    QuestionDecl, ScopeHeader,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term, TimeRef};
use dli_syntax::format_document;

/// Render a slice of staged declarations to canonical `.dli` lines.
///
/// ```
/// use dli_ast::decl::{Decl, EntityDecl, EntityKind};
/// use dli_ast::ident::Ident;
/// use dli_serve::author::render::render_decls;
/// let decl = Decl::Entity(EntityDecl {
///     kind: EntityKind::Person,
///     name: Ident::synthetic("mara"),
///     label: Some("Mara".to_owned()),
///     time: None,
///     slots: Vec::new(),
///     provenance: None,
/// });
/// assert_eq!(render_decls(&[decl]), vec!["person mara \"Mara\""]);
/// ```
#[must_use]
pub fn render_decls(decls: &[Decl]) -> Vec<String> {
    let document = Document {
        scope: None,
        decls: sanitized(decls),
    };
    non_empty_lines(&format_document(&document))
}

/// Render the whole staged set under a scope header — the canonical surface
/// `author.materialize` returns as file lines. The `scope <name>` header comes
/// from the one formatter (its keyword tracks DL-0016), and the blank line the
/// formatter emits between header and body is **preserved** so the joined
/// contents are the canonical `.dli` a Door H author would write and
/// `deloreis fmt --check` accepts (koan 7 round-trip; R3/B3). Only the trailing
/// newline is dropped so the lines join cleanly.
///
/// ```
/// use dli_serve::author::render::render_scoped;
/// assert_eq!(render_scoped("tpth", &[]), vec!["scope tpth"]);
/// ```
#[must_use]
pub fn render_scoped(scope: &str, decls: &[Decl]) -> Vec<String> {
    let document = Document {
        scope: Some(ScopeHeader {
            name: Ident::synthetic(escape_controls(scope)),
            span: Span::synthetic(),
        }),
        decls: sanitized(decls),
    };
    canonical_lines(&format_document(&document))
}

/// Render the option lines a single `author.add_option` produced: the canonical
/// `option <id> "<label>"` line the formatter emits beneath its question, without the
/// question header (the mutation added an option, not a question).
///
/// ```
/// use dli_serve::author::render::render_option;
/// assert_eq!(render_option("where-the-first-ward-stood", "the-tidewall", "The Tidewall"),
///            vec!["  option the-tidewall \"The Tidewall\""]);
/// ```
#[must_use]
pub fn render_option(question: &str, option: &str, label: &str) -> Vec<String> {
    let synthetic = QuestionDecl {
        name: Ident::synthetic(escape_controls(question)),
        prompt: None,
        options: vec![OptionClause {
            name: Ident::synthetic(escape_controls(option)),
            label: Some(escape_controls(label)),
            implies: None,
        }],
        provenance: None,
    };
    let document = Document {
        scope: None,
        decls: vec![Decl::Question(synthetic)],
    };
    // Drop the `question <name>` header; keep only the option line(s).
    non_empty_lines(&format_document(&document))
        .into_iter()
        .skip(1)
        .collect()
}

/// Escape the forbidden characters in a string so it can never break out of, or
/// visually reorder, its line. `\n`, `\r`, `\t` render as their two-character
/// escapes; any other [forbidden](is_forbidden_char) character (C0/C1 controls,
/// DEL, and the Cf format / bidi / zero-width set) renders as `\u{XXXX}`.
/// Ordinary text is returned unchanged, so well-formed input renders
/// byte-identically to before.
///
/// ```
/// use dli_serve::author::render::escape_controls;
/// assert_eq!(escape_controls("mara"), "mara");
/// assert_eq!(escape_controls("s\nperson forged"), "s\\nperson forged");
/// // U+202E RIGHT-TO-LEFT OVERRIDE cannot silently reverse the visible line.
/// assert_eq!(escape_controls("Mara\u{202e}x"), "Mara\\u{202e}x");
/// ```
#[must_use]
pub fn escape_controls(text: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if is_forbidden_char(ch) => {
                let _ = write!(out, "\\u{{{:04x}}}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out
}

/// Whether a character is forbidden in a rendered field: a C0/C1 control or DEL
/// (`char::is_control`, Unicode category Cc), or one of the Cf format / bidi /
/// zero-width characters that reorder or hide text in the review diff — the
/// bidi overrides and isolates (U+202A–202E, U+2066–2069), the zero-width and
/// directional marks (U+200B–200F, U+2060–2064), the BOM (U+FEFF), the soft
/// hyphen (U+00AD), and the Arabic letter mark (U+061C). Rejecting and escaping
/// these keeps the supervision surface faithful (R3/B2).
#[must_use]
pub fn is_forbidden_char(ch: char) -> bool {
    ch.is_control()
        || matches!(u32::from(ch),
            0x00AD | 0x061C
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF)
}

/// Split rendered text into canonical lines: keep every internal line, including
/// the blank line after a `scope` header, and drop only the trailing newline
/// (R3/B3).
fn canonical_lines(text: &str) -> Vec<String> {
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .map(str::to_owned)
        .collect()
}

// --- display sanitization (never mutates the staged AST) --------------------

fn sanitized(decls: &[Decl]) -> Vec<Decl> {
    decls.iter().map(sanitize_decl).collect()
}

fn sanitize_decl(decl: &Decl) -> Decl {
    match decl {
        Decl::Entity(e) => Decl::Entity(EntityDecl {
            kind: e.kind,
            name: sanitize_ident(&e.name),
            label: e.label.as_deref().map(escape_controls),
            time: e.time.clone(),
            slots: e.slots.iter().map(sanitize_attr).collect(),
            provenance: e.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Attr(a) => Decl::Attr(sanitize_attr(a)),
        Decl::Fact(f) => Decl::Fact(FactDecl {
            atom: sanitize_atom(&f.atom),
            triple: f.triple,
            provenance: f.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Before(b) => Decl::Before(BeforeDecl {
            earlier: sanitize_ident(&b.earlier),
            later: sanitize_ident(&b.later),
            provenance: b.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Question(q) => Decl::Question(QuestionDecl {
            name: sanitize_ident(&q.name),
            prompt: q.prompt.as_deref().map(escape_controls),
            options: q.options.iter().map(sanitize_option).collect(),
            provenance: q.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Hypothesis(h) => Decl::Hypothesis(HypothesisDecl {
            name: sanitize_ident(&h.name),
            statement: h.statement.as_deref().map(escape_controls),
        }),
        // Door T never stages the other declaration kinds; pass them through.
        other => other.clone(),
    }
}

fn sanitize_ident(id: &Ident) -> Ident {
    Ident {
        text: escape_controls(&id.text),
        span: id.span,
    }
}

fn sanitize_term(term: &Term) -> Term {
    match term {
        Term::Var(id) => Term::Var(sanitize_ident(id)),
        Term::Sym(id) => Term::Sym(sanitize_ident(id)),
        Term::Str(s) => Term::Str(escape_controls(s)),
        Term::Int(n) => Term::Int(*n),
        Term::Time(TimeRef::Era(id)) => Term::Time(TimeRef::Era(sanitize_ident(id))),
        Term::Time(TimeRef::Event(id)) => Term::Time(TimeRef::Event(sanitize_ident(id))),
    }
}

fn sanitize_atom(atom: &AtomExpr) -> AtomExpr {
    AtomExpr {
        pred: sanitize_ident(&atom.pred),
        args: atom.args.iter().map(sanitize_term).collect(),
    }
}

fn sanitize_attr(attr: &AttrDecl) -> AttrDecl {
    AttrDecl {
        subject: sanitize_ident(&attr.subject),
        attr: sanitize_ident(&attr.attr),
        value: sanitize_term(&attr.value),
        provenance: attr.provenance.as_ref().map(sanitize_annot),
    }
}

fn sanitize_option(option: &OptionClause) -> OptionClause {
    OptionClause {
        name: sanitize_ident(&option.name),
        label: option.label.as_ref().map(|label| escape_controls(label)),
        implies: option.implies.as_ref().map(sanitize_atom),
    }
}

fn sanitize_cond(cond: &CondExpr) -> CondExpr {
    match cond {
        CondExpr::Hyp(id) => CondExpr::Hyp(sanitize_ident(id)),
        CondExpr::Not(inner) => CondExpr::Not(Box::new(sanitize_cond(inner))),
        CondExpr::And(parts) => CondExpr::And(parts.iter().map(sanitize_cond).collect()),
        CondExpr::Or(parts) => CondExpr::Or(parts.iter().map(sanitize_cond).collect()),
    }
}

fn sanitize_annot(annot: &Annotation) -> Annotation {
    Annotation {
        marker: annot.marker,
        citations: annot
            .citations
            .iter()
            .map(|c| CitationRef {
                id: sanitize_ident(&c.id),
            })
            .collect(),
        condition: annot.condition.as_ref().map(sanitize_cond),
        tags: annot.tags.iter().map(|t| escape_controls(t)).collect(),
        span: annot.span,
    }
}

/// Split rendered text into non-empty lines.
fn non_empty_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}
