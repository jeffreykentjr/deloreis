//! The `outline.py` parity corpus, ported (`CRAFT-1`).
//!
//! Each test mirrors a case in the consuming repo's `tests/test_craft_outline.py`
//! (the parity oracle's own suite): the codes the legacy `check_outline` /
//! `check_season` produce are the codes the dli-world craft checker must produce,
//! under the `outline.` family. Two tests go further and pin the port against the
//! *live* oracle: the vendored real season outline
//! (`worlds/tpth/vendor/fantasy-40f6679-craft/`) and the committed example
//! (`examples/saltmarket-season.yaml`), whose verdicts are captured from
//! `fantasy craft outline` run at the pinned SHA.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::collections::BTreeSet;

use dli_world::check::Severity;
use dli_world::craft::outline::{
    Advance, ArcPromise, AudioLoad, AudioPlan, CarryScope, CharacterSlider, EpisodeOutline,
    MaterialAssumptionLedger, MaterialUnknown, MiceKind, MiceThread, NarratorMode, NarratorTask,
    Promise, PromiseKind, Scene, SceneWork, SeasonOutline, SfxLoad, SfxSemanticLoad, TryFailBeat,
    TryFailOutcome,
};
use dli_world::craft::season::{
    CheckOptions, DEFAULT_EPISODE_POLICY, EpisodePolicy, check_episode, check_season,
    worst_severity,
};

/// The default outline policy (= the legacy `policy::` consts), passed to every
/// `check_episode`/`check_season` call so behavior is byte-identical.
const POLICY: &EpisodePolicy = &DEFAULT_EPISODE_POLICY;

// -- helpers ------------------------------------------------------------------

fn codes(findings: &[dli_world::check::WorldFinding]) -> BTreeSet<String> {
    findings.iter().map(|f| f.code.clone()).collect()
}

fn has(findings: &[dli_world::check::WorldFinding], code: &str) -> bool {
    findings.iter().any(|f| f.code == format!("outline.{code}"))
}

fn scene(id: &str, cast: &[&str], advances: Vec<Advance>, silence: bool) -> Scene {
    Scene {
        id: id.to_owned(),
        summary: id.to_owned(),
        speaking_cast: cast.iter().map(|s| (*s).to_owned()).collect(),
        advances,
        silence_beat: silence,
    }
}

