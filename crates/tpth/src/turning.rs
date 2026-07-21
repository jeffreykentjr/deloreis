//! The Turning binding (`SIM-2`): the world's oracular deck as **data loaded by
//! the generic [`dli_game`] engine plus a thin binding**.
//!
//! The whole structural deck — its axes (Grounds, Frith-roles, Weathers), its card
//! families, its cyclic legal-move relation, its layouts, its variant deltas, and
//! three of its four structural invariants — is **pure data**, authored in
//! `worlds/tpth/turning/the-turning-rules.json` and loaded with
//! [`Ruleset::from_rules`]. What genuinely cannot be data is the world's own
//! *verdict predicate* and its bespoke *reciprocity invariant*; those enter the
//! engine as the two named predicates in [`turning_bindings`]. The honest-versus-
//! rigged study ([`honest_vs_rigged`]) composes two generic [`DealBattery`]s over
//! the loaded honest and keep-stamped rulesets.
//!
//! The Turning is the world's oracular deck — game-first, oracle-late — ratified
//! whole in `D-0148` and woken from the world's own substrate by the tarot-remap
//! method (`D-0147`). A count from the world's own sky: four Grounds carrying
//! sixteen Hands (the four Frith-roles on each Ground), twelve Weathers (a closed
//! ring with no highest card), and the one Reckoner at the hub make **twenty-nine
//! leaves**, one for each night of the moon's turning.
//!
//! # Counsel, never prophecy
//!
//! A reading's verdict is counsel-shaped, never win-or-lose: the registered
//! `turning-counsel` predicate reveals a [`VERDICT_QUESTION`], a
//! [`VERDICT_CONTRADICTION`], or a [`VERDICT_MISSING_PREMISE`]. Foreclosure — the
//! keep-stamped rig cutting the hard Weathers so a frightened person can never reach
//! the debt-square — is a deck-level fact read from
//! [`Ruleset::available_layouts`], and the [`honest_vs_rigged`] study measures
//! exactly that difference.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_sim::manifest::RunManifest;

use dli_game::{DealBattery, GameBindings, ReadingInvariant, ReadingMetric, ReadingObservable};

pub use dli_game::model::{
    AttrValue, Axis, AxisValue, Card, Decoration, DeltaOp, Layer, Layout, MoveRule, Ordering,
    Provenance, Reading, Ruleset, Standing, StructuralInvariant, Variant, Verdict, VerdictRule,
    enumerate_structural_invariants,
};

/// The Turning deck as data — the north star fixture the whole binding loads.
pub const TURNING_RULES: &str = include_str!("../../../worlds/tpth/turning/the-turning-rules.json");

/// The keep-stamped variant delta as data (`D-0148`): cut the hard Weathers.
pub const KEEP_STAMPED_VARIANT: &str =
    include_str!("../../../worlds/tpth/turning/variants/keep-stamped.json");

/// The salt-fringe variant delta as data (`D-0148`): re-suit the Grounds.
pub const SALT_FRINGE_VARIANT: &str =
    include_str!("../../../worlds/tpth/turning/variants/salt-fringe.json");

// ---------------------------------------------------------------------------
// The registered verdict — counsel, never prophecy
// ---------------------------------------------------------------------------

/// The counsel verdict for a complete, honest reading: it returns a better
/// question.
pub const VERDICT_QUESTION: &str = "question";
/// The counsel verdict for a spread that laid a reciprocal Weather pair — the turn
/// shows both directions at once, a contradiction to walk.
pub const VERDICT_CONTRADICTION: &str = "contradiction";
/// The counsel verdict for a wheel that jammed before the layout filled — a premise
/// is missing, the reading cannot yet counsel.
pub const VERDICT_MISSING_PREMISE: &str = "missing-premise";

/// The name the ruleset's [`VerdictRule::Registered`] looks the counsel predicate
/// up by.
pub const COUNSEL: &str = "turning-counsel";
/// The name the ruleset's [`StructuralInvariant::Registered`] looks the reciprocity
/// predicate up by.
pub const RECIPROCITY_CLOSED: &str = "reciprocity-closed";

