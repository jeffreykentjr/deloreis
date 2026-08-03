//! Front-end structuring of a `practice` declaration interior (LS-9).
//!
//! Before LS-9 this ~200-line hand-rolled interior parser lived in `dli-ledger`
//! (`practice.rs`), which re-entered the front end through
//! [`parse_clause_atom`] at compile time. LS-9 moves it
//! behind the front end: [`structure_practice_body`] is the single producer of
//! the typed [`PracticeDef`], mirroring
//! `structure_rule_body`. `dli-ledger` becomes a
//! reader of the typed AST.
//!
//! The interior grammar is nine keyword-fronted forms:
//!
//! ```text
//! use <target>                                     # a consumed layer
//! kind <name> = <v1> | <v2> | …                    # a declared finite kind
//! kind <name> from <source>                        #   …or one drawn from a layer
//! role <name> : <kind> [requires <conditions>]     # a cast role and its kind
//! resource <name>[(<arg>)] : <kind …>              # a resource and its algebra law
//! rule <form>(<args>) [requires <premises>]        # a law of the practice
//! condition <name> requires <predicate …>          # a side condition
//! error <name> refines L<n> <discriminant …> [signals <signal>]
//! effect <form> <detail …>                         # what a casting does
//! cost <c1> | <c2> | …                             # the currency vocabulary
//! ```
//!
//! A line whose first word is not one of those keywords is a **continuation** of
//! the form above it. Atom-shaped heads (`lay(figure, leaf)`,
//! `acquaintance(name)`) are read through [`parse_clause_atom`], the one shared
//! way captured clause text becomes a typed atom.

use std::collections::BTreeSet;

use dli_ast::annot::Annotation;
use dli_ast::clause::Clause;
use dli_ast::ident::Ident;
use dli_ast::practice::{
    ConditionDef, EffectDef, ErrorClassDef, GradeDirection, PracticeBody, PracticeDef,
    ResourceDecl, ResourceKind, RoleDef, RuleDef, SortDef, StructuralClass,
};
use dli_ast::span::Span;
use dli_ast::term::Term;

use crate::parser::parse_clause_atom;

/// The interior keywords that open a practice-block form. A clause opening with
/// any other word continues the form above it.
const ITEM_KEYWORDS: &[&str] = &[
    "use",
    "kind",
    "role",
    "resource",
    "rule",
    "condition",
    "error",
    "effect",
    "cost",
];

/// Structure a `practice` declaration's captured clauses into its typed
/// [`PracticeBody`] (LS-9, `docs/design-notes/lorestore-roadmap.md`).
///
/// The raw `clauses` are retained verbatim on the body so the formatter
/// round-trips the surface byte-for-byte; the structured [`PracticeDef`] is
/// produced by folding continuation lines into their opening forms and parsing
/// each. The produced def leaves `cycles_in_scope` empty — the document's cyclic
/// vocabulary is overlaid by `dli-ledger` at compile time (DL-0017).
///
/// This is the one **front-end lowering** LS-9's v1 → v2 store upcast reuses at
/// the load boundary to deepen an old string-bodied practice carriage
/// (`dli_lorestore::upcast`); the parser is the single producer of the typed
/// interior.
#[must_use]
pub fn structure_practice_body(
    clauses: &[Clause],
    name: &Ident,
    provenance: Option<&Annotation>,
) -> PracticeBody {
    let mut def = PracticeDef {
        name: name.clone(),
        marker: provenance.map(|a| a.marker),
        uses: Vec::new(),
        sorts: Vec::new(),
        roles: Vec::new(),
        resources: Vec::new(),
        rules: Vec::new(),
        conditions: Vec::new(),
        errors: Vec::new(),
        effects: Vec::new(),
        costs: Vec::new(),
        cycles_in_scope: BTreeSet::new(),
        span: name.span,
    };
    for item in reassemble(clauses) {
        absorb(&mut def, &item);
    }
    PracticeBody {
        clauses: clauses.to_vec(),
        def: Box::new(def),
    }
}

