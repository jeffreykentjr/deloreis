#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `CHECK-1` coverage: the six continuity classes, the fact-check verdicts, the
//! unified lowering (zero Loader-vs-eval status divergence), and shadow parity
//! against the pinned oracle over the vendored corpus.
//!
//! Offline and deterministic: the parity test reads `worlds/tpth/vendor` and
//! `worlds/tpth/vendor/.../killed_tokens.yaml` from the repo, never the network and
//! never `uv`. The pinned-oracle run itself ("canon OK: 0 violations") is recorded
//! out of band; this test asserts *our* side reproduces it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Status;

use dli_eval::query::Query;
use dli_eval::world::World;

use dli_world::check::factcheck::{KilledToken, KilledTokens, MatchMode, Verdict, factcheck};
use dli_world::check::{self, CompiledWorld};
use dli_world::draft::{DraftMeta, DraftPayload};
use dli_world::model::LoadFinding;
use dli_world::pack::Pack;
use dli_world::{check_draft, compile};

// --- builders ----------------------------------------------------------------

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn ratified(cite: &str) -> Annotation {
    marker(ProvenanceMarker::Ratified, &[cite])
}

/// A `ratify <decision>` declaration — makes the decision a known one, so a
/// citation to it carries authority (B1 defense-in-depth: a citation alone does
/// not mint a decision's existence).
fn ratify_decl(decision: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(decision),
        body: Vec::new(),
    })
}

fn marker(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn atom(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| Term::Sym(id(a))).collect(),
    }
}

fn fact(pred: &str, args: &[&str], provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance,
    })
}

