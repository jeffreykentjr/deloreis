#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loader front-end (FRONT-1) coverage: status gating and load-time
//! well-formedness over hand-built AST documents.

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    BeforeDecl, Decl, Document, EntityDecl, EntityKind, FactDecl, KindDecl, RuleDecl, ScopeHeader,
    TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term, TimeRef};

use dli_core::ann::{Status, Theory};
use dli_core::valuation::Valuation;

use dli_eval::query::{Query, QueryOpts};
use dli_eval::world::World;
use dli_time::chronology::{TemporalVerdict, TimePoint};

use dli_world::loader::{Loader, example_docs};
use dli_world::model::{LoadFinding, Provenance};
use dli_world::pack::Pack;

fn ident(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn ann(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    ann_cond(marker, cites, None)
}

fn ann_cond(marker: ProvenanceMarker, cites: &[&str], condition: Option<CondExpr>) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: ident(c) }).collect(),
        condition,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn atom(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: ident(pred),
        args: args.iter().map(|a| Term::Sym(ident(a))).collect(),
    }
}

fn fact(pred: &str, args: &[&str], triple: bool, provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple,
        provenance,
    })
}

fn sort(name: &str, provenance: Option<Annotation>) -> Decl {
    Decl::Kind(KindDecl {
        name: ident(name),
        provenance,
    })
}

fn doc(pack: &str, decls: Vec<Decl>) -> Document {
    Document {
        scope: Some(ScopeHeader {
            name: ident(pack),
            span: Span::synthetic(),
        }),
        decls,
    }
}

fn has_code(out: &dli_world::loader::LoadOutcome, code: &str) -> bool {
    out.findings.iter().any(|f| f.code() == code)
}

#[test]
fn golden_hand_built_world_loads_with_zero_findings() {
    let out = Loader::load(&example_docs());
    assert!(out.findings.is_empty(), "unexpected: {:?}", out.findings);
    assert_eq!(
        out.world.findings(),
        out.findings,
        "findings mirror the world"
    );
    assert_eq!(out.world.entities().len(), 3);
    assert_eq!(out.world.rule_count(), 0);
    // the compiled world carries the annotated source atoms
    assert_eq!(out.world.atoms().len(), 5);
    assert!(!out.world.facts_about("tide-bell").is_empty());
    // the event-variable table carries the cited decisions
    assert!(out.world.var_atom("D-0001").is_some());
}

#[test]
fn missing_provenance_is_a_finding_and_fail_closed() {
    let docs = vec![doc(
        "saltmarket",
        vec![fact("lives-in", &["mara", "saltmarket"], true, None)],
    )];
    let out = Loader::load(&docs);
    assert!(has_code(&out, "load.missing-provenance"));
    let claim = &out.world.atoms()[0];
    assert!(claim.is_fail_closed());
    assert_eq!(claim.kind, Provenance::Unprovenanced);
}

#[test]
fn malformed_annotation_is_fail_closed() {
    // The fail-closed theorem (spec 02 §3): a ledger marker with no ruling to
    // cite cannot compile, so the claim loads with P⁺ = FALSE and a finding.
    let docs = vec![doc(
        "saltmarket",
        vec![fact(
            "adjacent",
            &["a", "b"],
            false,
            Some(ann(ProvenanceMarker::Ratified, &[])),
        )],
    )];
    let out = Loader::load(&docs);
    assert!(has_code(&out, "load.malformed-provenance"));
    assert!(out.world.atoms()[0].is_fail_closed());
}

#[test]
fn empty_hypothesis_condition_is_fail_closed() {
    let docs = vec![doc(
        "saltmarket",
        vec![fact(
            "rings",
            &["tide-bell", "saltmarket"],
            true,
            Some(ann_cond(
                ProvenanceMarker::Proposed,
                &[],
                Some(CondExpr::Hyp(ident("   "))),
            )),
        )],
    )];
    let out = Loader::load(&docs);
    assert!(has_code(&out, "load.malformed-provenance"));
    assert!(out.world.atoms()[0].is_fail_closed());
}