/// A well-formed episode in the fractal-kishōtenketsu shape — the port of
/// `test_craft_outline.clean_episode()`; passes `check_episode` clean.
fn clean_episode() -> EpisodeOutline {
    EpisodeOutline {
        id: "ep1".to_owned(),
        title: "The Failing Ward".to_owned(),
        hook_by_seconds: 45,
        ki: "the apprentice, alone, re-tying a storm-ward knot".to_owned(),
        sho: "the ward keeps souring; the anchor arrives".to_owned(),
        ten: "the trouble is the parish water enclosed upstream, a systemic cause".to_owned(),
        ketsu: "the proof spoken, the valley resettled, the hearth-beat".to_owned(),
        scenes: vec![
            scene("s1", &["Apprentice"], vec![Advance::Plot], false),
            scene(
                "s2",
                &["Apprentice", "Anchor"],
                vec![Advance::Revelation],
                false,
            ),
            scene(
                "s3",
                &["Apprentice", "Anchor"],
                vec![Advance::Relationship],
                true,
            ),
        ],
        promises: vec![
            Promise {
                id: "p1".to_owned(),
                kind: PromiseKind::Mystery,
                statement: "why does the ward sour".to_owned(),
                made_in: "cold-open".to_owned(),
                payoff_ref: Some("s2".to_owned()),
                open_loop_carry: false,
            },
            Promise {
                id: "p2".to_owned(),
                kind: PromiseKind::Relationship,
                statement: "will the anchor adopt".to_owned(),
                made_in: "s1".to_owned(),
                payoff_ref: None,
                open_loop_carry: true,
            },
        ],
        mice_threads: vec![
            MiceThread {
                id: "m1".to_owned(),
                kind: MiceKind::Inquiry,
                open_beat: "s1".to_owned(),
                close_beat: "s3".to_owned(),
            },
            MiceThread {
                id: "m2".to_owned(),
                kind: MiceKind::Milieu,
                open_beat: "s2".to_owned(),
                close_beat: "s2".to_owned(),
            },
        ],
        tryfail_beats: vec![
            TryFailBeat {
                id: "t1".to_owned(),
                attempt: "seat the knot".to_owned(),
                outcome: TryFailOutcome::NoAnd,
            },
            TryFailBeat {
                id: "t2".to_owned(),
                attempt: "ask the anchor".to_owned(),
                outcome: TryFailOutcome::YesBut,
            },
            TryFailBeat {
                id: "t3".to_owned(),
                attempt: "speak the proof".to_owned(),
                outcome: TryFailOutcome::YesAnd,
            },
        ],
        major_characters: vec!["Apprentice".to_owned(), "Anchor".to_owned()],
        character_sliders: vec![
            CharacterSlider {
                character: "Apprentice".to_owned(),
                proactivity: 7,
                competence: 8,
                sympathy: 5,
                compensation: String::new(),
                scene_refs: vec!["s1".to_owned(), "s2".to_owned(), "s3".to_owned()],
            },
            CharacterSlider {
                character: "Anchor".to_owned(),
                proactivity: 5,
                competence: 8,
                sympathy: 4,
                compensation: String::new(),
                scene_refs: vec!["s2".to_owned(), "s3".to_owned()],
            },
        ],
        scene_work: vec![
            SceneWork {
                scene_id: "s1".to_owned(),
                viewpoint: Some("Apprentice".to_owned()),
                goal: "keep the ward seated".to_owned(),
                conflict: "the cord slips in correct hands".to_owned(),
                disaster: "the ward fails in a way that implies false material".to_owned(),
                outcome: TryFailOutcome::NoAnd,
                sequel_reaction: None,
                sequel_dilemma: None,
                sequel_decision: None,
            },
            SceneWork {
                scene_id: "s2".to_owned(),
                viewpoint: Some("Apprentice".to_owned()),
                goal: "get the anchor to read the failure".to_owned(),
                conflict: "the anchor's body is already overburdened".to_owned(),
                disaster: "the answer opens a larger material problem".to_owned(),
                outcome: TryFailOutcome::YesBut,
                sequel_reaction: Some("Aalto stops treating the stranger as rescue.".to_owned()),
                sequel_dilemma: Some("Tending her costs time while the water rises.".to_owned()),
                sequel_decision: Some("Care for the damaged body first.".to_owned()),
            },
            SceneWork {
                scene_id: "s3".to_owned(),
                viewpoint: Some("Apprentice".to_owned()),
                goal: "keep the anchor breathing".to_owned(),
                conflict: "the Archive overload has made competence bodily expensive".to_owned(),
                disaster: "the first household bond forms under cost".to_owned(),
                outcome: TryFailOutcome::YesAnd,
                sequel_reaction: None,
                sequel_dilemma: None,
                sequel_decision: None,
            },
        ],
        audio_plan: Some(AudioPlan {
            narrator_mode: NarratorMode::Windowpane,
            orientation_load: AudioLoad::High,
            sfx_load: SfxLoad::Sparse,
            sfx_semantic_load: SfxSemanticLoad::Support,
            narrator_tasks: vec![
                NarratorTask::Orientation,
                NarratorTask::Causality,
                NarratorTask::ObjectRelationship,
            ],
            direct_address: false,
        }),
        material_assumptions: None,
        loops_opened: vec!["who enclosed the water".to_owned()],
        loops_closed: vec!["why the ward fails".to_owned()],
        season_boundary: false,
    }
}

