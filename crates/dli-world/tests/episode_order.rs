#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Episode-order adapter coverage: the fold→slot projection, the order-rule and
//! slack wiring, and both carry-items (R1-L2 integrity, R2-G standing).

use std::collections::BTreeMap;

use dli_core::ann::{Ann, Theory};
use dli_core::atom::Atom;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;
use dli_time::chronology::{Chronology, TimePoint};
use dli_time::order_rules::{OrderFindingKind, OrderRule, SlotGuard, SlotRequirement};

use dli_world::episode::{
    EntityRef, Episode, EpisodeAnchor, KnowledgeDelta, Presence, QuestionRef, Scene,
};
use dli_world::episode_order::{
    OrderIntegrityKind, StandingOrder, check_order, episodes_to_slots, event_precedence,
    order_integrity, season_slack,
};

fn on_mic(name: &str) -> Presence {
    Presence::OnMic(EntityRef(name.to_owned()))
}

fn speaking_scene(id: &str, speakers: &[&str]) -> Scene {
    Scene {
        id: id.to_owned(),
        presence: speakers.iter().map(|s| on_mic(s)).collect(),
        ..Scene::default()
    }
}

// --- the fold→slot adapter ---------------------------------------------------

#[test]
fn full_team_configuration_holds_from_the_assembling_slot_onward() {
    let episodes = vec![
        Episode {
            id: "e5".to_owned(),
            production_order: 5,
            scenes: vec![speaking_scene("s1", &["ana"])],
            ..Episode::default()
        },
        Episode {
            id: "e8".to_owned(),
            production_order: 8,
            scenes: vec![speaking_scene("s1", &["ana", "bel"])],
            ..Episode::default()
        },
        Episode {
            id: "e12".to_owned(),
            production_order: 12,
            scenes: vec![speaking_scene("s1", &["ana"])],
            ..Episode::default()
        },
    ];
    let slots = episodes_to_slots(&episodes, &BTreeMap::new());
    // The full team (ana, bel) assembles at slot 8; before then, not full.
    assert!(!slots[0].configurations.contains("full-team"));
    assert!(slots[1].configurations.contains("full-team"));
    assert!(slots[2].configurations.contains("full-team"));
}

#[test]
fn learns_eras_are_projected_from_the_fold() {
    let episodes = vec![Episode {
        id: "flashback".to_owned(),
        production_order: 3,
        story_anchor: EpisodeAnchor::era("elder-cycles"),
        scenes: vec![Scene {
            id: "s1".to_owned(),
            knowledge_deltas: vec![KnowledgeDelta::listener_learns("the-first-lie")],
            ..Scene::default()
        }],
    }];
    let slots = episodes_to_slots(&episodes, &BTreeMap::new());
    assert!(slots[0].learns_eras.contains("elder-cycles"));
}

// --- order-rule wiring over real episodes ------------------------------------

#[test]
fn the_full_team_floor_flags_a_season_that_assembles_too_early() {
    // The full team assembles at slot 8, but the floor rule wants slot >= 10.
    let episodes = vec![
        Episode {
            id: "e5".to_owned(),
            production_order: 5,
            scenes: vec![speaking_scene("s1", &["ana"])],
            ..Episode::default()
        },
        Episode {
            id: "e8".to_owned(),
            production_order: 8,
            scenes: vec![speaking_scene("s1", &["ana", "bel"])],
            ..Episode::default()
        },
    ];
    let rule = OrderRule::guarded(
        "full-team-floor",
        SlotGuard::ListenerHasConfiguration("full-team".to_owned()),
        SlotRequirement::SlotAtLeast(10),
    );
    let findings = check_order(&episodes, &[rule], &BTreeMap::new());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].episode, "e8");
    assert_eq!(findings[0].kind, OrderFindingKind::SlotTooEarly);
}

