//! Unit tests over two tiny synthetic rulesets — a linear ranked deck (an
//! `Ascending` move, a generic `Completed` verdict, generic invariants,
//! decorations) and a rankless cyclic wheel (a `CyclicReach` move, a registered
//! verdict and a registered invariant). Between them they exercise every seam of
//! the generic engine before any world migration.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use dli_core::Grade;
use dli_core::atom::Atom;
use dli_core::valuation::Valuation;
use dli_sim::InvariantVerdict;

use dli_export::profile::{
    ExportOpts, ExportTarget, ProfileSpec, ProvenanceMode, Registry, RenderRule, Selection,
};

use crate::battery::{DealBattery, ReadingInvariant, ReadingMetric, ReadingObservable};
use crate::binding::GameBindings;
use crate::export::{self, follow_edge_count};
use crate::model::{
    AttrValue, DeltaOp, Provenance, Reading, Ruleset, Variant, Verdict,
    enumerate_structural_invariants,
};

fn export_opts() -> ExportOpts {
    ExportOpts {
        source_commit: "content-sha256:test-source".to_owned(),
        ..ExportOpts::default()
    }
}

// -- the synthetic rulesets -------------------------------------------------

/// A linear ranked deck: two marks by three tones, ascending within a mark.
const STEPPED_RULES: &str = r#"{
  "game_id": "stepped-deck",
  "region": "sample",
  "axes": [
    { "id": "mark", "values": [
      { "id": "dot", "name": "Dot" },
      { "id": "bar", "name": "Bar" }
    ] },
    { "id": "tone", "values": [
      { "id": "low",  "name": "Low",  "attrs": { "rank": 0 } },
      { "id": "mid",  "name": "Mid",  "attrs": { "rank": 1 } },
      { "id": "high", "name": "High", "attrs": { "rank": 2 } }
    ] }
  ],
  "layers": [
    { "id": "piece", "axes": ["mark", "tone"], "id_template": "{mark}-{tone}", "dealt": true }
  ],
  "ordering": "layer-product",
  "moves": { "rule": "ascending", "axis": "tone", "step": 1, "same": ["mark"] },
  "layouts": [
    { "name": "a-climb", "cards_dealt": 3, "requires_cards": [] },
    { "name": "a-step",  "cards_dealt": 2, "requires_cards": [] },
    { "name": "the-high-climb", "cards_dealt": 3, "requires_cards": ["dot-high"] }
  ],
  "verdict": { "verdict": "completed" },
  "invariants": [ { "invariant": "deck-is-a-set" }, { "invariant": "count-matches" } ],
  "decorations": [
    { "selector": { "mark": "dot" }, "attrs": { "emblem": "the-seed" } }
  ],
  "provenance": { "standing": "ratified", "cites": ["D-9001"] }
}"#;

/// A rankless cyclic wheel: four marks around a hub, with a registered verdict and
/// a registered pairing invariant.
const RING_RULES: &str = r#"{
  "game_id": "ring-deck",
  "region": "sample",
  "axes": [
    { "id": "phase", "values": [
      { "id": "p0", "name": "Phase Zero",  "attrs": { "opposite": "p2" } },
      { "id": "p1", "name": "Phase One",   "attrs": { "opposite": "p3" } },
      { "id": "p2", "name": "Phase Two",   "attrs": { "opposite": "p0" } },
      { "id": "p3", "name": "Phase Three", "attrs": { "opposite": "p1" } }
    ] }
  ],
  "layers": [
    { "id": "mark", "axes": ["phase"], "id_template": "mark-{phase}", "dealt": true },
    { "id": "hub",  "axes": [], "id_template": "hub", "dealt": false }
  ],
  "ordering": "layer-product",
  "moves": { "rule": "cyclic-reach", "reach": 2 },
  "layouts": [ { "name": "a-turn", "cards_dealt": 2, "requires_cards": [] } ],
  "verdict": { "verdict": "registered", "name": "counsel" },
  "invariants": [
    { "invariant": "deck-is-a-set" },
    { "invariant": "closed-ring" },
    { "invariant": "count-matches" },
    { "invariant": "registered", "name": "paired-phases" }
  ],
  "decorations": [],
  "provenance": { "standing": "ratified", "cites": ["D-9003"] }
}"#;

