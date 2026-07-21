#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The Turning's card + Ludii export, end to end (`EXP-3`, spec 09 §6) — asserted
//! through the **generic `dli_game::export` path** the world dispatch now delegates
//! to.
//!
//! These exercise `tpth::export::run` — the world dispatch that builds the Turning
//! ruleset from the region/variant selectors and hands it to the engine renderer.
//! The honest-vs-keep-stamped diff and the salt-fringe re-suit are the tests; the
//! direct `compile_*` entry points are checked alongside. The card/Ludii profiles
//! are world data (`worlds/tpth/export/profiles.json`), merged onto the engine's
//! neutral base.

use std::path::PathBuf;

use dli_export::profile::{ExportOpts, ExportTarget, Registry};
use tpth::export::{
    DifferentialStatus, compile_cards, compile_ludii, diff_decks, follow_edge_count, run,
};
use tpth::turning::{AttrValue, keep_stamped, salt_fringe, the_turning};

/// The engine's neutral base catalog merged with the `tpth` world's export
/// profiles, loaded from data (`worlds/tpth/export/profiles.json`) — the
/// `cards/lowlands@3`, `games/ludii@1`, `video/articy@1` profiles the world ships,
/// sourced from the world, not the kernel.
fn tpth_registry() -> Registry {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../worlds/tpth/export/profiles.json");
    Registry::builtin().merge(
        Registry::from_path(&path).expect("worlds/tpth/export/profiles.json loads and parses"),
    )
}

/// The figure a rendered card carries, per the profile's `figure` keep field.
fn figure_of<'a>(deck: &'a tpth::export::CardDeck, role: &str) -> Option<&'a str> {
    deck.cards
        .iter()
        .find(|card| card.kept.get("role").and_then(AttrValue::as_text) == Some(role))
        .and_then(|card| card.kept.get("figure").and_then(AttrValue::as_text))
}

// -- the worked card export (EXP-3, spec 09 §6) ------------------------------

#[test]
fn worked_card_export_honest_deck() {
    let outcome = run(
        &tpth_registry(),
        ExportTarget::Cards,
        &ExportOpts::default(),
    )
    .unwrap();
    let deck = outcome.artifact.as_cards().expect("a card deck");

    assert_eq!(deck.deck, "turning-lowlands-honest");
    assert_eq!(deck.profile, "cards/lowlands@3");
    assert_eq!(deck.card_count, 29);
    assert!(deck.layouts.contains(&"the-debt-square".to_owned()));

    // A Mender hand leaf carries the Pleacher (a kept `figure` field, from the
    // ruleset's decoration table) and its cited provenance (spec 09 §6).
    assert_eq!(figure_of(deck, "mender"), Some("the-pleacher"));
    assert_eq!(
        deck.cited,
        Some(vec!["D-0147".to_owned(), "D-0148".to_owned()])
    );
}

#[test]
fn worked_card_export_keep_stamped_diff_is_computed_not_asserted() {
    let registry = tpth_registry();

    let honest = run(&registry, ExportTarget::Cards, &ExportOpts::default()).unwrap();
    let honest = honest.artifact.as_cards().unwrap();

    // The keep-stamped variant is lore (a variant selector); the ruleset cuts the
    // hard Weathers, and the debt-square drops out of `available_layouts` by compile.
    let keep_stamped = run(
        &registry,
        ExportTarget::Cards,
        &ExportOpts {
            variant: Some("keep-stamped".to_owned()),
            ..ExportOpts::default()
        },
    )
    .unwrap();
    let keep_stamped = keep_stamped.artifact.as_cards().unwrap();

    assert_eq!(keep_stamped.deck, "turning-lowlands-keep-stamped");
    assert_eq!(keep_stamped.card_count, 26);
    assert!(!keep_stamped.layouts.contains(&"the-debt-square".to_owned()));

    // The diff itself is the shippable artifact (spec 09 §6), computed not asserted.
    let diff = diff_decks(honest, keep_stamped);
    assert_eq!(diff.card_count_before, 29);
    assert_eq!(diff.card_count_after, 26);
    assert_eq!(diff.layouts_removed, vec!["the-debt-square".to_owned()]);
    // Exactly the three hard Weathers (the Debt, the Sundering, the Bourn) fall out.
    assert_eq!(
        diff.cards_removed,
        vec![
            "weather-bourn".to_owned(),
            "weather-debt".to_owned(),
            "weather-sundering".to_owned(),
        ]
    );
}

