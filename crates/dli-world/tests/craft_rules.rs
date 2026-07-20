//! `CRAFT-2` + `CRAFT-3` oracle suite: the season-ledger join rules and the
//! principle knowledge base.
//!
//! The plan-oracle cases the brief fixes, each on a small season fixture: the
//! earned-outcomes join fires (a major win needs a prior same-thread failure, and
//! it joins *across episodes* — the move the merge makes possible);
//! competence-consistency reads the sliders as production fluents; a judge-bound
//! principle reports grade `attested`. The "a tension without a resolution is a
//! compile error" oracle is a `compile_fail` doctest on
//! [`dli_world::craft::kb::MissionFilter`] (it cannot be a runtime test — the value
//! does not exist), mirrored here by the structural invariant that every corpus
//! tension carries its resolution.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use dli_world::attest::{
    Attestation, AttestationStore, ContentHash, ModelRef, Policy, PromptRef, Question, Sampling,
};
use dli_world::check::{Severity, WorldFinding};
use dli_world::craft::kb::{self, Binding, Checkable, KernelRule, MissionFilter};
use dli_world::craft::outline::{AudioLoad, PromiseKind, TryFailOutcome};
use dli_world::craft::season::{
    ArcShape, BeatKind, CraftLedger, DEFAULT_THRESHOLDS, EpisodeMeta, LedgerBeat, LedgerPromise,
    LedgerScene, Magnitude, Reveal, Thresholds, Turn, check_season_rules,
};

// -- fixture builders ---------------------------------------------------------

fn episode(id: &str, slot: u32) -> EpisodeMeta {
    EpisodeMeta {
        id: id.to_owned(),
        slot,
        arc_shape: None,
        ten_position: None,
        audio_load: None,
        introduced_names: Vec::new(),
    }
}

fn beat(id: &str, ep: &str, thread: &str, kind: BeatKind, mag: Magnitude) -> LedgerBeat {
    LedgerBeat {
        id: id.to_owned(),
        episode: ep.to_owned(),
        scene: format!("{ep}-s1"),
        kind,
        thread: thread.to_owned(),
        magnitude: mag,
        outcome: None,
        by: String::new(),
        assisted: false,
    }
}

fn codes(findings: &[WorldFinding]) -> Vec<String> {
    findings.iter().map(|f| f.code.clone()).collect()
}

fn has(findings: &[WorldFinding], code: &str) -> bool {
    findings.iter().any(|f| f.code == code)
}

/// A three-episode ledger with the given beats, slotted 1..=3.
fn season_of(beats: Vec<LedgerBeat>) -> CraftLedger {
    CraftLedger {
        episodes: vec![episode("e1", 1), episode("e2", 2), episode("e3", 3)],
        beats,
        ..CraftLedger::default()
    }
}

// -- CRAFT-2: accomplishment-earned (the worked join) -------------------------

#[test]
fn empty_ledger_is_clean() {
    // Episodes only — no beats/promises/sliders/scenes — trips no rule.
    let ledger = CraftLedger {
        episodes: vec![episode("e1", 1), episode("e2", 2)],
        ..CraftLedger::default()
    };
    assert!(check_season_rules(&ledger, &DEFAULT_THRESHOLDS).is_empty());
}

#[test]
fn major_win_without_prior_failure_is_an_error() {
    let ledger = season_of(vec![beat(
        "b1",
        "e1",
        "ward",
        BeatKind::Accomplishment,
        Magnitude::Major,
    )]);
    let findings = check_season_rules(&ledger, &DEFAULT_THRESHOLDS);
    assert!(has(&findings, "craft.accomplishment-earned"));
    let f = findings
        .iter()
        .find(|f| f.code == "craft.accomplishment-earned")
        .unwrap();
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.grade.to_string(), "checked");
    assert_eq!(f.citations, vec!["D-0128".to_owned()]);
}