fn stepped() -> Ruleset {
    Ruleset::from_rules(STEPPED_RULES).expect("stepped rules parse")
}

fn ring() -> Ruleset {
    Ruleset::from_rules(RING_RULES).expect("ring rules parse")
}

// -- registered predicates (the world binding seam) -------------------------

fn counsel(_ruleset: &Ruleset, reading: &Reading) -> Verdict {
    if reading.holds("mark-p0") && reading.holds("mark-p2") {
        Verdict::new("opposed")
    } else if reading.is_filled() {
        Verdict::new("answered")
    } else {
        Verdict::new("open")
    }
}

fn paired_phases(ruleset: &Ruleset) -> Option<String> {
    let Some(axis) = ruleset.axis("phase") else {
        return Some("no phase axis".to_owned());
    };
    let present: std::collections::BTreeSet<&str> =
        axis.values.iter().map(|value| value.id.as_str()).collect();
    axis.values.iter().find_map(|value| {
        value
            .attrs
            .get("opposite")
            .and_then(AttrValue::as_text)
            .filter(|opposite| !present.contains(opposite))
            .map(|opposite| format!("phase '{}' names absent opposite '{opposite}'", value.id))
    })
}

fn ring_bindings() -> GameBindings {
    GameBindings::new()
        .with_verdict("counsel", counsel)
        .with_invariant("paired-phases", paired_phases)
}

// -- deck structure ---------------------------------------------------------

#[test]
fn loads_and_enumerates_cards_in_wheel_order() {
    let deck = stepped();
    assert_eq!(deck.cards().len(), 6);
    assert_eq!(
        deck.card_ids(),
        vec![
            "dot-low", "dot-mid", "dot-high", "bar-low", "bar-mid", "bar-high"
        ]
    );
    // The hub in the ring deck enumerates but is never a play.
    let wheel = ring();
    assert_eq!(wheel.card_ids().len(), 5);
    assert_eq!(wheel.card_ids().last().map(String::as_str), Some("hub"));
}

#[test]
fn ascending_follows_reads_rank_and_shared_axis() {
    let deck = stepped();
    assert!(deck.follows("dot-low", "dot-mid")); // same mark, rank +1
    assert!(!deck.follows("dot-low", "dot-high")); // rank +2, not +1
    assert!(!deck.follows("dot-low", "bar-mid")); // different mark
    assert!(!deck.follows("dot-high", "dot-low")); // descending
    assert!(!deck.follows("dot-mid", "dot-mid")); // never itself
}

#[test]
fn cyclic_follows_wraps_and_respects_reach() {
    let deck = ring();
    assert!(deck.follows("mark-p0", "mark-p1")); // forward 1
    assert!(deck.follows("mark-p0", "mark-p2")); // forward 2 (within reach)
    assert!(!deck.follows("mark-p0", "mark-p3")); // forward 3 (beyond reach)
    assert!(deck.follows("mark-p3", "mark-p0")); // wraps: forward 2
    assert!(!deck.follows("mark-p0", "mark-p0")); // never itself
}

#[test]
fn legal_moves_open_free_then_follow_and_exclude_hub() {
    let deck = ring();
    let open = Reading::open(deck.layout("a-turn").unwrap());
    // Opening play is free over the four dealt marks; the hub is never offered.
    let mut opening = deck.legal_moves(&open);
    opening.sort();
    assert_eq!(opening, vec!["mark-p0", "mark-p1", "mark-p2", "mark-p3"]);
    assert!(!opening.iter().any(|id| id == "hub"));

    let mut after = open.clone();
    after.laid.push("mark-p0".to_owned());
    let mut moves = deck.legal_moves(&after);
    moves.sort();
    assert_eq!(moves, vec!["mark-p1", "mark-p2"]); // within reach, not held
}

