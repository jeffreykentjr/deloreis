//! Recursive-descent parser over the flat record grammar.
//!
//! A `.dli` file is a sequence of flat, order-insensitive, line-oriented
//! records (spec `01-syntax.md` §2). Each record is one leading line plus
//! either an indented continuation/child block or a braced `{ … }` block. The
//! gentle subset — entities, slot facts, relations, questions, timeline
//! orderings, hypotheses — lowers into fully typed declarations; the expert
//! tier keeps a typed header and captures its interior clauses verbatim, so
//! nothing is dropped (koan 4) while the AST stays small.
//!
//! Recovery is per-record: a record that cannot be read yields a domain-spoken
//! [`Diagnostic`] and the parse continues at the next record. `parse_document`
//! therefore always returns a document *and* the diagnostics, never one or the
//! other.

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    AttrDecl, BannedDecl, BeforeDecl, CycleDecl, Decl, DialectDecl, Document, DraftDecl,
    EntityDecl, EntityKind, ExportDecl, FactDecl, ForkDecl, HypothesisDecl, KindDecl, NameDecl,
    NamePartDecl, NarrationDecl, OptionClause, PracticeDecl, PrincipleDecl, ProofDecl,
    QuestionDecl, RatifyDecl, RuleDecl, RuleDomain, ScopeHeader, StateDecl, TermDecl, TimeClause,
    UseDecl, VariantDecl, WordDecl,
};
use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::span::{FileId, Span};
use dli_ast::term::{AtomExpr, Term, TimeRef};

use crate::diag::{DiagClass, Diagnostic};
use crate::lexer::{Line, Token, TokenKind, lines, to_u32};

/// Parse source into the normative AST document plus any diagnostics.
///
/// This is the frozen front-end seam. The parse is total: it always yields a
/// [`Document`] (populated as far as the source allowed) alongside the
/// diagnostics for whatever it could not read.
///
/// ```
/// use dli_syntax::parser::parse_document;
///
/// let (doc, diags) = parse_document("scope saltmarket\nplace weir \"The Weir\" proposed\n");
/// assert!(diags.is_empty());
/// assert_eq!(doc.scope.unwrap().name.text, "saltmarket");
/// assert_eq!(doc.decls.len(), 1);
/// ```
#[must_use]
pub fn parse_document(input: &str) -> (Document, Vec<Diagnostic>) {
    let file = FileId(0);
    // Strip a leading UTF-8 BOM so it never lands inside the first token.
    let source = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut diags = lexical_diagnostics(source, file);
    let records = assemble_records(&lines(source, file), file, &mut diags);

    let mut scope = None;
    let mut items: Vec<Item> = Vec::new();
    for record in &records {
        let Some(first) = record.head.tokens.first() else {
            continue;
        };
        if first.kind == TokenKind::Word && first.text == "scope" {
            if let Some(name_tok) = record.head.tokens.get(1) {
                scope = Some(ScopeHeader {
                    name: ident_of(name_tok),
                    span: record.head.span,
                });
            }
            continue;
        }
        if first.kind == TokenKind::Word && first.text == "pack" {
            // `pack` is a killed form — the header keyword is now `scope` (koan 7).
            diags.push(renamed_keyword("pack", "scope", first.span));
            continue;
        }
        parse_record(record, &mut items, &mut diags);
    }

    let decls = fold_options(items, &mut diags);
    check_origins(&decls, &mut diags);
    (Document { scope, decls }, diags)
}

/// Pre-lexing scan for lexical hazards that would otherwise lose text silently:
/// curly / smart quotes where a straight string belongs (M7), and unterminated
/// string literals (L1). Runs on the BOM-stripped source; strings never span
/// lines, so an open string at a newline is unterminated.
fn lexical_diagnostics(source: &str, file: FileId) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut in_str = false;
    let mut in_comment = false;
    let mut escaped = false;
    let mut str_start = 0u32;
    for (i, c) in source.char_indices() {
        let at = crate::lexer::to_u32(i);
        if c == '\n' {
            if in_str {
                diags.push(unterminated_string(file, str_start, at));
                in_str = false;
            }
            in_comment = false;
            escaped = false;
            continue;
        }
        if in_comment {
            continue;
        }
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '#' => in_comment = true,
            '"' => {
                in_str = true;
                str_start = at;
            }
            '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}' => {
                let end = crate::lexer::to_u32(i + c.len_utf8());
                diags.push(Diagnostic::new(
                    DiagClass::CurlyQuote,
                    "that is a curly quote — write labels with straight quotes, like \"Camelot\".",
                    Span {
                        file,
                        start: at,
                        end,
                    },
                ));
            }
            _ => {}
        }
    }
    if in_str {
        diags.push(unterminated_string(
            file,
            str_start,
            crate::lexer::to_u32(source.len()),
        ));
    }
    diags
}

fn unterminated_string(file: FileId, start: u32, end: u32) -> Diagnostic {
    Diagnostic::new(
        DiagClass::UnterminatedString,
        "this string is never closed — add the closing \".",
        Span { file, start, end },
    )
}

/// Post-parse scan for the koan-1 surface obligation: a claim-bearing
/// declaration (an entity, a slot fact, a relation, an ordering) that carries no
/// provenance word at all is flagged (M1). Advisory only — the declaration
/// still stands and the loader remains the fail-closed authority — but the
/// surface must *say* a claim has no origin rather than imply it silently.
fn check_origins(decls: &[Decl], diags: &mut Vec<Diagnostic>) {
    for decl in decls {
        let (missing, span) = match decl {
            Decl::Entity(e) => (e.provenance.is_none(), e.name.span),
            Decl::Attr(a) => (a.provenance.is_none(), a.subject.span),
            Decl::Fact(f) => (f.provenance.is_none(), f.atom.pred.span),
            Decl::Before(b) => (b.provenance.is_none(), b.earlier.span),
            _ => continue,
        };
        if missing {
            diags.push(Diagnostic::new(
                DiagClass::OriginMissing,
                "this claim has no origin — end it with `proposed`, or cite a decision (`decided d-nnnn`).",
                span,
            ));
        }
    }
}

/// Structure a captured [`Clause`] into a single atom, when it is atom-shaped.
///
/// The expert tier's interior clauses are captured verbatim (see
/// [`dli_ast::clause::Clause`]); this is the one shared way for a downstream
/// lane — rule lowering, practice interiors — to turn such a clause into a typed
/// [`AtomExpr`] rather than growing its own parser. It recognizes exactly the
/// three fact shapes the top-level parser accepts, mapped identically:
///
/// * a relation atom `pred(args…)`,
/// * a triple `fact <subject> <relation> <object>` → `relation(subject, object)`,
/// * a slot fact `<subject>.<slot> = <value>` → `slot(subject, value)`.
///
/// A trailing provenance clause is tolerated (and ignored). Anything else — a
/// rule body `head :- body`, an operator expression, free text — yields `None`.
///
/// ```
/// use dli_syntax::parse_clause_atom;
/// use dli_ast::clause::Clause;
/// use dli_ast::span::Span;
///
/// let clause = |t: &str| Clause { text: t.to_owned(), span: Span::synthetic() };
///
/// let atom = parse_clause_atom(&clause("adjacent(the-shallows, saltmarket)")).unwrap();
/// assert_eq!(atom.pred.text, "adjacent");
/// assert_eq!(atom.args.len(), 2);
///
/// // A slot fact maps to `slot(subject, value)`.
/// let slot = parse_clause_atom(&clause("mara.trade = eel-fisher")).unwrap();
/// assert_eq!(slot.pred.text, "trade");
///
/// // A rule body is not a single atom.
/// assert!(parse_clause_atom(&clause("adjacent(A, B) :- adjacent(B, A)")).is_none());
/// ```
#[must_use]
pub fn parse_clause_atom(clause: &Clause) -> Option<AtomExpr> {
    let tokens: Vec<Token> = crate::lexer::lex(&clause.text, clause.span.file)
        .into_iter()
        .filter(|t| t.kind != TokenKind::Newline)
        .collect();
    let first = tokens.first()?;
    let mut sink: Vec<Diagnostic> = Vec::new();

    // `fact <subject> <relation> <object>` — the triple form.
    if first.kind == TokenKind::Word && first.text == "fact" {
        let mut cur = Cursor::new(&tokens);
        cur.bump();
        let decl = parse_fact_triple(&mut cur, &mut sink, clause.span)?;
        return cur.at_end().then_some(decl.atom);
    }

    // `<subject>.<slot> = <value>` — the slot-fact form.
    if first.kind == TokenKind::Word
        && first.text.contains('.')
        && tokens.get(1).map(|t| t.kind) == Some(TokenKind::Equals)
    {
        let mut cur = Cursor::new(&tokens);
        let attr = parse_attr(&mut cur, &mut sink, clause.span)?;
        if !cur.at_end() {
            return None;
        }
        return Some(AtomExpr {
            pred: attr.attr,
            args: vec![Term::Sym(attr.subject), attr.value],
        });
    }

    // `pred(args…)` — the relation form.
    if tokens.get(1).map(|t| t.kind) == Some(TokenKind::LParen) {
        let mut cur = Cursor::new(&tokens);
        let atom = parse_atom(&mut cur)?;
        let _ = parse_provenance(&mut cur, &mut sink);
        return cur.at_end().then_some(atom);
    }

    None
}