fn material_ledger() -> MaterialAssumptionLedger {
    MaterialAssumptionLedger {
        season_time_pressure: Some("late rains shorten the repair window".to_owned()),
        staple_dependency: Some("the household needs its stored staple".to_owned()),
        water_dependency: Some("the ward depends on the parish watercourse".to_owned()),
        fuel_dependency: Some("repair heat must come from carried fuel".to_owned()),
        shelter_dependency: Some("the work group needs a dry tying place".to_owned()),
        labor_group: Some("the boundary-working household".to_owned()),
        source: Some("local stores and the damaged boundary".to_owned()),
        transport: Some("hand-carried along the wet path".to_owned()),
        storage: Some("the household's covered store".to_owned()),
        failure_point: Some("water spoils the prepared binding".to_owned()),
        institution: Some("the parish protects access while the keep extracts".to_owned()),
        ritual_obligation: Some("reciprocal ward tending".to_owned()),
        canon_refs: vec!["D-0004".to_owned(), "D-0049".to_owned()],
        research_refs: vec!["RR-007".to_owned()],
        unknowns: vec![],
    }
}

// -- the ported oracle corpus -------------------------------------------------

#[test]
fn clean_episode_passes() {
    assert!(check_episode(&clean_episode(), CheckOptions::default(), POLICY).is_empty());
    assert_eq!(
        worst_severity(&check_episode(
            &clean_episode(),
            CheckOptions::default(),
            POLICY
        )),
        None
    );
}

#[test]
fn packet_methodology_ledgers_pass_clean() {
    let opts = CheckOptions {
        require_methodology: true,
        material_worldbuilding: false,
    };
    assert!(check_episode(&clean_episode(), opts, POLICY).is_empty());
}

#[test]
fn material_ledger_is_optional_outside_opt_in_review() {
    let ep = clean_episode();
    assert!(!has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "no-material-assumption-ledger"
    ));
    let opts = CheckOptions {
        require_methodology: false,
        material_worldbuilding: true,
    };
    assert!(has(
        &check_episode(&ep, opts, POLICY),
        "no-material-assumption-ledger"
    ));
}

#[test]
fn material_review_accepts_sourced_answers_and_explicit_unknowns() {
    let mut ep = clean_episode();
    let mut ledger = material_ledger();
    ledger.fuel_dependency = None;
    ledger.storage = None;
    ledger.unknowns = vec![MaterialUnknown::FuelDependency, MaterialUnknown::Storage];
    ep.material_assumptions = Some(ledger);

    let opts = CheckOptions {
        require_methodology: false,
        material_worldbuilding: true,
    };
    let findings = check_episode(&ep, opts, POLICY);
    assert!(!has(&findings, "material-assumption-incomplete"));
    assert!(!has(&findings, "unsupported-material-assumption"));
}

#[test]
fn material_review_warns_on_incomplete_and_unsupported_assertions() {
    let mut ep = clean_episode();
    ep.material_assumptions = Some(MaterialAssumptionLedger {
        season_time_pressure: Some("harvest is near".to_owned()),
        ..MaterialAssumptionLedger::default()
    });
    let opts = CheckOptions {
        require_methodology: false,
        material_worldbuilding: true,
    };
    let findings = check_episode(&ep, opts, POLICY);
    assert!(has(&findings, "material-assumption-incomplete"));
    assert!(has(&findings, "unsupported-material-assumption"));
}

#[test]
fn promise_needs_payoff_or_carry_not_both() {
    // Ported from the pydantic validator to a fail-closed finding.
    let mut ep = clean_episode();
    ep.promises = vec![Promise {
        id: "x".to_owned(),
        kind: PromiseKind::Mystery,
        statement: "q".to_owned(),
        made_in: "s1".to_owned(),
        payoff_ref: Some("s2".to_owned()),
        open_loop_carry: true,
    }];
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "promise-payoff-xor-carry"
    ));

    let mut ep = clean_episode();
    ep.promises = vec![Promise {
        id: "x".to_owned(),
        kind: PromiseKind::Mystery,
        statement: "q".to_owned(),
        made_in: "s1".to_owned(),
        payoff_ref: None,
        open_loop_carry: false,
    }];
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "promise-payoff-xor-carry"
    ));
}

