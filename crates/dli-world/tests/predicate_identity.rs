#![allow(clippy::expect_used, clippy::unwrap_used)]
//! The compiled-world predicate-identity lane: declaration-only intake,
//! certainty gating, closure, FD/fluent provenance, arity refusal, and order
//! independence.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    Decl, Document, EntityDecl, EntityKind, FactDecl, FunctionalDecl, InertialDecl,
    PredicateSameAsDecl, RatifyDecl, RuleDecl, SameAsDecl, ScopeHeader, StateDecl, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_world::check;
use dli_world::compile;
use dli_world::pack::Pack;

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn annotation(marker: ProvenanceMarker, decision: Option<&str>) -> Annotation {
    Annotation {
        marker,
        citations: decision
            .into_iter()
            .map(|decision| CitationRef { id: id(decision) })
            .collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratify(decision: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(decision),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn fact(predicate: &str, subject: &str, value: &str, marker: ProvenanceMarker) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(predicate),
            args: vec![Term::Sym(id(subject)), Term::Sym(id(value))],
        },
        triple: false,
        provenance: Some(annotation(marker, Some("D-1"))),
    })
}

fn functional(predicate: &str) -> Decl {
    Decl::Functional(FunctionalDecl {
        pred: id(predicate),
        provenance: Some(annotation(ProvenanceMarker::Ratified, Some("D-1"))),
    })
}

fn predicate_same_as(left: &str, right: &str, marker: ProvenanceMarker) -> Decl {
    Decl::PredicateSameAs(PredicateSameAsDecl {
        left: id(left),
        right: id(right),
        provenance: Some(annotation(marker, Some("D-1"))),
    })
}

fn entity_same_as(left: &str, right: &str) -> Decl {
    Decl::SameAs(SameAsDecl {
        left: id(left),
        right: id(right),
        provenance: Some(annotation(ProvenanceMarker::Ratified, Some("D-1"))),
    })
}

fn event(name: &str, decision: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind: EntityKind::Event,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(annotation(ProvenanceMarker::Ratified, Some(decision))),
    })
}

fn inertial(predicate: &str, decision: &str) -> Decl {
    Decl::Inertial(InertialDecl {
        pred: id(predicate),
        provenance: Some(annotation(ProvenanceMarker::Ratified, Some(decision))),
    })
}

fn state(slot: &str, value: &str, event_name: &str, decision: &str) -> Decl {
    Decl::State(StateDecl {
        name: id(slot),
        value: Some(Term::Sym(id(value))),
        time: Some(TimeClause::Since(id(event_name))),
        provenance: Some(annotation(ProvenanceMarker::Ratified, Some(decision))),
    })
}

fn rule(name: &str, text: &str) -> Decl {
    let clause = Clause {
        text: text.to_owned(),
        span: Span::synthetic(),
    };
    Decl::Rule(RuleDecl {
        name: id(name),
        domain: None,
        head: Vec::new(),
        body: dli_syntax::structure_rule_body(&[clause], &id(name), &[]),
        provenance: Some(annotation(ProvenanceMarker::Ratified, Some("D-1"))),
    })
}

fn pack(decls: Vec<Decl>) -> Pack {
    Pack {
        name: "predicate-test".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("predicate-test"),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

fn findings_with_code<'a>(
    findings: &'a [check::WorldFinding],
    code: &str,
) -> Vec<&'a check::WorldFinding> {
    findings
        .iter()
        .filter(|finding| finding.code == code)
        .collect()
}

fn accepted_decls(identity_first: bool) -> Vec<Decl> {
    let mut decls = vec![ratify("D-1"), functional("current-work")];
    let identity = predicate_same_as("current-work", "current-job", ProvenanceMarker::Ratified);
    let facts = vec![
        fact(
            "current-work",
            "mara",
            "software",
            ProvenanceMarker::Ratified,
        ),
        fact(
            "current-job",
            "mara",
            "employment",
            ProvenanceMarker::Ratified,
        ),
    ];
    if identity_first {
        decls.push(identity);
        decls.extend(facts);
    } else {
        decls.extend(facts);
        decls.push(identity);
    }
    decls
}