/// Parse a comma-separated proof argument list into semantic values.
///
/// This uses the language lexer, so a comma inside a quoted string is part of
/// one value rather than an argument separator. The proof parser validates the
/// same term/comma shape before a document can be replayed.
#[must_use]
pub fn parse_proof_arguments(text: &str) -> Option<Vec<String>> {
    let tokens: Vec<Token> = crate::lexer::lex(text, FileId(0))
        .into_iter()
        .filter(|token| token.kind != TokenKind::Newline)
        .collect();
    let mut cur = Cursor::new(&tokens);
    let mut values = Vec::new();
    if cur.at_end() {
        return Some(values);
    }
    loop {
        values.push(proof_term_value(cur.eat_term()?));
        if cur.at_end() {
            return Some(values);
        }
        if cur.peek_kind() != Some(TokenKind::Comma) {
            return None;
        }
        cur.bump();
        if cur.at_end() {
            return None;
        }
    }
}

/// Parse `with <name> = <term>, ...` proof bindings into semantic values.
///
/// The leading `with` is required. Quoted values are unquoted by the lexer and
/// may contain commas without changing the binding count.
#[must_use]
pub fn parse_proof_bindings(text: &str) -> Option<Vec<(String, String)>> {
    let tokens: Vec<Token> = crate::lexer::lex(text, FileId(0))
        .into_iter()
        .filter(|token| token.kind != TokenKind::Newline)
        .collect();
    let mut cur = Cursor::new(&tokens);
    if cur.peek_word_text() != Some("with") {
        return None;
    }
    cur.bump();
    let mut bindings = Vec::new();
    loop {
        let name = cur.eat_ident()?.text;
        if cur.peek_kind() != Some(TokenKind::Equals) {
            return None;
        }
        cur.bump();
        let value = proof_term_value(cur.eat_term()?);
        bindings.push((name, value));
        if cur.at_end() {
            return Some(bindings);
        }
        if cur.peek_kind() != Some(TokenKind::Comma) {
            return None;
        }
        cur.bump();
        if cur.at_end() {
            return None;
        }
    }
}

/// Split a proof close body into its claim and optional expectation.
///
/// The separator is the `expect` token after the complete balanced claim, not
/// matching text inside a quoted argument.
#[must_use]
pub fn parse_proof_close_parts(text: &str) -> Option<(String, Option<String>)> {
    let tokens: Vec<Token> = crate::lexer::lex(text, FileId(0))
        .into_iter()
        .filter(|token| token.kind != TokenKind::Newline)
        .collect();
    let mut cur = Cursor::new(&tokens);
    if !parse_proof_claim(&mut cur) {
        return None;
    }
    let atom_end = usize::try_from(tokens.get(cur.pos.checked_sub(1)?)?.span.end).ok()?;
    let atom = text.get(..atom_end)?.trim().to_owned();
    if cur.at_end() {
        return Some((atom, None));
    }
    if cur.peek_word_text() != Some("expect") {
        return None;
    }
    cur.bump();
    let verdict_start = usize::try_from(cur.peek()?.span.start).ok()?;
    let verdict = text.get(verdict_start..)?.trim();
    (!verdict.is_empty()).then(|| (atom, Some(verdict.to_owned())))
}

fn proof_term_value(term: Term) -> String {
    match term {
        Term::Var(value) => format!("${}", value.text),
        Term::Sym(value) | Term::Time(TimeRef::Era(value) | TimeRef::Event(value)) => value.text,
        Term::Str(value) => value,
        Term::Int(value) => value.to_string(),
    }
}

/// Structure a captured [`Clause`] into a derivation rule's head atom and its
/// body literals, when it is a `head :- body` (or `head : body`) datalog rule.
///
/// A sibling of [`parse_clause_atom`]: where that turns a clause into a single
/// atom, this turns a clause into a **rule**. The head is one atom; the body is
/// a comma-separated conjunction of literals after a `:-` (or `:`) separator, and
/// a literal prefixed by `not` is negated (`docs/spec/01-syntax.md`). Atoms may
/// be nullary (a bare predicate with no argument list). Surface variables are
/// `$`-prefixed (`$g`), matching the term grammar.
///
/// Returns `None` for anything that is not a clean datalog rule — no separator,
/// a checker-rule `requires`/operator body, unbalanced parens, or trailing junk
/// — so a non-rule clause is left to the lane that owns it.
///
/// ```
/// use dli_syntax::parse_rule_clause;
/// use dli_ast::clause::Clause;
/// use dli_ast::span::Span;
///
/// let clause = |t: &str| Clause { text: t.to_owned(), span: Span::synthetic() };
///
/// let (head, body) = parse_rule_clause(&clause("ward-holds($g) :- seated($g), not breached($g)")).unwrap();
/// assert_eq!(head.pred.text, "ward-holds");
/// assert_eq!(body.len(), 2);
/// assert!(!body[0].negated); // seated($g)
/// assert!(body[1].negated);  // not breached($g)
///
/// // A plain atom is not a rule (no separator).
/// assert!(parse_rule_clause(&clause("adjacent(a, b)")).is_none());
/// ```
#[must_use]
pub fn parse_rule_clause(clause: &Clause) -> Option<(AtomExpr, Vec<BodyLiteral>)> {
    let tokens: Vec<Token> = crate::lexer::lex(&clause.text, clause.span.file)
        .into_iter()
        .filter(|t| t.kind != TokenKind::Newline)
        .collect();
    let mut cur = Cursor::new(&tokens);

    let head = parse_rule_atom(&mut cur)?;
    // The head and body are split by a `:-` or `:` separator (a bareword token).
    match cur.peek_word_text() {
        Some(":-" | ":") => cur.bump(),
        _ => return None,
    }

    let mut body = Vec::new();
    loop {
        let negated = if cur.peek_word_text() == Some("not") {
            cur.bump();
            true
        } else {
            false
        };
        let atom = parse_rule_atom(&mut cur)?;
        body.push(BodyLiteral { atom, negated });
        if cur.peek_kind() == Some(TokenKind::Comma) {
            cur.bump();
        } else {
            break;
        }
    }

    // Nothing may be silently dropped (koan 4): a trailing operator body or
    // `requires` clause means this is not a plain datalog rule.
    cur.at_end().then_some((head, body))
}

/// Parse a rule atom `pred` or `pred(args…)`. Unlike [`parse_atom`], a nullary
/// atom (a bare predicate with no argument list) is accepted, so a propositional
/// rule (`p :- not q`) parses.
fn parse_rule_atom(cur: &mut Cursor) -> Option<AtomExpr> {
    let pred = cur.eat_ident()?;
    if cur.peek_kind() != Some(TokenKind::LParen) {
        return Some(AtomExpr {
            pred,
            args: Vec::new(),
        });
    }
    cur.bump();
    let mut args = Vec::new();
    if cur.peek_kind() != Some(TokenKind::RParen) {
        loop {
            args.push(cur.eat_term()?);
            if cur.peek_kind() == Some(TokenKind::Comma) {
                cur.bump();
            } else {
                break;
            }
        }
    }
    if cur.peek_kind() == Some(TokenKind::RParen) {
        cur.bump();
        Some(AtomExpr { pred, args })
    } else {
        None // unbalanced parentheses → not a clean atom
    }
}

/// Emit a `malformed` error if the record has unconsumed trailing tokens.
///
/// Nothing on a line may be silently dropped (koan 4): a declaration that read
/// its structured fields but left junk behind is flagged — showing the shape it
/// should have taken — rather than quietly discarding the surplus.
fn expect_consumed(cur: &Cursor, diags: &mut Vec<Diagnostic>, shape: &str) {
    if let Some(tok) = cur.peek() {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            format!("unexpected `{}` — {shape}", tok.text),
            tok.span,
        ));
    }
}

/// The authoritative set of keywords a declaration line may open with — exactly
/// what [`parse_document`] dispatches on. This is the single source of truth for
/// the grammar reference; the demoted probe's `KEYWORDS` re-exports it, so the
/// two can never drift.
pub const LINE_OPEN_KEYWORDS: &[&str] = &[
    "attest",
    "banned",
    "before",
    "cycle",
    "dialect",
    "draft",
    "era",
    "event",
    "export",
    "fact",
    "fork",
    "hypothesis",
    "kind",
    "name",
    "name-part",
    "narration",
    "option",
    "person",
    "place",
    "practice",
    "principle",
    "proof",
    "question",
    "ratify",
    "rule",
    "scope",
    "state",
    "term",
    "thing",
    "use",
    "variant",
    "word",
];