#[test]
fn worked_card_export_salt_fringe_regional_delta() {
    let outcome = run(
        &tpth_registry(),
        ExportTarget::Cards,
        &ExportOpts {
            region: Some("salt-fringe".to_owned()),
            ..ExportOpts::default()
        },
    )
    .unwrap();
    let deck = outcome.artifact.as_cards().unwrap();

    assert_eq!(deck.deck, "turning-salt-fringe-honest");
    // The moon-count holds (a re-suit, not a cut).
    assert_eq!(deck.card_count, 29);
    // The Net-mender takes the Mender's face (the re-faced decoration).
    assert_eq!(figure_of(deck, "mender"), Some("the-net-mender"));

    // The Tide and the Catch re-suit the Ground axis values in place — a fact of the
    // ruleset (Grounds are an axis, not a rendered card layer, so the re-suit is read
    // from the axis, not the card render).
    let fringe = salt_fringe();
    let ground = fringe.axis("ground").unwrap();
    assert_eq!(ground.value("wood").unwrap().name, "The Tide");
    assert_eq!(ground.value("field").unwrap().name, "The Catch");
}

// -- the Ludii L-GDL export (EXP-3, DL-0007) ---------------------------------

#[test]
fn ludii_export_is_a_structural_render_with_pending_differential() {
    let outcome = run(
        &tpth_registry(),
        ExportTarget::LudiiGdl,
        &ExportOpts::default(),
    )
    .unwrap();
    let gdl = outcome.artifact.as_ludii().expect("a Ludii export");

    let honest = the_turning();
    // Every leaf and every available layout appears — a straight render.
    for id in honest.card_ids() {
        assert!(gdl.program.contains(&id), "leaf {id} missing from L-GDL");
    }
    for layout in honest.available_layouts() {
        assert!(gdl.program.contains(&layout.name));
    }
    // The successor relation is enumerated, not computed.
    assert!(follow_edge_count(&honest) > 0);
    // The trace-agreement battery is a non-gating lane, recorded pending.
    assert!(matches!(
        gdl.differential,
        DifferentialStatus::Pending { .. }
    ));
}

#[test]
fn compile_ludii_directly_renders_each_leaf() {
    let registry = tpth_registry();
    let spec = registry.default_for(ExportTarget::LudiiGdl).unwrap();
    let gdl = compile_ludii(&the_turning(), spec, "certain");
    assert!(gdl.program.starts_with("(game \"turning-lowlands-honest\""));
    assert_eq!(gdl.profile, "games/ludii@1");
}

// -- direct card compile off the ruleset -------------------------------------

#[test]
fn compile_cards_directly_matches_the_ruleset_counts() {
    let registry = tpth_registry();
    let spec = registry.default_for(ExportTarget::Cards).unwrap();
    let honest = compile_cards(&the_turning(), spec, "certain");
    let keep = compile_cards(&keep_stamped(), spec, "certain");
    assert_eq!(honest.card_count, 29);
    assert_eq!(keep.card_count, 26);
    // The 29 enumerated cards render: 16 Hands, 12 Weathers, 1 Reckoner (Grounds are
    // an axis, not a card layer, so no separate ground emblems are rendered).
    assert_eq!(honest.cards.len(), 29);
    assert_eq!(
        honest.cards.iter().filter(|c| c.layer == "hand").count(),
        16
    );
}