#[test]
fn major_win_earned_by_a_prior_failure_two_episodes_back_is_clean() {
    // The failure lives two episodes before the win — the join the one-outline
    // checker could not make.
    let ledger = season_of(vec![
        beat("f1", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
        beat(
            "b1",
            "e3",
            "ward",
            BeatKind::Accomplishment,
            Magnitude::Major,
        ),
    ]);
    let findings = check_season_rules(&ledger, &DEFAULT_THRESHOLDS);
    assert!(
        !has(&findings, "craft.accomplishment-earned"),
        "a prior same-thread failure earns the win: {:?}",
        codes(&findings)
    );
}

#[test]
fn prior_failure_on_a_different_thread_does_not_earn_the_win() {
    let ledger = season_of(vec![
        beat("f1", "e1", "other", BeatKind::Fail, Magnitude::Minor),
        beat(
            "b1",
            "e2",
            "ward",
            BeatKind::Accomplishment,
            Magnitude::Major,
        ),
    ]);
    assert!(has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.accomplishment-earned"
    ));
}

#[test]
fn climax_wants_two_prior_failures() {
    // One prior failure: base rule satisfied, climax refinement warns.
    let one = season_of(vec![
        beat("f1", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
        beat(
            "c1",
            "e2",
            "ward",
            BeatKind::Accomplishment,
            Magnitude::Climax,
        ),
    ]);
    let findings = check_season_rules(&one, &DEFAULT_THRESHOLDS);
    assert!(!has(&findings, "craft.accomplishment-earned"));
    assert!(has(&findings, "craft.accomplishment-earned-climax"));
    assert_eq!(
        findings
            .iter()
            .find(|f| f.code == "craft.accomplishment-earned-climax")
            .unwrap()
            .severity,
        Severity::Warn
    );

    // Two prior failures: fully earned.
    let two = season_of(vec![
        beat("f1", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
        beat("f2", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
        beat(
            "c1",
            "e2",
            "ward",
            BeatKind::Accomplishment,
            Magnitude::Climax,
        ),
    ]);
    let findings = check_season_rules(&two, &DEFAULT_THRESHOLDS);
    assert!(!has(&findings, "craft.accomplishment-earned"));
    assert!(!has(&findings, "craft.accomplishment-earned-climax"));
}

#[test]
fn a_climaxed_thread_may_not_reopen() {
    let ledger = season_of(vec![
        beat("f1", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
        beat("f2", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
        beat(
            "c1",
            "e2",
            "ward",
            BeatKind::Accomplishment,
            Magnitude::Climax,
        ),
        // Re-opening the thread after its terminal win.
        beat("r1", "e3", "ward", BeatKind::Try, Magnitude::Minor),
    ]);
    let findings = check_season_rules(&ledger, &DEFAULT_THRESHOLDS);
    assert!(has(&findings, "craft.yes-and-terminal"));
    assert_eq!(
        findings
            .iter()
            .find(|f| f.code == "craft.yes-and-terminal")
            .unwrap()
            .severity,
        Severity::Error
    );
}

#[test]
fn a_prior_yes_but_outcome_earns_the_win_without_a_fail_beat() {
    // The cost is carried as a typed outcome on a non-fail beat — spec §3's
    // {yes-but, no-but, no-and}, not just the coarse `fail` kind.
    let mut cost = beat("t1", "e1", "ward", BeatKind::Try, Magnitude::Minor);
    cost.outcome = Some(TryFailOutcome::YesBut);
    let ledger = season_of(vec![
        cost,
        beat(
            "b1",
            "e2",
            "ward",
            BeatKind::Accomplishment,
            Magnitude::Major,
        ),
    ]);
    assert!(!has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.accomplishment-earned"
    ));
}

#[test]
fn a_yes_and_outcome_makes_a_thread_terminal() {
    let mut win = beat(
        "w1",
        "e1",
        "ward",
        BeatKind::Accomplishment,
        Magnitude::Minor,
    );
    win.outcome = Some(TryFailOutcome::YesAnd);
    let ledger = season_of(vec![
        win,
        beat("r1", "e2", "ward", BeatKind::Try, Magnitude::Minor),
    ]);
    assert!(has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.yes-and-terminal"
    ));
}

// -- CRAFT-2: competence-consistency (sliders as production fluents) ----------

fn win_by(character: &str, ep: &str, assisted: bool) -> LedgerBeat {
    LedgerBeat {
        by: character.to_owned(),
        assisted,
        ..beat("w", ep, "ward", BeatKind::Accomplishment, Magnitude::Major)
    }
}

fn slider(character: &str, ep: &str, competence: u8) -> dli_world::craft::season::SliderState {
    dli_world::craft::season::SliderState {
        character: character.to_owned(),
        episode: ep.to_owned(),
        proactivity: 6,
        competence,
        sympathy: 6,
    }
}

#[test]
fn undercompetent_unassisted_major_win_without_growth_or_price_warns() {
    let ledger = CraftLedger {
        episodes: vec![episode("e1", 1)],
        // A prior failure earns the accomplishment (isolates the competence rule).
        beats: vec![
            beat("f1", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
            win_by("enfys", "e1", false),
        ],
        sliders: vec![slider("enfys", "e1", 2)],
        ..CraftLedger::default()
    };
    let findings = check_season_rules(&ledger, &DEFAULT_THRESHOLDS);
    assert!(
        has(&findings, "craft.competence-consistency"),
        "{:?}",
        codes(&findings)
    );
    assert_eq!(
        findings
            .iter()
            .find(|f| f.code == "craft.competence-consistency")
            .unwrap()
            .severity,
        Severity::Warn
    );
}

#[test]
fn competence_win_is_clean_when_aided_or_grown_or_paid_for() {
    // Aid: the win is assisted.
    let aided = CraftLedger {
        episodes: vec![episode("e1", 1)],
        beats: vec![
            beat("f1", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
            win_by("enfys", "e1", true),
        ],
        sliders: vec![slider("enfys", "e1", 2)],
        ..CraftLedger::default()
    };
    assert!(!has(
        &check_season_rules(&aided, &DEFAULT_THRESHOLDS),
        "craft.competence-consistency"
    ));

    // Growth: competence rises across the season before the win.
    let grown = CraftLedger {
        episodes: vec![episode("e1", 1), episode("e2", 2)],
        beats: vec![
            beat("f1", "e1", "ward", BeatKind::Fail, Magnitude::Minor),
            win_by("enfys", "e2", false),
        ],
        sliders: vec![slider("enfys", "e1", 2), slider("enfys", "e2", 4)],
        ..CraftLedger::default()
    };
    assert!(!has(
        &check_season_rules(&grown, &DEFAULT_THRESHOLDS),
        "craft.competence-consistency"
    ));

    // Price: the character paid with a prior failure of their own.
    let priced = CraftLedger {
        episodes: vec![episode("e1", 1), episode("e2", 2)],
        beats: vec![
            LedgerBeat {
                by: "enfys".to_owned(),
                ..beat("p1", "e1", "ward", BeatKind::Fail, Magnitude::Minor)
            },
            win_by("enfys", "e2", false),
        ],
        sliders: vec![slider("enfys", "e1", 2), slider("enfys", "e2", 2)],
        ..CraftLedger::default()
    };
    assert!(!has(
        &check_season_rules(&priced, &DEFAULT_THRESHOLDS),
        "craft.competence-consistency"
    ));
}

#[test]
fn flat_competence_trajectory_reads_arc_flat() {
    let ledger = CraftLedger {
        episodes: vec![episode("e1", 1), episode("e2", 2)],
        sliders: vec![slider("anchor", "e1", 6), slider("anchor", "e2", 6)],
        ..CraftLedger::default()
    };
    assert!(has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.competence-arc-flat"
    ));
}

// -- CRAFT-2: reveal pacing, mysteries ---------------------------------------

#[test]
fn kishotenketsu_turn_must_be_fair_play() {
    let mut ledger = CraftLedger {
        episodes: vec![episode("e1", 1), episode("e2", 2), episode("e3", 3)],
        turns: vec![Turn {
            episode: "e3".to_owned(),
            reframing_fact: "the-well-was-poisoned".to_owned(),
            resolves_conflict: false,
        }],
        ..CraftLedger::default()
    };
    // No prior reveal of the reframing fact — unfair.
    assert!(has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.reveal-fair-play"
    ));

    // Seed the fact on the listener ledger an episode before the turn — fair.
    ledger.reveals.push(Reveal {
        fact: "the-well-was-poisoned".to_owned(),
        episode: "e1".to_owned(),
    });
    assert!(!has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.reveal-fair-play"
    ));
}

#[test]
fn open_mystery_untouched_too_long_starves() {
    let ledger = CraftLedger {
        episodes: (1..=6).map(|s| episode(&format!("e{s}"), s)).collect(),
        promises: vec![LedgerPromise {
            id: "the-hound".to_owned(),
            kind: PromiseKind::Mystery,
            opened_in: "e1".to_owned(),
            touched_in: Vec::new(),
            closed_in: None,
            declared_carry: false,
        }],
        ..CraftLedger::default()
    };
    assert!(has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.mystery-starved"
    ));
}

// -- CRAFT-2: marketability / clarity family ----------------------------------

#[test]
fn cold_open_discipline_is_enforced() {
    let ledger = CraftLedger {
        episodes: vec![episode("e1", 1)],
        scenes: vec![LedgerScene {
            id: "e1-cold".to_owned(),
            episode: "e1".to_owned(),
            cast: vec!["a".into(), "b".into(), "c".into()],
            cold_open: true,
            opens_loop: false,
            act: Some(1),
            deep_time: true,
        }],
        ..CraftLedger::default()
    };
    let findings = check_season_rules(&ledger, &DEFAULT_THRESHOLDS);
    // All three clauses fire under one rule code.
    assert!(has(&findings, "craft.cold-open-discipline"));
    assert!(
        findings
            .iter()
            .filter(|f| f.code == "craft.cold-open-discipline")
            .count()
            >= 3
    );
}

#[test]
fn scene_name_clash_is_an_error() {
    let mut confusable = BTreeSet::new();
    confusable.insert(("bram".to_owned(), "bran".to_owned())); // stored normalized
    let ledger = CraftLedger {
        episodes: vec![episode("e1", 1)],
        scenes: vec![LedgerScene {
            id: "e1-s1".to_owned(),
            episode: "e1".to_owned(),
            cast: vec!["bran".to_owned(), "bram".to_owned()],
            cold_open: false,
            opens_loop: false,
            act: None,
            deep_time: false,
        }],
        confusable_pairs: confusable,
        ..CraftLedger::default()
    };
    let findings = check_season_rules(&ledger, &DEFAULT_THRESHOLDS);
    assert!(has(&findings, "craft.scene-name-clash"));
    assert_eq!(
        findings
            .iter()
            .find(|f| f.code == "craft.scene-name-clash")
            .unwrap()
            .severity,
        Severity::Error
    );
}

#[test]
fn cast_introduction_band_and_budget() {
    let mut e1 = episode("e1", 1);
    e1.introduced_names = vec!["a".into(), "b".into(), "c".into(), "d".into()]; // over band (3)
    let ledger = CraftLedger {
        episodes: vec![e1],
        ..CraftLedger::default()
    };
    assert!(has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.cast-introduction-rate"
    ));

    // Season budget.
    let budgeted = Thresholds {
        season_cast_budget: Some(2),
        ..DEFAULT_THRESHOLDS
    };
    assert!(has(
        &check_season_rules(&ledger, &budgeted),
        "craft.cast-introduction-rate"
    ));
}