/// The authoritative line-open keyword set (see [`LINE_OPEN_KEYWORDS`]).
#[must_use]
pub fn line_open_keywords() -> &'static [&'static str] {
    LINE_OPEN_KEYWORDS
}

/// A parsed item before options are folded into their question or fork.
enum Item {
    Decl(Decl),
    Option(OptionClause, Span),
}

/// A logical record: a leading line plus its captured body clauses.
struct Record {
    head: Line,
    body: Vec<Clause>,
    /// Absolute source tokens for each body clause, before whitespace
    /// normalization. Kept parallel to `body` so child diagnostics retain
    /// exact spans without adding parser-only data to the normative AST.
    body_tokens: Vec<Vec<Token>>,
    /// Header tokens with any trailing `{` block-opener removed.
    head_tokens: Vec<Token>,
}

/// Group prepared lines into logical records.
///
/// A braced block that runs off the end of the file without its closing `}` is
/// diagnosed here as [`DiagClass::UnterminatedBlock`] rather than silently
/// swallowing every following record into one runaway body (koan 4).
fn assemble_records(lines: &[Line], file: FileId, diags: &mut Vec<Diagnostic>) -> Vec<Record> {
    let mut records = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let head = &lines[i];
        if head.is_blank() {
            i += 1;
            continue;
        }
        i += 1;
        if raw_opens_brace(&head.raw) {
            let (body, body_tokens, next, terminated) = collect_brace_body(head, lines, i, file);
            i = next;
            if !terminated {
                diags.push(Diagnostic::new(
                    DiagClass::UnterminatedBlock,
                    "this block is opened with `{` and never closed — add the `}` that ends it.",
                    head.span,
                ));
            }
            records.push(Record {
                head_tokens: tokens_before_brace(&head.tokens),
                head: head.clone(),
                body,
                body_tokens,
            });
        } else {
            let mut body = Vec::new();
            let mut body_tokens = Vec::new();
            while i < lines.len() {
                let line = &lines[i];
                if line.is_blank() || line.indent <= head.indent {
                    break;
                }
                body.push(Clause {
                    text: normalize_ws(&line.raw),
                    span: line.span,
                });
                body_tokens.push(line.tokens.clone());
                i += 1;
            }
            records.push(Record {
                head_tokens: head.tokens.clone(),
                head: head.clone(),
                body,
                body_tokens,
            });
        }
    }
    records
}

/// The byte offset of the first `{` that is not inside a string literal, if any.
///
/// A `{` inside a string (a label like `"a {big weir"`) is ordinary text and
/// must never open a block — doing so would swallow every following record into
/// a runaway body. Balanced inline braces (`ratify d { … }`) still count as an
/// opener because there *is* an out-of-string `{`.
fn open_brace_pos(raw: &str) -> Option<usize> {
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in raw.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_str => escaped = true,
            '"' => in_str = !in_str,
            '{' if !in_str => return Some(i),
            _ => {}
        }
    }
    None
}

/// Whether a line opens a `{` block outside any string literal.
fn raw_opens_brace(raw: &str) -> bool {
    open_brace_pos(raw).is_some()
}

/// The header tokens up to (not including) the first `{`.
fn tokens_before_brace(tokens: &[Token]) -> Vec<Token> {
    tokens
        .iter()
        .take_while(|t| t.kind != TokenKind::LBrace)
        .cloned()
        .collect()
}

/// Collect the clauses inside a braced block starting on `head`'s line.
///
/// Returns the captured clauses, the index of the line after the block, and
/// whether the block was actually closed — a `false` third field means the `{`
/// ran off the end of the file with no matching `}`.
fn collect_brace_body(
    head: &Line,
    lines: &[Line],
    start: usize,
    file: FileId,
) -> (Vec<Clause>, Vec<Vec<Token>>, usize, bool) {
    let mut body = Vec::new();
    let mut body_tokens = Vec::new();
    let mut depth = 1i32;
    // The remainder of the header line after its first out-of-string `{`.
    if let Some(pos) = open_brace_pos(&head.raw) {
        let after = &head.raw[pos + 1..];
        let base = head.span.start.saturating_add(to_u32(pos + 1));
        consume_brace_segment(after, base, file, &mut depth, &mut body, &mut body_tokens);
    }
    let mut i = start;
    while depth > 0 && i < lines.len() {
        let line = &lines[i];
        consume_brace_segment(
            &line.raw,
            line.span.start,
            file,
            &mut depth,
            &mut body,
            &mut body_tokens,
        );
        i += 1;
    }
    (body, body_tokens, i, depth == 0)
}

/// Fold one physical segment of a braced block into `body`, tracking depth.
fn consume_brace_segment(
    seg: &str,
    base: u32,
    file: FileId,
    depth: &mut i32,
    body: &mut Vec<Clause>,
    body_tokens: &mut Vec<Vec<Token>>,
) {
    let mut text = String::new();
    let mut in_str = false;
    let mut escaped = false;
    for c in seg.chars() {
        match c {
            _ if escaped => {
                escaped = false;
                text.push(c);
            }
            '\\' if in_str => {
                escaped = true;
                text.push(c);
            }
            '"' => {
                in_str = !in_str;
                text.push(c);
            }
            '{' if !in_str => *depth += 1,
            '}' if !in_str => {
                *depth -= 1;
                if *depth == 0 {
                    break;
                }
            }
            _ => text.push(c),
        }
    }
    let norm = normalize_ws(&text);
    if !norm.is_empty() {
        let leading = text.len().saturating_sub(text.trim_start().len());
        let start = base.saturating_add(to_u32(leading));
        body.push(Clause {
            text: norm,
            span: Span {
                file,
                start,
                end: base.saturating_add(to_u32(text.len())),
            },
        });
        body_tokens.push(
            crate::lexer::lex(seg, file)
                .into_iter()
                .filter(|token| {
                    token.kind != TokenKind::Newline
                        && token.kind != TokenKind::LBrace
                        && token.kind != TokenKind::RBrace
                })
                .map(|mut token| {
                    token.span.start = token.span.start.saturating_add(base);
                    token.span.end = token.span.end.saturating_add(base);
                    token
                })
                .collect(),
        );
    }
}

/// Collapse whitespace runs to single spaces and trim the ends.
fn normalize_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut pending_space = false;
    for ch in text.trim().chars() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            in_string = true;
            out.push(ch);
        } else if ch.is_whitespace() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(ch);
        }
    }
    out
}

