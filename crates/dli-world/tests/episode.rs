#![allow(clippy::unwrap_used, clippy::expect_used)]
//! EPI-1 oracle: the fold runs in production order; a flashback scene carries
//! its own anchor and folds correctly; `knowledge_at` distinguishes
//! listener-time from story-time.

use dli_world::episode::{
    Episode, EpisodeAnchor, FactDelta, FactRef, Knower, KnowledgeDelta, Presence, QuestionRef,
    Scene, fold,
};

use dli_time::wheel::WheelPos;

/// A scene that only introduces a fact and reveals it to the listener.
fn reveal_scene(id: &str, fact: &str, anchor: Option<EpisodeAnchor>) -> Scene {
    Scene {
        id: id.to_owned(),
        scene_anchor: anchor,
        fact_deltas: vec![FactDelta::of(fact)],
        knowledge_deltas: vec![KnowledgeDelta::listener_learns(fact)],
        ..Scene::default()
    }
}

/// A bare episode at a production slot and a story era.
fn episode(id: &str, order: u32, era: &str, scenes: Vec<Scene>) -> Episode {
    Episode {
        id: id.to_owned(),
        production_order: order,
        story_anchor: EpisodeAnchor::era(era),
        scenes,
    }
}

#[test]
fn fold_runs_in_production_order() {
    // Handed to the fold scrambled: slot 3, then 1, then 2.
    let e3 = episode(
        "e3",
        3,
        "the-beleaguered-present",
        vec![reveal_scene("s3", "third-fact", None)],
    );
    let e1 = episode(
        "e1",
        1,
        "the-beleaguered-present",
        vec![reveal_scene("s1", "first-fact", None)],
    );
    let e2 = episode(
        "e2",
        2,
        "the-beleaguered-present",
        vec![reveal_scene("s2", "second-fact", None)],
    );

    let world = fold(&[e3, e1, e2]);

    let first = FactRef("first-fact".to_owned());
    let second = FactRef("second-fact".to_owned());
    let third = FactRef("third-fact".to_owned());

    // At slot 1 the listener knows only the first fact; knowledge accrues
    // strictly along production order regardless of input order.
    let at1 = world.knowledge_at(1, &Knower::Listener);
    assert_eq!(at1.len(), 1);
    assert!(at1.contains(&first));

    let at2 = world.knowledge_at(2, &Knower::Listener);
    assert!(at2.contains(&first) && at2.contains(&second) && !at2.contains(&third));

    let at3 = world.knowledge_at(3, &Knower::Listener);
    assert!(at3.contains(&first) && at3.contains(&second) && at3.contains(&third));

    // The raw ledger is emitted in production order too.
    let orders: Vec<u32> = world
        .knowledge_records()
        .iter()
        .map(|r| r.production_order)
        .collect();
    assert_eq!(orders, vec![1, 2, 3]);
}

#[test]
fn flashback_scene_carries_own_anchor() {
    // Episode 1 is anchored to the story-present.
    let e1 = episode(
        "e1",
        1,
        "the-beleaguered-present",
        vec![reveal_scene("s1", "the-well-runs-dry", None)],
    );

    // Episode 5 (production) contains a scene anchored to an era EARLIER than
    // episode 1's anchor — a flashback.
    let flashback = reveal_scene(
        "s5-flashback",
        "the-old-pact",
        Some(EpisodeAnchor::era("elder-cycles").after_event("the-founding")),
    );
    let e5 = Episode {
        id: "e5".to_owned(),
        production_order: 5,
        story_anchor: EpisodeAnchor::era("the-beleaguered-present"),
        scenes: vec![flashback],
    };

    let world = fold(&[e1, e5]);
    let pact = FactRef("the-old-pact".to_owned());

    // The delta lands at the EARLY story anchor (elder-cycles), not the
    // episode's story-present anchor.
    let placement = world.story_placement(&pact).expect("fact was introduced");
    assert_eq!(placement.era.as_ref().unwrap().0, "elder-cycles");
    assert_eq!(placement.after.first().unwrap().0, "the-founding");

    // But the LISTENER learns it at production position 5 — not before.
    assert!(!world.knowledge_at(4, &Knower::Listener).contains(&pact));
    assert!(world.knowledge_at(5, &Knower::Listener).contains(&pact));

    // The placement's production slot records the listener's when; its anchor
    // records the world's when — the two dimensions, kept apart.
    let record = world.placements().iter().find(|p| p.fact == pact).unwrap();
    assert_eq!(record.production_order, 5);
    assert_eq!(record.anchor.era.as_ref().unwrap().0, "elder-cycles");
}