#[test]
fn dangling_payoff_is_an_error() {
    let mut ep = clean_episode();
    ep.promises[0].payoff_ref = Some("s99".to_owned());
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "dangling-payoff"
    ));
}

#[test]
fn packet_mode_requires_new_methodology_ledgers() {
    let mut ep = clean_episode();
    ep.character_sliders = vec![];
    ep.scene_work = vec![];
    ep.audio_plan = None;
    let opts = CheckOptions {
        require_methodology: true,
        material_worldbuilding: false,
    };
    let findings = check_episode(&ep, opts, POLICY);
    for code in [
        "no-character-slider-ledger",
        "major-character-slider-missing",
        "no-scene-work-ledger",
        "scene-work-missing",
        "no-audio-plan",
    ] {
        assert!(has(&findings, code), "missing {code}");
    }
}

#[test]
fn character_slider_ledger_flags_flat_and_dangling_refs() {
    let mut ep = clean_episode();
    ep.character_sliders = vec![CharacterSlider {
        character: "Apprentice".to_owned(),
        proactivity: 2,
        competence: 3,
        sympathy: 4,
        compensation: String::new(),
        scene_refs: vec!["s99".to_owned()],
    }];
    let findings = check_episode(&ep, CheckOptions::default(), POLICY);
    for code in [
        "character-slider-flat",
        "slider-compensation-missing",
        "character-slider-scene-unknown",
        "major-character-slider-missing",
    ] {
        assert!(has(&findings, code), "missing {code}");
    }
}

#[test]
fn scene_work_ledger_checks_refs_and_terminal_resolution() {
    let mut ep = clean_episode();
    ep.scene_work = vec![
        SceneWork {
            scene_id: "s1".to_owned(),
            viewpoint: None,
            goal: "solve it".to_owned(),
            conflict: "none".to_owned(),
            disaster: "too easy".to_owned(),
            outcome: TryFailOutcome::YesAnd,
            sequel_reaction: None,
            sequel_dilemma: None,
            sequel_decision: None,
        },
        SceneWork {
            scene_id: "s99".to_owned(),
            viewpoint: None,
            goal: "bad ref".to_owned(),
            conflict: "bad ref".to_owned(),
            disaster: "bad ref".to_owned(),
            outcome: TryFailOutcome::NoAnd,
            sequel_reaction: None,
            sequel_dilemma: None,
            sequel_decision: None,
        },
        SceneWork {
            scene_id: "s2".to_owned(),
            viewpoint: None,
            goal: "continue".to_owned(),
            conflict: "it worsens".to_owned(),
            disaster: "the thread stays open".to_owned(),
            outcome: TryFailOutcome::NoAnd,
            sequel_reaction: None,
            sequel_dilemma: None,
            sequel_decision: None,
        },
    ];
    let findings = check_episode(&ep, CheckOptions::default(), POLICY);
    for code in [
        "scene-work-unknown-scene",
        "scene-work-premature-resolution",
        "scene-work-chain-unsettled",
    ] {
        assert!(has(&findings, code), "missing {code}");
    }
}

#[test]
fn audio_plan_flags_failed_no_narrator_boundary() {
    let mut ep = clean_episode();
    ep.audio_plan = Some(AudioPlan {
        narrator_mode: NarratorMode::None,
        orientation_load: AudioLoad::High,
        sfx_load: SfxLoad::Heavy,
        sfx_semantic_load: SfxSemanticLoad::LoadBearing,
        narrator_tasks: vec![],
        direct_address: false,
    });
    let findings = check_episode(&ep, CheckOptions::default(), POLICY);
    for code in [
        "narrator-needed-for-orientation",
        "no-narrator-sfx-burden",
        "sfx-not-sparse",
        "sfx-load-bearing",
    ] {
        assert!(has(&findings, code), "missing {code}");
    }
}