/// Parse one record into zero or more items, recording any diagnostics.
///
/// The long `match` is the keyword dispatch table — one arm per surface
/// keyword — so its length is inherent, not accidental.
#[allow(clippy::too_many_lines)]
fn parse_record(record: &Record, items: &mut Vec<Item>, diags: &mut Vec<Diagnostic>) {
    let toks = &record.head_tokens;
    let Some(first) = toks.first() else {
        return;
    };
    let span = record.head.span;

    if first.kind != TokenKind::Word {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a declaration opens with a keyword — this line does not.",
            span,
        ));
        return;
    }

    let mut cur = Cursor::new(toks);
    let keyword = first.text.clone();
    cur.bump(); // consume the leading keyword

    match keyword.as_str() {
        "era" | "place" | "person" | "thing" | "event" => {
            if let Some(decl) = parse_entity(&keyword, &mut cur, record, diags, span) {
                items.push(Item::Decl(Decl::Entity(decl)));
            }
        }
        "question" => items.push(Item::Decl(Decl::Question(parse_question(
            &mut cur, record, diags,
        )))),
        "fork" => items.push(Item::Decl(Decl::Fork(parse_fork(&mut cur, record, diags)))),
        "option" => {
            if let Some(opt) = parse_option(&mut cur, diags, span) {
                items.push(Item::Option(opt, span));
            }
        }
        "before" => {
            if let Some(decl) = parse_before(&mut cur, diags, span) {
                items.push(Item::Decl(Decl::Before(decl)));
            }
        }
        "hypothesis" => {
            if let Some(decl) = parse_hypothesis(&mut cur, span, diags) {
                items.push(Item::Decl(Decl::Hypothesis(decl)));
            }
        }
        "fact" => {
            if let Some(decl) = parse_fact_triple(&mut cur, diags, span) {
                items.push(Item::Decl(Decl::Fact(decl)));
            }
        }
        "use" => match cur.eat_ident() {
            Some(target) => {
                expect_consumed(&cur, diags, "a `use` reads `use <target>`.");
                items.push(Item::Decl(Decl::Use(UseDecl { target, span })));
            }
            None => missing_name(diags, "use", span),
        },
        "kind" => match parse_simple_named(&mut cur, diags, span) {
            Some((name, provenance)) => {
                expect_consumed(&cur, diags, "a `kind` reads `kind <name> [origin]`.");
                items.push(Item::Decl(Decl::Kind(KindDecl { name, provenance })));
            }
            None => missing_name(diags, "kind", span),
        },
        "rule" => match cur.eat_ident() {
            Some(name) => {
                let domain = parse_rule_domain(&mut cur);
                let (head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Rule(RuleDecl {
                    name,
                    domain,
                    head,
                    body: record.body.clone(),
                    provenance,
                })));
            }
            None => missing_name(diags, "rule", span),
        },
        "state" => {
            if let Some(decl) = parse_state(&mut cur, diags, span) {
                items.push(Item::Decl(Decl::State(decl)));
            }
        }
        "cycle" => {
            if let Some(decl) = parse_cycle(&mut cur, diags, span) {
                items.push(Item::Decl(Decl::Cycle(decl)));
            }
        }
        "practice" => {
            if let Some((name, label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "practice")
            {
                items.push(Item::Decl(Decl::Practice(PracticeDecl {
                    name,
                    label,
                    body,
                    provenance,
                })));
            }
        }
        "dialect" => {
            if let Some((name, label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "dialect")
            {
                items.push(Item::Decl(Decl::Dialect(DialectDecl {
                    name,
                    label,
                    body,
                    provenance,
                })));
            }
        }
        "word" => match cur.eat_ident() {
            Some(name) => {
                let label = cur.eat_string();
                let mut name_parts = Vec::new();
                if cur.peek_kind() == Some(TokenKind::Equals) {
                    cur.bump();
                    while let Some(tok) = cur.peek() {
                        if tok.kind == TokenKind::Word && is_marker(&tok.text) {
                            break;
                        }
                        if matches!(tok.kind, TokenKind::Word | TokenKind::Int) {
                            name_parts.push(ident_of(tok));
                            cur.bump();
                        } else {
                            break;
                        }
                    }
                }
                let provenance = parse_provenance(&mut cur, diags);
                expect_consumed(
                    &cur,
                    diags,
                    "a `word` reads `word <name> [\"label\"] [= <parts…>] [origin]`.",
                );
                items.push(Item::Decl(Decl::Word(WordDecl {
                    name,
                    label,
                    name_parts,
                    provenance,
                })));
            }
            None => missing_name(diags, "word", span),
        },
        "name-part" => match cur.eat_ident() {
            Some(name) => {
                let gloss = cur.eat_string();
                let (head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::NamePart(NamePartDecl {
                    name,
                    gloss,
                    head,
                    provenance,
                })));
            }
            None => missing_name(diags, "name-part", span),
        },
        "draft" => {
            if let Some((name, label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "draft")
            {
                items.push(Item::Decl(Decl::Draft(DraftDecl {
                    name,
                    label,
                    body,
                    provenance,
                })));
            }
        }
        "attest" => match parse_simple_named(&mut cur, diags, span) {
            Some((question, provenance)) => {
                expect_consumed(
                    &cur,
                    diags,
                    "an `attest` reads `attest <question> [origin]`.",
                );
                items.push(Item::Decl(Decl::Attest(dli_ast::decl::AttestDecl {
                    question,
                    provenance,
                })));
            }
            None => missing_name(diags, "attest", span),
        },
        "export" => {
            if let Some((profile, label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "export")
            {
                items.push(Item::Decl(Decl::Export(ExportDecl {
                    profile,
                    label,
                    body,
                    provenance,
                })));
            }
        }
        "name" => match cur.eat_ident() {
            Some(name) => {
                let label = cur.eat_string();
                let (head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Name(NameDecl {
                    name,
                    label,
                    head,
                    body: record.body.clone(),
                    provenance,
                })));
            }
            None => missing_name(diags, "name", span),
        },
        "term" => match cur.eat_ident() {
            Some(name) => {
                let label = cur.eat_string();
                let (_head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Term(TermDecl {
                    name,
                    label,
                    provenance,
                })));
            }
            None => missing_name(diags, "term", span),
        },
        "banned" => {
            if let Some(decl) = parse_banned(&mut cur, span, diags) {
                items.push(Item::Decl(Decl::Banned(decl)));
            }
        }
        "narration" => {
            let (head, provenance) = split_head_prov(&mut cur, diags);
            items.push(Item::Decl(Decl::Narration(NarrationDecl {
                head,
                provenance,
            })));
        }
        "variant" => match cur.eat_ident() {
            Some(path) => {
                let (_head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Variant(VariantDecl {
                    path,
                    body: record.body.clone(),
                    provenance,
                })));
            }
            None => missing_name(diags, "variant", span),
        },
        "ratify" => match cur.eat_ident() {
            Some(decision) => {
                expect_consumed(&cur, diags, "a `ratify` reads `ratify <d-id> { … }`.");
                items.push(Item::Decl(Decl::Ratify(RatifyDecl {
                    decision,
                    body: record.body.clone(),
                })));
            }
            None => missing_name(diags, "ratify", span),
        },
        "principle" => match cur.eat_ident() {
            Some(name) => {
                let (head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Principle(PrincipleDecl {
                    name,
                    head,
                    body: record.body.clone(),
                    provenance,
                })));
            }
            None => missing_name(diags, "principle", span),
        },
        "proof" => match cur.eat_ident() {
            Some(name) => {
                let (head, provenance) = split_head_prov(&mut cur, diags);
                validate_proof_head(&head, span, diags);
                validate_proof_body(record, diags);
                items.push(Item::Decl(Decl::Proof(ProofDecl {
                    name,
                    head,
                    body: record.body.clone(),
                    provenance,
                })));
            }
            None => missing_name(diags, "proof", span),
        },
        _ if renamed_keyword_target(&keyword).is_some() => {
            // A killed form: the surface has one current word for each thing, and
            // an old spelling is refused with the word that replaced it (koan 7),
            // never silently accepted nor read as an unknown keyword.
            let target = renamed_keyword_target(&keyword).unwrap_or_default();
            diags.push(renamed_keyword(&keyword, target, first.span));
        }
        _ => {
            // Not a keyword. Two keyword-free surface forms are legal at top
            // level: a relation atom `pred(args)` and a slot fact
            // `subject.slot = value` (order-insensitive — a slot fact may stand
            // apart from its entity). Anything else is an unknown keyword.
            let second = toks.get(1).map(|t| t.kind);
            if second == Some(TokenKind::LParen) {
                let mut relcur = Cursor::new(toks);
                if let Some(atom) = parse_atom(&mut relcur) {
                    let provenance = parse_provenance(&mut relcur, diags);
                    expect_consumed(
                        &relcur,
                        diags,
                        "a relation reads `<pred>(<args>) [origin]`.",
                    );
                    items.push(Item::Decl(Decl::Fact(FactDecl {
                        atom,
                        triple: false,
                        provenance,
                    })));
                    return;
                }
            }
            if second == Some(TokenKind::Equals) && keyword.contains('.') {
                let mut attrcur = Cursor::new(toks);
                if let Some(attr) = parse_attr(&mut attrcur, diags, span) {
                    items.push(Item::Decl(Decl::Attr(attr)));
                    return;
                }
            }
            diags.push(Diagnostic::new(
                DiagClass::UnknownKeyword,
                format!(
                    "`{keyword}` is not a De Lore Is keyword — a declaration opens with a world word (era, place, person, thing, event, fact, question, …) or a relation like `pred(a, b)`."
                ),
                first.span,
            ));
        }
    }
}

const PROOF_CHILD_KEYWORDS: &[&str] = &[
    "premise",
    "step",
    "spend",
    "release",
    "redistribute",
    "reveal",
    "assume",
    "close",
];

fn validate_proof_head(head: &[String], span: Span, diags: &mut Vec<Diagnostic>) {
    if head.len() != 4
        || head[2] != "in"
        || head
            .iter()
            .any(|token| token.is_empty() || token.starts_with('"'))
    {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a proof header reads `proof <name> <role> <speaker> in <practice>`.",
            span,
        ));
    }
}