#[test]
fn no_deep_time_uses_the_supplied_act_map() {
    let episodes = vec![Episode {
        id: "deep".to_owned(),
        production_order: 2,
        story_anchor: EpisodeAnchor::era("elder-cycles"),
        scenes: vec![Scene {
            id: "s1".to_owned(),
            knowledge_deltas: vec![KnowledgeDelta::listener_learns("origins")],
            ..Scene::default()
        }],
    }];
    let rule = OrderRule::guarded(
        "no-deep-time-act-one",
        SlotGuard::ListenerLearnsDuringEra("elder-cycles".to_owned()),
        SlotRequirement::ActAtLeast(2),
    );

    // No act supplied → indeterminate (a ruling-request).
    let unknown = check_order(&episodes, std::slice::from_ref(&rule), &BTreeMap::new());
    assert_eq!(unknown[0].kind, OrderFindingKind::ActIndeterminate);

    // Act 1 supplied → a hard too-early finding.
    let mut acts = BTreeMap::new();
    acts.insert("deep".to_owned(), 1);
    let known = check_order(&episodes, &[rule], &acts);
    assert_eq!(known[0].kind, OrderFindingKind::ActTooEarly);
}

// --- slack wiring ------------------------------------------------------------

#[test]
fn a_reveal_and_its_payoff_pin_the_two_episodes() {
    let episodes = vec![
        Episode {
            id: "setup".to_owned(),
            production_order: 1,
            scenes: vec![Scene {
                id: "s1".to_owned(),
                opens: vec![QuestionRef("the-debt".to_owned())],
                ..Scene::default()
            }],
            ..Episode::default()
        },
        Episode {
            id: "payoff".to_owned(),
            production_order: 2,
            scenes: vec![Scene {
                id: "s1".to_owned(),
                closes: vec![QuestionRef("the-debt".to_owned())],
                ..Scene::default()
            }],
            ..Episode::default()
        },
    ];
    let report = season_slack(
        &episodes,
        &[],
        &BTreeMap::new(),
        &Valuation::new(),
        &Theory::new(),
    );
    assert!(report.is_feasible());
    // setup must precede payoff; with N = 2 both are pinned.
    let setup = report.for_episode("setup").unwrap();
    let payoff = report.for_episode("payoff").unwrap();
    assert_eq!((setup.earliest, setup.latest), (1, 1));
    assert_eq!((payoff.earliest, payoff.latest), (2, 2));
}

// --- carry-item R1-L2: production-order integrity ----------------------------

#[test]
fn duplicate_and_gapped_production_orders_are_reported() {
    let episodes = vec![
        Episode {
            id: "a".to_owned(),
            production_order: 1,
            ..Episode::default()
        },
        Episode {
            id: "b".to_owned(),
            production_order: 1,
            ..Episode::default()
        },
        Episode {
            id: "c".to_owned(),
            production_order: 4,
            ..Episode::default()
        },
    ];
    let findings = order_integrity(&episodes);
    let dup = findings
        .iter()
        .find(|f| f.kind == OrderIntegrityKind::DuplicateProductionOrder)
        .expect("a duplicate finding");
    assert_eq!(dup.slot, 1);
    assert_eq!(dup.episodes, vec!["a".to_owned(), "b".to_owned()]);

    let gaps: Vec<u32> = findings
        .iter()
        .filter(|f| f.kind == OrderIntegrityKind::GapInProductionOrder)
        .map(|f| f.slot)
        .collect();
    assert_eq!(gaps, vec![2, 3]);
}

#[test]
fn a_contiguous_unique_sequence_has_no_integrity_findings() {
    let episodes = vec![
        Episode {
            id: "a".to_owned(),
            production_order: 1,
            ..Episode::default()
        },
        Episode {
            id: "b".to_owned(),
            production_order: 2,
            ..Episode::default()
        },
    ];
    assert!(order_integrity(&episodes).is_empty());
}

// --- carry-item R2-G: standing-aware precedence surfaced at the episode layer -

#[test]
fn the_episode_layer_reads_precedence_through_standing() {
    // A proposed before-fact never hardens to a certain order at the episode
    // layer either — event_precedence re-exports the standing-aware read.
    let chrono = Chronology {
        edges: vec![(TimePoint(0), TimePoint(1))],
    };
    let proposed = Ann::supported(PosBool::pos(Atom(9)));
    let verdict = event_precedence(
        &chrono,
        &[proposed],
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(1),
    );
    assert_eq!(verdict, StandingOrder::PossiblyBefore);

    let ratified = event_precedence(
        &chrono,
        &[Ann::unconditional()],
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(1),
    );
    assert_eq!(ratified, StandingOrder::CertainlyBefore);
}