/// Fold one reassembled interior form into the growing [`PracticeDef`].
fn absorb(def: &mut PracticeDef, item: &RawItem) {
    let span = item.span;
    // `body` is the item text with its leading keyword removed.
    let body = item.text[item.keyword.len()..].trim();
    match item.keyword.as_str() {
        "use" => def.uses.push(body.to_owned()),
        "kind" => def.sorts.push(parse_sort(body, span)),
        "role" => {
            if let Some(role) = parse_role(body, span) {
                def.roles.push(role);
            }
        }
        "resource" => def.resources.push(parse_resource(body, span)),
        "rule" => def.rules.push(parse_rule(body, span)),
        "condition" => {
            if let Some(cond) = parse_condition(body, span) {
                def.conditions.push(cond);
            }
        }
        "error" => {
            if let Some(err) = parse_error(body, span) {
                def.errors.push(err);
            }
        }
        "effect" => def.effects.push(parse_effect(body, span)),
        "cost" => def.costs.extend(split_pipe(body, span)),
        _ => {}
    }
}

/// One reassembled practice-block form: a leading keyword and its full text
/// (continuation lines re-joined).
struct RawItem {
    keyword: String,
    text: String,
    span: Span,
}

/// Fold captured clauses into forms: a clause opening with an interior keyword
/// begins a form; any other clause continues the form above it.
fn reassemble(body: &[Clause]) -> Vec<RawItem> {
    let mut items: Vec<RawItem> = Vec::new();
    for clause in body {
        let first = clause.text.split_whitespace().next().unwrap_or_default();
        if ITEM_KEYWORDS.contains(&first) {
            items.push(RawItem {
                keyword: first.to_owned(),
                text: clause.text.clone(),
                span: clause.span,
            });
        } else if let Some(last) = items.last_mut() {
            last.text.push(' ');
            last.text.push_str(&clause.text);
        }
    }
    items
}

fn parse_sort(body: &str, span: Span) -> SortDef {
    if let Some((name, variants)) = body.split_once(" = ") {
        return SortDef {
            name: ident(name.trim(), span),
            variants: split_pipe(variants, span),
            source: None,
            span,
        };
    }
    if let Some((name, source)) = body.split_once(" from ") {
        return SortDef {
            name: ident(name.trim(), span),
            variants: Vec::new(),
            source: Some(source.trim().to_owned()),
            span,
        };
    }
    SortDef {
        name: ident(first_word(body), span),
        variants: Vec::new(),
        source: None,
        span,
    }
}

fn parse_role(body: &str, span: Span) -> Option<RoleDef> {
    let (name, rest) = body.split_once(" : ")?;
    let (sort, requires) = match rest.split_once(" requires ") {
        Some((sort, req)) => (sort.trim(), Some(req.trim().to_owned())),
        None => (rest.trim(), None),
    };
    Some(RoleDef {
        name: ident(name.trim(), span),
        sort: ident(first_word(sort), span),
        requires,
        span,
    })
}

fn parse_resource(body: &str, span: Span) -> ResourceDecl {
    let (head, law) = match body.split_once(" : ") {
        Some((head, law)) => (head.trim(), law.trim()),
        None => (body.trim(), ""),
    };
    let (name, arg) = named_head(head, span);
    let (kind, cap, order, direction) = parse_resource_kind(law, span);
    ResourceDecl {
        name,
        arg: arg.into_iter().next(),
        kind,
        cap,
        order,
        direction,
        law_text: law.to_owned(),
        span,
    }
}

/// Read the law phrase after the `:` into a closed-algebra tag and its params.
fn parse_resource_kind(
    law: &str,
    span: Span,
) -> (
    Option<ResourceKind>,
    Option<String>,
    Vec<Ident>,
    GradeDirection,
) {
    let words: Vec<&str> = law.split_whitespace().collect();
    match words.as_slice() {
        ["linear"] => (
            Some(ResourceKind::Linear { conserved: false }),
            None,
            Vec::new(),
            GradeDirection::Decays,
        ),
        ["linear", "conserved"] => (
            Some(ResourceKind::Linear { conserved: true }),
            None,
            Vec::new(),
            GradeDirection::Decays,
        ),
        ["affine", "bounded", cap @ ..] if !cap.is_empty() => (
            Some(ResourceKind::AffineBounded),
            Some(cap.join(" ")),
            Vec::new(),
            GradeDirection::Decays,
        ),
        ["graded", "monotone", order @ ..] if !order.is_empty() => {
            // `whole > worn > husk` decays; the reverse operator would ripen.
            let direction = if order.contains(&"<") {
                GradeDirection::Grows
            } else {
                GradeDirection::Decays
            };
            let grades = order
                .iter()
                .filter(|t| **t != ">" && **t != "<")
                .map(|t| ident(t, span))
                .collect();
            (Some(ResourceKind::GradedMonotone), None, grades, direction)
        }
        _ => (None, None, Vec::new(), GradeDirection::Decays),
    }
}

