#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Deterministic mutation checks for the retained fiction-continuity surface.
//!
//! Each test starts from or constructs typed lore and verifies a specific
//! changed verdict. The paraphrase case records an honest strict expected miss:
//! it fails if the offline checker ever starts recognizing that paraphrase.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_world::check::factcheck::{KilledToken, KilledTokens, MatchMode, Verdict, factcheck};
use dli_world::check::{self, CompiledWorld};
use dli_world::model::LoadFinding;
use dli_world::{Pack, compile};

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}
fn annotation(marker: ProvenanceMarker, citations: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: citations
            .iter()
            .map(|name| CitationRef { id: id(name) })
            .collect(),
        condition: None,
        bundle: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}
fn atom(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|arg| Term::Sym(id(arg))).collect(),
    }
}
fn fact(pred: &str, args: &[&str], provenance: Annotation) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance: Some(provenance),
    })
}
fn entity(kind: EntityKind, name: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(annotation(ProvenanceMarker::Ratified, &["D-1"])),
    })
}
fn corpus() -> Pack {
    Pack {
        name: "hollow".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("hollow"),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Ratify(RatifyDecl {
                    decision: id("D-1"),
                    body: dli_ast::language::RatifyBody::default(),
                }),
                entity(EntityKind::Person, "wren"),
                entity(EntityKind::Place, "thornfield"),
                fact(
                    "home",
                    &["wren", "thornfield"],
                    annotation(ProvenanceMarker::Ratified, &["D-1"]),
                ),
                fact(
                    "trade",
                    &["wren", "courier"],
                    annotation(ProvenanceMarker::Proposed, &[]),
                ),
            ],
        }],
    }
}
fn world() -> CompiledWorld {
    compile(&corpus()).unwrap()
}

#[test]
fn removing_a_ruling_fails_closed() {
    let mut pack = corpus();
    for decl in &mut pack.documents[0].decls {
        if let Decl::Fact(fact) = decl
            && fact.atom.pred.text == "home"
        {
            fact.provenance = Some(annotation(ProvenanceMarker::Ratified, &[]));
        }
    }
    let original = factcheck(&world(), &[atom("home", &["wren", "thornfield"])]);
    assert_eq!(original[0].verdict, Verdict::Supported);
    let mutated = compile(&pack).unwrap();
    assert!(
        mutated
            .model()
            .findings()
            .iter()
            .any(|finding| matches!(finding, LoadFinding::MalformedProvenance { .. }))
    );
    assert_ne!(
        factcheck(&mutated, &[atom("home", &["wren", "thornfield"])])[0].verdict,
        Verdict::Supported
    );
}

#[test]
fn duplicate_entity_is_detected() {
    let mut pack = corpus();
    assert!(check::run(&compile(&pack).unwrap()).is_empty());
    pack.documents[0]
        .decls
        .push(entity(EntityKind::Place, "thornfield"));
    let findings = check::run(&compile(&pack).unwrap());
    assert!(
        findings
            .iter()
            .any(|finding| finding.code == "load.duplicate-declaration"),
        "{findings:?}"
    );
}

#[test]
fn contradicting_the_ruled_value_is_detected() {
    let world = world();
    assert_eq!(
        factcheck(&world, &[atom("home", &["wren", "thornfield"])])[0].verdict,
        Verdict::Supported
    );
    assert_eq!(
        factcheck(&world, &[atom("home", &["wren", "ashby"])])[0].verdict,
        Verdict::Contradicted
    );
}

#[test]
fn unknown_and_proposed_claims_are_detected() {
    let world = world();
    let unknown = check::continuity::check_claims(&world, &[atom("home", &["stranger", "ashby"])]);
    assert!(
        unknown
            .iter()
            .any(|finding| finding.code == "continuity.unknown-entity"),
        "{unknown:?}"
    );
    let proposed = check::continuity::check_claims(&world, &[atom("trade", &["wren", "courier"])]);
    assert!(
        proposed
            .iter()
            .any(|finding| finding.code == "continuity.proposed-as-fact"),
        "{proposed:?}"
    );
}

#[test]
fn retired_form_is_detected() {
    let tokens = KilledTokens::new(vec![KilledToken {
        token: "old-harbor".to_owned(),
        mode: MatchMode::Word,
        decision: "D-2".to_owned(),
        scope: "retired place".to_owned(),
    }]);
    assert!(tokens.hit("new harbor").is_none());
    assert!(tokens.hit("old harbor").is_some());
}

#[test]
fn unstructured_paraphrase_is_an_explicit_strict_expected_miss() {
    // These unrelated predicates express the same intent only to a reader.
    // If the deterministic checker begins recognizing it, this assertion fails
    // and the documented miss must be reclassified instead of silently passing.
    let mut pack = corpus();
    pack.documents[0].decls.push(fact(
        "nature",
        &["wren", "courier"],
        annotation(ProvenanceMarker::Proposed, &[]),
    ));
    let findings = check::continuity::check_claims(
        &compile(&pack).unwrap(),
        &[atom("delivers", &["wren", "letters"])],
    );
    assert!(
        findings.is_empty(),
        "the known structured-input limitation changed: {findings:?}"
    );
}

#[test]
fn contradicting_a_prior_episode_is_detected() {
    let prior = [(
        ("home".to_owned(), "wren".to_owned()),
        "thornfield".to_owned(),
    )]
    .into_iter()
    .collect();
    let same =
        check::continuity::prior_contradictions(&[atom("home", &["wren", "thornfield"])], &prior);
    assert!(same.is_empty(), "{same:?}");
    let changed =
        check::continuity::prior_contradictions(&[atom("home", &["wren", "ashby"])], &prior);
    assert_eq!(changed.len(), 1, "{changed:?}");
    assert_eq!(changed[0].code, "continuity.prior-contradiction");
}

#[test]
fn citing_an_unknown_decision_cannot_make_a_claim_certain() {
    let mut pack = corpus();
    pack.documents[0].decls.push(fact(
        "occupation",
        &["wren", "courier"],
        annotation(ProvenanceMarker::Ratified, &["D-unknown"]),
    ));
    let world = compile(&pack).unwrap();
    let reports = factcheck(&world, &[atom("occupation", &["wren", "courier"])]);
    assert_ne!(reports[0].verdict, Verdict::Supported);
    assert!(
        check::run(&world)
            .iter()
            .any(|finding| finding.code == "citation.unknown-decision")
    );
}
