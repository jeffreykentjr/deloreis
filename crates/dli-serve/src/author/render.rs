//! Door T render (`AUTH-1`): staged declarations to canonical `.dli` lines.
//!
//! Door T never emits bespoke surface text. It renders staged AST through the
//! one canonical formatter [`dli_syntax::format_document`] — the exact form a
//! hand-written author would have written and the gate enforces (spec 11 §5;
//! DL-0003) — so both authoring populations are reviewed through one lens. A
//! synthetic scope-less document is formatted and split into non-empty lines:
//! one declaration renders as a mutation's `rendered`, and the whole staged set
//! renders as `author.diff`.
//!
//! # The `.dli` rendering is opt-in (LS-7)
//!
//! There is exactly one formatter and it lives in the parser crate, so LS-7
//! puts this whole rendering behind the `syntax` feature. **A default build
//! renders no lines**: every `rendered` field comes back empty, and the review
//! surface is the one the LS-5 contract already made canonical — the fact
//! objects `author.materialize` returns and the per-fact `AuthorDiffRow`s with
//! their before/after standing, neither of which passes through a formatter.
//! That is the direction contract 0.6.0 already set (`rendered: true` is the
//! opt-in, DL-0046); the default build simply has no opt-in to give.
//!
//! The sanitization below ([`escape_controls`], [`is_forbidden_char`]) is not
//! part of the formatter and stays on the default path — `is_forbidden_char`
//! also backs the `field.control-char` finding that bars materialization.
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
use dli_ast::clause::Clause;
use dli_ast::decl::{
    AttestDecl, AttrDecl, BannedDecl, BeforeDecl, Decl, DialectDecl, Document, EntityDecl,
    FactDecl, FunctionalDecl, HypothesisDecl, NameDecl, NamePartDecl, OptionClause, QuestionDecl,
    ScopeHeader, StateDecl, TermDecl, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::language::{
    CompositionStep, DialectBody, DialectClause, DialectItem, NameBody, NameClause, NameItem,
    NameKindSpec,
};
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term, TimeRef};

/// Format a synthetic document through the one canonical formatter, or `None`
/// when this build links no formatter to do it with (LS-7).
///
/// Every renderer in this module funnels through here, so "the default build
/// renders nothing" is one fact in one place rather than a per-function
/// convention.
// The `Option` is the seam, not this arm's choice: the parser-free arm below is
// the one that can answer `None`, and both must share a signature.
#[cfg(feature = "syntax")]
#[allow(clippy::unnecessary_wraps)]
fn format(document: &Document) -> Option<String> {
    Some(dli_syntax::format_document(document))
}

#[cfg(not(feature = "syntax"))]
fn format(_document: &Document) -> Option<String> {
    None
}

/// Whether this build can render staged declarations as canonical `.dli`.
///
/// `false` on a default build, where the fact objects and the structured diff
/// rows are the review surface; a caller that needs the *lines* — the one
/// remaining case is `author.discard`, which keys a rejection on a rendered
/// line — checks this and refuses by name rather than silently matching
/// nothing.
#[must_use]
pub const fn renders_dli() -> bool {
    cfg!(feature = "syntax")
}

/// Render a slice of staged declarations to canonical `.dli` lines. Empty on a
/// build without the `syntax` feature (see the module docs).
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
/// let lines = render_decls(&[decl]);
/// # #[cfg(feature = "syntax")]
/// assert_eq!(lines, vec!["person mara \"Mara\""]);
/// # #[cfg(not(feature = "syntax"))]
/// assert!(lines.is_empty());
/// ```
#[must_use]
pub fn render_decls(decls: &[Decl]) -> Vec<String> {
    let document = Document {
        scope: None,
        decls: sanitized(decls),
    };
    format(&document)
        .as_deref()
        .map(non_empty_lines)
        .unwrap_or_default()
}