#[test]
fn kishotenketsu_arc_shape_conformance() {
    let ledger = CraftLedger {
        episodes: vec![EpisodeMeta {
            arc_shape: Some(ArcShape::Kishotenketsu),
            ten_position: Some(0.20), // out of the 0.55..0.80 band
            ..episode("e1", 1)
        }],
        turns: vec![Turn {
            episode: "e1".to_owned(),
            reframing_fact: "x".to_owned(),
            resolves_conflict: true, // a ten must not resolve a conflict
        }],
        // Seed the reveal so reveal-fair-play does not also fire.
        reveals: vec![Reveal {
            fact: "x".to_owned(),
            episode: "e1".to_owned(),
        }],
        ..CraftLedger::default()
    };
    let findings = check_season_rules(&ledger, &DEFAULT_THRESHOLDS);
    let arc: Vec<_> = findings
        .iter()
        .filter(|f| f.code == "craft.arc-shape")
        .collect();
    assert_eq!(
        arc.len(),
        2,
        "out-of-band ten and clash-resolution ten: {:?}",
        codes(&findings)
    );
    assert!(arc.iter().any(|f| f.citations == vec!["D-0052".to_owned()]));
}

#[test]
fn audio_load_trajectory_flags_a_high_run() {
    let high = |id: &str, slot: u32| EpisodeMeta {
        audio_load: Some(AudioLoad::High),
        ..episode(id, slot)
    };
    let ledger = CraftLedger {
        episodes: vec![high("e1", 1), high("e2", 2), high("e3", 3)],
        ..CraftLedger::default()
    };
    assert!(has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.audio-load-trajectory"
    ));
}