/// Guard the captured proof body before a malformed child can be folded into the
/// preceding operation and disappear during formatting or replay.
fn validate_proof_body(record: &Record, diags: &mut Vec<Diagnostic>) {
    let nonempty = record
        .body_tokens
        .iter()
        .filter(|tokens| !tokens.is_empty())
        .collect::<Vec<_>>();
    let closes = nonempty
        .iter()
        .enumerate()
        .filter(|(_, tokens)| {
            tokens
                .first()
                .is_some_and(|token| token.kind == TokenKind::Word && token.text == "close")
        })
        .collect::<Vec<_>>();
    match closes.as_slice() {
        [] => diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a proof must end with exactly one `close`.",
            record.head.span,
        )),
        [(position, tokens)] if *position + 1 != nonempty.len() => {
            diags.push(Diagnostic::new(
                DiagClass::Malformed,
                "the proof's single `close` must be its final operation.",
                tokens[0].span,
            ));
        }
        [_, second, ..] => diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a proof carries more than one `close`; its conclusion must be unique.",
            second.1[0].span,
        )),
        [_] => {}
    }

    for (clause, tokens) in record.body.iter().zip(&record.body_tokens) {
        let Some(first) = tokens.first() else {
            continue;
        };
        if first.kind != TokenKind::Word {
            diags.push(Diagnostic::new(
                DiagClass::Malformed,
                "a proof line opens with an operation keyword, not quoted text.",
                first.span,
            ));
            continue;
        }
        let keyword = first.text.as_str();
        if !PROOF_CHILD_KEYWORDS.contains(&keyword) {
            diags.push(Diagnostic::new(
                DiagClass::Malformed,
                format!(
                    "a proof line cannot begin with `{keyword}` — write premise, assume, step, \
                     spend, release, redistribute, reveal, or close."
                ),
                first.span,
            ));
            continue;
        }

        match keyword {
            "spend" | "release" | "redistribute" | "reveal" => {
                if tokens.len() != 2 || tokens[1].kind != TokenKind::Word {
                    let span = tokens.get(2).map_or(clause.span, |token| token.span);
                    diags.push(Diagnostic::new(
                        DiagClass::Malformed,
                        format!("unexpected proof text — `{keyword}` reads `{keyword} <label>`."),
                        span,
                    ));
                }
            }
            "close" => validate_proof_close(tokens, clause.span, diags),
            "premise" | "assume" => {
                validate_proof_premise(keyword, tokens, clause.span, diags);
            }
            "step" => validate_proof_step(tokens, clause.span, diags),
            _ => {}
        }
    }
}

fn validate_proof_close(tokens: &[Token], span: Span, diags: &mut Vec<Diagnostic>) {
    let mut cur = Cursor::new(tokens);
    cur.bump();
    if !parse_proof_claim(&mut cur) {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a proof close reads `close <atom> [expect holds|blocked|fails-at <error>]`.",
            span,
        ));
        return;
    }
    if cur.at_end() {
        return;
    }
    if cur.peek_word_text() != Some("expect") {
        proof_unexpected(
            &cur,
            diags,
            "a proof close ends after its atom or continues with `expect <verdict>`.",
        );
        return;
    }
    cur.bump();
    match cur.peek_word_text() {
        Some("holds" | "blocked") => cur.bump(),
        Some("fails-at") => {
            cur.bump();
            if cur.eat_ident().is_none() {
                diags.push(Diagnostic::new(
                    DiagClass::Malformed,
                    "`expect fails-at` must name the expected error.",
                    span,
                ));
                return;
            }
        }
        Some(_) => {
            // The proof checker owns the domain finding for an unknown verdict
            // token. Consume exactly one here so surplus text is still syntax.
            cur.bump();
        }
        None => {
            diags.push(Diagnostic::new(
                DiagClass::Malformed,
                "`expect` must name holds, blocked, or fails-at <error>.",
                span,
            ));
            return;
        }
    }
    proof_expect_end(
        &cur,
        diags,
        "a proof close reads `close <atom> [expect holds|blocked|fails-at <error>]`.",
    );
}

fn validate_proof_premise(
    keyword: &str,
    tokens: &[Token],
    span: Span,
    diags: &mut Vec<Diagnostic>,
) {
    let mut cur = Cursor::new(tokens);
    cur.bump();
    if cur.eat_ident().is_none() || !parse_proof_claim(&mut cur) {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            format!("a proof {keyword} reads `{keyword} <label> <claim> [evidence]`."),
            span,
        ));
        return;
    }
    if cur.peek_kind() == Some(TokenKind::Equals) {
        cur.bump();
        if cur.eat_term().is_none() {
            proof_unexpected(
                &cur,
                diags,
                "a proof comparison must name one value after `=`.",
            );
            return;
        }
    }
    let mut grade_seen: Option<&Token> = None;
    while !cur.at_end() {
        let Some(token) = cur.peek() else {
            break;
        };
        match token.text.as_str() {
            "cited" | "drawn-against" => {
                let modifier = token.text.clone();
                cur.bump();
                if cur.eat_ident().is_none() {
                    diags.push(Diagnostic::new(
                        DiagClass::Malformed,
                        format!("`{modifier}` must name one source label."),
                        token.span,
                    ));
                    return;
                }
            }
            "cue" => {
                let token_span = token.span;
                cur.bump();
                if cur.eat_string().is_none() {
                    diags.push(Diagnostic::new(
                        DiagClass::Malformed,
                        "`cue` must carry one quoted source cue.",
                        token_span,
                    ));
                    return;
                }
            }
            "from-scene" | "observed" | "asserted" | "attested" | "tested" | "checked" => {
                if matches!(
                    token.text.as_str(),
                    "asserted" | "attested" | "tested" | "checked"
                ) {
                    if !validate_proof_grade(keyword, token, grade_seen, diags) {
                        return;
                    }
                    grade_seen = Some(token);
                }
                cur.bump();
            }
            _ => {
                diags.push(Diagnostic::new(
                    DiagClass::Malformed,
                    format!(
                        "unexpected `{}` after proof evidence — write a cited source, cue, \
                         observation, draw, or grade.",
                        token.text
                    ),
                    token.span,
                ));
                return;
            }
        }
    }
}

fn validate_proof_grade(
    keyword: &str,
    grade: &Token,
    prior: Option<&Token>,
    diags: &mut Vec<Diagnostic>,
) -> bool {
    if let Some(prior) = prior {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            format!(
                "a proof {keyword} carries both `{}` and `{}` — write one evidence grade.",
                prior.text, grade.text
            ),
            grade.span,
        ));
        return false;
    }
    if keyword == "assume" && grade.text != "asserted" {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            format!(
                "an assumption is always `asserted`; it cannot claim `{}`.",
                grade.text
            ),
            grade.span,
        ));
        return false;
    }
    true
}

/// Consume one proof claim. A bare identifier is a nullary atom; parenthesized
/// atoms may carry nested function terms such as
/// `holds(person, consent(group, act))`.
fn parse_proof_claim(cur: &mut Cursor<'_>) -> bool {
    if cur.eat_ident().is_none() {
        return false;
    }
    if cur.peek_kind() != Some(TokenKind::LParen) {
        return true;
    }
    cur.bump();
    if cur.peek_kind() == Some(TokenKind::RParen) {
        cur.bump();
        return true;
    }
    loop {
        let nested = cur.remaining().get(1).map(|token| token.kind) == Some(TokenKind::LParen);
        if (nested && !parse_proof_claim(cur)) || (!nested && cur.eat_term().is_none()) {
            return false;
        }
        match cur.peek_kind() {
            Some(TokenKind::Comma) => cur.bump(),
            Some(TokenKind::RParen) => {
                cur.bump();
                return true;
            }
            _ => return false,
        }
    }
}

fn validate_proof_step(tokens: &[Token], span: Span, diags: &mut Vec<Diagnostic>) {
    if tokens.len() < 3 || tokens[1].kind != TokenKind::Word || tokens[2].kind != TokenKind::Word {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a proof step reads `step <label> <operation> ...`.",
            span,
        ));
        return;
    }
    match tokens[2].text.as_str() {
        "instantiate" => validate_instantiate_step(tokens, span, diags),
        "chain" => validate_chain_step(tokens, span, diags),
        _ => validate_custom_step(tokens, span, diags),
    }
}

fn validate_custom_step(tokens: &[Token], span: Span, diags: &mut Vec<Diagnostic>) {
    let mut cur = Cursor::new(tokens);
    cur.bump();
    let _ = cur.eat_ident();
    cur.bump();
    if cur.at_end() {
        return;
    }
    loop {
        if cur.eat_term().is_none() {
            proof_unexpected(
                &cur,
                diags,
                "a custom step reads `step <label> <declared-rule> [<value>, ...]`.",
            );
            return;
        }
        if cur.at_end() {
            return;
        }
        if cur.peek_kind() != Some(TokenKind::Comma) {
            proof_unexpected(
                &cur,
                diags,
                "custom-step arguments are comma-separated terms.",
            );
            return;
        }
        cur.bump();
        if cur.at_end() {
            diags.push(Diagnostic::new(
                DiagClass::Malformed,
                "a custom step cannot end with an empty argument.",
                span,
            ));
            return;
        }
    }
}

fn validate_instantiate_step(tokens: &[Token], span: Span, diags: &mut Vec<Diagnostic>) {
    let mut cur = Cursor::new(tokens);
    cur.bump();
    let _ = cur.eat_ident();
    cur.bump();
    if cur.eat_ident().is_none() || cur.peek_word_text() != Some("with") {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "an instantiate step reads `step <label> instantiate <form> with <name> = <value> ...`.",
            span,
        ));
        return;
    }
    cur.bump();
    loop {
        if cur.eat_ident().is_none() || cur.peek_kind() != Some(TokenKind::Equals) {
            proof_unexpected(
                &cur,
                diags,
                "a binding reads `<name> = <value>`, with bindings separated by commas.",
            );
            return;
        }
        cur.bump();
        if cur.eat_term().is_none() {
            proof_unexpected(&cur, diags, "a binding must carry one value after `=`.");
            return;
        }
        if cur.peek_kind() != Some(TokenKind::Comma) {
            break;
        }
        cur.bump();
    }
    proof_expect_end(
        &cur,
        diags,
        "an instantiate step ends after its comma-separated bindings.",
    );
}

