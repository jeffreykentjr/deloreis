#![allow(clippy::unwrap_used)]
//! The shipped standard 52-card exemplar, driven end to end through the generic
//! `dli-game` engine (design note `sim-genericization.md` §"The standard 52-card
//! exemplar", Step 3).
//!
//! This loads `examples/games/french-deck/{rules.json,profiles.json}` — a plain
//! standard deck, no world lore — as a generic `Ruleset` and exercises the engine's
//! *ranked/linear* path (`MoveRule::Ascending`): a straight-flush run of same-suit,
//! rank+1 cards. The Turning migration proves the *rankless cyclic* path
//! (`CyclicReach`); one engine spanning both is the genericity proof, so this test
//! is the linear half of that proof. No world binding is needed — `GameBindings`
//! is empty and the verdict is the generic `Completed`.

use dli_core::Grade;
use dli_sim::InvariantVerdict;

use dli_export::profile::{ExportOpts, ExportTarget, Registry};

use dli_game::battery::{
    DealBattery, DealCheckpoint, ReadingInvariant, ReadingMetric, ReadingObservable,
};
use dli_game::binding::GameBindings;
use dli_game::export::{self, follow_edge_count};
use dli_game::model::{
    AttrValue, Reading, Ruleset, Variant, Verdict, enumerate_structural_invariants,
};

/// The exemplar's rules, baked in at compile time from the shipped example — the
/// north star of `Ruleset::from_rules`: a whole game loads from data.
const FRENCH_DECK_RULES: &str = include_str!("../../../examples/games/french-deck/rules.json");

/// The exemplar's export profiles (a `cards` and a `ludii-gdl` profile), likewise
/// from the shipped example.
const FRENCH_DECK_PROFILES: &str =
    include_str!("../../../examples/games/french-deck/profiles.json");

/// The shipped piquet variant delta — the public-domain analogue of the Turning's
/// keep-stamped fork: cutting the low ranks forecloses a layout by the compile.
const FRENCH_DECK_PIQUET: &str =
    include_str!("../../../examples/games/french-deck/variants/piquet.json");

fn french_deck() -> Ruleset {
    Ruleset::from_rules(FRENCH_DECK_RULES).expect("the shipped french-deck rules parse")
}

fn french_deck_registry() -> Registry {
    serde_json::from_str::<Registry>(FRENCH_DECK_PROFILES)
        .expect("the shipped french-deck profiles parse as a Registry")
}

fn piquet_variant() -> Variant {
    serde_json::from_str(FRENCH_DECK_PIQUET).expect("the shipped piquet variant parses")
}

// -- the deck is exactly a standard 52-card deck -----------------------------

#[test]
fn enumerates_exactly_fifty_two_distinct_cards() {
    let deck = french_deck();
    assert_eq!(deck.cards().len(), 52, "4 suits x 13 ranks");
    assert_eq!(deck.card_count(), 52);

    let ids = deck.card_ids();
    assert_eq!(ids.len(), 52);
    assert!(ids.iter().any(|id| id == "ace-of-spades"));
    assert!(ids.iter().any(|id| id == "king-of-clubs"));
    // The template renders `{rank}-of-{suit}` for every card.
    assert!(ids.iter().all(|id| id.contains("-of-")));
}

// -- the piquet variant fork cuts to 32 cards and drops the full suit --------

#[test]
fn piquet_fork_cuts_to_thirty_two_cards_and_drops_the_full_suit() {
    let standard = french_deck();
    // The full 52-card deck can deal the full 13-card suit.
    assert_eq!(standard.cards().len(), 52);
    assert!(
        standard.layout("the-full-suit").is_some(),
        "the standard deck deals the full suit"
    );

    // Forking the piquet delta yields a *proposed* ruleset (a variant is a claim
    // awaiting a ruling) whose deck already reflects the cut.
    let piquet = standard.fork(&piquet_variant());
    assert!(!piquet.is_assertable(), "a forked variant is proposed");
    assert_eq!(piquet.game_id, "french-deck-piquet");

    // Cutting ranks 2..6 leaves 8 ranks x 4 suits = a 32-card piquet deck.
    assert_eq!(piquet.cards().len(), 32, "8 ranks x 4 suits");
    assert_eq!(piquet.card_count(), 32);
    assert!(
        piquet.card_ids().iter().all(|id| !id.starts_with("two-of-")
            && !id.starts_with("three-of-")
            && !id.starts_with("four-of-")
            && !id.starts_with("five-of-")
            && !id.starts_with("six-of-")),
        "the low ranks are gone"
    );
    assert!(piquet.card_ids().iter().any(|id| id == "seven-of-spades"));
    assert!(piquet.card_ids().iter().any(|id| id == "ace-of-clubs"));

    // The-full-suit reads `two-of-spades`, which the cut removed, so it falls out
    // of the compile — the public-domain analogue of the keep-stamped deck losing
    // the-debt-square (09-exports §6). The loss is a compile result, not a hand edit.
    assert!(
        piquet.layout("the-full-suit").is_none(),
        "the full suit is foreclosed by the cut"
    );
    assert!(
        piquet
            .available_layouts()
            .iter()
            .all(|layout| layout.name != "the-full-suit")
    );
    // A run is still dealable — it requires no cut card.
    assert!(piquet.layout("a-run").is_some());
}

