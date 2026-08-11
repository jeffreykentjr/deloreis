#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Issue 49, evaluator side: `functional <pred>` is a constraint, not a fact.
//!
//! `World::lower` must lower it to nothing at all — no fact, and (unlike the
//! inert `state` of issue 47) no finding either, because nothing is dropped: the
//! loader records the predicate in `WorldModel::functional_predicates` and the
//! durable store carries the declaration whole. The probes are differential: the
//! lowered world with the declarations must be *identical* to the world without
//! them, so a lowering that minted a proposal index or a finding for one fails
//! here.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, FunctionalDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Status;
use dli_eval::EvalError;
use dli_eval::query::Query;
use dli_eval::world::World;

fn id(t: &str) -> Ident {
    Ident::synthetic(t)
}

fn marker(m: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker: m,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratified(cite: &str) -> Annotation {
    marker(ProvenanceMarker::Ratified, &[cite])
}

/// A bare proposal: no citation, no tags, so it mints a fresh proposal variable.
fn proposed() -> Annotation {
    marker(ProvenanceMarker::Proposed, &[])
}

fn ratify_decl(d: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(d),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn fact(pred: &str, args: &[&str], prov: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(pred),
            args: args.iter().map(|a| Term::Sym(id(a))).collect(),
        },
        triple: false,
        provenance: prov,
    })
}

fn functional(pred: &str, prov: Option<Annotation>) -> Decl {
    Decl::Functional(FunctionalDecl {
        pred: id(pred),
        provenance: prov,
    })
}

fn document(decls: Vec<Decl>) -> Document {
    Document {
        scope: Some(ScopeHeader {
            name: id("weir"),
            span: Span::synthetic(),
        }),
        decls,
    }
}

/// The declarations under test, threaded between two bare proposals so a burned
/// proposal index would move the following fact's support variable.
fn decls(with_functional: bool) -> Vec<Decl> {
    let mut decls = vec![
        ratify_decl("D-1"),
        fact("rank", &["gwern", "high"], Some(ratified("D-1"))),
    ];
    if with_functional {
        decls.push(functional("rank", Some(ratified("D-1"))));
    }
    decls.push(fact("linked", &["gwern", "enclosure"], Some(proposed())));
    if with_functional {
        decls.push(functional("mood", Some(proposed())));
    }
    decls.push(fact("omen", &["gwern", "storm"], Some(proposed())));
    decls
}

/// A `functional` declaration carries no fact — and, because nothing about it is
/// dropped, no finding. Contrast `eval_inert_state_is_a_finding_not_a_silent_drop`
/// in `adversarial_state_honesty.rs`: an inert *state* names a fact the author
/// meant and the evaluator cannot build, so koan 4 demands a finding there. A
/// constraint has no fact to lose.
#[test]
fn eval_functional_decl_carries_no_fact_and_no_finding() {
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        functional("rank", Some(ratified("D-1"))),
    ]))
    .expect("lowers");
    assert!(
        world.facts.is_empty(),
        "a functional declaration is a constraint, not a fact: {:?}",
        world.facts
    );
    assert!(
        world.findings.is_empty(),
        "nothing is dropped, so nothing is reported: {:?}",
        world.findings
    );
    assert!(
        world.derived.is_empty() && world.rules.is_empty(),
        "and it derives nothing"
    );
}

/// The differential probe: the lowered world is byte-for-byte the world without
/// the declarations. This is what a burned proposal index, a stray interned
/// name, or an extra finding would break.
#[test]
fn eval_functional_decl_is_a_no_op_on_the_lowered_world() {
    let with = World::lower(&document(decls(true))).expect("lowers");
    let without = World::lower(&document(decls(false))).expect("lowers");
    assert_eq!(
        serde_json::to_string(&with).expect("serializes"),
        serde_json::to_string(&without).expect("serializes"),
        "a carriage-only declaration must leave the evaluable core untouched"
    );
    // Not vacuous: the world under test has facts and proposal variables in it,
    // and the proposal indices are the ones a burned index would shift.
    assert_eq!(with.facts.len(), 3, "three facts lowered");
    assert_eq!(
        with.interner.names().to_vec(),
        vec!["D-1", "proposal#0", "proposal#1"],
        "one proposal variable per bare proposal — the functional decls mint none"
    );
}

/// The standings of the neighbouring facts are unmoved — the observable
/// consequence of the lock-step above.
#[test]
fn eval_functional_decl_leaves_neighbouring_standings_intact() {
    let world = World::lower(&document(decls(true)))
        .expect("lowers")
        .fixpoint()
        .expect("saturates");
    for (pred, args, expected) in [
        ("rank", ["gwern", "high"], Status::Certain),
        ("linked", ["gwern", "enclosure"], Status::Possible),
        ("omen", ["gwern", "storm"], Status::Possible),
    ] {
        let answer = world
            .query(&Query {
                goal: AtomExpr {
                    pred: id(pred),
                    args: args.iter().map(|a| Term::Sym(id(a))).collect(),
                },
            })
            .expect("known pred");
        let status = answer.rows.first().map_or(Status::Dead, |row| row.status);
        assert_eq!(status, expected, "{pred} standing moved: {answer:?}");
    }
    // The declaration does not introduce the predicate into the evaluable core
    // at all: `mood` is bare-declared functional and named by no fact, so the
    // kernel does not know it. (An unknown goal is an error, not an empty
    // answer — `unknown_goal_predicate_is_an_error` in `smoke.rs`.)
    let error = world
        .query(&Query {
            goal: AtomExpr {
                pred: id("mood"),
                args: vec![Term::Var(id("v"))],
            },
        })
        .expect_err("a constraint declares no relation");
    assert_eq!(
        error,
        EvalError::UnknownPredicate {
            name: "mood".to_owned()
        },
        "`functional mood` must not conjure a mood relation"
    );
}
