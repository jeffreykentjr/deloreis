#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Front 1: the loader (`dli_world::compile`) and the evaluator
//! (`dli_eval::world::World::lower`) must lower a state to the SAME ground atom
//! and answer a query identically, for tricky names/values.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, RatifyDecl, ScopeHeader, StateDecl, TimeClause};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_eval::query::Query;
use dli_eval::world::World;
use dli_world::compile;
use dli_world::pack::Pack;

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn ratified(cite: &str) -> Annotation {
    Annotation {
        marker: ProvenanceMarker::Ratified,
        citations: vec![CitationRef { id: id(cite) }],
        condition: None,
        bundle: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratify_decl(d: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(d),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn state(name: &str, value: Option<Term>, time: Option<TimeClause>) -> Decl {
    Decl::State(StateDecl {
        name: id(name),
        value,
        time,
        provenance: Some(ratified("D-1")),
    })
}

/// Query both paths for `goal` and return (`loader_rows`, `eval_rows`) each as a
/// sorted vec of `binding=value|status` strings.
fn both_paths(decls: Vec<Decl>, goal: AtomExpr, bind: &str) -> (Vec<String>, Vec<String>) {
    let doc = Document {
        scope: Some(ScopeHeader {
            name: id("weir"),
            span: Span::synthetic(),
        }),
        decls: decls.clone(),
    };
    // Loader path.
    let compiled = compile(&Pack {
        name: "weir".to_owned(),
        documents: vec![doc.clone()],
    })
    .expect("loader compiles");
    let l = compiled
        .model()
        .world
        .query(&Query { goal: goal.clone() })
        .expect("loader query");
    let mut lrows: Vec<String> = l
        .rows
        .iter()
        .map(|r| format!("{}={:?}|{:?}", bind, r.bindings.get(bind), r.status))
        .collect();
    lrows.sort();
    // Eval path (Document with no scope; the front end is independent).
    let eval_doc = Document { scope: None, decls };
    let world = World::lower(&eval_doc)
        .expect("eval lowers")
        .fixpoint()
        .expect("fixpoint");
    let e = world.query(&Query { goal }).expect("eval query");
    let mut erows: Vec<String> = e
        .rows
        .iter()
        .map(|r| format!("{}={:?}|{:?}", bind, r.bindings.get(bind), r.status))
        .collect();
    erows.sort();
    (lrows, erows)
}

#[test]
fn multi_dot_name_agrees() {
    // a.b.c = v  ->  split_once('.') => subject "a", pred "b.c"  =>  b.c(a, v)
    let (l, e) = both_paths(
        vec![
            ratify_decl("D-1"),
            state("a.b.c", Some(Term::Sym(id("v"))), None),
        ],
        AtomExpr {
            pred: id("b.c"),
            args: vec![Term::Sym(id("a")), Term::Var(id("x"))],
        },
        "x",
    );
    assert_eq!(l, e, "multi-dot loader vs eval");
    assert_eq!(
        l,
        vec!["x=Some(\"v\")|Certain".to_owned()],
        "b.c(a, v) queryable: {l:?}"
    );
}

#[test]
fn str_value_agrees() {
    let (l, e) = both_paths(
        vec![
            ratify_decl("D-1"),
            state("joren.title", Some(Term::Str("The Weir".to_owned())), None),
        ],
        AtomExpr {
            pred: id("title"),
            args: vec![Term::Sym(id("joren")), Term::Var(id("x"))],
        },
        "x",
    );
    assert_eq!(l, e, "Str loader vs eval");
    assert_eq!(l, vec!["x=Some(\"The Weir\")|Certain".to_owned()], "{l:?}");
}

#[test]
fn int_value_agrees() {
    let (l, e) = both_paths(
        vec![
            ratify_decl("D-1"),
            state("joren.depth", Some(Term::Int(7)), None),
        ],
        AtomExpr {
            pred: id("depth"),
            args: vec![Term::Sym(id("joren")), Term::Var(id("x"))],
        },
        "x",
    );
    assert_eq!(l, e, "Int loader vs eval");
    assert_eq!(l, vec!["x=Some(\"7\")|Certain".to_owned()], "{l:?}");
}

#[test]
fn dotted_valueless_agrees() {
    // joren.alive -> alive(joren), unary holding.
    let (l, e) = both_paths(
        vec![ratify_decl("D-1"), state("joren.alive", None, None)],
        AtomExpr {
            pred: id("alive"),
            args: vec![Term::Var(id("s"))],
        },
        "s",
    );
    assert_eq!(l, e, "dotted-valueless loader vs eval");
    assert_eq!(l, vec!["s=Some(\"joren\")|Certain".to_owned()], "{l:?}");
}

#[test]
fn undotted_valued_agrees() {
    // foo = bar -> foo(bar).
    let (l, e) = both_paths(
        vec![
            ratify_decl("D-1"),
            state("foo", Some(Term::Sym(id("bar"))), None),
        ],
        AtomExpr {
            pred: id("foo"),
            args: vec![Term::Var(id("v"))],
        },
        "v",
    );
    assert_eq!(l, e, "undotted-valued loader vs eval");
    assert_eq!(l, vec!["v=Some(\"bar\")|Certain".to_owned()], "{l:?}");
}