#[test]
fn accepted_predicate_identity_creates_an_fd_finding_with_structured_support() {
    let world = compile(&pack(accepted_decls(false))).expect("compiles");
    let findings = check::run(&world);
    let fd = findings_with_code(&findings, "continuity.fd-violation");
    assert_eq!(fd.len(), 1, "one merged functional slot: {fd:?}");
    let data = fd[0].data.as_ref().expect("structured FD data");
    assert_eq!(data["pred"], "current-job");
    let identity = data["predicate_identity"]
        .as_array()
        .expect("identity support");
    assert_eq!(identity[0]["left"], "current-job");
    assert_eq!(identity[0]["right"], "current-work");
    assert_eq!(identity[0]["citations"], serde_json::json!(["D-1"]));
    assert!(fd[0].citations.contains(&"D-1".to_owned()));
    let promotions = findings_with_code(&findings, "predicate-identity.policy-promoted");
    assert_eq!(promotions.len(), 1);
    assert_eq!(
        promotions[0].data.as_ref().expect("promotion data")["policies"],
        serde_json::json!(["functional"])
    );
}

#[test]
fn proposed_predicate_identity_does_not_activate_fd_check() {
    let mut decls = accepted_decls(false);
    decls.pop();
    decls.push(predicate_same_as(
        "current-work",
        "current-job",
        ProvenanceMarker::Proposed,
    ));
    let world = compile(&pack(decls)).expect("compiles");
    assert!(findings_with_code(&check::run(&world), "continuity.fd-violation").is_empty());
}

#[test]
fn missing_predicate_identity_provenance_is_reported_not_dropped() {
    let mut decls = vec![ratify("D-1"), functional("current-work")];
    decls.extend([
        fact(
            "current-work",
            "mara",
            "software",
            ProvenanceMarker::Ratified,
        ),
        fact(
            "current-job",
            "mara",
            "employment",
            ProvenanceMarker::Ratified,
        ),
        Decl::PredicateSameAs(PredicateSameAsDecl {
            left: id("current-work"),
            right: id("current-job"),
            provenance: None,
        }),
    ]);
    let world = compile(&pack(decls)).expect("missing provenance is graded");
    assert!(
        world
            .model
            .findings()
            .iter()
            .any(|finding| finding.code() == "load.missing-provenance")
    );
    assert!(findings_with_code(&check::run(&world), "continuity.fd-violation").is_empty());
}

#[test]
fn facts_before_and_after_identity_have_identical_verdicts_and_support() {
    let before = compile(&pack(accepted_decls(false))).expect("compiles");
    let after = compile(&pack(accepted_decls(true))).expect("compiles");
    assert_eq!(check::run(&before), check::run(&after));
    assert_eq!(
        serde_json::to_value(&before.model.world).expect("serializes"),
        serde_json::to_value(&after.model.world).expect("serializes")
    );
}

#[test]
fn transitive_predicate_identity_and_self_merge_are_stable() {
    let mut decls = vec![ratify("D-1"), functional("a")];
    decls.extend([
        fact("a", "mara", "one", ProvenanceMarker::Ratified),
        fact("c", "mara", "two", ProvenanceMarker::Ratified),
        predicate_same_as("a", "b", ProvenanceMarker::Ratified),
        predicate_same_as("b", "c", ProvenanceMarker::Ratified),
        predicate_same_as("c", "c", ProvenanceMarker::Ratified),
    ]);
    let world = compile(&pack(decls)).expect("compiles");
    let findings = check::run(&world);
    let fd = findings_with_code(&findings, "continuity.fd-violation");
    assert_eq!(fd.len(), 1, "transitive class reaches one slot: {fd:?}");
}

#[test]
fn incompatible_observed_arity_is_reported_and_does_not_partially_merge() {
    let mut decls = vec![ratify("D-1"), functional("left")];
    decls.extend([
        fact("left", "mara", "one", ProvenanceMarker::Ratified),
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: id("right"),
                args: vec![
                    Term::Sym(id("mara")),
                    Term::Sym(id("two")),
                    Term::Sym(id("extra")),
                ],
            },
            triple: false,
            provenance: Some(annotation(ProvenanceMarker::Ratified, Some("D-1"))),
        }),
        predicate_same_as("left", "right", ProvenanceMarker::Ratified),
    ]);
    let world = compile(&pack(decls)).expect("graded mismatch is not a compiler crash");
    let findings = check::run(&world);
    let mismatch = findings_with_code(&findings, "predicate-identity.arity-mismatch");
    assert_eq!(mismatch.len(), 1, "one component mismatch: {mismatch:?}");
    assert_eq!(
        mismatch[0].data.as_ref().expect("structured mismatch")["arities"],
        serde_json::json!([2, 3])
    );
    assert!(findings_with_code(&findings, "continuity.fd-violation").is_empty());
}