#[test]
fn terminal_and_generic_verdict() {
    let deck = stepped();
    let empty = GameBindings::new();

    // A full ascending climb fills the layout: completed.
    let mut climb = Reading::open(deck.layout("a-climb").unwrap());
    for id in ["dot-low", "dot-mid", "dot-high"] {
        assert!(!deck.is_terminal(&climb));
        climb.laid.push(id.to_owned());
    }
    assert!(deck.is_terminal(&climb));
    assert_eq!(deck.verdict(&climb, &empty).id, Verdict::COMPLETED);

    // Opening on the top rank jams immediately: incomplete.
    let mut jammed = Reading::open(deck.layout("a-climb").unwrap());
    jammed.laid.push("dot-high".to_owned());
    assert!(deck.is_terminal(&jammed));
    assert_eq!(deck.verdict(&jammed, &empty).id, Verdict::INCOMPLETE);
}

// -- structural invariants --------------------------------------------------

#[test]
fn generic_invariants_hold_and_catch_a_collision() {
    let verdicts = enumerate_structural_invariants(&stepped(), &GameBindings::new());
    assert_eq!(verdicts.len(), 2);
    assert!(verdicts.iter().all(InvariantVerdict::held));

    // Two zero-axis layers rendering the same id collide: not a set, count off.
    let bad = r#"{
      "game_id": "bad", "region": "sample", "axes": [],
      "layers": [
        { "id": "a", "axes": [], "id_template": "x", "dealt": true },
        { "id": "b", "axes": [], "id_template": "x", "dealt": true }
      ],
      "ordering": "layer-product",
      "moves": { "rule": "free" },
      "layouts": [ { "name": "one", "cards_dealt": 1 } ],
      "verdict": { "verdict": "completed" },
      "invariants": [ { "invariant": "deck-is-a-set" }, { "invariant": "count-matches" } ],
      "decorations": [],
      "provenance": { "standing": "ratified", "cites": ["D-9004"] }
    }"#;
    let doctored = Ruleset::from_rules(bad).unwrap();
    let verdicts = enumerate_structural_invariants(&doctored, &GameBindings::new());
    assert!(verdicts.iter().all(|verdict| !verdict.held()));
}

// -- the registered seam ----------------------------------------------------

#[test]
fn registered_verdict_and_invariant_resolve_through_bindings() {
    let deck = ring();
    let bindings = ring_bindings();

    let mut opposed = Reading::open(deck.layout("a-turn").unwrap());
    opposed.laid.push("mark-p0".to_owned());
    opposed.laid.push("mark-p2".to_owned());
    assert_eq!(deck.verdict(&opposed, &bindings).id, "opposed");

    // With no predicate bound, a registered verdict is `unbound`, and the
    // registered invariant surfaces as a breach rather than silently holding.
    assert_eq!(
        deck.verdict(&opposed, &GameBindings::new()).id,
        Verdict::UNBOUND
    );
    let bound = enumerate_structural_invariants(&deck, &bindings);
    assert!(bound.iter().all(InvariantVerdict::held));
    let unbound = enumerate_structural_invariants(&deck, &GameBindings::new());
    assert!(
        unbound
            .iter()
            .any(|verdict| verdict.invariant == "paired-phases" && !verdict.held())
    );
}

// -- variants as deltas -----------------------------------------------------