// -- the ranked/linear straight-flush run relation ---------------------------

#[test]
fn ascending_run_is_same_suit_rank_plus_one() {
    let deck = french_deck();

    // Same suit, one rank higher: a legal run step.
    assert!(deck.follows("ace-of-spades", "two-of-spades"));
    assert!(deck.follows("ten-of-hearts", "jack-of-hearts"));
    assert!(deck.follows("queen-of-clubs", "king-of-clubs"));

    // Right rank step, wrong suit: not a run.
    assert!(!deck.follows("ace-of-spades", "two-of-hearts"));

    // Two ranks up, or descending, or itself: never a step.
    assert!(!deck.follows("ace-of-spades", "three-of-spades"));
    assert!(!deck.follows("three-of-spades", "two-of-spades"));
    assert!(!deck.follows("ace-of-spades", "ace-of-spades"));

    // The king is the top rank: nothing in the deck follows it.
    assert!(
        deck.card_ids()
            .iter()
            .all(|id| !deck.follows("king-of-spades", id)),
        "no card outranks the king within its suit"
    );
}

// -- legal moves, terminals, and the generic verdict over a run --------------

#[test]
fn legal_moves_and_terminal_and_completed_verdict() {
    let deck = french_deck();
    let empty = GameBindings::new();
    let layout = deck.layout("a-run").expect("a-run is dealable");

    // The opening play is free over all 52 dealt cards.
    let open = Reading::open(layout);
    assert_eq!(deck.legal_moves(&open).len(), 52);
    assert!(!deck.is_terminal(&open));

    // After an opening lay, the only legal move is the same-suit successor.
    let mut run = open.clone();
    run.laid.push("ace-of-spades".to_owned());
    assert_eq!(deck.legal_moves(&run), vec!["two-of-spades".to_owned()]);

    // Drive a full same-suit run; it fills the 5-card layout and completes.
    for id in [
        "two-of-spades",
        "three-of-spades",
        "four-of-spades",
        "five-of-spades",
    ] {
        assert!(!deck.is_terminal(&run));
        let moves = deck.legal_moves(&run);
        assert!(moves.contains(&id.to_owned()));
        run.laid.push(id.to_owned());
    }
    assert!(deck.is_terminal(&run));
    assert_eq!(deck.verdict(&run, &empty).id, Verdict::COMPLETED);

    // Opening near the top jams before the layout fills: incomplete.
    let mut jammed = Reading::open(layout);
    for id in [
        "ten-of-clubs",
        "jack-of-clubs",
        "queen-of-clubs",
        "king-of-clubs",
    ] {
        jammed.laid.push(id.to_owned());
    }
    assert!(
        deck.legal_moves(&jammed).is_empty(),
        "the king jams the run"
    );
    assert!(deck.is_terminal(&jammed));
    assert_eq!(deck.verdict(&jammed, &empty).id, Verdict::INCOMPLETE);
}

// -- the generic structural invariants ---------------------------------------

#[test]
fn deck_is_a_set_and_count_matches_hold() {
    let verdicts = enumerate_structural_invariants(&french_deck(), &GameBindings::new());
    assert_eq!(verdicts.len(), 2);
    assert!(verdicts.iter().all(InvariantVerdict::held));
    assert!(
        verdicts
            .iter()
            .any(|verdict| verdict.invariant == "deck-is-a-set")
    );
    assert!(
        verdicts
            .iter()
            .any(|verdict| verdict.invariant == "count-matches")
    );
}

// -- a seeded deal battery folds an honest manifest --------------------------

