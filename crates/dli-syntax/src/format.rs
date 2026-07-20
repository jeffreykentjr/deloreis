//! Formatter: the one canonical rendering (charter koan 7).
//!
//! There is exactly one canonical surface form (DL-0003). A writer types
//! approximately; saving produces this form, and the committed tree contains
//! only it. The formatter is total over the AST and idempotent — formatting its
//! own output reproduces it byte for byte — and it round-trips with the parser:
//! parsing formatted output yields the same document (modulo source spans).
//! Consecutive rows of the same shape are column-aligned, so a run of entities
//! or slots reads as a table.

use dli_ast::annot::{Annotation, CondExpr};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    AttrDecl, Decl, Document, EntityDecl, EntityKind, FactDecl, OptionClause, TimeClause,
};
use dli_ast::term::{AtomExpr, Term, TimeRef};

const INDENT: &str = "  ";

/// Render a document to canonical concrete syntax.
///
/// ```
/// use dli_syntax::parser::parse_document;
/// use dli_syntax::format::format_document;
///
/// let (doc, diags) = parse_document("scope p\nplace weir \"The Weir\"  proposed\n");
/// assert!(diags.is_empty());
/// let text = format_document(&doc);
/// // Formatting is idempotent.
/// let (again, _) = parse_document(&text);
/// assert_eq!(format_document(&again), text);
/// ```
#[must_use]
pub fn format_document(document: &Document) -> String {
    let mut out = String::new();
    if let Some(scope) = &document.scope {
        out.push_str("scope ");
        out.push_str(&scope.name.text);
        out.push('\n');
        if !document.decls.is_empty() {
            out.push('\n');
        }
    }

    // Group consecutive rows of the same shape for column alignment; anything
    // that is not an alignable row flushes the current batch and prints raw.
    let mut batch: Vec<Row> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for decl in &document.decls {
        match classify(decl) {
            Emit::Row(row) => {
                if batch.first().is_some_and(|first| first.key != row.key) {
                    flush(&mut batch, &mut lines);
                }
                batch.push(row);
            }
            Emit::Raw(block) => {
                flush(&mut batch, &mut lines);
                lines.extend(block);
            }
        }
    }
    flush(&mut batch, &mut lines);

    out.push_str(&lines.join("\n"));
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Parse `source` and render its canonical form, refusing if the parse produced
/// any error-severity diagnostic.
///
/// This is the safe entry point for a `fmt` command. A document with a
/// malformed line parses to an AST that has *dropped* that line, so rendering it
/// would silently delete the author's text on save — the fmt path must refuse
/// rather than destroy lore (koan 4, nothing silently dropped). Advisory
/// warnings (e.g. a marker-less claim) do not block: such claims are captured in
/// full and format losslessly.
///
/// Returns `Ok(canonical_text)` when the parse is error-free, or `Err(errors)`
/// with the error-severity diagnostics that make a rewrite unsafe.
///
/// ```
/// use dli_syntax::format_source;
///
/// assert!(format_source("place weir \"The Weir\" proposed\n").is_ok());
/// // A malformed line would be dropped on a blind re-render, so fmt refuses.
/// assert!(format_source("fact mara lives-in\n").is_err());
/// ```
pub fn format_source(source: &str) -> Result<String, Vec<crate::diag::Diagnostic>> {
    let (document, diagnostics) = crate::parser::parse_document(source);
    let errors: Vec<crate::diag::Diagnostic> = diagnostics
        .into_iter()
        .filter(|diag| diag.class.is_error())
        .collect();
    if errors.is_empty() {
        Ok(format_document(&document))
    } else {
        Err(errors)
    }
}

/// An alignment group key: rows with equal keys align together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKey {
    Entity(EntityKind),
    Attr,
    Fact,
}

/// An alignable row: header cells plus already-rendered child lines.
struct Row {
    key: RowKey,
    cells: Vec<String>,
    children: Vec<String>,
}

/// How a declaration renders.
enum Emit {
    Row(Row),
    Raw(Vec<String>),
}

fn classify(decl: &Decl) -> Emit {
    match decl {
        Decl::Entity(entity) => classify_entity(entity),
        Decl::Attr(attr) => Emit::Row(Row {
            key: RowKey::Attr,
            cells: attr_cells(attr),
            children: Vec::new(),
        }),
        Decl::Fact(fact) => Emit::Row(Row {
            key: RowKey::Fact,
            cells: fact_cells(fact),
            children: Vec::new(),
        }),
        other => Emit::Raw(render_raw(other)),
    }
}

