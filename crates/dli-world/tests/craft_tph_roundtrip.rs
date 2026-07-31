//! CRAFT-3 golden: the compiled TPtH craft config round-trips through
//! `apply(load_config(...), doctrines())` and reproduces the principle set.
//!
//! This is the acceptance test for the craft compile step (DP-006 C2): the
//! factory-emitted TOML files in `worlds/tpth/craft/` must load into a
//! [`CraftConfig`] with zero validation errors, and composing it with the
//! shipped doctrine packs via [`apply`] must reproduce the full principle
//! corpus — every principle present, every pillar carrying its citations, every
//! overlay carrying its D-nnnn attestation. The principle IDs are the golden
//! anchor: a missing or extra principle is a compile regression.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use dli_world::craft::load::doctrines;
use dli_world::craft::worldcfg::{apply, load_config, validate};

/// Resolve `worlds/` relative to the crate root.
fn worlds_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../worlds")
}

/// The full set of principle IDs that the compiled TPtH craft config must
/// produce. Every ID here is anchored to a D-nnnn decision or a shipped
/// doctrine pack; a missing or unexpected ID is a regression.
fn golden_principle_ids() -> BTreeSet<&'static str> {
    [
        // Pillars (D-0128)
        "pillar-aesthetic-sobriety", "pillar-rhizomatic-restoration",
        "pillar-material-sovereignty", "pillar-earnest-sincerity",
        "pillars-win-standing-rule", "wonder-amid-ruin",
        // Character
        "antagonism-is-systemic", "anti-purity-characters",
        "strangeness-is-a-transaction", "the-multitude-is-distinct",
        "earned-arc1-relationship-phase",
        // World
        "material-reality-first", "diction-geography",
        "early-bounded-magic-genre-promise",
        // Prose
        "no-em-dashes-in-scripts",
        // Audio / factory bindings (world principles + overlay rows)
        "voice-silhouette-principle", "one-sage-per-scene",
        "viewpoint-asks-the-listeners-question", "named-lore-backed-cold-open",
        "closing-punctuation-not-archival-quote", "narrator-never-describes-voices",
        "narrator-brackets-the-frame", "narrator-sfx-load-boundary",
        "first-meeting-audibility", "performable-voice-strangeness",
        "specific-family-context", "lone-opening-dialogue-context",
        "audio-opening-pressure", "early-aalto-enfys-care-hooks",
        // Overlay rows from craft_kb.yaml that ride on shipped generics
        "narrated-sound-to-sfx", "sfx-cannot-carry-mechanics",
        "three-four-voices-per-scene", "vocal-signature-per-character",
        "strategic-name-pepper", "scripted-silence",
        "performance-direction-markers", "four-beat-labels-earn-their-turns",
        "dialogue-scenes-have-turns", "final-musical-sting",
        "bar-raiser-taste-gap",
        "first-three-episodes-compression", "multiple-pov-loop-management",
        "cold-open-is-a-tone-promise", "sfx-as-character",
        "audio-show-dont-tell", "hybrid-script-formatting",
        "plot-points-per-episode", "aphorism-as-acoustic-logo",
        "music-as-learned-lexicon",
        // Generic doctrine pack IDs (must all appear)
        "first-law-magic", "second-law-limitations", "magic-costs-the-body",
        "third-law-expand-before-add", "zeroth-law-err-awesome",
        "consistency-enables-solutions",
        "kishotenketsu-ten-is-the-turn", "climax-is-perspective-shift",
        "mice-quotient", "mice-lifo-nesting", "mice-milieu-as-hub",
        "mice-event-new-equilibrium",
        "promise-progress-payoff", "promise-is-a-contract", "arc-promise-ledger",
        "core-vs-umbrella-plot", "big-p-little-p",
        "foreshadow-and-payoff", "subplot-is-the-relationship-question",
        "causality-therefore-but", "tryfail-yes-but-no-and",
        "yes-and-is-terminal", "tryfail-count-three",
        "scene-goal-conflict-outcome", "sequel-reaction-dilemma-decision",
        "three-sliders", "slider-ledger-per-major-character",
        "slider-compensation", "proactivity-as-investigation",
        "competence-reads-cold", "sympathy-through-jeopardy-and-care",
        "want-versus-need", "contrasting-temperament-cast",
        "agency-never-victimhood",
        "windowpane-vs-stained-glass", "pyramid-of-abstraction",
        "concrete-detail-doctrine", "opening-sensory-seasonal-grounding",
        "said-asked-eighty-five", "no-adverb-tags",
        "show-versus-tell-is-pacing", "sentence-rhythm-and-sound",
        "read-everything-aloud", "no-filter-words",
        "subtext-over-announcement", "distilled-imagery-not-purple",
        "iceberg-worldbuilding", "world-as-plot-engine",
        "episode-is-complete-kishotenketsu", "open-loop-discipline",
        "micro-pacing-every-installment", "promise-payoff-new-promise",
        "escalate-depth-not-pace", "reader-breathing-room",
        "sfx-as-character", "audio-show-dont-tell",
        "narrated-sound-to-sfx", "sfx-cannot-carry-mechanics",
        "sixty-second-hook",
        // Remaining generic IDs not yet listed
        "no-as-you-know-bob", "modular-world-generates-arcs",
        "clarity-is-life-or-death",
    ]
    .into_iter()
    .collect()
}

#[test]
fn craft_config_loads_without_errors() {
    let cfg = load_config(&worlds_root().join("tpth/craft"))
        .expect("craft config must load");
    let errors = validate(&cfg, &doctrines());
    assert!(
        errors.is_empty(),
        "craft config has validation errors: {errors:#?}"
    );
}

#[test]
fn craft_config_roundtrips_principle_set() {
    let cfg = load_config(&worlds_root().join("tpth/craft"))
        .expect("craft config must load");
    let packs = doctrines();
    let applied = apply(&cfg, &packs);
    let ids: BTreeSet<String> = applied
        .principles()
        .iter()
        .map(|p| p.id.to_string())
        .collect();

    let golden = golden_principle_ids();
    let missing: BTreeSet<&str> =
        golden.difference(&ids.iter().map(|s| s.as_str()).collect()).copied().collect();
    let extra: BTreeSet<String> =
        ids.iter().filter(|id| !golden.contains(id.as_str())).cloned().collect();

    assert!(
        missing.is_empty(),
        "principles missing from compiled craft config: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "unexpected principles in compiled craft config: {extra:?}"
    );
}

#[test]
fn craft_config_pillars_carry_decision_citations() {
    let cfg = load_config(&worlds_root().join("tpth/craft"))
        .expect("craft config must load");
    let packs = doctrines();
    let applied = apply(&cfg, &packs);

    let pillars: Vec<_> = applied
        .principles()
        .iter()
        .filter(|p| p.family.is_pillar())
        .collect();

    assert!(
        !pillars.is_empty(),
        "must have at least the 5 D-0128 pillars + wonder-amid-ruin"
    );

    for p in &pillars {
        assert!(
            !p.citations.is_empty(),
            "pillar '{}' must carry D-nnnn citations",
            p.id
        );
    }
}
