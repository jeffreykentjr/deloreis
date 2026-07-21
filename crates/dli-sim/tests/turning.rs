//! The SIM-2 oracle, end to end.
//!
//! Each test pins one clause of the brief's oracle: legal-move enumeration under
//! deltas, the `D-0147`→`D-0148` fork/ratify replay, the honest-versus-rigged
//! study yielding a measurable difference, determinism (same seed → identical
//! manifest), and the structural-invariant enumeration. The ruleset is exercised
//! as *data*: every check reads the same [`Ruleset`] the export lane compiles, so
//! a green suite is evidence the rules are really rules and really exportable.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use dli_sim::turning::{
    HandRole, Layer, Leaf, Reading, Ruleset, Standing, StructuralInvariant, TurningBattery,
    Variant, Verdict, enumerate_structural_invariants, honest_vs_rigged,
};
use dli_sim::turning::{ReadingInvariant, ReadingMetric, ReadingObservable};

/// The committed export-facing fixture EXP-3 compiles against.
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../worlds/tpth/turning/the-turning-rules.json")
}

// --- deck structure ---------------------------------------------------------

#[test]
fn honest_deck_runs_the_moon_at_twenty_nine() {
    let deck = Ruleset::lowlands_honest();
    assert_eq!(deck.leaf_count(), 29, "16 Hands + 12 Weathers + 1 Reckoner");
    let leaves = deck.leaves();
    assert_eq!(leaves.len(), 29);
    let hands = leaves.iter().filter(|l| l.layer() == Layer::Hand).count();
    let weathers = leaves
        .iter()
        .filter(|l| l.layer() == Layer::Weather)
        .count();
    let reckoners = leaves
        .iter()
        .filter(|l| l.layer() == Layer::Reckoner)
        .count();
    assert_eq!((hands, weathers, reckoners), (16, 12, 1));
}

#[test]
fn leaf_ids_and_figures_match_the_worked_export() {
    // The spec §6 worked export pins `hand-mender-field` → the Pleacher and
    // `weather-debt` → ring_next `weather-fallow`. The ruleset must agree.
    let deck = Ruleset::lowlands_honest();
    let mender_field = Leaf::Hand {
        role: HandRole::Mender,
        ground: "field".to_owned(),
    };
    assert_eq!(mender_field.id(), "hand-mender-field");
    assert_eq!(
        deck.figure_for(&mender_field).map(|f| f.id.as_str()),
        Some("the-pleacher")
    );
    let debt = deck.weathers.iter().find(|w| w.id == "debt").unwrap();
    assert_eq!(debt.ring_next, "fallow"); // leaf id weather-fallow
}

// --- legal-move enumeration under deltas ------------------------------------

#[test]
fn legal_move_enumeration_shrinks_under_the_keep_stamped_delta() {
    let honest = Ruleset::lowlands_honest();
    let round = honest.layout("a-round").unwrap().clone();
    let opening = Reading::open(&round);

    // On the honest deck the opening play may lay any of the hard Weathers.
    let honest_moves = honest.legal_moves(&opening);
    for hard in ["weather-debt", "weather-sundering", "weather-bourn"] {
        assert!(
            honest_moves.contains(&hard.to_owned()),
            "honest lacks {hard}"
        );
    }

    // The keep-stamped delta cuts them from the deck, so they vanish from the
    // legal-move set — the enumeration is delta-sensitive, computed not asserted.
    let rigged = Ruleset::keep_stamped();
    let rigged_moves = rigged.legal_moves(&Reading::open(&round));
    for hard in ["weather-debt", "weather-sundering", "weather-bourn"] {
        assert!(!rigged_moves.contains(&hard.to_owned()), "rig kept {hard}");
    }
    // And every rigged move is a legal honest move — a cut only removes plays.
    let honest_set: std::collections::BTreeSet<_> = honest_moves.iter().collect();
    assert!(rigged_moves.iter().all(|m| honest_set.contains(m)));
}

#[test]
fn follows_is_forward_bounded_and_never_reflexive() {
    let deck = Ruleset::lowlands_honest();
    let wheel: Vec<String> = deck.leaves().iter().map(Leaf::id).collect();
    let reach = usize::try_from(deck.reach).unwrap();
    for (i, prev) in wheel.iter().enumerate() {
        assert!(!deck.follows(prev, prev), "a leaf never follows itself");
        for step in 1..=reach {
            let next = &wheel[(i + step) % wheel.len()];
            assert!(deck.follows(prev, next), "within reach must follow");
        }
        // Just past reach is not a legal follow.
        let beyond = &wheel[(i + reach + 1) % wheel.len()];
        assert!(!deck.follows(prev, beyond), "beyond reach must not follow");
    }
}