#[test]
fn thresholds_are_policy_data_the_bands_move() {
    // Tightening the reveal-lead band changes the verdict with no code change.
    let ledger = CraftLedger {
        episodes: vec![episode("e1", 1), episode("e2", 2)],
        turns: vec![Turn {
            episode: "e2".to_owned(),
            reframing_fact: "f".to_owned(),
            resolves_conflict: false,
        }],
        reveals: vec![Reveal {
            fact: "f".to_owned(),
            episode: "e1".to_owned(),
        }],
        ..CraftLedger::default()
    };
    // Lead 1: the reveal one episode earlier is fair.
    assert!(!has(
        &check_season_rules(&ledger, &DEFAULT_THRESHOLDS),
        "craft.reveal-fair-play"
    ));
    // Lead 2: the same reveal is now too late.
    let strict = Thresholds {
        reveal_lead_episodes: 2,
        ..DEFAULT_THRESHOLDS
    };
    assert!(has(
        &check_season_rules(&ledger, &strict),
        "craft.reveal-fair-play"
    ));
}

// -- CRAFT-3: the knowledge base ---------------------------------------------

#[test]
fn kb_ports_the_whole_corpus_cleanly() {
    assert_eq!(
        kb::PRINCIPLES.len(),
        107,
        "the pinned corpus carries 107 principles"
    );
    assert!(
        kb::audit().is_empty(),
        "duplicate ids / phase-less entries: {:?}",
        kb::audit()
    );
}