#[test]
fn unrelated_multiple_observed_arities_do_not_raise_identity_findings() {
    let world = compile(&pack(vec![
        ratify("D-1"),
        fact("marker", "mara", "one", ProvenanceMarker::Ratified),
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred: id("marker"),
                args: vec![Term::Sym(id("mara")), Term::Sym(id("two"))],
            },
            triple: false,
            provenance: Some(annotation(ProvenanceMarker::Ratified, Some("D-1"))),
        }),
    ]))
    .expect("unrelated arities compile");
    assert!(
        findings_with_code(&check::run(&world), "predicate-identity.arity-mismatch").is_empty()
    );
}

#[test]
fn entity_and_predicate_identity_are_confluent_in_arrival_order() {
    let mut common = vec![
        ratify("D-1"),
        functional("current-work"),
        fact(
            "current-work",
            "mara",
            "software",
            ProvenanceMarker::Ratified,
        ),
        fact(
            "current-job",
            "moira",
            "employment",
            ProvenanceMarker::Ratified,
        ),
    ];
    let entity = entity_same_as("mara", "moira");
    let predicate = predicate_same_as("current-work", "current-job", ProvenanceMarker::Ratified);
    let mut entity_first = common.clone();
    entity_first.insert(1, entity.clone());
    entity_first.insert(2, predicate.clone());
    let mut predicate_first = std::mem::take(&mut common);
    predicate_first.insert(1, predicate);
    predicate_first.insert(2, entity);

    let left = compile(&pack(entity_first)).expect("entity-first compiles");
    let right = compile(&pack(predicate_first)).expect("predicate-first compiles");
    assert_eq!(check::run(&left), check::run(&right));
    assert_eq!(
        serde_json::to_value(&left.model.world).expect("serializes"),
        serde_json::to_value(&right.model.world).expect("serializes")
    );
}

#[test]
fn merged_predicate_fluent_spans_clash_and_defeat_unpersists_them() {
    let decls = vec![
        ratify("D-inertia"),
        ratify("D-left"),
        ratify("D-right"),
        ratify("D-merge"),
        event("e", "D-left"),
        inertial("current-work", "D-inertia"),
        state("mara.current-work", "software", "e", "D-left"),
        state("mara.current-job", "employment", "e", "D-right"),
        predicate_same_as("current-work", "current-job", ProvenanceMarker::Ratified),
    ];
    // The helper's identity declaration cites D-1, so use a directly-built
    // equivalent with the dedicated merge decision for an isolated defeat.
    let mut decls = decls;
    if let Decl::PredicateSameAs(identity) = decls.last_mut().expect("identity") {
        identity.provenance = Some(annotation(ProvenanceMarker::Ratified, Some("D-merge")));
    }
    let mut world = compile(&pack(decls)).expect("compiles");
    let findings = check::run(&world);
    let clashes = findings_with_code(&findings, "continuity.fluent-contradiction");
    assert_eq!(
        clashes.len(),
        1,
        "merged fluent slots overlap: {clashes:?}; fluents={:?}; identity={:?}; canon={:?}; valuation={:?}; theory={:?}; statuses={:?}",
        world.model.fluents.keys(),
        world.model.predicate_identity,
        world
            .model
            .predicate_identity
            .certain_canon(&world.model.world.valuation, &world.model.world.theory,),
        world.model.world.valuation,
        world.model.world.theory,
        world
            .model
            .predicate_identity
            .links
            .iter()
            .map(|link| link
                .ann
                .status(&world.model.world.valuation, &world.model.world.theory))
            .collect::<Vec<_>>()
    );
    assert!(clashes[0].citations.contains(&"D-merge".to_owned()));
    assert!(clashes[0].data.as_ref().is_some_and(|data| {
        data["predicate_identity"]
            .as_array()
            .is_some_and(|links| !links.is_empty())
    }));

    let merge_var = world.model.var_atom("D-merge").expect("merge decision");
    world.model.world.valuation.set(merge_var, false);
    let after = check::run(&world);
    assert!(findings_with_code(&after, "continuity.fluent-contradiction").is_empty());
}

#[test]
fn compiled_rules_use_predicate_identity_in_bodies_and_heads() {
    let world = compile(&pack(vec![
        ratify("D-1"),
        fact(
            "current-work",
            "mara",
            "software",
            ProvenanceMarker::Ratified,
        ),
        predicate_same_as("current-work", "current-job", ProvenanceMarker::Ratified),
        rule(
            "employment-from-job",
            "employed($who) :- current-job($who, software)",
        ),
    ]))
    .expect("compiles");
    let employed = world
        .model
        .world
        .derived
        .iter()
        .find(|row| row.atom.pred == "employed" && row.atom.args == ["mara"])
        .expect("rule body sees the merged predicate");
    assert_eq!(
        employed
            .ann()
            .status(&world.model.world.valuation, &world.model.world.theory),
        dli_core::ann::Status::Certain
    );
}