fn validate_chain_step(tokens: &[Token], span: Span, diags: &mut Vec<Diagnostic>) {
    let mut cur = Cursor::new(tokens);
    cur.bump();
    let _ = cur.eat_ident();
    cur.bump();
    if cur.peek_word_text() != Some("from") {
        proof_unexpected(
            &cur,
            diags,
            "a chain step reads `step <label> chain from <labels> by <law> conclusion <atom>`.",
        );
        return;
    }
    cur.bump();
    let mut joins = 0usize;
    loop {
        if cur.peek_word_text() == Some("by") {
            break;
        }
        if cur.eat_ident().is_none() {
            proof_unexpected(&cur, diags, "a chain's `from` list names prior labels.");
            return;
        }
        joins += 1;
        if cur.peek_kind() == Some(TokenKind::Comma) {
            cur.bump();
            if cur.at_end() || cur.peek_word_text() == Some("by") {
                diags.push(Diagnostic::new(
                    DiagClass::Malformed,
                    "a chain's `from` list cannot end with an empty label.",
                    cur.here(),
                ));
                return;
            }
        } else if cur.peek_word_text() != Some("by") {
            proof_unexpected(
                &cur,
                diags,
                "chain labels are comma-separated and followed by `by <law>`.",
            );
            return;
        }
    }
    if joins == 0 {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a chain step must name at least one prior label after `from`.",
            span,
        ));
        return;
    }
    cur.bump();
    if cur.eat_ident().is_none() || cur.peek_word_text() != Some("conclusion") {
        proof_unexpected(
            &cur,
            diags,
            "a chain's law is followed by `conclusion <atom>`.",
        );
        return;
    }
    cur.bump();
    if !parse_proof_claim(&mut cur) {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a chain's conclusion must be one relation atom.",
            span,
        ));
        return;
    }
    proof_expect_end(&cur, diags, "a chain step ends after its conclusion atom.");
}

fn proof_expect_end(cur: &Cursor, diags: &mut Vec<Diagnostic>, shape: &str) {
    if !cur.at_end() {
        proof_unexpected(cur, diags, shape);
    }
}

fn proof_unexpected(cur: &Cursor, diags: &mut Vec<Diagnostic>, shape: &str) {
    let message = cur.peek().map_or_else(
        || format!("unexpected end of proof line — {shape}"),
        |token| format!("unexpected `{}` — {shape}", token.text),
    );
    diags.push(Diagnostic::new(DiagClass::Malformed, message, cur.here()));
}

/// Fold standalone `option` items into the preceding question or fork.
fn fold_options(items: Vec<Item>, diags: &mut Vec<Diagnostic>) -> Vec<Decl> {
    let mut decls: Vec<Decl> = Vec::new();
    for item in items {
        match item {
            Item::Decl(decl) => decls.push(decl),
            Item::Option(opt, span) => match decls.last_mut() {
                Some(Decl::Question(q)) => q.options.push(opt),
                Some(Decl::Fork(f)) => f.options.push(opt),
                _ => diags.push(Diagnostic::new(
                    DiagClass::StrayOption,
                    "an `option` belongs to a `question` or a `fork` — none is open above it.",
                    span,
                )),
            },
        }
    }
    decls
}

// --- entity ---------------------------------------------------------------

fn parse_entity(
    keyword: &str,
    cur: &mut Cursor,
    record: &Record,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Option<EntityDecl> {
    let kind = match keyword {
        "era" => EntityKind::Era,
        "place" => EntityKind::Place,
        "person" => EntityKind::Person,
        "thing" => EntityKind::Thing,
        "event" => EntityKind::Event,
        _ => return None,
    };
    let Some(name) = cur.eat_ident() else {
        missing_name(diags, keyword, span);
        return None;
    };

    // An `era name < a < b < c [provenance]` precedence chain (which may wrap
    // into the body). Flatten the header remainder and any continuation lines
    // into one token stream, take chain members up to the first provenance
    // marker, then parse the trailing annotation — so `decided d-0001` is read
    // as provenance, never swallowed into the chain as two fake eras (V1).
    if cur.peek_word_text() == Some("<") {
        let mut toks: Vec<Token> = cur.remaining().to_vec();
        for clause in &record.body {
            toks.extend(
                crate::lexer::lex(&clause.text, clause.span.file)
                    .into_iter()
                    .filter(|t| t.kind != TokenKind::Newline),
            );
        }
        let mut chain_cur = Cursor::new(&toks);
        let mut chain = Vec::new();
        while let Some(tok) = chain_cur.peek() {
            if tok.kind == TokenKind::Word && tok.text == "<" {
                chain_cur.bump();
            } else if tok.kind == TokenKind::Word && is_marker(&tok.text) {
                break;
            } else if matches!(tok.kind, TokenKind::Word | TokenKind::Int) {
                chain.push(ident_of(tok));
                chain_cur.bump();
            } else {
                break;
            }
        }
        let provenance = parse_provenance(&mut chain_cur, diags);
        return Some(EntityDecl {
            kind,
            name,
            label: None,
            time: Some(TimeClause::Order(chain)),
            slots: Vec::new(),
            provenance,
        });
    }

    let label = cur.eat_string();
    let time = parse_time_clause(cur);
    let provenance = parse_provenance(cur, diags);
    expect_consumed(
        cur,
        diags,
        "an entity reads `<kind> <name> [\"label\"] [after|during …] [origin]`.",
    );
    let slots = record
        .body
        .iter()
        .filter_map(|clause| parse_attr_clause(clause, diags))
        .collect();

    Some(EntityDecl {
        kind,
        name,
        label,
        time,
        slots,
        provenance,
    })
}

fn parse_time_clause(cur: &mut Cursor) -> Option<TimeClause> {
    let kw = cur.peek_word_text()?;
    let clause = match kw {
        "after" => TimeClause::After,
        "during" => TimeClause::During,
        "since" => TimeClause::Since,
        "until" => TimeClause::Until,
        _ => return None,
    };
    cur.bump();
    let target = cur.eat_ident()?;
    Some(clause(target))
}

fn parse_attr_clause(clause: &Clause, diags: &mut Vec<Diagnostic>) -> Option<AttrDecl> {
    let toks = crate::lexer::lex(&clause.text, clause.span.file);
    let toks: Vec<Token> = toks
        .into_iter()
        .filter(|t| t.kind != TokenKind::Newline)
        .collect();
    let mut cur = Cursor::new(&toks);
    let decl = parse_attr(&mut cur, diags, clause.span);
    if decl.is_none() {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            format!(
                "`{}` is not a slot fact — write `subject.slot = value`.",
                clause.text
            ),
            clause.span,
        ));
    }
    decl
}

// --- facts and attributes -------------------------------------------------

fn parse_attr(cur: &mut Cursor, diags: &mut Vec<Diagnostic>, span: Span) -> Option<AttrDecl> {
    let head = cur.eat_word_text()?;
    let (subject_text, attr_text) = head.split_once('.')?;
    let subject = Ident {
        text: subject_text.to_owned(),
        span,
    };
    let attr = Ident {
        text: attr_text.to_owned(),
        span,
    };
    if cur.peek_kind() != Some(TokenKind::Equals) {
        return None;
    }
    cur.bump();
    let value = cur.eat_term()?;
    let provenance = parse_provenance(cur, diags);
    expect_consumed(
        cur,
        diags,
        "a slot fact reads `<subject>.<slot> = <value> [origin]`.",
    );
    Some(AttrDecl {
        subject,
        attr,
        value,
        provenance,
    })
}

fn parse_fact_triple(
    cur: &mut Cursor,
    diags: &mut Vec<Diagnostic>,
    span: Span,
) -> Option<FactDecl> {
    const SHAPE: &str = "a `fact` reads `fact <subject> <relation> <object>`.";
    let Some(subject) = cur.eat_ident() else {
        diags.push(Diagnostic::new(DiagClass::Malformed, SHAPE, span));
        return None;
    };
    let Some(pred) = cur.eat_ident() else {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            format!("{SHAPE} the relation and object are missing"),
            span,
        ));
        return None;
    };
    let Some(object) = cur.eat_term() else {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            format!("{SHAPE} the object is missing"),
            span,
        ));
        return None;
    };
    let atom = AtomExpr {
        pred,
        args: vec![Term::Sym(subject), object],
    };
    let provenance = parse_provenance(cur, diags);
    expect_consumed(cur, diags, SHAPE);
    Some(FactDecl {
        atom,
        triple: true,
        provenance,
    })
}

