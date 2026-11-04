#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Loader front-end (FRONT-1) coverage: status gating and load-time
//! well-formedness over hand-built AST documents.

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    AttrDecl, BeforeDecl, Decl, Document, EntityDecl, EntityKind, FactDecl, KindDecl, RuleDecl,
    ScopeHeader, StateDecl, TimeClause,
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

#[test]
fn chronology_cycle_points_to_a_certain_edge_source() {
    let source = "scope storybook\nratify d-1\nera first < second proposed\nera first < second ratified d-1\nera second < first ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let finding = dli_world::check::run(&world)
        .into_iter()
        .find(|finding| finding.code == "continuity.chronology-cycle")
        .unwrap();
    let span = finding.span.unwrap();
    let line = source[..span.start as usize]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1;
    assert!(
        line == 4 || line == 5,
        "must identify a certain edge, got line {line}"
    );
}

#[test]
fn functional_violation_uses_a_real_source_for_equivalent_values() {
    let source = "scope storybook\nratify d-0001\nfunctional home ratified d-0001\nfact wren home Shore ratified d-0001\nfact wren home shore ratified d-0001\nfact wren home Hill ratified d-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let mut world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    for (index, atom) in world
        .model
        .atoms
        .iter_mut()
        .filter(|atom| atom.atom.pred.text == "home")
        .enumerate()
    {
        if index != 1 {
            atom.span = Span::synthetic();
        }
    }
    let findings = dli_world::check::run(&world);
    let finding = findings
        .iter()
        .find(|finding| finding.code == "continuity.fd-violation")
        .unwrap();
    let span = finding.span.unwrap();
    let line = source[..span.start as usize]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count()
        + 1;
    assert_eq!(line, 5);
}

#[test]
fn unanchored_fluent_location_uses_its_exact_source_or_none() {
    let source = "scope storybook\nratify d-1\nevent first ratified d-1\ninertial home ratified d-1\nstate wren.home = ashby since first ratified d-1\nstate wren.home = ashby ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let mut world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let warning = |world: &dli_world::check::CompiledWorld| {
        dli_world::check::run(world)
            .into_iter()
            .find(|finding| finding.code == "continuity.inertia-unanchored")
            .unwrap()
    };
    let span = warning(&world).span.unwrap();
    let line = source[..span.start as usize]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count()
        + 1;
    assert_eq!(
        line, 6,
        "must point to the unanchored declaration, not the earlier same-value state"
    );
    world.model.temporal_sources_complete = false;
    assert_eq!(
        warning(&world).span,
        None,
        "do not guess when source mappings are incomplete"
    );
}

#[test]
fn unordered_fluent_location_uses_a_contributing_state_or_none() {
    let source = "scope storybook\nratify d-1\nevent first ratified d-1\nevent second ratified d-1\ninertial home ratified d-1\nstate wren.home = ashby since first ratified d-1\nstate wren.home = thornfield since second ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let mut world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let warning = |world: &dli_world::check::CompiledWorld| {
        dli_world::check::run(world)
            .into_iter()
            .find(|finding| finding.code == "continuity.fluent-order-undeclared")
            .unwrap()
    };
    let span = warning(&world).span.unwrap();
    let line = source[..span.start as usize]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count()
        + 1;
    assert!(
        [6, 7].contains(&line),
        "must point to one of the unordered states, got {line}"
    );
    world.model.temporal_sources_complete = false;
    assert_eq!(
        warning(&world).span,
        None,
        "do not guess when source mappings are incomplete"
    );
}

#[test]
fn unordered_fluent_location_survives_aliasing_and_missing_source_entries() {
    let source = "scope storybook\nratify d-1\nevent first ratified d-1\nevent second ratified d-1\ninertial home ratified d-1\nsame-as wren = robin ratified d-1\nstate wren.home = ashby since first ratified d-1\nstate robin.home = thornfield since second ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let mut world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let warning = |world: &dli_world::check::CompiledWorld| {
        dli_world::check::run(world)
            .into_iter()
            .find(|finding| finding.code == "continuity.fluent-order-undeclared")
            .unwrap()
    };
    let first_state = world
        .model
        .atoms
        .iter()
        .position(|atom| {
            atom.atom.pred.text == "home"
                && atom
                    .atom
                    .args
                    .first()
                    .is_some_and(|arg| matches!(arg, Term::Sym(id) if id.text == "wren"))
        })
        .unwrap();
    world.model.temporal_atom_facts.remove(&first_state);
    let span = warning(&world).span.unwrap();
    let line = source[..span.start as usize]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count()
        + 1;
    assert_eq!(
        line, 8,
        "must locate the other contributing state across the identity merge"
    );
    world.model.temporal_atom_facts.clear();
    assert_eq!(
        warning(&world).span,
        None,
        "do not guess when both source entries are missing"
    );
}

