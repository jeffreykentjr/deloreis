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
    AttrDecl, BannedDecl, BeforeDecl, CraftDecl, Decl, Document, DraftDecl, EntityDecl, EntityKind,
    ExportDecl, FactDecl, FluentDecl, ForkDecl, HypothesisDecl, LedgerDecl, LexemeDecl,
    MorphemeDecl, NameDecl, NarrationDecl, OptionClause, OrderRuleDecl, PackHeader, PracticeDecl,
    PrincipleDecl, QuestionDecl, RatifyDecl, RegisterDecl, RuleDecl, SortDecl, TermDecl,
    TimeClause, UseDecl, VariantDecl, WheelDecl,
};
use dli_ast::ident::Ident;
use dli_ast::span::{FileId, Span};
use dli_ast::term::{AtomExpr, Term};

use crate::diag::{DiagClass, Diagnostic};
use crate::lexer::{Line, Token, TokenKind, lines};

/// Parse source into the normative AST document plus any diagnostics.
///
/// This is the frozen front-end seam. The parse is total: it always yields a
/// [`Document`] (populated as far as the source allowed) alongside the
/// diagnostics for whatever it could not read.
///
/// ```
/// use dli_syntax::parser::parse_document;
///
/// let (doc, diags) = parse_document("pack saltmarket\nplace weir \"The Weir\" proposed\n");
/// assert!(diags.is_empty());
/// assert_eq!(doc.pack.unwrap().name.text, "saltmarket");
/// assert_eq!(doc.decls.len(), 1);
/// ```
#[must_use]
pub fn parse_document(input: &str) -> (Document, Vec<Diagnostic>) {
    let file = FileId(0);
    // Strip a leading UTF-8 BOM so it never lands inside the first token.
    let source = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut diags = lexical_diagnostics(source, file);
    let records = assemble_records(&lines(source, file), file);

    let mut pack = None;
    let mut items: Vec<Item> = Vec::new();
    for record in &records {
        let Some(first) = record.head.tokens.first() else {
            continue;
        };
        if first.kind == TokenKind::Word && first.text == "pack" {
            if let Some(name_tok) = record.head.tokens.get(1) {
                pack = Some(PackHeader {
                    name: ident_of(name_tok),
                    span: record.head.span,
                });
            }
            continue;
        }
        parse_record(record, &mut items, &mut diags);
    }

    let decls = fold_options(items, &mut diags);
    check_origins(&decls, &mut diags);
    (Document { pack, decls }, diags)
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
                    "that is a curly quote — write labels with straight quotes, like \"Saltmarket\".",
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

/// A parsed item before options are folded into their question or fork.
enum Item {
    Decl(Decl),
    Option(OptionClause, Span),
}

/// A logical record: a leading line plus its captured body clauses.
struct Record {
    head: Line,
    body: Vec<Clause>,
    /// Header tokens with any trailing `{` block-opener removed.
    head_tokens: Vec<Token>,
}

/// Group prepared lines into logical records.
fn assemble_records(lines: &[Line], file: FileId) -> Vec<Record> {
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
            let (body, next) = collect_brace_body(head, lines, i, file);
            i = next;
            records.push(Record {
                head_tokens: tokens_before_brace(&head.tokens),
                head: head.clone(),
                body,
            });
        } else {
            let mut body = Vec::new();
            while i < lines.len() {
                let line = &lines[i];
                if line.is_blank() || line.indent <= head.indent {
                    break;
                }
                body.push(Clause {
                    text: normalize_ws(&line.raw),
                    span: line.span,
                });
                i += 1;
            }
            records.push(Record {
                head_tokens: head.tokens.clone(),
                head: head.clone(),
                body,
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
fn collect_brace_body(
    head: &Line,
    lines: &[Line],
    start: usize,
    _file: FileId,
) -> (Vec<Clause>, usize) {
    let mut body = Vec::new();
    let mut depth = 1i32;
    // The remainder of the header line after its first out-of-string `{`.
    if let Some(pos) = open_brace_pos(&head.raw) {
        let after = &head.raw[pos + 1..];
        consume_brace_segment(after, head.span, &mut depth, &mut body);
    }
    let mut i = start;
    while depth > 0 && i < lines.len() {
        let line = &lines[i];
        consume_brace_segment(&line.raw, line.span, &mut depth, &mut body);
        i += 1;
    }
    (body, i)
}

/// Fold one physical segment of a braced block into `body`, tracking depth.
fn consume_brace_segment(seg: &str, span: Span, depth: &mut i32, body: &mut Vec<Clause>) {
    let mut text = String::new();
    let mut in_str = false;
    for c in seg.chars() {
        match c {
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
        body.push(Clause { text: norm, span });
    }
}

/// Collapse whitespace runs to single spaces and trim the ends.
fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
            Some(target) => items.push(Item::Decl(Decl::Use(UseDecl { target, span }))),
            None => missing_name(diags, "use", span),
        },
        "sort" => match parse_simple_named(&mut cur, diags, span) {
            Some((name, provenance)) => {
                items.push(Item::Decl(Decl::Sort(SortDecl { name, provenance })));
            }
            None => missing_name(diags, "sort", span),
        },
        "rule" => match cur.eat_ident() {
            Some(name) => {
                let (head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Rule(RuleDecl {
                    name,
                    head,
                    body: record.body.clone(),
                    provenance,
                })));
            }
            None => missing_name(diags, "rule", span),
        },
        "fluent" => {
            if let Some(decl) = parse_fluent(&mut cur, diags, span) {
                items.push(Item::Decl(Decl::Fluent(decl)));
            }
        }
        "wheel" => {
            if let Some(decl) = parse_wheel(&mut cur, diags, span) {
                items.push(Item::Decl(Decl::Wheel(decl)));
            }
        }
        "practice" => {
            if let Some((name, _label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "practice")
            {
                items.push(Item::Decl(Decl::Practice(PracticeDecl {
                    name,
                    body,
                    provenance,
                })));
            }
        }
        "register" => {
            if let Some((name, label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "register")
            {
                items.push(Item::Decl(Decl::Register(RegisterDecl {
                    name,
                    label,
                    body,
                    provenance,
                })));
            }
        }
        "lexeme" => match parse_simple_named(&mut cur, diags, span) {
            Some((name, provenance)) => {
                items.push(Item::Decl(Decl::Lexeme(LexemeDecl {
                    name,
                    morphemes: Vec::new(),
                    provenance,
                })));
            }
            None => missing_name(diags, "lexeme", span),
        },
        "morpheme" => match cur.eat_ident() {
            Some(name) => {
                let gloss = cur.eat_string();
                let (head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Morpheme(MorphemeDecl {
                    name,
                    gloss,
                    head,
                    provenance,
                })));
            }
            None => missing_name(diags, "morpheme", span),
        },
        "craft" => {
            if let Some((name, _label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "craft")
            {
                items.push(Item::Decl(Decl::Craft(CraftDecl {
                    name,
                    body,
                    provenance,
                })));
            }
        }
        "draft" => {
            if let Some((name, _label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "draft")
            {
                items.push(Item::Decl(Decl::Draft(DraftDecl {
                    name,
                    body,
                    provenance,
                })));
            }
        }
        "attest" => match parse_simple_named(&mut cur, diags, span) {
            Some((question, provenance)) => {
                items.push(Item::Decl(Decl::Attest(dli_ast::decl::AttestDecl {
                    question,
                    provenance,
                })));
            }
            None => missing_name(diags, "attest", span),
        },
        "export" => {
            if let Some((profile, _label, _head, body, provenance)) =
                parse_named_block(&mut cur, record, diags, span, "export")
            {
                items.push(Item::Decl(Decl::Export(ExportDecl {
                    profile,
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
            Some(decision) => items.push(Item::Decl(Decl::Ratify(RatifyDecl {
                decision,
                body: record.body.clone(),
            }))),
            None => missing_name(diags, "ratify", span),
        },
        "order-rule" => match cur.eat_ident() {
            Some(name) => {
                let (_head, provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::OrderRule(OrderRuleDecl {
                    name,
                    body: record.body.clone(),
                    provenance,
                })));
            }
            None => missing_name(diags, "order-rule", span),
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
        "ledger" => match cur.eat_ident() {
            Some(name) => {
                let (head, _provenance) = split_head_prov(&mut cur, diags);
                items.push(Item::Decl(Decl::Ledger(LedgerDecl {
                    name,
                    head,
                    body: record.body.clone(),
                })));
            }
            None => missing_name(diags, "ledger", span),
        },
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
    let subject = cur.eat_ident()?;
    let Some(pred) = cur.eat_ident() else {
        diags.push(Diagnostic::new(
            DiagClass::Malformed,
            "a `fact` reads `fact <subject> <relation> <object>`.",
            span,
        ));
        return None;
    };
    let object = cur.eat_term()?;
    let atom = AtomExpr {
        pred,
        args: vec![Term::Sym(subject), object],
    };
    let provenance = parse_provenance(cur, diags);
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
    if cur.peek_kind() == Some(TokenKind::RParen) {
        cur.bump();
    }
    Some(AtomExpr { pred, args })
}

// --- questions, forks, options -------------------------------------------

fn parse_question(cur: &mut Cursor, record: &Record, diags: &mut Vec<Diagnostic>) -> QuestionDecl {
    let name = cur.eat_ident().unwrap_or_else(|| synthetic_ident(cur));
    let prompt = cur.eat_string();
    let provenance = parse_provenance(cur, diags);
    let options = record.body.iter().filter_map(parse_option_clause).collect();
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
    for clause in &record.body {
        if let Some(opt) = parse_option_clause(clause) {
            options.push(opt);
        } else {
            body.push(clause.clone());
        }
    }
    ForkDecl {
        question,
        options,
        body,
        provenance,
    }
}

fn parse_option(cur: &mut Cursor, _diags: &mut [Diagnostic], _span: Span) -> Option<OptionClause> {
    let name = cur.eat_ident()?;
    let implies = if cur.peek_word_text() == Some("implies") {
        cur.bump();
        parse_atom(cur)
    } else {
        None
    };
    Some(OptionClause { name, implies })
}

fn parse_option_clause(clause: &Clause) -> Option<OptionClause> {
    let toks: Vec<Token> = crate::lexer::lex(&clause.text, clause.span.file)
        .into_iter()
        .filter(|t| t.kind != TokenKind::Newline)
        .collect();
    let first = toks.first()?;
    if first.kind != TokenKind::Word || first.text != "option" {
        return None;
    }
    let mut cur = Cursor::new(&toks);
    cur.bump();
    let mut noop = Vec::new();
    parse_option(&mut cur, &mut noop, clause.span)
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
    Some(BannedDecl {
        word,
        reason,
        provenance,
    })
}

// --- fluents, wheels ------------------------------------------------------

fn parse_fluent(cur: &mut Cursor, diags: &mut Vec<Diagnostic>, span: Span) -> Option<FluentDecl> {
    let Some(name) = cur.eat_ident() else {
        missing_name(diags, "fluent", span);
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
    Some(FluentDecl {
        name,
        value,
        time,
        provenance,
    })
}

fn parse_wheel(cur: &mut Cursor, diags: &mut Vec<Diagnostic>, span: Span) -> Option<WheelDecl> {
    let Some(name) = cur.eat_ident() else {
        missing_name(diags, "wheel", span);
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
    Some(WheelDecl {
        name,
        modulus,
        spokes,
        provenance,
    })
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
        TokenKind::Str => format!("\"{}\"", tok.text),
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