/// The `turning-counsel` verdict predicate: a filled reading laying a reciprocal
/// Weather pair reads as a [`VERDICT_CONTRADICTION`], else a [`VERDICT_QUESTION`];
/// an unfilled (jammed) reading is a [`VERDICT_MISSING_PREMISE`]. It never returns a
/// fixed future and never a winner.
#[must_use]
fn turning_counsel(ruleset: &Ruleset, reading: &Reading) -> Verdict {
    if !reading.is_filled() {
        return Verdict::new(VERDICT_MISSING_PREMISE);
    }
    if holds_reciprocal_pair(ruleset, reading) {
        Verdict::new(VERDICT_CONTRADICTION)
    } else {
        Verdict::new(VERDICT_QUESTION)
    }
}

/// Whether a reading laid both halves of some reciprocal Weather pair. Reads the
/// `reciprocal` attr each Weather value carries in the loaded ruleset data.
fn holds_reciprocal_pair(ruleset: &Ruleset, reading: &Reading) -> bool {
    let Some(weather) = ruleset.axis("weather") else {
        return false;
    };
    weather.values.iter().any(|value| {
        value
            .attrs
            .get("reciprocal")
            .and_then(AttrValue::as_text)
            .is_some_and(|partner| {
                reading.holds(&format!("weather-{}", value.id))
                    && reading.holds(&format!("weather-{partner}"))
            })
    })
}

/// The `reciprocity-closed` structural-invariant predicate: every reciprocal
/// Weather present must have its partner present too. Returns a breach reason if a
/// present Weather keeps a `reciprocal` pointing at a value the deck no longer
/// enumerates, else [`None`].
#[must_use]
fn reciprocity_closed(ruleset: &Ruleset) -> Option<String> {
    let weather = ruleset.axis("weather")?;
    let present: BTreeSet<&str> = weather
        .values
        .iter()
        .map(|value| value.id.as_str())
        .collect();
    weather.values.iter().find_map(|value| {
        value
            .attrs
            .get("reciprocal")
            .and_then(AttrValue::as_text)
            .and_then(|partner| {
                (!present.contains(partner)).then(|| {
                    format!(
                        "the Weather '{}' keeps its reciprocal '{partner}' but it was cut",
                        value.id
                    )
                })
            })
    })
}

/// The Turning's game bindings: the two named predicates the engine cannot derive
/// from data — the counsel verdict and the reciprocity invariant. Every consumer
/// that judges a Turning reading or sweeps its structural invariants passes this.
#[must_use]
pub fn turning_bindings() -> GameBindings {
    GameBindings::new()
        .with_verdict(COUNSEL, turning_counsel)
        .with_invariant(RECIPROCITY_CLOSED, reciprocity_closed)
}

// ---------------------------------------------------------------------------
// The ruleset constructors — load data, apply the variant, ratify
// ---------------------------------------------------------------------------

/// The `D-0147` remap-method touchstone: the full lowlands anatomy loaded from
/// data, but **only proposed** — structurally unassertable
/// ([`Ruleset::is_assertable`] is `false`) until an authorial ratification. This is
/// the fork end of the `D-0147`→`D-0148` fork/ratify replay.
#[must_use]
pub fn remap_touchstone() -> Ruleset {
    let mut deck = load(TURNING_RULES, "the-turning-rules.json");
    deck.provenance = Provenance::proposed("D-0147");
    deck
}

/// The canonical lowlands honest deck, ratified in `D-0148` — the deck woken by the
/// `D-0147` remap touchstone and then ratified. Loaded whole from the ratified data
/// fixture.
#[must_use]
pub fn the_turning() -> Ruleset {
    load(TURNING_RULES, "the-turning-rules.json")
}

/// The keep-stamped deck: the honest deck under the `D-0148` keep-stamped variant,
/// which cuts the hard Weathers. A tool for the mind inverted into a cage — every
/// reading soothes, and the debt-square can no longer be dealt.
#[must_use]
pub fn keep_stamped() -> Ruleset {
    the_turning().fork(&keep_stamped_variant()).ratify("D-0148")
}

/// The salt-fringe regional deck: the honest deck under the `D-0148` salt-fringe
/// variant, which re-suits the Grounds (the Tide and the Catch) and re-faces the
/// Mender as the Net-mender. Still twenty-nine leaves — the moon-count holds across
/// regions.
#[must_use]
pub fn salt_fringe() -> Ruleset {
    the_turning().fork(&salt_fringe_variant()).ratify("D-0148")
}

/// The keep-stamped variant delta, loaded from data.
#[must_use]
pub fn keep_stamped_variant() -> Variant {
    load_variant(KEEP_STAMPED_VARIANT, "variants/keep-stamped.json")
}