#[test]
fn draft_conflict_location_skips_synthetic_contributors() {
    let source = "scope storybook\nfunctional home proposed\nfact wren home ashby proposed a\nfact wren home thornfield proposed b\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let mut world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let first = world
        .model
        .atoms
        .iter_mut()
        .find(|atom| atom.atom.pred.text == "home")
        .unwrap();
    first.span = Span::synthetic();
    let warning = dli_world::check::run(&world)
        .into_iter()
        .find(|finding| finding.code == "continuity.pending-conflict")
        .unwrap();
    let span = warning.span.unwrap();
    let line = source[..span.start as usize]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count()
        + 1;
    assert_eq!(line, 4, "must locate the other contributing draft");
    for atom in &mut world.model.atoms {
        atom.span = Span::synthetic();
    }
    let warning = dli_world::check::run(&world)
        .into_iter()
        .find(|finding| finding.code == "continuity.pending-conflict")
        .unwrap();
    assert_eq!(
        warning.span, None,
        "synthetic sources must remain unlocated"
    );
}

/// Build a typed rule body from block-clause texts, exactly as the front end
/// structures a captured `rule` body (LS-9): each datalog clause is pre-parsed,
/// non-datalog clauses keep their text with a `None` datalog.
fn rule_body(texts: &[&str]) -> dli_ast::rule::RuleBody {
    let clauses: Vec<Clause> = texts
        .iter()
        .map(|t| Clause {
            text: (*t).to_owned(),
            span: Span::synthetic(),
        })
        .collect();
    dli_syntax::structure_rule_body(&clauses, &ident("rule"), &[])
}

fn ann(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    ann_cond(marker, cites, None)
}

fn ann_cond(marker: ProvenanceMarker, cites: &[&str], condition: Option<CondExpr>) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: ident(c) }).collect(),
        condition,
        bundle: None,
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