fn classify_entity(entity: &EntityDecl) -> Emit {
    if let Some(TimeClause::Order(chain)) = &entity.time {
        let mut line = format!("{} {}", entity.kind.keyword(), entity.name.text);
        for step in chain {
            line.push_str(" < ");
            line.push_str(&step.text);
        }
        if let Some(prov) = &entity.provenance {
            line.push(' ');
            line.push_str(&render_provenance(prov));
        }
        return Emit::Raw(vec![line]);
    }
    let mut children = Vec::new();
    if !entity.slots.is_empty() {
        let rows: Vec<Vec<String>> = entity.slots.iter().map(attr_cells).collect();
        for line in align(&rows) {
            children.push(format!("{INDENT}{line}"));
        }
    }
    Emit::Row(Row {
        key: RowKey::Entity(entity.kind),
        cells: entity_cells(entity),
        children,
    })
}

fn entity_cells(entity: &EntityDecl) -> Vec<String> {
    let label = entity.label.as_deref().map(quote).unwrap_or_default();
    let mut tail = String::new();
    if let Some(time) = &entity.time {
        tail.push_str(&render_time(time));
    }
    if let Some(prov) = &entity.provenance {
        if !tail.is_empty() {
            tail.push(' ');
        }
        tail.push_str(&render_provenance(prov));
    }
    vec![
        entity.kind.keyword().to_owned(),
        entity.name.text.clone(),
        label,
        tail,
    ]
}

fn attr_cells(attr: &AttrDecl) -> Vec<String> {
    let tail = attr
        .provenance
        .as_ref()
        .map(render_provenance)
        .unwrap_or_default();
    vec![
        format!("{}.{}", attr.subject.text, attr.attr.text),
        "=".to_owned(),
        render_term(&attr.value),
        tail,
    ]
}

fn fact_cells(fact: &FactDecl) -> Vec<String> {
    let atom = if fact.triple {
        render_triple(&fact.atom)
    } else {
        render_atom(&fact.atom)
    };
    let tail = fact
        .provenance
        .as_ref()
        .map(render_provenance)
        .unwrap_or_default();
    vec![atom, tail]
}