fn parse_rule(body: &str, span: Span) -> RuleDef {
    let (head, requires) = match body.split_once(" requires ") {
        Some((head, req)) => (head.trim(), Some(req.trim().to_owned())),
        None => (body.trim(), None),
    };
    let (form, args) = named_head(head, span);
    RuleDef {
        form,
        args,
        requires,
        span,
    }
}

fn parse_condition(body: &str, span: Span) -> Option<ConditionDef> {
    let (name, requires) = body.split_once(" requires ")?;
    Some(ConditionDef {
        name: ident(name.trim(), span),
        requires: requires.trim().to_owned(),
        span,
    })
}

fn parse_error(body: &str, span: Span) -> Option<ErrorClassDef> {
    let (name, rest) = body.split_once(" refines ")?;
    let rest = rest.trim();
    let (class_token, detail) = match rest.split_once(char::is_whitespace) {
        Some((token, detail)) => (token, detail.trim()),
        None => (rest, ""),
    };
    // Split off a trailing `signals <signal>` from the discriminant detail.
    let (discriminant, signal) = match detail.split_once(" signals ") {
        Some((disc, sig)) => (disc.trim().to_owned(), Some(sig.trim().to_owned())),
        None => (detail.to_owned(), None),
    };
    let on_resource = discriminant
        .split_once(" on ")
        .map(|(_, rest)| ident(first_word(rest), span));
    let over_cycles = parse_over_cycles(&discriminant, span);
    Some(ErrorClassDef {
        name: ident(name.trim(), span),
        refines_token: class_token.to_owned(),
        class: StructuralClass::from_token(class_token),
        discriminant,
        signal,
        on_resource,
        over_cycles,
        span,
    })
}

/// The `over cycle <c1>, <c2>` clause naming the cycles a timing failure is
/// seated on. Both the current `over cycle` and the retired `over wheel` spelling
/// are read so nothing is dropped (koan 4); the retired form is separately
/// diagnosed by the ledger's `check_practice` (DL-0016), so the reference
/// resolves either way and the writer is still told to update it.
fn parse_over_cycles(discriminant: &str, span: Span) -> Vec<Ident> {
    for marker in ["over cycle ", "over wheel "] {
        if let Some(at) = discriminant.find(marker) {
            return discriminant[at + marker.len()..]
                .split(',')
                .map(|w| ident(first_word(w.trim()), span))
                .filter(|i| !i.text.is_empty())
                .collect();
        }
    }
    Vec::new()
}

fn parse_effect(body: &str, span: Span) -> EffectDef {
    let (form, detail) = match body.split_once(char::is_whitespace) {
        Some((form, detail)) => (form, detail.trim().to_owned()),
        None => (body, String::new()),
    };
    EffectDef {
        form: ident(form, span),
        detail,
        span,
    }
}

// --- token helpers ----------------------------------------------------------

/// Split a `name(arg, …)` head into its name and argument idents through the
/// shared [`parse_clause_atom`]; a bare `name` yields no arguments.
fn named_head(head: &str, span: Span) -> (Ident, Vec<Ident>) {
    let clause = Clause {
        text: head.to_owned(),
        span,
    };
    if let Some(atom) = parse_clause_atom(&clause) {
        let args = atom
            .args
            .iter()
            .map(|term| ident(&term_text(term), span))
            .collect();
        return (ident(&atom.pred.text, span), args);
    }
    (ident(first_word(head), span), Vec::new())
}

/// Split a `a | b | c` list into its idents.
fn split_pipe(text: &str, span: Span) -> Vec<Ident> {
    text.split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| ident(first_word(s), span))
        .collect()
}

fn first_word(text: &str) -> &str {
    text.split_whitespace().next().unwrap_or_default()
}

fn ident(text: &str, span: Span) -> Ident {
    Ident {
        text: text.to_owned(),
        span,
    }
}

fn term_text(term: &Term) -> String {
    match term {
        Term::Var(id) | Term::Sym(id) => id.text.clone(),
        Term::Str(s) => s.clone(),
        Term::Int(n) => n.to_string(),
        Term::Time(_) => String::new(),
    }
}