fn state_decl(
    name: &str,
    value: Option<Term>,
    time: Option<TimeClause>,
    provenance: Option<Annotation>,
) -> Decl {
    Decl::State(StateDecl {
        name: ident(name),
        value,
        time,
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
fn string_attributes_are_evaluable_facts() {
    let docs = vec![doc(
        "storybook",
        vec![Decl::Attr(AttrDecl {
            subject: ident("lyra"),
            attr: ident("role"),
            value: Term::Str("river warden".to_owned()),
            provenance: Some(ann(ProvenanceMarker::Proposed, &[])),
        })],
    )];
    let compiled = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: docs,
    })
    .expect("attribute world compiles");
    let answer = compiled
        .model()
        .world
        .query(&Query {
            goal: AtomExpr {
                pred: ident("role"),
                args: vec![Term::Sym(ident("lyra")), Term::Var(ident("value"))],
            },
        })
        .expect("attribute predicate is evaluable");

    assert_eq!(answer.rows.len(), 1);
    assert_eq!(
        answer.rows[0].bindings.get("value").map(String::as_str),
        Some("river warden")
    );
}

#[test]
fn state_declarations_are_evaluable_facts() {
    // Issue 47: a `state` lowers to a queryable ground fact, exactly as an
    // attribute does — `state joren.water-status = commons` answers a query for
    // `water-status(joren, ?)`.
    let docs = vec![doc(
        "weir",
        vec![state_decl(
            "joren.water-status",
            Some(Term::Sym(ident("commons"))),
            None,
            Some(ann(ProvenanceMarker::Proposed, &[])),
        )],
    )];
    let compiled = dli_world::compile(&Pack {
        name: "weir".to_owned(),
        documents: docs,
    })
    .expect("state world compiles");
    let answer = compiled
        .model()
        .world
        .query(&Query {
            goal: AtomExpr {
                pred: ident("water-status"),
                args: vec![Term::Sym(ident("joren")), Term::Var(ident("value"))],
            },
        })
        .expect("the state predicate is evaluable");
    assert_eq!(answer.rows.len(), 1);
    assert_eq!(
        answer.rows[0].bindings.get("value").map(String::as_str),
        Some("commons")
    );
}

#[test]
fn missing_provenance_state_is_a_finding_and_fail_closed() {
    // A provenance-less state claims something from nowhere, like any other slot.
    let docs = vec![doc(
        "weir",
        vec![state_decl(
            "joren.water-status",
            Some(Term::Sym(ident("commons"))),
            None,
            None,
        )],
    )];
    let out = Loader::load(&docs);
    assert!(has_code(&out, "load.missing-provenance"));
    assert!(out.world.atoms()[0].is_fail_closed());
}

#[test]
fn an_inert_state_declaration_surfaces_a_finding() {
    // Koan 4 / DL-0029: a state that names nothing evaluable (undotted, no value)
    // is surfaced as a finding rather than silently dropped.
    let docs = vec![doc(
        "weir",
        vec![state_decl(
            "waterlogged",
            None,
            None,
            Some(ann(ProvenanceMarker::Proposed, &[])),
        )],
    )];
    let out = Loader::load(&docs);
    assert!(
        has_code(&out, "load.inert-declaration"),
        "unexpected: {:?}",
        out.findings
    );
}

#[test]
fn missing_provenance_is_a_finding_and_fail_closed() {
    let docs = vec![doc(
        "storybook",
        vec![fact("lives-in", &["lyra", "storybook"], true, None)],
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
        "storybook",
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
        "storybook",
        vec![fact(
            "rings",
            &["bellsong", "storybook"],
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
        "storybook",
        vec![fact(
            "rings",
            &["bellsong", "storybook"],
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
        "storybook",
        vec![fact(
            "adjacent",
            &["storybook", "riverside"],
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
        "storybook",
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
fn temporal_clause_requires_a_declared_target_of_the_right_kind() {
    let event = Decl::Entity(EntityDecl {
        kind: EntityKind::Event,
        name: ident("harvest"),
        label: None,
        time: Some(TimeClause::During(ident("missing-age"))),
        slots: Vec::new(),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
    });
    let out = Loader::load(&[doc("timeline", vec![event])]);

    assert!(out.findings.iter().any(|finding| matches!(
        finding,
        LoadFinding::UnknownTemporalReference {
            expected_kind,
            target,
            ..
        } if expected_kind == "era" && target == "missing-age"
    )));
    assert!(has_code(&out, "load.unknown-temporal-reference"));
}

#[test]
fn temporal_clause_accepts_forward_and_era_chain_declarations() {
    let event = Decl::Entity(EntityDecl {
        kind: EntityKind::Event,
        name: ident("harvest"),
        label: None,
        time: Some(TimeClause::During(ident("second-age"))),
        slots: Vec::new(),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
    });
    let eras = Decl::Entity(EntityDecl {
        kind: EntityKind::Era,
        name: ident("first-age"),
        label: None,
        time: Some(TimeClause::Order(vec![ident("second-age")])),
        slots: Vec::new(),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0001"])),
    });
    let out = Loader::load(&[doc("timeline", vec![event, eras])]);

    assert!(!has_code(&out, "load.unknown-temporal-reference"));
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
        body: rule_body(&["adjacent(A, B) :- adjacent(B, A)"]),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let docs = vec![doc("storybook", vec![rule])];
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
        body: rule_body(&["ward-holds($g) :- seated($g), not breached($g)"]),
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
        body: rule_body(&["risky($x) :- not b($x)"]),
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
    // Domain rules must not populate the derivation-rule set, even when their
    // body looks like Datalog. The loader reports their unchecked constraint.
    let rule = Decl::Rule(RuleDecl {
        name: ident("full-team-floor"),
        domain: Some(dli_ast::decl::RuleDomain::Order),
        head: Vec::new(),
        body: rule_body(&["held($e) :- listener-has($e)"]),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let out = Loader::load(&[doc("order", vec![rule])]);
    assert_eq!(out.world.rule_count(), 0);
    assert!(has_code(&out, "rule.unchecked-domain"));
    assert!(
        !has_code(&out, "load.malformed-world-rule"),
        "domain rules are not bare world rules"
    );
}

#[test]
fn malformed_bare_world_rules_are_load_findings() {
    for body in [None, Some("beat $b requires slot >= 10")] {
        let rule = Decl::Rule(RuleDecl {
            name: ident("checker"),
            domain: None,
            head: Vec::new(),
            body: body.map(|text| rule_body(&[text])).unwrap_or_default(),
            provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
        });
        let out = Loader::load(&[doc("world", vec![rule])]);
        assert!(
            has_code(&out, "load.malformed-world-rule"),
            "invalid bare rule was silently dropped: {body:?}"
        );
        assert_eq!(out.world.rule_count(), 0);
    }
}

#[test]
fn each_malformed_world_rule_clause_is_reported() {
    let rule = Decl::Rule(RuleDecl {
        name: ident("mixed"),
        domain: None,
        head: Vec::new(),
        body: rule_body(&["reachable($x) :- adjacent($x)", "free text"]),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let out = Loader::load(&[doc("world", vec![rule])]);
    assert_eq!(out.world.rule_count(), 1, "valid Datalog must still lower");
    assert_eq!(
        out.findings
            .iter()
            .filter(|finding| finding.code() == "load.malformed-world-rule")
            .count(),
        1
    );
}

#[test]
fn a_rule_without_provenance_is_a_finding() {
    let rule = Decl::Rule(RuleDecl {
        name: ident("adjacency-symmetric"),
        domain: None,
        head: Vec::new(),
        body: dli_ast::rule::RuleBody::default(),
        provenance: None,
    });
    let docs = vec![doc("storybook", vec![rule])];
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
        body: rule_body(&["p :- not q"]),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-0003"])),
    });
    let q_rule = Decl::Rule(RuleDecl {
        name: ident("q-rule"),
        domain: None,
        head: Vec::new(),
        body: rule_body(&["q :- not p"]),
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
fn entity_sorts_and_all_era_chain_members_are_queryable() {
    let entity = |kind, name: &str, time| {
        Decl::Entity(EntityDecl {
            kind,
            name: ident(name),
            label: None,
            time,
            slots: Vec::new(),
            provenance: Some(ann(ProvenanceMarker::Proposed, &[])),
        })
    };
    let compiled = dli_world::compile(&Pack {
        name: "test".to_owned(),
        documents: vec![doc(
            "test",
            vec![
                entity(EntityKind::Person, "ada", None),
                entity(
                    EntityKind::Era,
                    "first",
                    Some(TimeClause::Order(vec![ident("second"), ident("third")])),
                ),
            ],
        )],
    })
    .expect("world compiles");
    let query = |predicate: &str| {
        compiled
            .model()
            .world
            .query(&Query {
                goal: AtomExpr {
                    pred: ident(predicate),
                    args: vec![Term::Var(ident("item"))],
                },
            })
            .expect("declared sort is queryable")
            .rows
            .into_iter()
            .map(|row| row.bindings["item"].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(query("person"), ["ada"]);
    assert_eq!(query("era"), ["first", "second", "third"]);
}

#[test]
fn entity_support_matches_direct_lowering_for_shared_and_untagged_proposals() {
    let entity = |name: &str, tags: &[&str]| {
        let mut provenance = ann(ProvenanceMarker::Proposed, &[]);
        provenance.tags = tags.iter().map(|tag| (*tag).to_owned()).collect();
        Decl::Entity(EntityDecl {
            kind: EntityKind::Person,
            name: ident(name),
            label: None,
            time: None,
            slots: Vec::new(),
            provenance: Some(provenance),
        })
    };
    let document = doc(
        "test",
        vec![
            entity("ada", &["group"]),
            fact(
                "knows",
                &["ada", "bea"],
                false,
                Some(ann(ProvenanceMarker::Proposed, &[])),
            ),
            entity("bea", &["group"]),
            entity("cy", &[]),
        ],
    );
    let loaded = Loader::load(std::slice::from_ref(&document)).world.world;
    let direct = World::lower(&document).expect("direct lowering");
    assert_eq!(loaded.interner.names(), direct.interner.names());
    assert_eq!(loaded.facts, direct.facts);
    assert_eq!(loaded.facts[0].ann, loaded.facts[2].ann);
    assert_ne!(loaded.facts[0].ann, loaded.facts[3].ann);
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

#[test]
fn era_chain_members_keep_source_and_upgrade_prior_mentions() {
    let source = "scope story\nratify d-1\nmentions(wren, noon) ratified d-1\nera dawn < noon < dusk ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "story".into(),
        documents: vec![document],
    })
    .unwrap();
    for member in ["noon", "dusk"] {
        assert!(
            compiled
                .model
                .facts_about(member)
                .iter()
                .any(|atom| atom.atom.pred.text == "era" && atom.span.end > atom.span.start),
            "missing source for {member}"
        );
        let entity = compiled
            .model
            .entities()
            .iter()
            .find(|entry| entry.name == member)
            .unwrap();
        assert_eq!(entity.sort.as_deref(), Some("era"));
    }
}

#[test]
fn explicit_proposal_bundle_never_overflows_the_counter() {
    let source = "scope story\nperson wren \"Wren\" proposed-bundle \"story#4294967294\"\nperson robin \"Robin\" proposed\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let world = dli_world::compile(&Pack {
        name: "story".into(),
        documents: vec![document],
    })
    .unwrap();
    assert!(
        world
            .model
            .findings()
            .iter()
            .any(|finding| finding.message().contains("sequence is exhausted"))
    );
}
