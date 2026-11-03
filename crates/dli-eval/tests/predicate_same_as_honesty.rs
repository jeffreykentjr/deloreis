#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Kernel coverage for structured predicate identity.
//!
//! This suite deliberately constructs `Decl::PredicateSameAs` directly: the
//! declaration has no parser or `.dli` rendering. The tests pin the evaluator
//! side of the typed carriage, including support/defeat and rule/query paths.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, PredicateSameAsDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Status;
use dli_eval::query::Query;
use dli_eval::why::Why;
use dli_eval::world::{GroundAtom, World};

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn annotation(marker: ProvenanceMarker, citations: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: citations
            .iter()
            .map(|citation| CitationRef { id: id(citation) })
            .collect(),
        condition: None,
        bundle: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratified(decision: &str) -> Annotation {
    annotation(ProvenanceMarker::Ratified, &[decision])
}

fn proposed() -> Annotation {
    annotation(ProvenanceMarker::Proposed, &[])
}

fn ratify(decision: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(decision),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn fact(pred: &str, args: &[&str], provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(pred),
            args: args.iter().map(|arg| Term::Sym(id(arg))).collect(),
        },
        triple: false,
        provenance,
    })
}

fn predicate_same_as(left: &str, right: &str, provenance: Option<Annotation>) -> Decl {
    Decl::PredicateSameAs(PredicateSameAsDecl {
        left: id(left),
        right: id(right),
        provenance,
    })
}

fn document(decls: Vec<Decl>) -> Document {
    Document {
        scope: Some(ScopeHeader {
            name: id("kernel"),
            span: Span::synthetic(),
        }),
        decls,
    }
}

fn query(pred: &str, args: &[&str]) -> Query {
    Query {
        goal: AtomExpr {
            pred: id(pred),
            args: args.iter().map(|arg| Term::Sym(id(arg))).collect(),
        },
    }
}

fn status(world: &World, pred: &str, args: &[&str]) -> Status {
    world
        .query(&query(pred, args))
        .expect("known predicate")
        .rows
        .first()
        .map_or(Status::Dead, |row| row.status)
}

fn source_fact(pred: &str, subject: &str) -> GroundAtom {
    GroundAtom {
        pred: pred.to_owned(),
        args: vec![subject.to_owned()],
    }
}