#[test]
fn every_corpus_tension_carries_its_resolution() {
    // The type makes this a compile guarantee (see the `compile_fail` doctest on
    // `MissionFilter`); here we confirm the corpus actually exercises the arm.
    let tensions: Vec<_> = kb::PRINCIPLES
        .iter()
        .filter(|p| matches!(p.mission_filter, MissionFilter::Tension { .. }))
        .collect();
    assert_eq!(tensions.len(), 6);
    assert!(tensions.iter().all(|p| p.resolution().is_some()));
}

#[test]
fn coverage_and_debt_partition_the_corpus() {
    let c = kb::coverage();
    assert_eq!(c.total, kb::PRINCIPLES.len());
    assert_eq!(c.kernel_rule + c.factory + c.judge + c.declaration, c.total);
    assert_eq!(c.deterministic(), c.kernel_rule + c.factory);
    // Checkable-debt is exactly the judge + declaration-only principles (spec §5).
    let debt = kb::checkable_debt();
    assert_eq!(debt.len(), c.debt());
    assert_eq!(debt.len(), c.judge + c.declaration);
    assert!(
        debt.iter()
            .all(|p| matches!(p.checkable(), Checkable::Judge | Checkable::None))
    );
}

#[test]
fn kernel_rule_bindings_all_resolve() {
    // The closed `KernelRule` enum makes a dangling deterministic ref a compile
    // error (see the `compile_fail` doctest on `Binding`). Confirm every kernel
    // binding names a rule in the registry, and the id round-trips.
    for p in kb::PRINCIPLES {
        if let Binding::Rule(rule) = p.binding {
            assert_eq!(KernelRule::from_id(rule.id()), Some(rule));
        }
    }
    assert_eq!(KernelRule::from_id("no-such-rule"), None);
}