#[test]
fn scene_anchor_falls_back_to_episode_anchor() {
    // No scene anchor: the fact is so when the episode is so.
    let e1 = episode(
        "e7",
        7,
        "the-long-creep",
        vec![reveal_scene("s1", "the-stones-break", None)],
    );
    let world = fold(&[e1]);
    let fact = FactRef("the-stones-break".to_owned());
    let placement = world.story_placement(&fact).unwrap();
    assert_eq!(placement.era.as_ref().unwrap().0, "the-long-creep");
}

#[test]
fn knowledge_at_distinguishes_listener_from_cast() {
    // In one scene the listener learns a fact but a character does not; in the
    // next, the character learns it too. The two ledgers are independent.
    let s1 = Scene {
        id: "s1".to_owned(),
        knowledge_deltas: vec![KnowledgeDelta::listener_learns("the-ward-is-false")],
        fact_deltas: vec![FactDelta::of("the-ward-is-false")],
        ..Scene::default()
    };
    let s2 = Scene {
        id: "s2".to_owned(),
        knowledge_deltas: vec![KnowledgeDelta::cast_learns("enfys", "the-ward-is-false")],
        ..Scene::default()
    };
    let e1 = episode("e1", 1, "the-beleaguered-present", vec![s1]);
    let e2 = episode("e2", 2, "the-beleaguered-present", vec![s2]);

    let world = fold(&[e1, e2]);
    let fact = FactRef("the-ward-is-false".to_owned());
    let enfys = Knower::cast("enfys");

    // Dramatic irony at slot 1: listener knows, character does not.
    assert!(world.knowledge_at(1, &Knower::Listener).contains(&fact));
    assert!(!world.knowledge_at(1, &enfys).contains(&fact));

    // The character catches up at slot 2.
    assert!(world.knowledge_at(2, &enfys).contains(&fact));
}

#[test]
fn open_loop_ledger_tracks_introduced_vs_resolved() {
    // A loop opened in episode 1 (production) and closed in episode 3, handed to
    // the fold out of order.
    let e1 = Episode {
        id: "e1".to_owned(),
        production_order: 1,
        story_anchor: EpisodeAnchor::era("the-beleaguered-present"),
        scenes: vec![Scene {
            id: "s1".to_owned(),
            opens: vec![QuestionRef("who-drowned-the-field".to_owned())],
            ..Scene::default()
        }],
    };
    let e3 = Episode {
        id: "e3".to_owned(),
        production_order: 3,
        story_anchor: EpisodeAnchor::era("the-beleaguered-present"),
        scenes: vec![Scene {
            id: "s3".to_owned(),
            closes: vec![QuestionRef("who-drowned-the-field".to_owned())],
            ..Scene::default()
        }],
    };

    let world = fold(&[e3, e1]);
    let q = QuestionRef("who-drowned-the-field".to_owned());

    // Open at slot 2 (introduced, not yet resolved).
    let at2 = world.open_loops_at(2);
    assert_eq!(at2.open, vec![q.clone()]);
    assert!(at2.closed.is_empty());

    // Resolved by slot 3.
    let at3 = world.open_loops_at(3);
    assert!(at3.open.is_empty());
    assert_eq!(at3.closed, vec![q.clone()]);

    // The season-final ledger agrees.
    let final_ledger = world.open_loops();
    assert!(final_ledger.open.is_empty());
    assert_eq!(final_ledger.closed, vec![q]);
}

#[test]
fn presence_records_on_and_off_mic() {
    let scene = Scene {
        id: "s1".to_owned(),
        presence: vec![
            Presence::OnMic(dli_world::episode::EntityRef("enfys".to_owned())),
            Presence::OffMic(dli_world::episode::EntityRef("anchor".to_owned())),
        ],
        ..Scene::default()
    };
    assert_eq!(scene.presence[0].entity().0, "enfys");
    assert_eq!(scene.presence[1].entity().0, "anchor");
}

#[test]
fn episode_round_trips_through_json() {
    let ep = Episode {
        id: "e14".to_owned(),
        production_order: 14,
        story_anchor: EpisodeAnchor::era("the-beleaguered-present")
            .after_event("e13")
            .with_phase(WheelPos {
                wheel: "tide".to_owned(),
                index: 1,
            }),
        scenes: vec![reveal_scene(
            "s3",
            "the-drowned-field",
            Some(
                EpisodeAnchor::era("the-long-creep")
                    .after_event("protagonists-taking")
                    .before_event("tithe-of-minds-sweeps"),
            ),
        )],
    };
    let json = serde_json::to_string(&ep).unwrap();
    let back: Episode = serde_json::from_str(&json).unwrap();
    assert_eq!(ep, back);
}
