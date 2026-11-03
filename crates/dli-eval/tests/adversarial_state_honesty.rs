#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Front 4 (honesty / koan 4) probes of the evaluator's state lowering.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, StateDecl, TimeClause};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Status;
use dli_eval::query::Query;
use dli_eval::world::World;

fn id(t: &str) -> Ident {
    Ident::synthetic(t)
}

fn state(
    name: &str,
    value: Option<Term>,
    time: Option<TimeClause>,
    prov: Option<Annotation>,
) -> Decl {
    Decl::State(StateDecl {
        name: id(name),
        value,
        time,
        provenance: prov,
    })
}

/// An inert (undotted, valueless) state carries no fact — but is recorded as a
/// finding, never silently dropped. Issue 47 named `World::lower` a koan-4
/// offender; the evaluator now mirrors the loader's `load.inert-declaration`.
#[test]
fn eval_inert_state_is_a_finding_not_a_silent_drop() {
    let doc = Document {
        scope: None,
        decls: vec![state(
            "waterlogged",
            None,
            None,
            Some(Annotation {
                marker: ProvenanceMarker::Ratified,
                citations: vec![CitationRef { id: id("D-1") }],
                condition: None,
                bundle: None,
                tags: Vec::new(),
                span: Span::synthetic(),
            }),
        )],
    };
    let world = World::lower(&doc).expect("lowers");
    assert!(world.facts.is_empty(), "inert state carries no fact");
    assert!(
        world
            .findings
            .iter()
            .any(|f| f.code == "load.inert-declaration"),
        "koan 4: the evaluator must record a finding for an inert state, got {:?}",
        world.findings
    );
}

/// A provenance-less state in the evaluator fails closed (support FALSE => never
/// certain) — but silently, with no finding.
#[test]
fn eval_provenanceless_state_fails_closed() {
    let doc = Document {
        scope: None,
        decls: vec![state(
            "joren.water-status",
            Some(Term::Sym(id("commons"))),
            None,
            None,
        )],
    };
    let world = World::lower(&doc)
        .expect("lowers")
        .fixpoint()
        .expect("fixpoint");
    let answer = world
        .query(&Query {
            goal: AtomExpr {
                pred: id("water-status"),
                args: vec![Term::Sym(id("joren")), Term::Var(id("v"))],
            },
        })
        .expect("query");
    // The fact exists but must never read certain.
    assert!(
        answer.rows.iter().all(|r| r.status != Status::Certain),
        "a provenance-less state must not be certain: {answer:?}"
    );
}