// --- the D-0147 → D-0148 fork/ratify replay ---------------------------------

#[test]
fn remap_touchstone_is_proposed_and_ratifies_to_the_turning() {
    // D-0147: the remap-method touchstone wakes the deck, but only proposed —
    // structurally unassertable until an authorial ruling.
    let touchstone = Ruleset::remap_touchstone();
    assert!(
        !touchstone.is_assertable(),
        "a proposed deck is shadow-only"
    );
    assert_eq!(touchstone.provenance.standing, Standing::Proposed);
    assert_eq!(touchstone.provenance.cites, vec!["D-0147".to_owned()]);

    // D-0148: ratified whole. Now assertable, citing both the touchstone it was
    // woken from and the decision that ratified it, and equal to the canonical
    // deck (which is *defined* as this very path — the replay is literal).
    let ratified = touchstone.ratify("D-0148");
    assert!(ratified.is_assertable());
    assert_eq!(ratified.provenance.standing, Standing::Ratified);
    assert_eq!(ratified.provenance.cites, vec!["D-0147", "D-0148"]);
    assert_eq!(ratified, Ruleset::lowlands_honest());
}

#[test]
fn a_variant_fork_is_proposed_until_ratified() {
    let honest = Ruleset::lowlands_honest();

    // Forking the keep-stamped delta yields a *proposed* ruleset — a variant is a
    // claim awaiting a ruling — yet its legal-move enumeration already reflects
    // the delta before ratification (the rules changed; the standing has not).
    let proposed = honest.fork(&Variant::keep_stamped());
    assert!(!proposed.is_assertable());
    assert_eq!(proposed.leaf_count(), 26);
    assert!(proposed.layout("the-debt-square").is_none());

    let ratified = proposed.ratify("D-0148");
    assert!(ratified.is_assertable());
    assert_eq!(ratified, Ruleset::keep_stamped());
}

// --- the honest-versus-rigged study -----------------------------------------

#[test]
fn honest_vs_rigged_yields_a_measurable_difference() {
    let report = honest_vs_rigged(20_260_720, 400);

    // The allegory as data: the Debt is reachable honestly, never under the rig.
    assert!(report.shows_the_difference());
    assert!(
        report.debt_appearances_honest > 0,
        "honest can reach the Debt"
    );
    assert_eq!(report.debt_appearances_rigged, 0, "the rig forecloses it");
    assert!(report.debt_square_honest);
    assert!(!report.debt_square_rigged, "the debt-square is foreclosed");

    // Both manifests are honest by the SIM-1 oracle (metrics tested, invariants
    // checked). The never-Debt invariant *holds* under the rig (an exact,
    // re-derivable zero) and is *breached* under the honest deck, with a witness.
    assert!(report.honest.grades_are_honest());
    assert!(report.rigged.grades_are_honest());
    assert!(
        report.rigged.invariants[0].held(),
        "rig never deals the Debt"
    );
    assert!(
        !report.honest.invariants[0].held(),
        "honest does deal the Debt"
    );
    assert_eq!(report.honest.metrics[0].sum, report.debt_appearances_honest);
}

#[test]
fn the_study_is_deterministic() {
    let a = honest_vs_rigged(7, 200);
    let b = honest_vs_rigged(7, 200);
    assert_eq!(a.honest.canonical_bytes(), b.honest.canonical_bytes());
    assert_eq!(a.rigged.canonical_bytes(), b.rigged.canonical_bytes());
    // A different seed is a real input: it moves the honest appearance count.
    let c = honest_vs_rigged(999_983, 200);
    assert_ne!(a.honest.canonical_bytes(), c.honest.canonical_bytes());
}

// --- determinism of the deal battery ----------------------------------------

fn whole_round_battery(seed: u64, trials: u64) -> TurningBattery {
    TurningBattery {
        ruleset: Ruleset::lowlands_honest(),
        layout: "the-whole-round".to_owned(),
        seed,
        trials,
        metrics: vec![ReadingMetric {
            name: "contradiction-rate".to_owned(),
            observable: ReadingObservable::ContradictionRate,
        }],
        invariants: vec![
            ReadingInvariant::DeckConserved,
            ReadingInvariant::OnlyLegalPlays,
        ],
    }
}

#[test]
fn same_seed_same_manifest_different_seed_differs() {
    let battery = whole_round_battery(20_260_720, 300);
    let first = battery.run();
    let second = battery.run();
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.digest(), second.digest());

    let other = whole_round_battery(1, 300).run();
    assert_ne!(first.canonical_bytes(), other.canonical_bytes());
}