#[test]
fn cut_axis_value_drops_cards_and_forecloses_a_layout() {
    let base = stepped();
    assert!(base.layout("the-high-climb").is_some());

    let variant = Variant {
        name: "stepped/low-only".to_owned(),
        game_id: "stepped-low-only".to_owned(),
        ops: vec![DeltaOp::CutAxisValue {
            axis: "tone".to_owned(),
            value: "high".to_owned(),
        }],
        provenance: Provenance::proposed("D-9002"),
    };
    let cut = base.fork(&variant);
    assert!(!cut.is_assertable()); // a fork is only proposed
    assert_eq!(cut.cards().len(), 4);
    assert!(cut.layout("the-high-climb").is_none()); // dot-high was cut

    let ratified = cut.ratify("D-9002");
    assert!(ratified.is_assertable());
    assert_eq!(ratified.provenance.cites, vec!["D-9002".to_owned()]);
}

// -- the deal battery -------------------------------------------------------

#[test]
fn deal_battery_folds_an_honest_deterministic_manifest() {
    let battery = DealBattery {
        ruleset: stepped(),
        layout: "a-climb".to_owned(),
        seed: 20_260_721,
        trials: 300,
        metrics: vec![
            ReadingMetric {
                name: "completed-rate".to_owned(),
                observable: ReadingObservable::VerdictIs {
                    verdict_id: Verdict::COMPLETED.to_owned(),
                },
            },
            ReadingMetric {
                name: "high-appears".to_owned(),
                observable: ReadingObservable::CardAppears {
                    card: "dot-high".to_owned(),
                },
            },
        ],
        invariants: vec![
            ReadingInvariant::DeckConserved,
            ReadingInvariant::OnlyLegalPlays,
        ],
    };
    let manifest = battery.run(&GameBindings::new());
    assert!(manifest.grades_are_honest());
    assert_eq!(manifest.trials, 300);
    assert_eq!(manifest.metrics[0].grade(), Grade::Tested);
    assert_eq!(manifest.invariants[0].grade(), Grade::Checked);
    // Conservation and legality always hold; the completed rate is genuinely mixed.
    assert!(manifest.invariants.iter().all(InvariantVerdict::held));
    assert!(manifest.metrics[0].sum > 0 && manifest.metrics[0].sum < 300);

    // Same seed replays byte-for-byte.
    let again = battery.run(&GameBindings::new());
    assert_eq!(manifest, again);
}

#[test]
fn deal_battery_observes_a_registered_verdict() {
    let battery = DealBattery {
        ruleset: ring(),
        layout: "a-turn".to_owned(),
        seed: 20_260_721,
        trials: 200,
        metrics: vec![ReadingMetric {
            name: "opposed-rate".to_owned(),
            observable: ReadingObservable::VerdictIs {
                verdict_id: "opposed".to_owned(),
            },
        }],
        invariants: vec![ReadingInvariant::DeckConserved],
    };
    let manifest = battery.run(&ring_bindings());
    assert!(manifest.grades_are_honest());
    assert!(manifest.metrics[0].sum > 0); // some readings lay both opposites
}

// -- the exports ------------------------------------------------------------

fn export_registry() -> Registry {
    let mut renders = BTreeMap::new();
    renders.insert(
        "piece".to_owned(),
        RenderRule {
            id: "{tone}-{mark}".to_owned(),
            keep: vec![
                "tone".to_owned(),
                "mark".to_owned(),
                "emblem".to_owned(),
                "name".to_owned(),
            ],
        },
    );
    Registry {
        profiles: vec![
            ProfileSpec {
                target: ExportTarget::Cards,
                name: "stepped-cards".to_owned(),
                version: 1,
                selects: Selection::default(),
                renders,
                carries_provenance: ProvenanceMode::DevMetadata,
            },
            ProfileSpec {
                target: ExportTarget::LudiiGdl,
                name: "stepped-ludii".to_owned(),
                version: 1,
                selects: Selection::default(),
                renders: BTreeMap::new(),
                carries_provenance: ProvenanceMode::Stripped,
            },
        ],
    }
}