#[test]
fn audio_plan_rejects_host_or_direct_address() {
    let mut ep = clean_episode();
    ep.audio_plan = Some(AudioPlan {
        narrator_mode: NarratorMode::Host,
        direct_address: true,
        ..AudioPlan::default()
    });
    let findings = check_episode(&ep, CheckOptions::default(), POLICY);
    assert!(has(&findings, "narrator-host-frame"));
    assert!(has(&findings, "narrator-direct-address"));
    assert_eq!(worst_severity(&findings), Some(Severity::Error));
}

#[test]
fn mice_interleaving_is_an_error() {
    let mut ep = clean_episode();
    ep.mice_threads = vec![
        MiceThread {
            id: "m1".to_owned(),
            kind: MiceKind::Inquiry,
            open_beat: "s1".to_owned(),
            close_beat: "s2".to_owned(),
        },
        MiceThread {
            id: "m2".to_owned(),
            kind: MiceKind::Event,
            open_beat: "s2".to_owned(),
            close_beat: "s3".to_owned(),
        },
    ];
    let findings = check_episode(&ep, CheckOptions::default(), POLICY);
    assert!(has(&findings, "mice-interleaved"));
    assert_eq!(worst_severity(&findings), Some(Severity::Error));
}

#[test]
fn proper_nesting_and_disjoint_pass() {
    let mut ep = clean_episode();
    ep.mice_threads = vec![
        MiceThread {
            id: "m1".to_owned(),
            kind: MiceKind::Inquiry,
            open_beat: "s1".to_owned(),
            close_beat: "s3".to_owned(),
        },
        MiceThread {
            id: "m2".to_owned(),
            kind: MiceKind::Milieu,
            open_beat: "s2".to_owned(),
            close_beat: "s2".to_owned(),
        },
    ];
    assert!(!has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "mice-interleaved"
    ));
}

#[test]
fn premature_yes_and_warns() {
    let mut ep = clean_episode();
    ep.tryfail_beats = vec![
        TryFailBeat {
            id: "t1".to_owned(),
            attempt: "a".to_owned(),
            outcome: TryFailOutcome::YesAnd,
        },
        TryFailBeat {
            id: "t2".to_owned(),
            attempt: "b".to_owned(),
            outcome: TryFailOutcome::NoAnd,
        },
    ];
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "premature-resolution"
    ));
}

#[test]
fn cast_over_four_warns() {
    let mut ep = clean_episode();
    ep.scenes[0].speaking_cast = ["A", "B", "C", "D", "E"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "cast-over-four"
    ));
}

#[test]
fn idle_scene_warns() {
    let mut ep = clean_episode();
    ep.scenes[1].advances = vec![];
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "scene-idles"
    ));
}

#[test]
fn hook_late_warns() {
    let mut ep = clean_episode();
    ep.hook_by_seconds = 120;
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "hook-late"
    ));
}

#[test]
fn loop_ledger_warns_on_empty() {
    let mut ep = clean_episode();
    ep.loops_opened = vec![];
    ep.loops_closed = vec![];
    let findings = check_episode(&ep, CheckOptions::default(), POLICY);
    assert!(has(&findings, "no-loop-opened"));
    assert!(has(&findings, "no-loop-closed"));
}

#[test]
fn season_boundary_excuses_unclosed_loop() {
    let mut ep = clean_episode();
    ep.loops_closed = vec![];
    ep.season_boundary = true;
    assert!(!has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "no-loop-closed"
    ));
}

#[test]
fn ten_reading_as_battle_warns() {
    let mut ep = clean_episode();
    ep.ten = "the hero draws a blade and cuts down the magister in a final blow".to_owned();
    assert!(has(
        &check_episode(&ep, CheckOptions::default(), POLICY),
        "ten-reads-as-battle"
    ));
}