/// Render everything that is not an alignable gentle row.
///
/// One arm per keyword: the length is the size of the surface, not accidental.
#[allow(clippy::too_many_lines)]
fn render_raw(decl: &Decl) -> Vec<String> {
    match decl {
        Decl::Question(q) => {
            let mut header = format!("question {}", q.name.text);
            if let Some(prompt) = &q.prompt {
                header.push(' ');
                header.push_str(&quote(prompt));
            }
            append_provenance(&mut header, q.provenance.as_ref());
            let mut lines = vec![header.trim_end().to_owned()];
            lines.extend(option_lines(&q.options));
            lines
        }
        Decl::Fork(f) => {
            let mut header = format!("fork {}", f.question.text);
            append_provenance(&mut header, f.provenance.as_ref());
            let mut lines = vec![header.trim_end().to_owned()];
            lines.extend(option_lines(&f.options));
            lines.extend(clause_lines(&f.body));
            lines
        }
        Decl::Before(b) => {
            let mut line = format!("before {} -> {}", b.earlier.text, b.later.text);
            append_provenance(&mut line, b.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Hypothesis(h) => {
            let mut line = format!("hypothesis {}", h.name.text);
            if let Some(statement) = &h.statement {
                line.push(' ');
                line.push_str(&quote(statement));
            }
            vec![line]
        }
        Decl::Use(u) => vec![format!("use {}", u.target.text)],
        Decl::Kind(s) => {
            let mut line = format!("kind {}", s.name.text);
            append_provenance(&mut line, s.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Rule(r) => {
            let mut header = format!("rule {}", r.name.text);
            if let Some(domain) = r.domain {
                header.push_str(" over ");
                header.push_str(domain.keyword());
            }
            append_head(&mut header, &r.head);
            append_provenance(&mut header, r.provenance.as_ref());
            let mut lines = vec![header.trim_end().to_owned()];
            lines.extend(clause_lines(&r.body));
            lines
        }
        Decl::State(fl) => {
            let mut line = format!("state {}", fl.name.text);
            if let Some(value) = &fl.value {
                line.push_str(" = ");
                line.push_str(&render_term(value));
            }
            if let Some(time) = &fl.time {
                line.push(' ');
                line.push_str(&render_time(time));
            }
            append_provenance(&mut line, fl.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Cycle(w) => {
            let mut line = format!("cycle {}", w.name.text);
            if let Some(modulus) = w.modulus {
                line.push('(');
                line.push_str(&modulus.to_string());
                line.push(')');
            }
            if !w.spokes.is_empty() {
                line.push_str(" = ");
                let spokes: Vec<&str> = w.spokes.iter().map(|s| s.text.as_str()).collect();
                line.push_str(&spokes.join(" | "));
            }
            append_provenance(&mut line, w.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Practice(p) => braced(
            "practice",
            &p.name.text,
            None,
            &[],
            p.provenance.as_ref(),
            &p.body,
        ),
        Decl::Dialect(r) => braced(
            "dialect",
            &r.name.text,
            r.label.as_deref(),
            &[],
            r.provenance.as_ref(),
            &r.body,
        ),
        Decl::Draft(d) => braced(
            "draft",
            &d.name.text,
            None,
            &[],
            d.provenance.as_ref(),
            &d.body,
        ),
        Decl::Export(e) => braced(
            "export",
            &e.profile.text,
            None,
            &[],
            e.provenance.as_ref(),
            &e.body,
        ),
        Decl::Proof(l) => braced("proof", &l.name.text, None, &l.head, None, &l.body),
        Decl::Ratify(r) => braced("ratify", &r.decision.text, None, &[], None, &r.body),
        Decl::Word(l) => {
            let mut line = format!("word {}", l.name.text);
            for part in &l.name_parts {
                line.push(' ');
                line.push_str(&part.text);
            }
            append_provenance(&mut line, l.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::NamePart(m) => {
            let mut line = format!("name-part {}", m.name.text);
            if let Some(gloss) = &m.gloss {
                line.push(' ');
                line.push_str(&quote(gloss));
            }
            append_head(&mut line, &m.head);
            append_provenance(&mut line, m.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Attest(a) => {
            let mut line = format!("attest {}", a.question.text);
            append_provenance(&mut line, a.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Name(n) => {
            let mut header = format!("name {}", n.name.text);
            if let Some(label) = &n.label {
                header.push(' ');
                header.push_str(&quote(label));
            }
            append_head(&mut header, &n.head);
            append_provenance(&mut header, n.provenance.as_ref());
            let mut lines = vec![header.trim_end().to_owned()];
            lines.extend(clause_lines(&n.body));
            lines
        }
        Decl::Term(t) => {
            let mut line = format!("term {}", t.name.text);
            if let Some(label) = &t.label {
                line.push(' ');
                line.push_str(&quote(label));
            }
            append_provenance(&mut line, t.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Banned(b) => {
            let mut line = format!("banned {}", b.word.text);
            if let Some(prov) = &b.provenance {
                for cite in &prov.citations {
                    line.push(' ');
                    line.push_str(&cite.id.text);
                }
            }
            if let Some(reason) = &b.reason {
                line.push(' ');
                line.push_str(&quote(reason));
            }
            vec![line]
        }
        Decl::Narration(n) => {
            let mut line = "narration".to_owned();
            append_head(&mut line, &n.head);
            append_provenance(&mut line, n.provenance.as_ref());
            vec![line.trim_end().to_owned()]
        }
        Decl::Variant(v) => {
            let mut header = format!("variant {}", v.path.text);
            append_provenance(&mut header, v.provenance.as_ref());
            let mut lines = vec![header.trim_end().to_owned()];
            lines.extend(clause_lines(&v.body));
            lines
        }
        Decl::Principle(p) => {
            let mut header = format!("principle {}", p.name.text);
            append_head(&mut header, &p.head);
            append_provenance(&mut header, p.provenance.as_ref());
            let mut lines = vec![header.trim_end().to_owned()];
            lines.extend(clause_lines(&p.body));
            lines
        }
        // Entity / Attr / Fact are handled as rows, never raw.
        Decl::Entity(_) | Decl::Attr(_) | Decl::Fact(_) => Vec::new(),
    }
}

fn braced(
    keyword: &str,
    name: &str,
    label: Option<&str>,
    head: &[String],
    provenance: Option<&Annotation>,
    body: &[Clause],
) -> Vec<String> {
    let mut header = format!("{keyword} {name}");
    if let Some(label) = label {
        header.push(' ');
        header.push_str(&quote(label));
    }
    append_head(&mut header, head);
    append_provenance(&mut header, provenance);
    if body.is_empty() {
        return vec![header.trim_end().to_owned()];
    }
    let mut lines = vec![format!("{} {{", header.trim_end())];
    lines.extend(clause_lines(body));
    lines.push("}".to_owned());
    lines
}

fn option_lines(options: &[OptionClause]) -> Vec<String> {
    let rows: Vec<Vec<String>> = options
        .iter()
        .map(|opt| {
            let implies = opt
                .implies
                .as_ref()
                .map(|atom| format!("implies {}", render_atom(atom)))
                .unwrap_or_default();
            vec!["option".to_owned(), opt.name.text.clone(), implies]
        })
        .collect();
    align(&rows)
        .into_iter()
        .map(|line| format!("{INDENT}{line}"))
        .collect()
}

fn clause_lines(body: &[Clause]) -> Vec<String> {
    body.iter()
        .map(|clause| format!("{INDENT}{}", clause.text))
        .collect()
}

fn append_head(line: &mut String, head: &[String]) {
    for token in head {
        line.push(' ');
        line.push_str(token);
    }
}

fn append_provenance(line: &mut String, provenance: Option<&Annotation>) {
    if let Some(prov) = provenance {
        line.push(' ');
        line.push_str(&render_provenance(prov));
    }
}

/// Column-align a set of rows: pad every cell to its column width, join with a
/// single space, and trim the trailing padding.
fn align(rows: &[Vec<String>]) -> Vec<String> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    rows.iter()
        .map(|row| {
            let mut line = String::new();
            for (i, cell) in row.iter().enumerate() {
                if i > 0 {
                    line.push(' ');
                }
                line.push_str(cell);
                if i + 1 < row.len() {
                    let pad = widths[i].saturating_sub(cell.chars().count());
                    line.extend(std::iter::repeat_n(' ', pad));
                }
            }
            line.trim_end().to_owned()
        })
        .collect()
}

fn flush(batch: &mut Vec<Row>, lines: &mut Vec<String>) {
    if batch.is_empty() {
        return;
    }
    let rows: Vec<Vec<String>> = batch.iter().map(|row| row.cells.clone()).collect();
    let aligned = align(&rows);
    for (line, row) in aligned.into_iter().zip(batch.iter()) {
        lines.push(line);
        lines.extend(row.children.iter().cloned());
    }
    batch.clear();
}

// --- leaf renderers -------------------------------------------------------

fn render_provenance(prov: &Annotation) -> String {
    let mut out = prov.marker.keyword().to_owned();
    for cite in &prov.citations {
        out.push(' ');
        out.push_str(&cite.id.text);
    }
    for tag in &prov.tags {
        out.push(' ');
        out.push_str(tag);
    }
    if let Some(cond) = &prov.condition {
        out.push_str(" assumes ");
        out.push_str(&render_cond(cond));
    }
    out
}

fn render_cond(cond: &CondExpr) -> String {
    match cond {
        CondExpr::Hyp(h) => h.text.clone(),
        CondExpr::Not(inner) => format!("not {}", render_cond(inner)),
        CondExpr::And(parts) => parts
            .iter()
            .map(render_cond)
            .collect::<Vec<_>>()
            .join(" and "),
        CondExpr::Or(parts) => parts
            .iter()
            .map(render_cond)
            .collect::<Vec<_>>()
            .join(" or "),
    }
}

fn render_time(time: &TimeClause) -> String {
    match time {
        TimeClause::After(x) => format!("after {}", x.text),
        TimeClause::During(x) => format!("during {}", x.text),
        TimeClause::Since(x) => format!("since {}", x.text),
        TimeClause::Until(x) => format!("until {}", x.text),
        TimeClause::Order(chain) => {
            let steps: Vec<&str> = chain.iter().map(|s| s.text.as_str()).collect();
            format!("< {}", steps.join(" < "))
        }
    }
}

fn render_atom(atom: &AtomExpr) -> String {
    let args: Vec<String> = atom.args.iter().map(render_term).collect();
    format!("{}({})", atom.pred.text, args.join(", "))
}

fn render_triple(atom: &AtomExpr) -> String {
    let subject = atom.args.first().map(render_term).unwrap_or_default();
    let object = atom.args.get(1).map(render_term).unwrap_or_default();
    format!("fact {subject} {} {object}", atom.pred.text)
}

fn render_term(term: &Term) -> String {
    match term {
        Term::Var(v) => format!("${}", v.text),
        Term::Sym(s) => s.text.clone(),
        Term::Str(s) => quote(s),
        Term::Int(n) => n.to_string(),
        Term::Time(TimeRef::Era(x) | TimeRef::Event(x)) => x.text.clone(),
    }
}

fn quote(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