#[test]
fn predicate_identity_is_declaration_only_and_symmetric_transitive() {
    let world = World::lower(&document(vec![
        ratify("D-1"),
        predicate_same_as("current-work", "current-job", Some(ratified("D-1"))),
        predicate_same_as("current-job", "occupation", Some(ratified("D-1"))),
        fact("current-work", &["nara"], Some(ratified("D-1"))),
        // Similar spelling and overlapping value are not an identity claim.
        fact("current-workshop", &["nara"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    for predicate in ["current-work", "current-job", "occupation"] {
        assert_eq!(status(&world, predicate, &["nara"]), Status::Certain);
    }
    assert_eq!(
        status(&world, "current-workshop", &["nara"]),
        Status::Certain
    );
    assert_eq!(
        status(&world, "current-job", &["someone-else"]),
        Status::Dead
    );
    assert!(world.predicate_same_as_links.len() == 2);
    assert!(
        world
            .facts
            .iter()
            .all(|fact| fact.atom.pred != "predicate-same-as")
    );
}

#[test]
fn identity_supports_copies_and_defeat_unmerges_without_deleting_source() {
    let world = World::lower(&document(vec![
        ratify("D-fact"),
        ratify("D-identity"),
        fact("current-work", &["nara"], Some(ratified("D-fact"))),
        predicate_same_as("current-work", "current-job", Some(ratified("D-identity"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert_eq!(status(&world, "current-job", &["nara"]), Status::Certain);
    let copy = world
        .derived
        .iter()
        .find(|row| row.atom == source_fact("current-job", "nara"))
        .expect("identity creates the alias row");
    let why = Why::of_support(&copy.support, &world.interner);
    let names: Vec<&str> = why.rendered.iter().map(String::as_str).collect();
    assert!(names.iter().any(|name| name.contains("D-fact")));
    assert!(names.iter().any(|name| name.contains("D-identity")));

    let mut defeated = world.clone();
    let identity = defeated.interner.intern("D-identity");
    defeated.valuation.set(identity, false);
    assert_eq!(
        status(&defeated, "current-work", &["nara"]),
        Status::Certain
    );
    assert_eq!(status(&defeated, "current-job", &["nara"]), Status::Dead);
    assert!(
        defeated
            .facts
            .iter()
            .any(|fact| fact.atom == source_fact("current-work", "nara"))
    );
}

#[test]
fn proposed_identity_is_possible_until_its_support_is_ruled() {
    let world = World::lower(&document(vec![
        ratify("D-fact"),
        fact("current-work", &["nara"], Some(ratified("D-fact"))),
        predicate_same_as("current-work", "current-job", Some(proposed())),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");
    assert_eq!(status(&world, "current-work", &["nara"]), Status::Certain);
    assert_eq!(status(&world, "current-job", &["nara"]), Status::Possible);

    let proposal = world
        .interner
        .names()
        .iter()
        .find(|name| name.starts_with("h."))
        .expect("the proposed identity has a support variable")
        .clone();
    let mut supposed = world.clone();
    let proposal_atom = supposed.interner.intern(&proposal);
    supposed.valuation.set(proposal_atom, true);
    assert_eq!(status(&supposed, "current-job", &["nara"]), Status::Certain);
}

#[test]
fn rules_use_identity_for_bodies_and_heads() {
    let mut world = World::new();
    world.assert_fact(
        source_fact("current-work", "nara"),
        dli_core::ann::Ann::unconditional(),
    );
    world.assert_predicate_same_as(
        "current-work",
        "current-job",
        dli_core::ann::Ann::unconditional(),
    );
    world
        .define_rule(
            "employed-from-job",
            AtomExpr {
                pred: id("employed"),
                args: vec![Term::Var(id("who"))],
            },
            vec![BodyLiteral::pos(AtomExpr {
                pred: id("current-job"),
                args: vec![Term::Var(id("who"))],
            })],
        )
        .expect("range restricted");
    world
        .define_rule(
            "job-from-work",
            AtomExpr {
                pred: id("current-job"),
                args: vec![Term::Var(id("who"))],
            },
            vec![BodyLiteral::pos(AtomExpr {
                pred: id("current-work"),
                args: vec![Term::Var(id("who"))],
            })],
        )
        .expect("range restricted");
    dli_eval::world::synthesize_predicate_alias_rules(&mut world);
    let world = world.fixpoint().expect("saturates");

    assert_eq!(status(&world, "current-job", &["nara"]), Status::Certain);
    assert_eq!(status(&world, "employed", &["nara"]), Status::Certain);
}

#[test]
fn self_merge_and_identity_free_world_are_behavior_neutral() {
    let base = World::lower(&document(vec![
        ratify("D-1"),
        fact("rank", &["nara", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers");
    let self_merge = World::lower(&document(vec![
        ratify("D-1"),
        predicate_same_as("rank", "rank", Some(ratified("D-1"))),
        fact("rank", &["nara", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers");
    let same = World::lower(&document(vec![
        ratify("D-1"),
        fact("rank", &["nara", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers");
    assert!(self_merge.rules.is_empty());
    assert!(
        self_merge
            .facts
            .iter()
            .all(|fact| fact.atom.pred != "predicate-same-as:link")
    );
    assert_eq!(
        serde_json::to_string(&base).expect("serializes"),
        serde_json::to_string(&same).expect("serializes")
    );
    assert_eq!(
        status(
            &self_merge.fixpoint().expect("saturates"),
            "rank",
            &["nara", "gold"]
        ),
        Status::Certain
    );
}

#[test]
fn ordinary_predicate_same_as_fact_never_activates_identity() {
    let world = World::lower(&document(vec![
        ratify("D-1"),
        fact(
            "predicate-same-as",
            &["current-work", "current-job"],
            Some(ratified("D-1")),
        ),
        fact("current-work", &["nara"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");
    assert_eq!(status(&world, "current-work", &["nara"]), Status::Certain);
    assert!(
        world
            .facts
            .iter()
            .all(|fact| fact.atom.pred != "current-job")
    );
    assert!(
        world
            .derived
            .iter()
            .all(|row| row.atom.pred != "current-job")
    );
    assert!(world.predicate_same_as_links.is_empty());
}

#[test]
fn zero_predicate_identity_preserves_serialized_world() {
    let document = document(vec![
        ratify("D-1"),
        fact("rank", &["nara", "gold"], Some(ratified("D-1"))),
    ]);
    let lowered = World::lower(&document).expect("lowers");
    assert!(lowered.predicate_same_as_links.is_empty());
    assert!(lowered.rules.is_empty());
    assert!(
        !serde_json::to_string(&lowered)
            .expect("serializes")
            .contains("predicate_same_as_links")
    );
}