#[test]
fn season_loop_accounting() {
    let mut a = clean_episode();
    a.id = "e1".to_owned();
    a.loops_opened = vec!["thread-A".to_owned()];
    a.loops_closed = vec!["why the ward fails".to_owned()];
    let mut b = clean_episode();
    b.id = "e2".to_owned();
    b.loops_opened = vec![];
    b.loops_closed = vec!["thread-A".to_owned()];
    let season = SeasonOutline {
        id: "s1".to_owned(),
        title: "Season One".to_owned(),
        promises: vec![],
        episodes: vec![a, b],
    };
    assert!(has(
        &check_season(&season, CheckOptions::default(), POLICY),
        "loop-closed-unopened"
    ));
}

#[test]
fn arc_promise_requires_payoff_or_carry() {
    // Ported from the pydantic validator to a fail-closed finding.
    let neither = ArcPromise {
        id: "p".to_owned(),
        kind: PromiseKind::Umbrella,
        statement: "the household will form".to_owned(),
        opened_in: "arc-open".to_owned(),
        payoff_ref: None,
        carry_scope: None,
    };
    let both = ArcPromise {
        id: "p".to_owned(),
        kind: PromiseKind::Umbrella,
        statement: "the household will form".to_owned(),
        opened_in: "arc-open".to_owned(),
        payoff_ref: Some("e1".to_owned()),
        carry_scope: Some(CarryScope::Act),
    };
    for promise in [neither, both] {
        let season = SeasonOutline {
            id: "s1".to_owned(),
            title: "Season One".to_owned(),
            promises: vec![promise],
            episodes: vec![],
        };
        assert!(has(
            &check_season(&season, CheckOptions::default(), POLICY),
            "arc-promise-payoff-xor-carry"
        ));
    }
}

#[test]
fn season_promise_ledger_checks_cross_episode_refs() {
    let mut a = clean_episode();
    a.id = "e1".to_owned();
    let mut b = clean_episode();
    b.id = "e2".to_owned();
    let season = SeasonOutline {
        id: "s1".to_owned(),
        title: "Season One".to_owned(),
        promises: vec![
            ArcPromise {
                id: "household".to_owned(),
                kind: PromiseKind::Umbrella,
                statement: "will the household form".to_owned(),
                opened_in: "arc-open".to_owned(),
                payoff_ref: Some("e2".to_owned()),
                carry_scope: None,
            },
            ArcPromise {
                id: "bad".to_owned(),
                kind: PromiseKind::Mystery,
                statement: "bad refs".to_owned(),
                opened_in: "missing".to_owned(),
                payoff_ref: Some("also-missing".to_owned()),
                carry_scope: None,
            },
        ],
        episodes: vec![a, b],
    };
    let findings = check_season(&season, CheckOptions::default(), POLICY);
    assert!(has(&findings, "season-promise-open-unknown"));
    assert!(has(&findings, "season-promise-payoff-unknown"));
}

#[test]
fn packet_mode_requires_season_promise_ledger() {
    let season = SeasonOutline {
        id: "s1".to_owned(),
        title: "Season One".to_owned(),
        promises: vec![],
        episodes: vec![clean_episode()],
    };
    let opts = CheckOptions {
        require_methodology: true,
        material_worldbuilding: false,
    };
    assert!(has(
        &check_season(&season, opts, POLICY),
        "no-season-promise-ledger"
    ));
}

// -- live-oracle parity: the vendored real outline ----------------------------