/// The salt-fringe variant delta, loaded from data.
#[must_use]
pub fn salt_fringe_variant() -> Variant {
    load_variant(SALT_FRINGE_VARIANT, "variants/salt-fringe.json")
}

fn load(json: &str, what: &str) -> Ruleset {
    Ruleset::from_rules(json).unwrap_or_else(|err| panic!("{what} parses as a ruleset: {err}"))
}

fn load_variant(json: &str, what: &str) -> Variant {
    serde_json::from_str(json).unwrap_or_else(|err| panic!("{what} parses as a variant: {err}"))
}

// ---------------------------------------------------------------------------
// The honest-versus-rigged study
// ---------------------------------------------------------------------------

/// The measurable difference between the honest deck and its keep-stamped rig — the
/// allegory shipped as data. Produced by [`honest_vs_rigged`].
///
/// The load-bearing claims are exact: under the rig the Debt leaf appears in
/// **zero** readings (an invariant that *holds*, `checked`) while under the honest
/// deck it appears (the same invariant *breached*, with a witness), and the
/// debt-square is available honest, foreclosed rigged. The appearance *rate* is a
/// `tested` estimate riding alongside.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StudyReport {
    /// The honest deck's run manifest.
    pub honest: RunManifest,
    /// The keep-stamped deck's run manifest.
    pub rigged: RunManifest,
    /// The number of honest readings in which the Debt appeared (a positive count —
    /// the deck can reach the question).
    pub debt_appearances_honest: i64,
    /// The number of rigged readings in which the Debt appeared — exactly zero.
    pub debt_appearances_rigged: i64,
    /// Whether the debt-square is dealable on the honest deck (true).
    pub debt_square_honest: bool,
    /// Whether the debt-square is dealable on the rigged deck (false — foreclosed).
    pub debt_square_rigged: bool,
}

impl StudyReport {
    /// Whether the study shows the allegory: the Debt reachable honestly and never
    /// under the rig, and the debt-square available honestly and foreclosed under
    /// the rig. The whole point of the study, as one predicate.
    #[must_use]
    pub fn shows_the_difference(&self) -> bool {
        self.debt_appearances_honest > 0
            && self.debt_appearances_rigged == 0
            && self.debt_square_honest
            && !self.debt_square_rigged
    }
}

/// Run the honest-versus-rigged study: deal the same seeded battery of everyday
/// rounds from the honest lowlands deck and from its keep-stamped rig, and report
/// the measurable difference. Two generic [`DealBattery`]s over the two loaded
/// rulesets — the rig discovered as data, never announced.
///
/// ```
/// use tpth::turning::honest_vs_rigged;
///
/// let report = honest_vs_rigged(20_260_720, 400);
/// assert!(report.shows_the_difference());
/// assert_eq!(report.debt_appearances_rigged, 0); // the licensed deck cannot deal it
/// assert!(report.debt_appearances_honest > 0);    // the honest deck can
/// assert!(report.honest.grades_are_honest());
/// ```
#[must_use]
pub fn honest_vs_rigged(seed: u64, trials: u64) -> StudyReport {
    let debt = ReadingMetric {
        name: "debt-appearance-rate".to_owned(),
        observable: ReadingObservable::CardAppears {
            card: "weather-debt".to_owned(),
        },
    };
    let never_debt = ReadingInvariant::CardNeverAppears {
        card: "weather-debt".to_owned(),
    };
    let bindings = turning_bindings();

    let battery = |ruleset: Ruleset| DealBattery {
        ruleset,
        layout: "a-round".to_owned(),
        seed,
        trials,
        metrics: vec![debt.clone()],
        invariants: vec![never_debt.clone(), ReadingInvariant::DeckConserved],
    };

    let honest_deck = the_turning();
    let rigged_deck = keep_stamped();
    let honest = battery(honest_deck.clone()).run(&bindings);
    let rigged = battery(rigged_deck.clone()).run(&bindings);

    StudyReport {
        debt_appearances_honest: honest.metrics[0].sum,
        debt_appearances_rigged: rigged.metrics[0].sum,
        debt_square_honest: honest_deck.layout("the-debt-square").is_some(),
        debt_square_rigged: rigged_deck.layout("the-debt-square").is_some(),
        honest,
        rigged,
    }
}