fn parse_atom(cur: &mut Cursor) -> Option<AtomExpr> {
    let pred = cur.eat_ident()?;
    if cur.peek_kind() != Some(TokenKind::LParen) {
        return None;
    }
    cur.bump();
    let mut args = Vec::new();
    if cur.peek_kind() != Some(TokenKind::RParen) {
        loop {
            let term = cur.eat_term()?;
            args.push(term);
            match cur.peek_kind() {
                Some(TokenKind::Comma) => {
                    cur.bump();
                }
                _ => break,
            }
        }
    }
    if cur.peek_kind() != Some(TokenKind::RParen) {
        return None;
    }
    cur.bump();
    Some(AtomExpr { pred, args })
}

// --- questions, forks, options -------------------------------------------

fn parse_question(cur: &mut Cursor, record: &Record, diags: &mut Vec<Diagnostic>) -> QuestionDecl {
    let name = cur.eat_ident().unwrap_or_else(|| {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a `question` reads `question <name> [\"<prompt>\"]` — the name is missing.",
            cur.here(),
        ));
        synthetic_ident(cur)
    });
    let prompt = cur.eat_string();
    let provenance = parse_provenance(cur, diags);
    expect_consumed(
        cur,
        diags,
        "a `question` reads `question <name> [\"<prompt>\"]`.",
    );
    let mut options = Vec::new();
    for (clause, tokens) in record.body.iter().zip(&record.body_tokens) {
        match parse_option_clause(clause, tokens, diags) {
            ChildOption::Parsed(option) => options.push(option),
            ChildOption::Malformed => {}
            ChildOption::NotOption => non_option_child("question", clause, diags),
        }
    }
    QuestionDecl {
        name,
        prompt,
        options,
        provenance,
    }
}

fn parse_fork(cur: &mut Cursor, record: &Record, diags: &mut Vec<Diagnostic>) -> ForkDecl {
    let question = cur.eat_ident().unwrap_or_else(|| synthetic_ident(cur));
    let provenance = parse_provenance(cur, diags);
    let mut options = Vec::new();
    let mut body = Vec::new();
    for (clause, tokens) in record.body.iter().zip(&record.body_tokens) {
        match parse_option_clause(clause, tokens, diags) {
            ChildOption::Parsed(option) => options.push(option),
            ChildOption::Malformed => {}
            ChildOption::NotOption => {
                body.push(clause.clone());
            }
        }
    }
    ForkDecl {
        question,
        options,
        body,
        provenance,
    }
}

fn parse_option(cur: &mut Cursor, diags: &mut Vec<Diagnostic>, span: Span) -> Option<OptionClause> {
    let Some(name) = cur.eat_ident() else {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "an `option` needs a name — write `option <name> [\"<label>\"] [implies <atom>]`.",
            span,
        ));
        return None;
    };
    let label = cur.eat_string();
    let implies = if cur.peek_word_text() == Some("implies") {
        cur.bump();
        if let Some(atom) = parse_atom(cur) {
            Some(atom)
        } else {
            diags.push(Diagnostic::new(
                DiagClass::Malformed,
                "`implies` requires an atom.",
                span,
            ));
            None
        }
    } else {
        None
    };
    if cur.peek_word_text() == Some("assumes") {
        let killed_span = cur.peek().map_or(span, |token| token.span);
        cur.bump();
        let _ = cur.eat_ident();
        diags.push(Diagnostic::new(
            DiagClass::KilledForm,
            "`option ... assumes` was retired — put `assumes <hypothesis>` on the claim's provenance so the claim itself carries the hypothesis gate.",
            killed_span,
        ));
    }
    expect_consumed(
        cur,
        diags,
        "an `option` reads `option <name> [\"<label>\"] [implies <atom>]`.",
    );
    Some(OptionClause {
        name,
        label,
        implies,
    })
}

enum ChildOption {
    NotOption,
    Malformed,
    Parsed(OptionClause),
}

fn parse_option_clause(
    clause: &Clause,
    tokens: &[Token],
    diags: &mut Vec<Diagnostic>,
) -> ChildOption {
    let Some(first) = tokens.first() else {
        return ChildOption::NotOption;
    };
    if first.kind != TokenKind::Word || first.text != "option" {
        return ChildOption::NotOption;
    }
    let mut cur = Cursor::new(tokens);
    cur.bump();
    parse_option(&mut cur, diags, clause.span).map_or(ChildOption::Malformed, ChildOption::Parsed)
}

fn non_option_child(container: &str, clause: &Clause, diags: &mut Vec<Diagnostic>) {
    diags.push(Diagnostic::new(
        DiagClass::Malformed,
        format!(
            "a `{container}` body contains only `option` lines — move `{}` to its own declaration.",
            clause.text
        ),
        clause.span,
    ));
}

// --- timeline -------------------------------------------------------------

fn parse_before(cur: &mut Cursor, diags: &mut Vec<Diagnostic>, span: Span) -> Option<BeforeDecl> {
    let earlier = cur.eat_ident()?;
    // The arrow `->` is a lone word token between the two events.
    if cur.peek_word_text() == Some("->") {
        cur.bump();
    }
    let Some(later) = cur.eat_ident() else {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a `before` reads `before <earlier> -> <later>`.",
            span,
        ));
        return None;
    };
    let provenance = parse_provenance(cur, diags);
    expect_consumed(
        cur,
        diags,
        "a `before` reads `before <earlier> -> <later> [origin]`.",
    );
    Some(BeforeDecl {
        earlier,
        later,
        provenance,
    })
}

// --- hypotheses, banned ---------------------------------------------------

fn parse_hypothesis(
    cur: &mut Cursor,
    span: Span,
    diags: &mut Vec<Diagnostic>,
) -> Option<HypothesisDecl> {
    let Some(name) = cur.eat_ident() else {
        missing_name(diags, "hypothesis", span);
        return None;
    };
    let statement = cur.eat_string();
    expect_consumed(
        cur,
        diags,
        "a `hypothesis` reads `hypothesis <name> \"<statement>\"`.",
    );
    Some(HypothesisDecl { name, statement })
}

fn parse_banned(cur: &mut Cursor, span: Span, diags: &mut Vec<Diagnostic>) -> Option<BannedDecl> {
    let Some(word) = cur.eat_ident() else {
        missing_name(diags, "banned", span);
        return None;
    };
    let mut citations = Vec::new();
    while let Some(text) = cur.peek_word_text() {
        if is_citation(text) {
            citations.push(CitationRef {
                id: cur.eat_ident()?,
            });
        } else {
            break;
        }
    }
    let reason = cur.eat_string();
    let provenance = if citations.is_empty() {
        None
    } else {
        Some(Annotation {
            marker: ProvenanceMarker::Decided,
            citations,
            condition: None,
            tags: Vec::new(),
            span,
        })
    };
    expect_consumed(
        cur,
        diags,
        "a `banned` reads `banned <word> <d-id> \"<reason>\"`.",
    );
    Some(BannedDecl {
        word,
        reason,
        provenance,
    })
}

// --- states, cycles -------------------------------------------------------

fn parse_state(cur: &mut Cursor, diags: &mut Vec<Diagnostic>, span: Span) -> Option<StateDecl> {
    let Some(name) = cur.eat_ident() else {
        missing_name(diags, "state", span);
        return None;
    };
    let value = if cur.peek_kind() == Some(TokenKind::Equals) {
        cur.bump();
        cur.eat_term()
    } else {
        None
    };
    let time = parse_time_clause(cur);
    let provenance = parse_provenance(cur, diags);
    Some(StateDecl {
        name,
        value,
        time,
        provenance,
    })
}

fn parse_cycle(cur: &mut Cursor, diags: &mut Vec<Diagnostic>, span: Span) -> Option<CycleDecl> {
    let Some(name) = cur.eat_ident() else {
        missing_name(diags, "cycle", span);
        return None;
    };
    let mut modulus = None;
    if cur.peek_kind() == Some(TokenKind::LParen) {
        cur.bump();
        if let Some(text) = cur.eat_word_text() {
            modulus = text.parse::<i64>().ok();
        }
        if cur.peek_kind() == Some(TokenKind::RParen) {
            cur.bump();
        }
    }
    let mut spokes = Vec::new();
    if cur.peek_kind() == Some(TokenKind::Equals) {
        cur.bump();
        while let Some(tok) = cur.peek() {
            if tok.kind == TokenKind::Word && tok.text == "|" {
                cur.bump();
            } else if tok.kind == TokenKind::Word && !is_marker(&tok.text) {
                spokes.push(ident_of(tok));
                cur.bump();
            } else {
                break;
            }
        }
    }
    let provenance = parse_provenance(cur, diags);
    Some(CycleDecl {
        name,
        modulus,
        spokes,
        provenance,
    })
}

/// Read an optional `over <domain>` clause after a rule's name. Consumes the two
/// tokens only when both `over` and a known domain (`order`/`season`) are present;
/// otherwise the cursor is left untouched and the rule is a world rule.
fn parse_rule_domain(cur: &mut Cursor) -> Option<RuleDomain> {
    let rest = cur.remaining();
    if rest.first().map(|t| t.text.as_str()) != Some("over") {
        return None;
    }
    let domain = match rest.get(1).map(|t| t.text.as_str()) {
        Some("order") => RuleDomain::Order,
        Some("season") => RuleDomain::Season,
        _ => return None,
    };
    cur.bump(); // `over`
    cur.bump(); // the domain word
    Some(domain)
}