#[test]
fn card_export_honors_the_render_rule() {
    let registry = export_registry();
    let deck = export::run(&registry, ExportTarget::Cards, &stepped(), &export_opts())
        .expect("cards export");
    let cards = deck.artifact.as_cards().expect("a card deck");
    assert_eq!(cards.card_count, 6);
    assert_eq!(cards.cards.len(), 6);
    assert_eq!(cards.cited, Some(vec!["D-9001".to_owned()]));

    let low_dot = cards
        .cards
        .iter()
        .find(|card| card.id == "low-dot")
        .unwrap();
    assert_eq!(low_dot.kept.get("mark"), Some(&AttrValue::text("dot")));
    assert_eq!(low_dot.kept.get("name"), Some(&AttrValue::text("Low")));
    assert_eq!(
        low_dot.kept.get("emblem"),
        Some(&AttrValue::text("the-seed"))
    );

    // A card the decoration does not select keeps no emblem.
    let low_bar = cards
        .cards
        .iter()
        .find(|card| card.id == "low-bar")
        .unwrap();
    assert!(!low_bar.kept.contains_key("emblem"));

    let mut layouts = cards.layouts.clone();
    layouts.sort();
    assert_eq!(layouts, vec!["a-climb", "a-step", "the-high-climb"]);
}

#[test]
fn game_exports_require_a_source_anchor() {
    let error = export::run(
        &export_registry(),
        ExportTarget::Cards,
        &stepped(),
        &ExportOpts::default(),
    )
    .unwrap_err();
    assert_eq!(error, dli_export::ExportError::MissingSourceAnchor);
}

