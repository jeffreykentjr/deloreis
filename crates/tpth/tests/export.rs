#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The Turning's world-side card + Ludii export, end to end (`EXP-3`, spec 09 §6).
//!
//! These exercise `tpth::export::run` — the world dispatch that builds the Turning
//! ruleset directly and renders it, needing no compiled world. The honest-vs-keep-
//! stamped diff and the salt-fringe re-suit are the tests; the direct `compile_*`
//! entry points are checked alongside. The card/Ludii profiles are world data
//! (`worlds/tpth/export/profiles.json`), merged onto the engine's neutral base.

use std::path::PathBuf;

use dli_export::profile::{ExportOpts, ExportTarget, Registry};
use tpth::export::{
    DifferentialStatus, TurningArtifact, compile_cards, compile_ludii, diff_decks,
    follow_edge_count, run,
};
use tpth::turning::Ruleset;

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
    assert_eq!(deck.compiled_from.profile, "cards/lowlands@3");
    assert_eq!(deck.compiled_from.variant, None);
    assert_eq!(deck.leaf_count, 29);
    assert!(deck.layouts.contains(&"the-debt-square".to_owned()));

    // A Mender hand leaf carries the Pleacher and its cited provenance (spec 09 §6).
    let mender = deck
        .leaves
        .iter()
        .find(|leaf| leaf.role.as_deref() == Some("mender"))
        .expect("a mender hand leaf");
    assert_eq!(mender.figure.as_deref(), Some("the-pleacher"));
    assert_eq!(
        mender.meta.as_ref().map(|meta| meta.cited.clone()),
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
    assert_eq!(
        keep_stamped.compiled_from.variant.as_deref(),
        Some("turning/keep-stamped")
    );
    assert_eq!(keep_stamped.leaf_count, 26);
    assert!(!keep_stamped.layouts.contains(&"the-debt-square".to_owned()));

    // The diff itself is the shippable artifact (spec 09 §6).
    let diff = diff_decks(honest, keep_stamped);
    assert_eq!(diff.leaf_count_before, 29);
    assert_eq!(diff.leaf_count_after, 26);
    assert_eq!(diff.layouts_removed, vec!["the-debt-square".to_owned()]);
    // Exactly the three hard Weathers (the Debt, the Sundering, the Bourn) fall out.
    assert_eq!(
        diff.leaves_removed,
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
    // The Tide and the Catch re-suit Grounds; the moon-count holds (a re-suit).
    assert_eq!(deck.leaf_count, 29);
    let ground_names: Vec<&str> = deck
        .leaves
        .iter()
        .filter(|leaf| leaf.layer == "ground")
        .filter_map(|leaf| leaf.name.as_deref())
        .collect();
    assert!(ground_names.contains(&"The Tide"));
    assert!(ground_names.contains(&"The Catch"));
    // The Net-mender takes the Mender's face.
    let mender = deck
        .leaves
        .iter()
        .find(|leaf| leaf.role.as_deref() == Some("mender"))
        .unwrap();
    assert_eq!(mender.figure.as_deref(), Some("the-net-mender"));
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
    let TurningArtifact::LudiiGdl(gdl) = outcome.artifact else {
        panic!("expected a Ludii export");
    };

    let honest = Ruleset::lowlands_honest();
    // Every leaf and every available layout appears — a straight render.
    for leaf in honest.leaves() {
        assert!(
            gdl.program.contains(&leaf.id()),
            "leaf {} missing from L-GDL",
            leaf.id()
        );
    }
    for layout in honest.available_layouts() {
        assert!(gdl.program.contains(&layout.name));
    }
    // The successor relation is enumerated, not computed.
    assert!(follow_edge_count(&honest) > 0);
    // The trace-agreement battery is a non-gating lab lane, recorded pending.
    assert!(matches!(
        gdl.differential,
        DifferentialStatus::Pending { .. }
    ));
}

#[test]
fn compile_ludii_directly_renders_each_leaf() {
    let registry = tpth_registry();
    let spec = registry.default_for(ExportTarget::LudiiGdl).unwrap();
    let gdl = compile_ludii(&Ruleset::lowlands_honest(), spec, "certain");
    assert!(gdl.program.starts_with("(game \"turning-lowlands-honest\""));
    assert_eq!(gdl.profile, "games/ludii@1");
}

// -- direct card compile off the ruleset -------------------------------------

#[test]
fn compile_cards_directly_matches_the_ruleset_counts() {
    let registry = tpth_registry();
    let spec = registry.default_for(ExportTarget::Cards).unwrap();
    let honest = compile_cards(
        &Ruleset::lowlands_honest(),
        spec,
        "certain",
        &ExportOpts::default(),
    );
    let keep = compile_cards(
        &Ruleset::keep_stamped(),
        spec,
        "certain",
        &ExportOpts::default(),
    );
    assert_eq!(honest.leaf_count, 29);
    assert_eq!(keep.leaf_count, 26);
    // Ground emblems are surfaced but not part of the moon-count.
    assert_eq!(
        honest.leaves.iter().filter(|l| l.layer == "ground").count(),
        4
    );
}