#[test]
fn proposed_lore_is_structurally_unassertable() {
    let docs = vec![doc(
        "saltmarket",
        vec![fact(
            "rings",
            &["tide-bell", "saltmarket"],
            true,
            Some(ann(ProvenanceMarker::Proposed, &[])),
        )],
    )];
    let out = Loader::load(&docs);
    assert!(out.findings.is_empty());
    let claim = &out.world.atoms()[0];
    assert_eq!(claim.kind, Provenance::Proposed);
    assert!(!claim.is_fail_closed());
    // a live shadow, never certain, until a ruling retires the hypothesis
    assert_eq!(
        claim.ann.status(&Valuation::new(), &Theory::new()),
        Status::Possible
    );
}

#[test]
fn decided_lore_becomes_certain_when_its_decision_is_ruled() {
    let docs = vec![doc(
        "saltmarket",
        vec![fact(
            "adjacent",
            &["saltmarket", "the-shallows"],
            false,
            Some(ann(ProvenanceMarker::Decided, &["D-0002"])),
        )],
    )];
    let out = Loader::load(&docs);
    assert!(out.findings.is_empty());
    let claim = &out.world.atoms()[0];
    assert_eq!(claim.kind, Provenance::Decided);
    // open ledger: a shadow, not yet assertable
    assert_eq!(
        claim.ann.status(&Valuation::new(), &Theory::new()),
        Status::Possible
    );
    // rule the cited decision true — assertable, no re-derivation needed
    let decision = out.world.var_atom("D-0002").expect("citation interned");
    let mut nu = Valuation::new();
    nu.set(decision, true);
    assert_eq!(claim.ann.status(&nu, &Theory::new()), Status::Certain);
}

#[test]
fn duplicate_declaration_within_a_pack_is_a_finding() {
    let docs = vec![doc(
        "saltmarket",
        vec![
            sort("place", Some(ann(ProvenanceMarker::Ratified, &["D-0001"]))),
            sort("place", Some(ann(ProvenanceMarker::Ratified, &["D-0001"]))),
        ],
    )];
    let out = Loader::load(&docs);
    assert!(out.findings.iter().any(|f| matches!(
        f,
        LoadFinding::DuplicateDeclaration { name, .. } if name == "place"
    )));
}

#[test]
fn same_name_across_two_packs_is_a_collision() {
    let docs = vec![
        doc(
            "north",
            vec![sort(
                "place",
                Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
            )],
        ),
        doc(
            "south",
            vec![sort(
                "place",
                Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
            )],
        ),
    ];
    let out = Loader::load(&docs);
    assert!(out.findings.iter().any(|f| matches!(
        f,
        LoadFinding::CrossPackCollision { name, .. } if name == "place"
    )));
}