#[test]
fn a_judge_bound_principle_reports_attested() {
    let principle = kb::by_id("promise-progress-payoff").expect("a judge principle");
    assert!(matches!(principle.binding, Binding::Attest(_)));

    let mut store = AttestationStore::new();
    let question = principle
        .judge_question("ppp(episode-01)", "episode-01")
        .expect("judge principle poses a question");
    store.append(attestation(question, "yes"));

    let finding = kb::evaluate_judge(principle, &store, "ppp(episode-01)", "episode-01")
        .expect("a judge principle yields a finding");
    assert_eq!(finding.grade.to_string(), "attested");
    assert_eq!(finding.code, "craft.promise-progress-payoff");
}

#[test]
fn a_missing_attestation_is_reported_not_guessed() {
    let principle = kb::by_id("promise-progress-payoff").unwrap();
    let store = AttestationStore::new();
    let finding = kb::evaluate_judge(principle, &store, "ppp(e1)", "e1").unwrap();
    assert_eq!(finding.code, "craft.attestation-missing");
    // The kernel never fabricates a judgment, so this stays a checked absence.
    assert_eq!(finding.grade.to_string(), "checked");
}

#[test]
fn a_non_judge_principle_has_no_attestation_to_consume() {
    // `promise-is-a-contract` binds a deterministic rule, not a judge dimension.
    let principle = kb::by_id("promise-is-a-contract").expect("a deterministic principle");
    assert!(matches!(principle.binding, Binding::Rule(_)));
    let store = AttestationStore::new();
    assert!(kb::evaluate_judge(principle, &store, "x", "y").is_none());
}

#[test]
fn why_renders_pillars_first() {
    // Every finding the KB explains lists pillar-family doctrine ahead of generic
    // advice (D-0128 — the pillar wins).
    for code in [
        "outline.premature-resolution",
        "craft.accomplishment-earned",
    ] {
        let principles = kb::why(code);
        let first_generic = principles.iter().position(|p| !p.is_pillar());
        let last_pillar = principles.iter().rposition(|p| p.is_pillar());
        if let (Some(g), Some(pil)) = (first_generic, last_pillar) {
            assert!(
                pil < g,
                "a pillar sorts after generic advice in why({code})"
            );
        }
    }
}

#[test]
fn explain_renders_statement_and_resolution() {
    let tension = kb::PRINCIPLES
        .iter()
        .find(|p| matches!(p.mission_filter, MissionFilter::Tension { .. }))
        .unwrap();
    let rendered = tension.explain();
    assert!(rendered.contains(tension.statement));
    assert!(rendered.contains("resolution:"));
    assert!(rendered.contains("source:"));
}

// -- attestation fixture ------------------------------------------------------

fn attestation(question: Question, answer: &str) -> Attestation {
    Attestation {
        question,
        answer: answer.to_owned(),
        rationale_hash: "sha256:rationale".to_owned(),
        model: ModelRef {
            provider: "bedrock".to_owned(),
            id: "claude-fable-5".to_owned(),
            as_of: "2026-07-20".to_owned(),
        },
        prompt: PromptRef {
            template: "craft/judge_ppp@v1".to_owned(),
            hash: "sha256:prompt".to_owned(),
        },
        params: Sampling {
            temperature: "0.0".to_owned(),
            max_tokens: 512,
            samples: 1,
            agreement: "1/1".to_owned(),
        },
        policy: Policy {
            reattest: "on-model-change".to_owned(),
        },
        supersedes: None,
        sweep: "lab/campaign-craft/attest-0001.manifest.json".to_owned(),
        fingerprint: ContentHash("sha256:facts".to_owned()),
    }
}