#[test]
fn deal_battery_folds_an_honest_deterministic_manifest() {
    let battery = DealBattery {
        ruleset: french_deck(),
        layout: "a-run".to_owned(),
        seed: 20_260_721,
        trials: 200,
        metrics: vec![
            ReadingMetric {
                name: "completed-rate".to_owned(),
                observable: ReadingObservable::VerdictIs {
                    verdict_id: Verdict::COMPLETED.to_owned(),
                },
            },
            ReadingMetric {
                name: "five-of-spades-appears".to_owned(),
                observable: ReadingObservable::CardAppears {
                    card: "five-of-spades".to_owned(),
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
    assert_eq!(manifest.trials, 200);
    // Rates grade `tested`; exact sweeps grade `checked`.
    assert_eq!(manifest.metrics[0].grade(), Grade::Tested);
    assert_eq!(manifest.invariants[0].grade(), Grade::Checked);
    // Conservation and legality always hold.
    assert!(manifest.invariants.iter().all(InvariantVerdict::held));
    // The completed rate is genuinely mixed: runs opening on ace..nine complete,
    // runs opening on ten..king jam.
    assert!(manifest.metrics[0].sum > 0 && manifest.metrics[0].sum < 200);
    // The five of spades falls in some runs (those opening ace..five of spades).
    assert!(manifest.metrics[1].sum > 0);

    // Same seed replays byte-for-byte.
    let again = battery.run(&GameBindings::new());
    assert_eq!(manifest, again);
}

#[test]
fn deal_checkpoint_resumes_to_the_same_manifest_and_rejects_forgery() {
    let battery = DealBattery {
        ruleset: french_deck(),
        layout: "a-run".to_owned(),
        seed: 7,
        trials: 9,
        metrics: vec![ReadingMetric {
            name: "completed-rate".to_owned(),
            observable: ReadingObservable::VerdictIs {
                verdict_id: Verdict::COMPLETED.to_owned(),
            },
        }],
        invariants: vec![
            ReadingInvariant::DeckConserved,
            ReadingInvariant::OnlyLegalPlays,
        ],
    };
    let mut checkpoint = DealCheckpoint::initial(&battery, 4);
    assert!(!checkpoint.advance_boundary(&battery).unwrap());
    assert_eq!(checkpoint.trials_done, 4);
    assert!(!checkpoint.advance_boundary(&battery).unwrap());
    assert!(checkpoint.advance_boundary(&battery).unwrap());
    assert_eq!(checkpoint.lineage, vec![4, 8, 9]);
    let mut straight = battery.run(&GameBindings::new());
    straight.checkpoint_lineage = vec![4, 8, 9];
    assert_eq!(checkpoint.finalize(&battery), straight);

    let mut forged = checkpoint.clone();
    forged.metrics[0].sum += 1;
    assert!(forged.validate_for(&battery).is_err());

    let mut malformed = checkpoint;
    malformed.lineage.clear();
    malformed.metrics[0].trials += 1;
    assert!(malformed.validate_persisted_shape_for(&battery).is_err());
}

// -- both exports render from the shipped profiles ---------------------------

#[test]
fn card_export_renders_every_card_from_the_profile() {
    let registry = french_deck_registry();
    let deck = french_deck();
    let export = export::run(
        &registry,
        ExportTarget::Cards,
        &deck,
        &ExportOpts::default(),
    )
    .expect("the cards export runs");
    let cards = export.artifact.as_cards().expect("a card deck");

    assert_eq!(cards.deck, "french-deck");
    assert_eq!(cards.profile, "cards/french-deck@1");
    assert_eq!(cards.card_count, 52);
    assert_eq!(cards.cards.len(), 52);

    // The render rule renders `{rank}-of-{suit}` and keeps rank/suit/name.
    let ace = cards
        .cards
        .iter()
        .find(|card| card.id == "ace-of-spades")
        .expect("the ace of spades renders");
    assert_eq!(ace.kept.get("rank"), Some(&AttrValue::text("ace")));
    assert_eq!(ace.kept.get("suit"), Some(&AttrValue::text("spades")));
    assert_eq!(ace.kept.get("name"), Some(&AttrValue::text("Ace")));
    assert!(cards.cards.iter().any(|card| card.id == "king-of-clubs"));

    // Both layouts compile (neither requires a cut card), and dev-metadata carries
    // the cited provenance.
    let mut layouts = cards.layouts.clone();
    layouts.sort();
    assert_eq!(
        layouts,
        vec!["a-run".to_owned(), "the-full-suit".to_owned()]
    );
    assert_eq!(
        cards.cited,
        Some(vec!["DL-0017".to_owned(), "DL-0018".to_owned()])
    );
}

#[test]
fn ludii_export_renders_components_and_run_edges() {
    let registry = french_deck_registry();
    let deck = french_deck();
    let export = export::run(
        &registry,
        ExportTarget::LudiiGdl,
        &deck,
        &ExportOpts::default(),
    )
    .expect("the ludii export runs");
    let ludii = export.artifact.as_ludii().expect("a ludii export");

    assert_eq!(ludii.profile, "games/french-deck-ludii@1");

    // 52 components, one per card.
    assert_eq!(ludii.program.matches("(component \"").count(), 52);
    for id in ["ace-of-spades", "king-of-clubs"] {
        assert!(ludii.program.contains(&format!("(component \"{id}\")")));
    }

    // The same-suit rank+1 run edges: 4 suits x 12 consecutive pairs = 48.
    assert!(
        ludii
            .program
            .contains("(move (from \"ace-of-spades\") (to \"two-of-spades\"))")
    );
    assert_eq!(follow_edge_count(&deck), 48);
}