#[test]
fn a_well_formed_rule_loads_totally_and_clean() {
    // Rules arrive with a captured (text) body; the loader stays total and
    // checks the rule's provenance. Typed head/body intake, range restriction,
    // and stratification go through the evaluator's rule seam, so the loader
    // does not lower the text body and `rule_count` stays 0 here.
    let rule = Decl::Rule(RuleDecl {
        name: ident("adjacency-symmetric"),
        domain: None,
        head: Vec::new(),
        body: vec![Clause {
            text: "adjacent(A, B) :- adjacent(B, A)".to_owned(),
            span: Span::synthetic(),
        }],
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let docs = vec![doc("saltmarket", vec![rule])];
    let out = Loader::load(&docs);
    assert!(out.findings.is_empty(), "unexpected: {:?}", out.findings);
    assert_eq!(out.world.rule_count(), 0);
}

#[test]
fn a_rule_without_provenance_is_a_finding() {
    let rule = Decl::Rule(RuleDecl {
        name: ident("adjacency-symmetric"),
        domain: None,
        head: Vec::new(),
        body: Vec::new(),
        provenance: None,
    });
    let docs = vec![doc("saltmarket", vec![rule])];
    let out = Loader::load(&docs);
    assert!(has_code(&out, "load.missing-provenance"));
}

#[test]
#[ignore = "enabled when rule-body negation + typed rule lowering land"]
fn negative_dependency_cycle_is_a_load_finding() {
    // The loader already consumes the dli_eval::stratify seam in `load` and maps
    // Err(Unstratifiable) -> LoadFinding::Unstratifiable. The finding cannot
    // *fire* yet: the loader lowers no typed rules (rule bodies are captured
    // text), and negation lives only in annotation conditions, so a program like
    //     p :- not q
    //     q :- not p
    // is not constructible as input. Un-ignore once rule-body negation exists and
    // RuleDecl -> typed world.rules lowering routes through World::define_rule.
    todo!("wire a negative-cycle document once structured rules + rule-body negation land")
}

// --- the compiled world's timeline is live (defect F) ------------------------

fn time_point(world: &World, name: &str) -> TimePoint {
    TimePoint(
        u32::try_from(
            world
                .time_points
                .names()
                .iter()
                .position(|candidate| candidate == name)
                .expect("time point present"),
        )
        .expect("index fits"),
    )
}

#[test]
fn before_declarations_build_a_live_chronology() {
    let docs = vec![doc(
        "chron",
        vec![Decl::Before(BeforeDecl {
            earlier: ident("past"),
            later: ident("future"),
            provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
        })],
    )];
    let world = Loader::load(&docs).world.world;
    let past = time_point(&world, "past");
    let future = time_point(&world, "future");
    assert_eq!(
        world.chronology.precedes(past, future),
        TemporalVerdict::Before
    );
    assert_eq!(
        world.chronology.precedes(future, past),
        TemporalVerdict::After
    );
}

#[test]
fn chronology_and_time_points_match_the_evaluator_lowering() {
    // The loader must build the timeline byte-for-byte as `World::lower` does:
    // era-order chains and `before`-decls, interned in document order. The chain
    // is head-exclusive as the parser emits it (`era first < second < third` →
    // name `first`, chain `[second, third]`); both lowerings prepend the head.
    let d = Document {
        scope: Some(ScopeHeader {
            name: ident("chron"),
            span: Span::synthetic(),
        }),
        decls: vec![
            Decl::Entity(EntityDecl {
                kind: EntityKind::Era,
                name: ident("first"),
                label: None,
                time: Some(TimeClause::Order(vec![ident("second"), ident("third")])),
                slots: Vec::new(),
                provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
            }),
            Decl::Before(BeforeDecl {
                earlier: ident("dawn"),
                later: ident("dusk"),
                provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
            }),
        ],
    };
    let loaded = Loader::load(std::slice::from_ref(&d)).world.world;
    let lowered = World::lower(&d).expect("lowers");
    assert_eq!(loaded.chronology, lowered.chronology, "chronology parity");
    assert_eq!(
        loaded.time_points.names(),
        lowered.time_points.names(),
        "time-point table parity"
    );
}

#[test]
fn as_of_filters_atoms_anchored_after_the_cutoff() {
    // dawn -> dusk, plus a fact anchored to dusk. Before F the chronology was
    // empty, so `strictly_after` was empty and as_of dropped nothing.
    let lit = Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: ident("lit"),
            args: vec![
                Term::Sym(ident("lamp")),
                Term::Time(TimeRef::Event(ident("dusk"))),
            ],
        },
        triple: false,
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
    });
    let docs = vec![doc(
        "chron",
        vec![
            Decl::Before(BeforeDecl {
                earlier: ident("dawn"),
                later: ident("dusk"),
                provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
            }),
            lit,
        ],
    )];
    let compiled = dli_world::compile(&Pack {
        name: "chron".to_owned(),
        documents: docs,
    })
    .expect("compiles");
    let world = &compiled.model().world;
    let goal = Query {
        goal: AtomExpr {
            pred: ident("lit"),
            args: vec![Term::Var(ident("what")), Term::Var(ident("when"))],
        },
    };
    let all = world
        .query_with(&goal, &QueryOpts::default())
        .expect("known pred");
    assert_eq!(all.rows.len(), 1, "the anchored fact is present unfiltered");
    let filtered = world
        .query_with(
            &goal,
            &QueryOpts {
                as_of: Some("dawn".to_owned()),
                ..QueryOpts::default()
            },
        )
        .expect("known pred");
    assert!(
        filtered.rows.is_empty(),
        "as_of dawn must drop the dusk-anchored fact"
    );
}

#[test]
fn a_lone_era_reaches_the_timeline_even_with_no_edge() {
    // A standalone era in no `before`-fact and no order chain must still be
    // interned, so the timeline lists it (finding #3): interning happens at the
    // declaration, not only at edge endpoints.
    let docs = vec![doc(
        "chron",
        vec![Decl::Entity(EntityDecl {
            kind: EntityKind::Era,
            name: ident("first-tides"),
            label: None,
            time: None,
            slots: Vec::new(),
            provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
        })],
    )];
    let world = Loader::load(&docs).world.world;
    assert!(
        world.time_points.names().iter().any(|n| n == "first-tides"),
        "a lone era must be interned into the time-point table"
    );
}
