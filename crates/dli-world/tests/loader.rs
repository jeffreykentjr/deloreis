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
    assert!(!out.world.facts_about("excalibur").is_empty());
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
    // A bare world rule now lowers: the loader parses its captured `head :- body`
    // clause into a typed derivation rule and routes it through the evaluator's
    // negation-aware intake, so `world.rules` populates. This positive,
    // stratifiable rule loads clean.
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
    assert_eq!(out.world.rule_count(), 1);
}

#[test]
fn a_bare_world_rule_with_negation_lowers() {
    // `ward-holds($g) :- seated($g), not breached($g)` — a range-restricted world
    // rule with a negated body literal lowers cleanly and populates world.rules.
    let rule = Decl::Rule(RuleDecl {
        name: ident("ward-holds"),
        domain: None,
        head: Vec::new(),
        body: vec![Clause {
            text: "ward-holds($g) :- seated($g), not breached($g)".to_owned(),
            span: Span::synthetic(),
        }],
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let out = Loader::load(&[doc("wards", vec![rule])]);
    assert!(out.findings.is_empty(), "unexpected: {:?}", out.findings);
    assert_eq!(out.world.rule_count(), 1);
    let lowered = &out.world.world.rules[0];
    assert_eq!(lowered.head.pred.text, "ward-holds");
    assert_eq!(lowered.body.len(), 2);
    assert!(!lowered.body[0].negated); // seated($g)
    assert!(lowered.body[1].negated); // not breached($g)
}

#[test]
fn a_negation_only_variable_is_a_range_restriction_finding() {
    // `risky($x) :- not b($x)` — $x appears only in a negated literal, so nothing
    // positive binds it. Range restriction rejects it as a load finding, and no
    // rule is lowered (datalog safety, spec 08 §1).
    let rule = Decl::Rule(RuleDecl {
        name: ident("risky"),
        domain: None,
        head: Vec::new(),
        body: vec![Clause {
            text: "risky($x) :- not b($x)".to_owned(),
            span: Span::synthetic(),
        }],
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let out = Loader::load(&[doc("wards", vec![rule])]);
    assert!(
        has_code(&out, "load.unbound-head-variable"),
        "{:?}",
        out.findings
    );
    assert_eq!(out.world.rule_count(), 0);
}

#[test]
fn an_over_domain_rule_is_not_lowered_as_a_world_rule() {
    // `rule … over order` / `… over season` are checker rules for other engines;
    // they must never populate the derivation-rule set even with a datalog-looking
    // body clause.
    let rule = Decl::Rule(RuleDecl {
        name: ident("full-team-floor"),
        domain: Some(dli_ast::decl::RuleDomain::Order),
        head: Vec::new(),
        body: vec![Clause {
            text: "held($e) :- listener-has($e)".to_owned(),
            span: Span::synthetic(),
        }],
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let out = Loader::load(&[doc("order", vec![rule])]);
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
fn negative_dependency_cycle_is_a_load_finding() {
    // Two bare world rules whose heads depend negatively on each other:
    //     p :- not q
    //     q :- not p
    // Each lowers cleanly (propositional, range-restricted), but the rule set has
    // no valid stratification — a negative cycle. The loader consumes the
    // dli_eval::stratify seam in `finish` and maps Err(Unstratifiable) ->
    // LoadFinding::Unstratifiable (spec 08 §1).
    let p_rule = Decl::Rule(RuleDecl {
        name: ident("p-rule"),
        domain: None,
        head: Vec::new(),
        body: vec![Clause {
            text: "p :- not q".to_owned(),
            span: Span::synthetic(),
        }],
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let q_rule = Decl::Rule(RuleDecl {
        name: ident("q-rule"),
        domain: None,
        head: Vec::new(),
        body: vec![Clause {
            text: "q :- not p".to_owned(),
            span: Span::synthetic(),
        }],
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let out = Loader::load(&[doc("cycle", vec![p_rule, q_rule])]);
    assert_eq!(out.world.rule_count(), 2, "both rules lower");
    assert!(
        out.findings
            .iter()
            .any(|f| matches!(f, LoadFinding::Unstratifiable { .. })),
        "expected an Unstratifiable finding: {:?}",
        out.findings
    );
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