#[test]
fn vendored_arc1_matches_oracle_verdict() {
    // The verdict captured from `fantasy craft outline` at the pinned SHA
    // (scripts/pinned-oracles.sh outline): exactly eight loop-never-closed
    // warnings, one per season-carry loop, and nothing else.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../worlds/tpth/vendor/fantasy-40f6679-craft/",
        "lab/campaign-014-structure-development/ARC1-CRAFT-SIDECAR.yaml"
    );
    let yaml = std::fs::read_to_string(path).expect("vendored ARC1 sidecar exists");
    let season: SeasonOutline =
        serde_yaml_ng::from_str(&yaml).expect("the vendored outline matches the schema");
    assert_eq!(
        season.episodes.len(),
        14,
        "the arc carries fourteen episodes"
    );

    let findings = check_season(&season, CheckOptions::default(), POLICY);
    assert_eq!(
        codes(&findings),
        BTreeSet::from(["outline.loop-never-closed".to_owned()])
    );
    assert_eq!(
        findings.len(),
        8,
        "eight season-carry loops, matching the oracle"
    );

    let mut loops: Vec<&str> = findings
        .iter()
        .map(|f| f.message.split(':').next().unwrap())
        .collect();
    loops.sort_unstable();
    assert_eq!(
        loops,
        vec!["act1.arc1"; 8],
        "each dangling loop attaches to the season id"
    );
}

// -- the committed example ----------------------------------------------------

#[test]
fn committed_season_example_deserializes_and_checks() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../worlds/tpth/season/saltmarket-season.yaml"
    );
    let yaml =
        std::fs::read_to_string(path).expect("worlds/tpth/season/saltmarket-season.yaml exists");
    let season: SeasonOutline =
        serde_yaml_ng::from_str(&yaml).expect("the committed example matches the schema");
    assert_eq!(season.episodes.len(), 2);

    // Each episode is clean at episode scope.
    for ep in &season.episodes {
        assert!(
            check_episode(ep, CheckOptions::default(), POLICY).is_empty(),
            "episode {} should be clean",
            ep.id
        );
    }

    // At season scope the only finding is the deliberate multi-season carry.
    let findings = check_season(&season, CheckOptions::default(), POLICY);
    assert_eq!(
        codes(&findings),
        BTreeSet::from(["outline.loop-never-closed".to_owned()])
    );
    assert_eq!(findings.len(), 1);
}

/// A deliberately-broken episode, deserialized from YAML and diffed against the
/// live oracle. The code set is the authoritative capture from
/// `scripts/pinned-oracles.sh outline --codes` at the pinned SHA (nine codes,
/// two errors and seven warnings) — a multi-family parity case exercising the
/// promise, MICE, scene, loop, ten, and hook checks at once, through the strict
/// serde types rather than hand-built structs.
#[test]
fn broken_episode_yaml_matches_oracle_code_set() {
    let yaml = r"
id: bad1
title: The Broken One
hook_by_seconds: 120
ten: the hero draws a sword and cuts down the bailiff in a final blow
scenes:
  - {id: s1, summary: crowd, speaking_cast: [A, B, C, D, E], advances: [plot]}
  - {id: s2, summary: idle, speaking_cast: [A], advances: []}
  - {id: s3, summary: end, speaking_cast: [A, B], advances: [relationship]}
promises:
  - {id: p1, kind: mystery, statement: q, made_in: cold-open, payoff_ref: s99}
mice_threads:
  - {id: m1, kind: inquiry, open_beat: s1, close_beat: s2}
  - {id: m2, kind: event, open_beat: s2, close_beat: s3}
tryfail_beats:
  - {id: t1, attempt: a, outcome: yes-and}
  - {id: t2, attempt: b, outcome: no-and}
loops_opened: []
loops_closed: []
";
    let ep: EpisodeOutline =
        serde_yaml_ng::from_str(yaml).expect("the broken episode matches the schema");
    let findings = check_episode(&ep, CheckOptions::default(), POLICY);

    let expected: BTreeSet<String> = [
        "cast-over-four",
        "dangling-payoff",
        "hook-late",
        "mice-interleaved",
        "no-loop-closed",
        "no-loop-opened",
        "premature-resolution",
        "scene-idles",
        "ten-reads-as-battle",
    ]
    .iter()
    .map(|c| format!("outline.{c}"))
    .collect();
    assert_eq!(
        codes(&findings),
        expected,
        "must match the oracle's code set"
    );
    assert_eq!(worst_severity(&findings), Some(Severity::Error));
}