/// Render the whole staged set under a scope header — the canonical surface
/// `author.materialize` returns as file lines. The `scope <name>` header comes
/// from the one formatter (its keyword tracks DL-0016), and the blank line the
/// formatter emits between header and body is **preserved** so the joined
/// contents are the canonical `.dli` a hand-written author would write and
/// `deloreis fmt --check` accepts (koan 7 round-trip; R3/B3). Only the trailing
/// newline is dropped so the lines join cleanly. Empty on a build without the
/// `syntax` feature (see the module docs).
///
/// ```
/// use dli_serve::author::render::render_scoped;
/// let lines = render_scoped("tpth", &[]);
/// # #[cfg(feature = "syntax")]
/// assert_eq!(lines, vec!["scope tpth"]);
/// # #[cfg(not(feature = "syntax"))]
/// assert!(lines.is_empty());
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
    format(&document)
        .as_deref()
        .map(canonical_lines)
        .unwrap_or_default()
}

/// Render the option lines a single `author.add_option` produced: the canonical
/// `option <id> "<label>"` line the formatter emits beneath its question, without the
/// question header (the mutation added an option, not a question). Empty on a
/// build without the `syntax` feature (see the module docs).
///
/// ```
/// use dli_serve::author::render::render_option;
/// let lines = render_option("where-the-first-ward-stood", "the-tidewall", "The Tidewall");
/// # #[cfg(feature = "syntax")]
/// assert_eq!(lines, vec!["  option the-tidewall \"The Tidewall\""]);
/// # #[cfg(not(feature = "syntax"))]
/// assert!(lines.is_empty());
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
    format(&document)
        .as_deref()
        .map(non_empty_lines)
        .unwrap_or_default()
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
            time: e.time.as_ref().map(sanitize_time),
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
        // The DM-4 expert declarations (contract 1.3.0). Door T stages these
        // from `author.add_dialect` and its siblings, so they need the same
        // display sanitization as everything above: a newline smuggled into a
        // gloss, a ban reason, or a dialect clause would otherwise split one
        // staged line into two in the review diff and forge a declaration the
        // reviewer reads as real (R3/C1).
        Decl::Dialect(d) => Decl::Dialect(DialectDecl {
            name: sanitize_ident(&d.name),
            label: d.label.as_deref().map(escape_controls),
            body: DialectBody {
                clauses: d
                    .body
                    .clauses
                    .iter()
                    .map(|c| DialectClause {
                        clause: sanitize_clause(&c.clause),
                        item: sanitize_dialect_item(&c.item),
                    })
                    .collect(),
            },
            provenance: d.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::NamePart(n) => Decl::NamePart(sanitize_name_part(n)),
        Decl::Name(n) => Decl::Name(NameDecl {
            name: sanitize_ident(&n.name),
            label: n.label.as_deref().map(escape_controls),
            head: n.head.iter().map(|t| escape_controls(t)).collect(),
            body: NameBody {
                clauses: n
                    .body
                    .clauses
                    .iter()
                    .map(|c| NameClause {
                        clause: sanitize_clause(&c.clause),
                        item: sanitize_name_item(&c.item),
                    })
                    .collect(),
            },
            provenance: n.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Term(t) => Decl::Term(TermDecl {
            name: sanitize_ident(&t.name),
            label: t.label.as_deref().map(escape_controls),
            provenance: t.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Banned(b) => Decl::Banned(BannedDecl {
            word: sanitize_ident(&b.word),
            reason: b.reason.as_deref().map(escape_controls),
            provenance: b.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::State(s) => Decl::State(StateDecl {
            name: sanitize_ident(&s.name),
            value: s.value.as_ref().map(sanitize_term),
            time: s.time.as_ref().map(sanitize_time),
            provenance: s.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Functional(f) => Decl::Functional(FunctionalDecl {
            pred: sanitize_ident(&f.pred),
            provenance: f.provenance.as_ref().map(sanitize_annot),
        }),
        Decl::Attest(a) => Decl::Attest(AttestDecl {
            question: sanitize_ident(&a.question),
            provenance: a.provenance.as_ref().map(sanitize_annot),
        }),
        // Door T stages no other declaration kind; pass them through.
        other => other.clone(),
    }
}

fn sanitize_clause(clause: &Clause) -> Clause {
    Clause {
        text: escape_controls(&clause.text),
        span: clause.span,
    }
}

fn sanitize_name_part(part: &NamePartDecl) -> NamePartDecl {
    NamePartDecl {
        name: sanitize_ident(&part.name),
        gloss: part.gloss.as_deref().map(escape_controls),
        head: part.head.iter().map(|t| escape_controls(t)).collect(),
        provenance: part.provenance.as_ref().map(sanitize_annot),
    }
}

fn sanitize_dialect_item(item: &DialectItem) -> DialectItem {
    let words = |list: &[String]| list.iter().map(|w| escape_controls(w)).collect::<Vec<_>>();
    match item {
        DialectItem::Basis { attested, text } => DialectItem::Basis {
            attested: *attested,
            text: text.as_deref().map(escape_controls),
        },
        DialectItem::Vowels(list) => DialectItem::Vowels(words(list)),
        DialectItem::Endings(list) => DialectItem::Endings(words(list)),
        DialectItem::Foreign(list) => DialectItem::Foreign(words(list)),
        DialectItem::Stress(keyword) => {
            DialectItem::Stress(keyword.as_deref().map(escape_controls))
        }
        DialectItem::NameKind(spec) => {
            DialectItem::NameKind(spec.as_ref().map(|spec| NameKindSpec {
                kind: escape_controls(&spec.kind),
                patterns: spec.patterns.iter().map(|roles| words(roles)).collect(),
            }))
        }
        DialectItem::NamePart(part) => DialectItem::NamePart(Box::new(sanitize_name_part(part))),
        DialectItem::ClusterMax(_) | DialectItem::SayabilityMax(_) | DialectItem::Other => {
            item.clone()
        }
    }
}

fn sanitize_name_item(item: &NameItem) -> NameItem {
    match item {
        NameItem::Composition(steps) => NameItem::Composition(
            steps
                .iter()
                .map(|step| CompositionStep {
                    role: escape_controls(&step.role),
                    morpheme: escape_controls(&step.morpheme),
                })
                .collect(),
        ),
        NameItem::Bearer {
            entity,
            provenance: bearer_provenance,
        } => NameItem::Bearer {
            entity: entity.as_deref().map(escape_controls),
            provenance: bearer_provenance.as_ref().map(sanitize_annot),
        },
        NameItem::Provenance(annot) => NameItem::Provenance(sanitize_annot(annot)),
        NameItem::Other => NameItem::Other,
    }
}

/// Sanitize a temporal clause's anchor idents.
///
/// A time clause names an era or event, and a name is free text that reaches the
/// formatter: `during <era>` renders the era verbatim. This arm was missing for
/// both `Decl::Entity` and `Decl::State`, and a newline in the anchor really did
/// split one staged declaration into two rendered lines — `state x during
/// the-flood\nstate forged during now` rendered as two `state` lines, the second
/// of which a reviewer reads as a declaration the author never made. The
/// `field.control-char` finding always fired and always barred materialize, so
/// canon was never reachable; what was forgeable was the **review diff**, which
/// this module's own contract says is forgery-proof. Found by a red-team probe
/// against `author.add_state`; the `Decl::Entity` half predates it and is
/// reachable through `author.add_event`'s `era`.
fn sanitize_time(time: &TimeClause) -> TimeClause {
    match time {
        TimeClause::After(point) => TimeClause::After(sanitize_ident(point)),
        TimeClause::During(point) => TimeClause::During(sanitize_ident(point)),
        TimeClause::Since(point) => TimeClause::Since(sanitize_ident(point)),
        TimeClause::Until(point) => TimeClause::Until(sanitize_ident(point)),
        TimeClause::Order(points) => TimeClause::Order(points.iter().map(sanitize_ident).collect()),
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