fn pack(name: &str, decls: Vec<Decl>) -> Pack {
    Pack {
        name: name.to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id(name),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

/// A small hand-built world: the anchor's gender is ratified, the hound's nature
/// ratified (both decisions declared, so a citation carries authority), and a
/// proposed origin sits in the shadow layer.
fn saltcast_world() -> CompiledWorld {
    compile(&pack(
        "cast",
        vec![
            ratify_decl("D-0096"),
            ratify_decl("D-0116"),
            fact("gender", &["anchor", "male"], Some(ratified("D-0096"))),
            fact(
                "nature",
                &["hound", "moor-spirit"],
                Some(ratified("D-0116")),
            ),
            fact(
                "origin",
                &["anchor", "swartby"],
                Some(marker(ProvenanceMarker::Proposed, &[])),
            ),
        ],
    ))
    .expect("compiles")
}

// --- fact-check verdicts -----------------------------------------------------

#[test]
fn factcheck_supported_carries_citations() {
    let world = saltcast_world();
    let reports = factcheck(&world, &[atom("gender", &["anchor", "male"])]);
    assert_eq!(reports[0].verdict, Verdict::Supported);
    assert_eq!(reports[0].citations, vec!["D-0096".to_owned()]);
}

#[test]
fn factcheck_contradicts_a_single_valued_slot() {
    let world = saltcast_world();
    let reports = factcheck(&world, &[atom("gender", &["anchor", "female"])]);
    assert_eq!(reports[0].verdict, Verdict::Contradicted);
    assert_eq!(reports[0].citations, vec!["D-0096".to_owned()]);
}

#[test]
fn factcheck_unsupported_when_canon_bears_no_certain_fact() {
    let world = saltcast_world();
    // `origin` is proposed-only (shadow), so there is no *certain* fact.
    let reports = factcheck(&world, &[atom("origin", &["anchor", "gwern"])]);
    assert_eq!(reports[0].verdict, Verdict::Unsupported);
}

// --- the continuity classes --------------------------------------------------

#[test]
fn proposed_as_fact_flags_a_shadow_slot_stated_as_settled() {
    let world = saltcast_world();
    let findings =
        check::continuity::check_claims(&world, &[atom("origin", &["anchor", "swartby"])]);
    assert!(
        findings
            .iter()
            .any(|f| f.code == "continuity.proposed-as-fact"),
        "{findings:?}"
    );
}

#[test]
fn intra_episode_contradiction_flags_one_slot_set_two_ways() {
    let world = saltcast_world();
    let findings = check::continuity::check_claims(
        &world,
        &[
            atom("gender", &["anchor", "male"]),
            atom("gender", &["anchor", "female"]),
        ],
    );
    assert!(
        findings
            .iter()
            .any(|f| f.code == "continuity.intra-episode-contradiction"),
        "{findings:?}"
    );
}

#[test]
fn unknown_entity_flags_a_claim_on_an_entity_canon_never_saw() {
    let world = saltcast_world();
    let findings = check::continuity::check_claims(&world, &[atom("gender", &["ghost", "female"])]);
    assert!(
        findings
            .iter()
            .any(|f| f.code == "continuity.unknown-entity"),
        "{findings:?}"
    );
}

#[test]
fn prior_contradiction_reverses_a_prior_ledger_value() {
    let prior: BTreeMap<(String, String), String> = [(
        ("gender".to_owned(), "anchor".to_owned()),
        "male".to_owned(),
    )]
    .into();
    let findings =
        check::continuity::prior_contradictions(&[atom("gender", &["anchor", "female"])], &prior);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "continuity.prior-contradiction");
}

// --- killed tokens (the C-store) ---------------------------------------------

#[test]
fn killed_tokens_match_by_word_and_phrase() {
    let killed = KilledTokens::new(vec![
        KilledToken {
            token: "Irondale".to_owned(),
            mode: MatchMode::Word,
            decision: "D-0101".to_owned(),
            scope: "region name".to_owned(),
        },
        KilledToken {
            token: "doom-wight".to_owned(),
            mode: MatchMode::Phrase,
            decision: "D-0097".to_owned(),
            scope: "hound name".to_owned(),
        },
    ]);
    assert!(killed.hit("the road to Irondale").is_some());
    assert!(killed.hit("Doom Wight rises").is_some()); // hyphen/space fold
    assert!(killed.hit("the ironclad gate").is_none()); // word boundary, not substring
}

#[test]
fn factcheck_contradicts_a_claim_that_revives_a_killed_token() {
    let killed = KilledTokens::new(vec![KilledToken {
        token: "irondale".to_owned(),
        mode: MatchMode::Word,
        decision: "D-0101".to_owned(),
        scope: "region name".to_owned(),
    }]);
    let world = saltcast_world().with_killed_tokens(killed);
    let reports = factcheck(&world, &[atom("home", &["anchor", "irondale"])]);
    assert_eq!(reports[0].verdict, Verdict::Contradicted);
    assert_eq!(reports[0].citations, vec!["D-0101".to_owned()]);
}

#[test]
fn check_draft_flags_an_introduced_killed_name() {
    let killed = KilledTokens::new(vec![KilledToken {
        token: "irondale".to_owned(),
        mode: MatchMode::Word,
        decision: "D-0101".to_owned(),
        scope: "region name".to_owned(),
    }]);
    let world = saltcast_world().with_killed_tokens(killed);
    let draft = DraftPayload {
        meta: DraftMeta::default(),
        names: vec!["Irondale".to_owned()],
        ..DraftPayload::default()
    };
    let findings = check_draft(&world, &draft);
    assert!(
        findings.iter().any(|f| f.message.contains("killed token")),
        "{findings:?}"
    );
}

// --- the unified lowering: zero Loader-vs-eval divergence --------------------

/// The variables ruled true, keyed by name (interner-order independent).
fn valuation_by_name(world: &World) -> BTreeMap<String, bool> {
    world
        .valuation
        .0
        .iter()
        .filter_map(|(atom, &value)| {
            world
                .interner
                .resolve(*atom)
                .map(|name| (name.to_owned(), value))
        })
        .collect()
}

/// A ground fact's standing on the *evaluated* side: query the saturation under
/// the ambient valuation; an absent row is [`Status::Dead`] (filtered out).
fn eval_status(ev: &World, pred: &str, args: &[&str]) -> Status {
    let answer = ev
        .query(&Query {
            goal: atom(pred, args),
        })
        .expect("known pred");
    answer.rows.first().map_or(Status::Dead, |row| row.status)
}

/// A ground fact's standing on the *loader* side: the matching loaded atom's
/// status under the compiled world's ledger valuation.
fn loaded_status(world: &CompiledWorld, pred: &str, args: &[&str]) -> Status {
    let rendered: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let loaded = world
        .model()
        .atoms()
        .iter()
        .find(|a| {
            a.atom.pred.text == pred
                && a.atom
                    .args
                    .iter()
                    .filter_map(term_text)
                    .eq(rendered.iter().cloned())
        })
        .expect("loaded atom present");
    loaded
        .ann
        .status(&world.model().world.valuation, &world.model().world.theory)
}

fn term_text(term: &Term) -> Option<String> {
    match term {
        Term::Sym(id) => Some(id.text.clone()),
        _ => None,
    }
}

#[test]
fn compile_unifies_the_two_lowerings_to_zero_status_divergence() {
    // A ratified (certain), a proposed (possible), and a no-provenance (dead)
    // fact — the three standings the R1 table names.
    let decls = vec![
        ratify_decl("D-0096"),
        fact("gender", &["anchor", "male"], Some(ratified("D-0096"))),
        fact(
            "omen",
            &["anchor", "storm"],
            Some(marker(ProvenanceMarker::Proposed, &[])),
        ),
        fact("secret", &["anchor", "hidden"], None),
    ];
    let doc = Document {
        scope: Some(ScopeHeader {
            name: id("cast"),
            span: Span::synthetic(),
        }),
        decls: decls.clone(),
    };
    let world = compile(&Pack {
        name: "cast".to_owned(),
        documents: vec![doc.clone()],
    })
    .expect("compiles");
    let ev = World::lower(&doc)
        .expect("lowers")
        .fixpoint()
        .expect("saturates");

    // 1. The ledger valuations converge (the reconciliation): both rule exactly
    //    the cited decision variables true, and nothing else.
    assert_eq!(
        valuation_by_name(&world.model().world),
        valuation_by_name(&ev),
        "compile and dli-eval must build the same ledger valuation",
    );
    assert_eq!(valuation_by_name(&ev), [("D-0096".to_owned(), true)].into(),);

    // 2. Every fact's standing agrees on both sides under that valuation.
    for (pred, value, expected) in [
        ("gender", "male", Status::Certain),
        ("omen", "storm", Status::Possible),
        ("secret", "hidden", Status::Dead),
    ] {
        let loaded = loaded_status(&world, pred, &["anchor", value]);
        let evaluated = eval_status(&ev, pred, &["anchor", value]);
        assert_eq!(loaded, evaluated, "{pred} divergence");
        assert_eq!(loaded, expected, "{pred} standing");
    }
}

#[test]
fn multi_citation_conjoins_and_supersession_degrades_on_both_sides() {
    // Ruling A: a ledger claim citing two decisions rests on BOTH — supersede
    // either and it degrades. A proposed claim with an explicit `assumes` is
    // gated by that hypothesis alone (no fresh proposal variable). The compiled
    // (loader) side and the evaluated side must agree, base and under supersession.
    let assumes = Annotation {
        marker: ProvenanceMarker::Proposed,
        citations: Vec::new(),
        condition: Some(dli_ast::annot::CondExpr::Hyp(id("guild-rang-it"))),
        tags: Vec::new(),
        span: Span::synthetic(),
    };
    let doc = Document {
        scope: Some(ScopeHeader {
            name: id("cast"),
            span: Span::synthetic(),
        }),
        decls: vec![
            ratify_decl("D-1"),
            ratify_decl("D-2"),
            fact(
                "kept-by",
                &["tide-bell", "guild"],
                Some(marker(ProvenanceMarker::Ratified, &["D-1", "D-2"])),
            ),
            fact("rings", &["tide-bell", "dawn"], Some(assumes)),
        ],
    };
    let world = compile(&Pack {
        name: "cast".to_owned(),
        documents: vec![doc.clone()],
    })
    .expect("compiles");
    let ev = World::lower(&doc)
        .expect("lowers")
        .fixpoint()
        .expect("saturates");

    // The ledger valuations converge on exactly the two cited decisions.
    assert_eq!(
        valuation_by_name(&world.model().world),
        valuation_by_name(&ev),
    );
    assert_eq!(
        valuation_by_name(&ev),
        [("D-1".to_owned(), true), ("D-2".to_owned(), true)].into(),
    );

    // Base standings agree: the two-citation fact is certain, the assumes-gated
    // proposal is a shadow.
    for (pred, args, expected) in [
        ("kept-by", ["tide-bell", "guild"], Status::Certain),
        ("rings", ["tide-bell", "dawn"], Status::Possible),
    ] {
        let loaded = loaded_status(&world, pred, &args);
        assert_eq!(
            loaded,
            eval_status(&ev, pred, &args),
            "{pred} base divergence"
        );
        assert_eq!(loaded, expected, "{pred} base standing");
    }

    // Supersede D-1 (rule it false). AND-lowering degrades the claim to dead on
    // BOTH sides; OR-lowering would leave it certain on D-2 — the defect A guards.
    let mut loader_nu = world.model().world.valuation.clone();
    loader_nu.set(world.model().var_atom("D-1").expect("D-1 interned"), false);
    let loaded_atom = world
        .model()
        .atoms()
        .iter()
        .find(|a| a.atom.pred.text == "kept-by")
        .expect("kept-by atom");
    let loaded_superseded = loaded_atom
        .ann
        .status(&loader_nu, &world.model().world.theory);

    let ev_d1 = dli_core::atom::Atom(
        u32::try_from(
            ev.interner
                .names()
                .iter()
                .position(|n| n == "D-1")
                .expect("D-1 in eval interner"),
        )
        .expect("fits"),
    );
    let mut ev_nu = ev.valuation.clone();
    ev_nu.set(ev_d1, false);
    let ev_superseded = ev
        .eval_under(
            &ev_nu,
            &Query {
                goal: atom("kept-by", &["tide-bell", "guild"]),
            },
        )
        .expect("known pred")
        .rows
        .first()
        .map_or(Status::Dead, |row| row.status);

    assert_eq!(loaded_superseded, ev_superseded, "supersession divergence");
    assert_eq!(
        loaded_superseded,
        Status::Dead,
        "superseding one of two cited decisions degrades the AND-supported claim",
    );
}

#[test]
fn options_and_hypotheses_lower_without_status_divergence() {
    // A question whose option implies a fact, plus a hypothesis (finding #1).
    // The loader must lower the option-gated fact and intern the option and
    // hypothesis variables — as `World::lower` lowers options — so a query sees
    // the implied predicate and a fork can `suppose` the option.
    let doc = Document {
        scope: Some(ScopeHeader {
            name: id("cast"),
            span: Span::synthetic(),
        }),
        decls: vec![
            ratify_decl("D-0096"),
            fact("gender", &["anchor", "male"], Some(ratified("D-0096"))),
            Decl::Question(dli_ast::decl::QuestionDecl {
                name: id("who-keeps-the-ledger"),
                prompt: None,
                options: vec![
                    dli_ast::decl::OptionClause {
                        name: id("old-fen"),
                        implies: Some(atom("keeps", &["ledger", "old-fen"])),
                    },
                    dli_ast::decl::OptionClause {
                        name: id("mara"),
                        implies: None,
                    },
                ],
                provenance: None,
            }),
            Decl::Hypothesis(dli_ast::decl::HypothesisDecl {
                name: id("the-guild-endures"),
                statement: None,
            }),
        ],
    };
    let world = compile(&Pack {
        name: "cast".to_owned(),
        documents: vec![doc.clone()],
    })
    .expect("compiles");
    let ev = World::lower(&doc)
        .expect("lowers")
        .fixpoint()
        .expect("saturates");

    // Valuations converge (only the cited decision is ruled; options and
    // hypotheses stay open), and the option-implied fact is a shadow on both.
    assert_eq!(
        valuation_by_name(&world.model().world),
        valuation_by_name(&ev),
    );
    let loaded = loaded_status(&world, "keeps", &["ledger", "old-fen"]);
    assert_eq!(
        loaded,
        eval_status(&ev, "keeps", &["ledger", "old-fen"]),
        "option-fact divergence",
    );
    assert_eq!(
        loaded,
        Status::Possible,
        "an option-gated fact is a shadow until the option is ruled",
    );

    // The option variable is interned on both sides; the hypothesis variable is
    // interned in the compiled world so a fork can `suppose` it.
    let has = |w: &World, name: &str| w.interner.names().iter().any(|n| n == name);
    assert!(
        has(&world.model().world, "old-fen"),
        "loader interns the option"
    );
    assert!(has(&ev, "old-fen"), "eval interns the option");
    assert!(
        has(&world.model().world, "the-guild-endures"),
        "the compiled world interns the hypothesis variable",
    );
}

// --- shadow parity over the vendored corpus ----------------------------------

#[derive(Debug, Deserialize)]
struct FactsDoc {
    #[serde(default)]
    entities: Vec<EntityRow>,
    #[serde(default)]
    relations: Vec<RelRow>,
    #[serde(default)]
    facts: Vec<FactRow>,
}

#[derive(Debug, Deserialize)]
struct EntityRow {
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RelRow {
    subject: String,
    kind: String,
    object: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FactRow {
    entity: String,
    key: String,
    value: serde_yaml_ng::Value,
    #[serde(default)]
    status: String,
    #[serde(default)]
    decisions: Vec<String>,
}

fn vendor_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../worlds/tpth/vendor/fantasy-40f6679")
}

fn render_value(value: &serde_yaml_ng::Value) -> String {
    match value {
        serde_yaml_ng::Value::String(s) => s.clone(),
        serde_yaml_ng::Value::Bool(b) => b.to_string(),
        serde_yaml_ng::Value::Number(n) => n.to_string(),
        other => serde_yaml_ng::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    }
}

fn provenance(status: &str, decisions: &[String]) -> Option<Annotation> {
    let marker = match status {
        "ratified" => ProvenanceMarker::Ratified,
        "provisional" => ProvenanceMarker::Provisional,
        "rejected" => return None, // guardrails are not asserted lore
        _ => ProvenanceMarker::Proposed,
    };
    let cites: Vec<&str> = if marker == ProvenanceMarker::Proposed {
        Vec::new()
    } else {
        decisions.iter().map(String::as_str).collect()
    };
    Some(self::marker(marker, &cites))
}

fn map_kind(kind: &str) -> EntityKind {
    match kind {
        "place" => EntityKind::Place,
        "era" => EntityKind::Era,
        "event" => EntityKind::Event,
        "person" | "character" => EntityKind::Person,
        _ => EntityKind::Thing,
    }
}

/// Bridge the vendored `facts.yaml` into deloreis documents: entities become
/// `entity` declarations, facts and relations become flat-triple `fact`s, and
/// every decision the corpus cites is declared with a `ratify` decl — the ledger
/// mirror of `decisions.md`, so cited authority is real (B1) rather than minted by
/// the citation alone.
fn corpus_pack(facts: &FactsDoc) -> Pack {
    let mut decls: Vec<Decl> = Vec::new();
    let mut decisions = std::collections::BTreeSet::new();
    for row in &facts.entities {
        decisions.extend(row.decisions.iter().cloned());
    }
    for row in &facts.facts {
        decisions.extend(row.decisions.iter().cloned());
    }
    for row in &facts.relations {
        decisions.extend(row.decisions.iter().cloned());
    }
    for decision in &decisions {
        decls.push(ratify_decl(decision));
    }
    let mut seen = std::collections::BTreeSet::new();
    for entity in &facts.entities {
        if seen.insert(entity.id.clone()) {
            decls.push(Decl::Entity(EntityDecl {
                kind: map_kind(&entity.kind),
                name: id(&entity.id),
                label: None,
                time: None,
                slots: Vec::new(),
                provenance: provenance(&entity.status, &entity.decisions),
            }));
        }
    }
    for row in &facts.facts {
        decls.push(fact(
            &row.key,
            &[&row.entity, &render_value(&row.value)],
            provenance(&row.status, &row.decisions),
        ));
    }
    for row in &facts.relations {
        decls.push(fact(
            &row.kind,
            &[&row.subject, &row.object],
            provenance(&row.status, &row.decisions),
        ));
    }
    Pack {
        name: "canon".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("canon"),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

fn load_corpus() -> FactsDoc {
    let path = vendor_root().join("canon/data/facts.yaml");
    let text = std::fs::read_to_string(&path).expect("read facts.yaml");
    serde_yaml_ng::from_str(&text).expect("parse facts.yaml")
}

#[test]
fn parity_clean_corpus_reports_zero_violations() {
    // The pinned oracle reports "canon OK: 0 violations" over this corpus. Our
    // compile()+run() over the same corpus must agree: no fail-closed load
    // finding, no continuity violation.
    let world = compile(&corpus_pack(&load_corpus())).expect("compiles");
    assert!(
        world.model().findings().is_empty(),
        "clean corpus load findings: {:?}",
        world.model().findings()
    );
    assert!(
        check::run(&world).is_empty(),
        "clean corpus continuity findings: {:?}",
        check::run(&world)
    );
}

#[test]
fn parity_malformed_provenance_fails_closed() {
    // Operator `break-status-marker`/`flip-proposed-to-ratified` (expected_catch:
    // provenance + assertability). Strip the citation off a ratified fact: the
    // ledger marker can no longer resolve, so it fails closed and the claim it
    // bore is no longer certain.
    let mut pack = corpus_pack(&load_corpus());
    for decl in &mut pack.documents[0].decls {
        if let Decl::Fact(f) = decl
            && f.atom.pred.text == "gender"
            && matches!(f.atom.args.first(), Some(Term::Sym(s)) if s.text == "anchor")
        {
            f.provenance = Some(marker(ProvenanceMarker::Ratified, &[])); // ratified, no citation
        }
    }
    let world = compile(&pack).expect("compiles");
    // provenance: the loader fails it closed and run() surfaces it.
    assert!(
        world
            .model()
            .findings()
            .iter()
            .any(|f| matches!(f, LoadFinding::MalformedProvenance { .. })),
        "expected malformed-provenance finding: {:?}",
        world.model().findings()
    );
    assert!(check::run(&world).iter().any(|f| f.family() == "load"));
    // assertability: the fact is no longer certain, so the claim is not supported.
    let reports = factcheck(&world, &[atom("gender", &["anchor", "male"])]);
    assert_ne!(reports[0].verdict, Verdict::Supported);
}

#[test]
fn parity_duplicate_declaration_is_caught() {
    // Operator `duplicate-id` (expected_catch: uniqueness). A second declaration
    // of an existing entity id collides within the pack.
    let mut pack = corpus_pack(&load_corpus());
    pack.documents[0].decls.push(Decl::Entity(EntityDecl {
        kind: EntityKind::Place,
        name: id("gwern"),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(ratified("D-0111")),
    }));
    let world = compile(&pack).expect("compiles");
    assert!(
        check::run(&world)
            .iter()
            .any(|f| f.code == "load.duplicate-declaration"),
        "expected duplicate-declaration finding: {:?}",
        check::run(&world)
    );
}

#[test]
fn parity_draft_contradiction_is_caught() {
    // Operator `contradict-anchor-gender` (expected_catch: contradiction). A draft
    // asserting the anchor is a woman clashes with certain canon (male, D-0096).
    let world = compile(&corpus_pack(&load_corpus())).expect("compiles");
    let draft = DraftPayload {
        meta: DraftMeta::default(),
        claims: vec![atom("gender", &["anchor", "female"])],
        ..DraftPayload::default()
    };
    let findings = check_draft(&world, &draft);
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("canon asserts male")),
        "expected an anchor-gender contradiction: {findings:?}"
    );
}

#[test]
fn killed_tokens_load_from_the_vendored_registry() {
    // The killed-token registry is real vendored data; a claim reviving `Irondale`
    // (killed D-0101) is rejected lore.
    #[derive(Deserialize)]
    struct KilledDoc {
        killed: Vec<KilledRow>,
    }
    #[derive(Deserialize)]
    struct KilledRow {
        token: String,
        #[serde(rename = "match", default)]
        mode: String,
        decision: String,
        #[serde(default)]
        scope: String,
    }
    let path = vendor_root().join("src/fantasy/propagate/data/killed_tokens.yaml");
    let doc: KilledDoc =
        serde_yaml_ng::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    let tokens = doc
        .killed
        .into_iter()
        .map(|row| KilledToken {
            token: row.token,
            mode: match row.mode.as_str() {
                "phrase" => MatchMode::Phrase,
                "substring" | "regex" => MatchMode::Substring,
                _ => MatchMode::Word,
            },
            decision: row.decision,
            scope: row.scope,
        })
        .collect();
    let killed = KilledTokens::new(tokens);
    assert!(killed.hit("Irondale").is_some());
    let world = saltcast_world().with_killed_tokens(killed);
    let reports = factcheck(&world, &[atom("region", &["anchor", "irondale"])]);
    assert_eq!(reports[0].verdict, Verdict::Contradicted);
}

// --- R2 red-team regressions -------------------------------------------------

#[test]
fn intra_episode_does_not_fire_on_a_multivalued_relation() {
    // R2/B: a place legitimately adjacent to two others is a multi-valued
    // relation. A draft restating both adjacencies must NOT read as an
    // intra-episode contradiction — single-valuedness is read from canon.
    let world = compile(&pack(
        "world",
        vec![
            fact(
                "adjacent",
                &["saltmarket", "the-shallows"],
                Some(ratified("D-1")),
            ),
            fact(
                "adjacent",
                &["saltmarket", "the-weir"],
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    let findings = check::continuity::check_claims(
        &world,
        &[
            atom("adjacent", &["saltmarket", "the-shallows"]),
            atom("adjacent", &["saltmarket", "the-weir"]),
        ],
    );
    assert!(
        findings
            .iter()
            .all(|f| f.code != "continuity.intra-episode-contradiction"),
        "a multi-valued relation must not intra-contradict: {findings:?}"
    );
}

#[test]
fn intra_episode_still_fires_on_a_single_valued_slot() {
    // The true-FD case must still fire: canon pins gender to one certain value, so
    // a draft asserting two genders for the anchor is a real contradiction.
    let world = saltcast_world(); // gender(anchor, male) is certain
    let findings = check::continuity::check_claims(
        &world,
        &[
            atom("gender", &["anchor", "male"]),
            atom("gender", &["anchor", "female"]),
        ],
    );
    assert!(
        findings
            .iter()
            .any(|f| f.code == "continuity.intra-episode-contradiction"),
        "a single-valued slot set two ways must intra-contradict: {findings:?}"
    );
}

#[test]
fn killed_token_survives_separator_stripping() {
    // R2/D: a killed hyphenated token is caught however a draft spaces it —
    // including the run-together evasion — across every match mode.
    for mode in [MatchMode::Word, MatchMode::Phrase, MatchMode::Substring] {
        let killed = KilledTokens::new(vec![KilledToken {
            token: "tide-bell".to_owned(),
            mode,
            decision: "D-9".to_owned(),
            scope: "killed name".to_owned(),
        }]);
        for text in [
            "the tide-bell rang",
            "the tide bell rang",
            "the tidebell rang",
        ] {
            assert!(
                killed.hit(text).is_some(),
                "mode {mode:?} missed the killed token in {text:?}"
            );
        }
    }
}

#[test]
fn killed_token_word_boundary_vs_substring() {
    // Documented boundary: Word/Phrase match the whole run only — a longer word
    // that merely contains it (`tidebells`) is not the killed token — while
    // Substring is liberal and matches anywhere.
    let make = |mode| {
        KilledTokens::new(vec![KilledToken {
            token: "tide-bell".to_owned(),
            mode,
            decision: "D-9".to_owned(),
            scope: "killed name".to_owned(),
        }])
    };
    assert!(make(MatchMode::Word).hit("the tidebells rang").is_none());
    assert!(make(MatchMode::Phrase).hit("the tidebells rang").is_none());
    assert!(
        make(MatchMode::Substring)
            .hit("the tidebells rang")
            .is_some()
    );
}

// --- H8: era-spine cycle detection ------------------------------------------

/// An era declaration `era name < c1 < c2 ...` as the parser emits it: the head
/// is the entity name, the chain is head-*exclusive*.
fn era(name: &str, chain: &[&str], cite: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind: EntityKind::Era,
        name: id(name),
        label: None,
        time: Some(TimeClause::Order(chain.iter().map(|c| id(c)).collect())),
        slots: Vec::new(),
        provenance: Some(ratified(cite)),
    })
}

#[test]
fn a_contradictory_era_spine_is_a_first_class_cycle_finding() {
    // H8: `era a < b < a` closes a precedence loop. The parser hands the loader
    // the chain head-exclusive (name `a`, chain `[b, a]`); prepending the head
    // makes both the a→b and b→a edges exist, so the SAME cycle detection the
    // `before`-graph uses reports it — never the old silent clean exit.
    let world = compile(&pack("h", vec![era("a", &["b", "a"], "d-0001")])).expect("compiles");
    let cycles: Vec<_> = check::run(&world)
        .into_iter()
        .filter(|f| f.code == "continuity.chronology-cycle")
        .collect();
    assert_eq!(cycles.len(), 1, "exactly one chronology-cycle finding");
    assert!(
        cycles[0].message.contains("a -> b") && cycles[0].message.contains("b -> a"),
        "blame names the closing edges: {}",
        cycles[0].message
    );
    // The order is a ruling: the finding is blamed on the declaration behind it.
    assert_eq!(cycles[0].citations, vec!["d-0001".to_owned()]);
}

#[test]
fn a_two_chain_era_contradiction_is_caught_and_blamed_on_both_rulings() {
    // The loop spans two declarations: `era a < b` (d-0001) + `era b < a`
    // (d-0002). Neither chain is a cycle alone; together they contradict.
    let world = compile(&pack(
        "h",
        vec![era("a", &["b"], "d-0001"), era("b", &["a"], "d-0002")],
    ))
    .expect("compiles");
    let cycles: Vec<_> = check::run(&world)
        .into_iter()
        .filter(|f| f.code == "continuity.chronology-cycle")
        .collect();
    assert_eq!(cycles.len(), 1);
    assert!(cycles[0].citations.contains(&"d-0001".to_owned()));
    assert!(cycles[0].citations.contains(&"d-0002".to_owned()));
}

#[test]
fn a_clean_era_spine_orders_its_head_and_raises_no_cycle() {
    // Regression on the head-drop: `era first < second < third` must ORDER the
    // head `first` (three consecutive edges, none dropped) and check clean.
    let world = compile(&pack(
        "h",
        vec![era("first", &["second", "third"], "d-0001")],
    ))
    .expect("compiles");
    assert!(
        check::run(&world)
            .iter()
            .all(|f| f.code != "continuity.chronology-cycle"),
        "a well-formed spine is not a cycle"
    );
    assert_eq!(world.model.world.chronology.edges.len(), 2);
    assert!(world.model.world.chronology.find_cycle(&[]).is_none());
    // The parallel ruling table stays in lock-step with the edges.
    assert_eq!(
        world.model.chronology_anns.len(),
        world.model.world.chronology.edges.len()
    );
}

// --- B1: forged-citation authority-mint --------------------------------------

#[test]
fn forged_citation_fails_closed_on_both_lowerings() {
    // B1: a claim cites a decision the corpus never declares (no `ratify`). A
    // citation alone must not mint authority — both lowerings leave the claim
    // non-certain AND raise `citation.unknown-decision`, and `check::run` surfaces
    // it. This is the test that proves the compile-side gate matches the eval side.
    let built = pack(
        "cast",
        vec![
            ratify_decl("D-0001"), // a real decision: the known set is non-empty
            // D-9999 is cited but never declared — a forged authority claim.
            fact("gender", &["anchor", "male"], Some(ratified("D-9999"))),
        ],
    );
    let doc = built.documents[0].clone();
    let world = compile(&built).expect("compiles");
    let ev = World::lower(&doc)
        .expect("lowers")
        .fixpoint()
        .expect("saturates");

    // Non-certain on both sides.
    assert_ne!(
        loaded_status(&world, "gender", &["anchor", "male"]),
        Status::Certain,
        "compile side minted authority from a forged citation",
    );
    assert_ne!(
        eval_status(&ev, "gender", &["anchor", "male"]),
        Status::Certain,
        "eval side minted authority from a forged citation",
    );

    // Both raise the forged-authority finding, and check::run surfaces it.
    let forged = |findings: &[dli_eval::world::LowerFinding]| {
        findings
            .iter()
            .any(|f| f.code == "citation.unknown-decision")
    };
    assert!(
        forged(&world.model().world.findings),
        "compile findings: {:?}",
        world.model().world.findings
    );
    assert!(forged(&ev.findings), "eval findings: {:?}", ev.findings);
    assert!(
        check::run(&world)
            .iter()
            .any(|f| f.code == "citation.unknown-decision"),
        "check::run must surface the forged-citation finding",
    );
}