#[test]
fn game_exports_reject_lore_world_fork_valuations() {
    let mut valuation = Valuation::default();
    valuation.set(Atom(999), true);
    let error = export::run(
        &export_registry(),
        ExportTarget::Cards,
        &stepped(),
        &ExportOpts {
            fork: Some(valuation),
            ..export_opts()
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        dli_export::ExportError::UnsupportedOption {
            target: ExportTarget::Cards,
            option: "fork",
        }
    );
}

#[test]
fn game_exports_reject_unapplied_lore_options() {
    let target = ExportTarget::Cards;
    let mut stamped = export_opts();
    stamped.keep_stamps =
        serde_json::from_str(r#"[{"scope":["piece"],"visibility":"removed"}]"#).unwrap();
    let cases = [
        (stamped, "keep_stamps"),
        (
            ExportOpts {
                region: Some("elsewhere".to_owned()),
                ..export_opts()
            },
            "region",
        ),
        (
            ExportOpts {
                variant: Some("alternate".to_owned()),
                ..export_opts()
            },
            "variant",
        ),
    ];

    for (opts, option) in cases {
        let error = export::run(&export_registry(), target, &stepped(), &opts).unwrap_err();
        assert_eq!(
            error,
            dli_export::ExportError::UnsupportedOption { target, option }
        );
    }
}

#[test]
fn game_exports_reject_proposed_rulesets() {
    let variant = Variant {
        name: "proposal".to_owned(),
        game_id: "stepped-proposal".to_owned(),
        ops: Vec::new(),
        provenance: Provenance::proposed("D-9003"),
    };
    let error = export::run(
        &export_registry(),
        ExportTarget::Cards,
        &stepped().fork(&variant),
        &export_opts(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        dli_export::ExportError::UnassertableSource {
            target: ExportTarget::Cards,
        }
    );
}

#[test]
fn card_profiles_reject_unknown_selectors_templates_and_kept_fields() {
    let mut profile = export_registry().profiles.remove(0);
    profile.selects.relations.push("invented".to_owned());
    assert!(matches!(
        export::compile_cards(&stepped(), &profile, "certain"),
        Err(dli_export::ExportError::InvalidProfile { .. })
    ));

    profile.selects.relations.clear();
    profile.renders.get_mut("piece").unwrap().id = "{invented}".to_owned();
    assert!(matches!(
        export::compile_cards(&stepped(), &profile, "certain"),
        Err(dli_export::ExportError::InvalidProfile { .. })
    ));

    profile.renders.get_mut("piece").unwrap().id = "stray}-{tone}-{mark}".to_owned();
    assert!(matches!(
        export::compile_cards(&stepped(), &profile, "certain"),
        Err(dli_export::ExportError::InvalidProfile { .. })
    ));

    let rule = profile.renders.get_mut("piece").unwrap();
    rule.id = "{tone}-{mark}".to_owned();
    rule.keep.push("invented".to_owned());
    assert!(matches!(
        export::compile_cards(&stepped(), &profile, "certain"),
        Err(dli_export::ExportError::InvalidProfile { .. })
    ));
}

#[test]
fn card_profiles_reject_rendered_id_collisions() {
    let mut profile = export_registry().profiles.remove(0);
    profile.renders.get_mut("piece").unwrap().id = "same-id".to_owned();
    assert!(matches!(
        export::compile_cards(&stepped(), &profile, "certain"),
        Err(dli_export::ExportError::InvalidProfile { .. })
    ));
}

#[test]
fn ludii_export_renders_components_and_successor_edges() {
    let registry = export_registry();
    let deck = export::run(
        &registry,
        ExportTarget::LudiiGdl,
        &stepped(),
        &export_opts(),
    )
    .expect("ludii export");
    let ludii = deck.artifact.as_ludii().expect("a ludii export");
    for id in ["dot-low", "dot-mid", "dot-high", "bar-high"] {
        assert!(ludii.program.contains(&format!("(component \"{id}\")")));
    }
    assert!(
        ludii
            .program
            .contains("(move (from \"dot-low\") (to \"dot-mid\"))")
    );
    // Four ascending edges: two climbs of two steps each.
    assert_eq!(follow_edge_count(&stepped()), 4);
}

#[test]
fn ludii_export_escapes_every_caller_controlled_string() {
    let mut ruleset = stepped();
    ruleset.game_id = "deck\"\n(players 99)\t\\end\u{0001}".to_owned();
    ruleset.axes[0].values[0].id = "dot\"\n(players 98)\t\\end".to_owned();
    ruleset.layouts[0].name = "layout\"\r(result win)".to_owned();

    let registry = export_registry();
    let spec = registry.default_for(ExportTarget::LudiiGdl).unwrap();
    let ludii = export::compile_ludii(&ruleset, spec, "certain").unwrap();

    assert!(
        ludii
            .program
            .contains("(game \"deck\\\"\\n(players 99)\\t\\\\end\\u0001\"")
    );
    assert!(
        ludii
            .program
            .contains("(component \"dot\\\"\\n(players 98)\\t\\\\end-low\")")
    );
    assert!(
        ludii
            .program
            .contains("(from \"dot\\\"\\n(players 98)\\t\\\\end-low\")")
    );
    assert!(
        ludii
            .program
            .contains("(dealt \"layout\\\"\\r(result win)\")")
    );
    assert!(!ludii.program.contains("deck\"\n(players 99)"));
    assert!(!ludii.program.contains("dot\"\n(players 98)"));
    assert!(!ludii.program.contains("layout\"\r(result win)"));
}

#[test]
fn diff_decks_reports_the_variant_loss() {
    let registry = export_registry();
    let spec = registry.default_for(ExportTarget::Cards).unwrap();

    let variant = Variant {
        name: "stepped/low-only".to_owned(),
        game_id: "stepped-low-only".to_owned(),
        ops: vec![DeltaOp::CutAxisValue {
            axis: "tone".to_owned(),
            value: "high".to_owned(),
        }],
        provenance: Provenance::proposed("D-9002"),
    };
    let honest = export::compile_cards(&stepped(), spec, "certain").unwrap();
    let cut = export::compile_cards(&stepped().fork(&variant), spec, "certain").unwrap();

    let diff = export::diff_decks(&honest, &cut);
    assert_eq!((diff.card_count_before, diff.card_count_after), (6, 4));
    assert_eq!(diff.cards_removed.len(), 2); // both high-tone cards, rendered ids
    assert_eq!(diff.layouts_removed, vec!["the-high-climb".to_owned()]);
}