#[test]
fn a_deal_is_a_pure_function_of_seed_and_index() {
    let battery = whole_round_battery(42, 0);
    for index in [0, 1, 7, 99] {
        assert_eq!(battery.deal(index), battery.deal(index));
    }
    // A different index draws a different reading (with overwhelming probability
    // for a multi-leaf layout); at minimum the two are independently reproducible.
    assert_eq!(battery.deal(3).laid, battery.deal(3).laid);
}

#[test]
fn every_dealt_reading_is_legal_and_conserves_the_deck() {
    // Over a full battery, the two structural reading-invariants hold exactly, so
    // both grade `checked` — no reading reaches an illegal state or a doubled leaf.
    let manifest = whole_round_battery(20_260_720, 500).run();
    assert!(manifest.grades_are_honest());
    for verdict in &manifest.invariants {
        assert!(verdict.held(), "{verdict:?}");
        assert_eq!(verdict.grade(), dli_core::Grade::Checked);
    }
}

// --- verdicts are counsel-shaped --------------------------------------------

#[test]
fn a_reading_verdict_is_counsel_never_a_winner() {
    let deck = Ruleset::lowlands_honest();
    let round = deck.layout("a-round").unwrap().clone();

    // A filled round laying a reciprocal Weather pair reads as a Contradiction.
    let contradiction = Reading {
        layout: round.name.clone(),
        leaves_dealt: 3,
        laid: vec![
            "weather-greening".to_owned(),
            "weather-rot".to_owned(),
            "hand-taker-wood".to_owned(),
        ],
    };
    assert_eq!(deck.verdict(&contradiction), Verdict::Contradiction);

    // A filled round without a reciprocal pair returns a better Question.
    let question = Reading {
        layout: round.name.clone(),
        leaves_dealt: 3,
        laid: vec![
            "hand-taker-wood".to_owned(),
            "weather-greening".to_owned(),
            "hand-keeper-water".to_owned(),
        ],
    };
    assert_eq!(deck.verdict(&question), Verdict::Question);

    // An unfilled reading is a missing premise, not a loss.
    let jammed = Reading {
        layout: round.name.clone(),
        leaves_dealt: 3,
        laid: vec!["hand-taker-wood".to_owned()],
    };
    assert_eq!(deck.verdict(&jammed), Verdict::MissingPremise);
}

// --- structural invariant enumeration ---------------------------------------

#[test]
fn structural_invariants_hold_on_the_shipped_decks() {
    for deck in [
        Ruleset::lowlands_honest(),
        Ruleset::keep_stamped(),
        Ruleset::salt_fringe(),
    ] {
        let verdicts = enumerate_structural_invariants(&deck);
        assert_eq!(verdicts.len(), StructuralInvariant::ALL.len());
        for verdict in verdicts {
            assert!(verdict.held(), "{}: {verdict:?}", deck.deck_id);
            assert_eq!(verdict.grade(), dli_core::Grade::Checked);
        }
    }
}

#[test]
fn structural_invariants_have_teeth() {
    // A hand-corrupted deck must be caught, so the enumeration is a real check and
    // not a rubber stamp. A doubled Ground both duplicates ids (DeckIsASet) and
    // makes the arithmetic count outrun the distinct wheel (MoonCount).
    let mut broken = Ruleset::lowlands_honest();
    broken.grounds.push(broken.grounds[0].clone());
    assert!(StructuralInvariant::DeckIsASet.breach(&broken).is_some());
    assert!(StructuralInvariant::MoonCount.breach(&broken).is_some());

    // A dangling reciprocal pointer — a half-pair with no partner — is caught.
    let mut dangling = Ruleset::lowlands_honest();
    dangling.weathers[0].reciprocal = Some("nowhere".to_owned());
    assert!(
        StructuralInvariant::ReciprocityClosed
            .breach(&dangling)
            .is_some()
    );
}

// --- the export-facing seam is a frozen, round-tripping artifact -------------

#[test]
fn the_export_fixture_round_trips_to_the_canonical_deck() {
    let canonical = Ruleset::lowlands_honest();
    let rendered = serde_json::to_string_pretty(&canonical).unwrap() + "\n";

    // Regenerate the committed fixture on demand (REGEN_TURNING_FIXTURE=1), else
    // assert the code still deserializes byte-for-byte from it.
    if std::env::var_os("REGEN_TURNING_FIXTURE").is_some() {
        std::fs::write(fixture_path(), &rendered).unwrap();
    }
    let committed = std::fs::read_to_string(fixture_path())
        .expect("worlds/tpth/turning/the-turning-rules.json is committed");
    let from_disk: Ruleset = serde_json::from_str(&committed).unwrap();
    assert_eq!(
        from_disk, canonical,
        "the fixture and the code have drifted"
    );
    assert_eq!(
        committed, rendered,
        "re-serializing the deck is byte-stable"
    );
}