// --- generic keyword blocks ----------------------------------------------

type NamedBlock = (
    Ident,
    Option<String>,
    Vec<String>,
    Vec<Clause>,
    Option<Annotation>,
);

fn parse_named_block(
    cur: &mut Cursor,
    record: &Record,
    diags: &mut Vec<Diagnostic>,
    span: Span,
    keyword: &str,
) -> Option<NamedBlock> {
    let Some(name) = cur.eat_ident() else {
        missing_name(diags, keyword, span);
        return None;
    };
    let label = cur.eat_string();
    let (head, provenance) = split_head_prov(cur, diags);
    Some((name, label, head, record.body.clone(), provenance))
}

fn parse_simple_named(
    cur: &mut Cursor,
    diags: &mut Vec<Diagnostic>,
    _span: Span,
) -> Option<(Ident, Option<Annotation>)> {
    let name = cur.eat_ident()?;
    let provenance = parse_provenance(cur, diags);
    Some((name, provenance))
}

/// Collect header tokens up to the first provenance marker, then the provenance.
fn split_head_prov(
    cur: &mut Cursor,
    diags: &mut Vec<Diagnostic>,
) -> (Vec<String>, Option<Annotation>) {
    let mut head = Vec::new();
    while let Some(tok) = cur.peek() {
        if tok.kind == TokenKind::Word && is_marker(&tok.text) {
            break;
        }
        head.push(token_to_string(tok));
        cur.bump();
    }
    let provenance = parse_provenance(cur, diags);
    (head, provenance)
}

// --- provenance -----------------------------------------------------------

fn parse_provenance(cur: &mut Cursor, diags: &mut Vec<Diagnostic>) -> Option<Annotation> {
    let marker_text = cur.peek_word_text()?;
    let marker = match marker_text {
        "proposed" => ProvenanceMarker::Proposed,
        "decided" => ProvenanceMarker::Decided,
        "ratified" => ProvenanceMarker::Ratified,
        "provisional" => ProvenanceMarker::Provisional,
        _ => return None,
    };
    let marker_span = cur.peek().map_or(Span::synthetic(), |t| t.span);
    cur.bump();

    let mut citations = Vec::new();
    let mut condition = None;
    let mut tags = Vec::new();
    while let Some(tok) = cur.peek() {
        if tok.kind == TokenKind::Word && is_citation(&tok.text) {
            citations.push(CitationRef { id: ident_of(tok) });
            cur.bump();
        } else if tok.kind == TokenKind::Word && tok.text == "assumes" {
            cur.bump();
            condition = parse_condition(cur);
        } else if tok.kind == TokenKind::Word && is_marker(&tok.text) {
            // A second marker on one line is a conflict, not a tag (M2): keep
            // the first, flag the rest rather than silently demoting it.
            diags.push(Diagnostic::new(
                DiagClass::ConflictingMarkers,
                format!(
                    "a claim carries one origin — `{}` already set it; drop `{}`.",
                    marker.keyword(),
                    tok.text
                ),
                tok.span,
            ));
            cur.bump();
        } else {
            tags.push(token_to_string(tok));
            cur.bump();
        }
    }

    if marker.requires_citation() && citations.is_empty() {
        diags.push(Diagnostic::new(
            DiagClass::OriginRequired,
            format!(
                "a `{}` claim needs a decision behind it — cite `d-nnnn`, or mark it `proposed`.",
                marker.keyword()
            ),
            marker_span,
        ));
    }

    Some(Annotation {
        marker,
        citations,
        condition,
        tags,
        span: marker_span,
    })
}

fn parse_condition(cur: &mut Cursor) -> Option<CondExpr> {
    let primary = parse_condition_primary(cur)?;
    match cur.peek_word_text() {
        Some("and") => {
            let mut parts = vec![primary];
            while cur.peek_word_text() == Some("and") {
                cur.bump();
                if let Some(next) = parse_condition_primary(cur) {
                    parts.push(next);
                }
            }
            Some(CondExpr::And(parts))
        }
        Some("or") => {
            let mut parts = vec![primary];
            while cur.peek_word_text() == Some("or") {
                cur.bump();
                if let Some(next) = parse_condition_primary(cur) {
                    parts.push(next);
                }
            }
            Some(CondExpr::Or(parts))
        }
        _ => Some(primary),
    }
}

fn parse_condition_primary(cur: &mut Cursor) -> Option<CondExpr> {
    if cur.peek_word_text() == Some("not") {
        cur.bump();
        return parse_condition_primary(cur).map(|inner| CondExpr::Not(Box::new(inner)));
    }
    let hyp = cur.eat_ident()?;
    Some(CondExpr::Hyp(hyp))
}

fn is_marker(text: &str) -> bool {
    matches!(text, "proposed" | "decided" | "ratified" | "provisional")
}

/// Whether a token reads as a decision citation, e.g. `d-0001` or `D-0146`.
fn is_citation(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() || !(bytes[0] == b'd' || bytes[0] == b'D') {
        return false;
    }
    let rest = &text[1..];
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

// --- helpers --------------------------------------------------------------

fn ident_of(tok: &Token) -> Ident {
    Ident {
        text: tok.text.clone(),
        span: tok.span,
    }
}

fn synthetic_ident(cur: &Cursor) -> Ident {
    Ident {
        text: String::new(),
        span: cur.here(),
    }
}

fn token_to_string(tok: &Token) -> String {
    match tok.kind {
        TokenKind::Str => {
            let escaped = tok.text.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{escaped}\"")
        }
        _ => tok.text.clone(),
    }
}

fn missing_name(diags: &mut Vec<Diagnostic>, keyword: &str, span: Span) {
    diags.push(Diagnostic::new(
        DiagClass::Malformed,
        format!("`{keyword}` needs a name after it."),
        span,
    ));
}

/// The current spelling a retired keyword was renamed to, or `None` if `keyword`
/// is not a killed form. Every word the surface-v1 naming pass retired lives here
/// (DL-0016), so an old spelling is refused with the word that replaced it rather
/// than read as an unknown keyword.
fn renamed_keyword_target(keyword: &str) -> Option<&'static str> {
    Some(match keyword {
        "pack" => "scope",
        "fluent" => "state",
        "wheel" => "cycle",
        "morpheme" => "name-part",
        "lexeme" => "word",
        "register" => "dialect",
        "sort" => "kind",
        "ledger" => "proof",
        "order-rule" => "rule … over order",
        "craft" => "rule … over season",
        _ => return None,
    })
}

/// A domain-spoken diagnostic that a retired keyword was renamed (koan 9).
fn renamed_keyword(old: &str, new: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagClass::RenamedKeyword,
        format!("`{old}` was renamed — write `{new}`."),
        span,
    )
}

/// A cursor over a header token slice.
struct Cursor<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(toks: &'a [Token]) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.toks.get(self.pos)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn remaining(&self) -> &'a [Token] {
        &self.toks[self.pos.min(self.toks.len())..]
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(|t| t.kind)
    }

    fn peek_word_text(&self) -> Option<&'a str> {
        self.peek().and_then(|t| {
            if t.kind == TokenKind::Word {
                Some(t.text.as_str())
            } else {
                None
            }
        })
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn here(&self) -> Span {
        self.peek()
            .or_else(|| self.toks.last())
            .map_or(Span::synthetic(), |t| t.span)
    }

    fn eat_word_text(&mut self) -> Option<String> {
        let tok = self.peek()?;
        if matches!(tok.kind, TokenKind::Word | TokenKind::Int) {
            let text = tok.text.clone();
            self.bump();
            Some(text)
        } else {
            None
        }
    }

    fn eat_ident(&mut self) -> Option<Ident> {
        let tok = self.peek()?;
        if matches!(tok.kind, TokenKind::Word | TokenKind::Int) {
            let ident = ident_of(tok);
            self.bump();
            Some(ident)
        } else {
            None
        }
    }

    fn eat_string(&mut self) -> Option<String> {
        let tok = self.peek()?;
        if tok.kind == TokenKind::Str {
            let text = tok.text.clone();
            self.bump();
            Some(text)
        } else {
            None
        }
    }

    fn eat_term(&mut self) -> Option<Term> {
        let tok = self.peek()?;
        let term = match tok.kind {
            TokenKind::Str => Term::Str(tok.text.clone()),
            TokenKind::Int => tok
                .text
                .parse::<i64>()
                .map_or_else(|_| Term::Sym(ident_of(tok)), Term::Int),
            TokenKind::Word => {
                if let Some(var) = tok.text.strip_prefix('$') {
                    Term::Var(Ident {
                        text: var.to_owned(),
                        span: tok.span,
                    })
                } else {
                    Term::Sym(ident_of(tok))
                }
            }
            _ => return None,
        };
        self.bump();
        Some(term)
    }
}
