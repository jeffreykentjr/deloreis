//! The Turning engine (`SIM-2`): deck structure, legal-move relations,
//! terminal conditions, variants-as-deltas, invariant enumeration, and the
//! honest-versus-rigged study. Exports soundly to Ludii L-GDL (`DL-0007`).
//!
//! The Turning is the world's oracular deck — game-first, oracle-late — ratified
//! whole in `D-0148` and woken from the world's own substrate by the tarot-remap
//! method (`D-0147`). This module models it as a **typed ruleset that is data,
//! not a simulator**: the [`Ruleset`] is a plain description of leaves, a
//! legal-move relation, terminal conditions, and variant deltas, with no run
//! state entangled in it. Three consumers read that one structure —
//!
//! - the **reading game** ([`Ruleset::legal_moves`], [`Ruleset::is_terminal`],
//!   [`Ruleset::verdict`]) enumerates what plays follow what and how a reading
//!   closes;
//! - the **deal battery** ([`TurningBattery`]) simulates seeded readings through
//!   SIM-1's seams — [`SplitMix64`] for the draws,
//!   [`RunManifest`] for the honest record — so the
//!   Turning's structural invariants grade `checked` and its rates grade
//!   `tested`, by the same oracle SIM-1 enforces;
//! - the **card/GDL export** (`EXP-3`) compiles the same [`Ruleset`] to a card
//!   set and Ludii L-GDL. Because the ruleset holds no simulator state, that
//!   compile is a straight structural render — the `DL-0007` triviality bar: *if
//!   the export is not trivial, the rules were entangled with the kernel.*
//!
//! # The deck (`D-0148`)
//!
//! A count from the world's own sky: four **Grounds** (the commons-domains, the
//! suits) carrying sixteen **Hands** (the four Frith-roles — Taker, Keeper,
//! Giver, Mender — on each Ground), twelve **Weathers** (the Wyrd's cyclical
//! conditions, a closed ring with no highest card), and the one **Reckoner** (the
//! querent's own standpoint at the hub) make **twenty-nine leaves**, one for each
//! night of the moon's turning. The wheel is never a ladder: no leaf outranks
//! another, so a reading asks where on the turn a thing stands and which way it
//! moves ([`Ruleset::follows`]), never what conquers what.
//!
//! # Counsel, never prophecy
//!
//! A reading's [`Verdict`] is counsel-shaped, never win-or-lose: it reveals a
//! [`Verdict::Question`], a [`Verdict::Contradiction`], a
//! [`Verdict::MissingPremise`], or — on a keep-stamped deck — a
//! [`Verdict::Foreclosed`] question the licensed deck cannot deal. That last one
//! is the allegory as data: the keep-stamped variant ([`Variant::keep_stamped`])
//! cuts the hard Weathers so a frightened person can never reach the debt-square,
//! and the [`honest_vs_rigged`] study measures exactly that difference.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::manifest::{
    InvariantOutcome, InvariantVerdict, MetricEstimate, RunManifest, digest_json,
};
use crate::rng::{SplitMix64, stream_value};

// ---------------------------------------------------------------------------
// Provenance — the fork/ratify standing (mirrors `dli_ast::annot::ProvenanceMarker`)
// ---------------------------------------------------------------------------

/// A ruleset's standing on the ratification path.
///
/// This mirrors the world layer's provenance vocabulary
/// (`dli_ast::annot::ProvenanceMarker`) at the granularity a ruleset needs: a
/// proposed ruleset is structurally *unassertable* (the same status-gate the lore
/// engine applies to proposed lore), and only an authorial ratification lifts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// An agent's or method's proposal, awaiting a ruling — not yet assertable.
    Proposed,
    /// A provisional ruling, assertable but open to revision.
    Provisional,
    /// A ratified ruling — assertable canon.
    Ratified,
}

impl Standing {
    /// Whether a claim at this standing may be asserted (everything but
    /// [`Proposed`](Standing::Proposed), which is shadow-only).
    #[must_use]
    pub const fn assertable(self) -> bool {
        !matches!(self, Self::Proposed)
    }
}

/// Where a ruleset (or a variant delta) came from: a standing and the decisions
/// it cites, in citation order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    /// The standing on the ratification path.
    pub standing: Standing,
    /// The decision ids cited, e.g. `["D-0147", "D-0148"]`, in the order added.
    pub cites: Vec<String>,
}

impl Provenance {
    /// A proposed provenance citing one decision (the remap touchstone, `D-0147`).
    #[must_use]
    pub fn proposed(cite: &str) -> Self {
        Self {
            standing: Standing::Proposed,
            cites: vec![cite.to_owned()],
        }
    }

    /// Ratify: lift the standing to [`Ratified`](Standing::Ratified) and record
    /// the ratifying decision, keeping the earlier citations (a ratified ruleset
    /// still cites the touchstone it was woken from).
    #[must_use]
    pub fn ratified_by(mut self, decision: &str) -> Self {
        self.standing = Standing::Ratified;
        if !self.cites.iter().any(|c| c == decision) {
            self.cites.push(decision.to_owned());
        }
        self
    }
}

// ---------------------------------------------------------------------------
// Dimensions — Grounds, Hands, Weathers, Figures
// ---------------------------------------------------------------------------

/// A **Ground**: a commons-domain where taking and giving happen, and the suit a
/// reading is *about*. The lowlands deck keeps four (Wood, Water, Field, Hearth);
/// the set is region-variable canon, so a salt-fringe deck reckons in the Tide
/// and the Catch instead ([`Variant::salt_fringe`]).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct Ground {
    /// The stable id, e.g. `hearth` (the leaf id is `ground-hearth`).
    pub id: String,
    /// The display name, e.g. `The Hearth`.
    pub name: String,
}

impl Ground {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
        }
    }
}

/// A **Hand**: one of the four working roles of the Frith-cycle, the agent acting
/// on a Ground. These are moments of the law, not social ranks — the court cards
/// themselves teach the round of take, keep, give back, mend.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum HandRole {
    /// Draws from the commons.
    Taker,
    /// Holds the saved thing.
    Keeper,
    /// Returns it.
    Giver,
    /// Repairs the wear.
    Mender,
}

impl HandRole {
    /// The four Hands, in the canonical Frith-cycle order (take, keep, give, mend).
    pub const ALL: [HandRole; 4] = [Self::Taker, Self::Keeper, Self::Giver, Self::Mender];

    /// The lowercase id token, e.g. `mender` (a Hand leaf id is `hand-mender-field`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Taker => "taker",
            Self::Keeper => "keeper",
            Self::Giver => "giver",
            Self::Mender => "mender",
        }
    }
}

/// Whether a Weather is one of a reciprocal pair or a lone passage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WeatherKind {
    /// One of a reciprocal pair (Greening/Rot, Flood/Dearth, …).
    Reciprocal,
    /// A lone passage (the Road, the Bourn, the Debt, the Fallow).
    Passage,
}

/// A **Weather**: one of the Wyrd's cyclical conditions, the not-fate weather that
/// falls on any Ground. The twelve form a closed ring — the Rot leads back round
/// to the Greening — with no highest card, so a reading never asks what conquers
/// what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Weather {
    /// The stable id, e.g. `debt` (the leaf id is `weather-debt`).
    pub id: String,
    /// The display name, e.g. `The Debt`.
    pub name: String,
    /// Reciprocal or passage.
    pub kind: WeatherKind,
    /// The next Weather on the closed weather-ring (`debt`'s is `fallow`;
    /// `fallow`'s wraps to `greening`). Distinct from the leaf(29) wheel the
    /// legal-move relation turns on — this is the twelve-Weather ring alone.
    pub ring_next: String,
    /// The reciprocal partner id, for a [`Reciprocal`](WeatherKind::Reciprocal)
    /// Weather (`greening`↔`rot`); [`None`] for a passage.
    pub reciprocal: Option<String>,
    /// Whether the keep-stamped variant cuts this Weather (the hard Weathers —
    /// the Debt, the Sundering, the Bourn — that a licensed deck must not deal).
    pub hard: bool,
}

/// Where a folk-figure sits on the grid: it rides a Hand-role across every
/// Ground, or a single Weather.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "seat", rename_all = "kebab-case")]
pub enum FigureSeat {
    /// The figure wears this Hand-role on every Ground (the Pleacher is the
    /// lowlands Mender, on the Wood and the Water and the Field and the Hearth).
    Hand {
        /// The Hand-role worn.
        role: HandRole,
    },
    /// The figure rides this one Weather (the Keener rides the Sundering).
    Weather {
        /// The Weather ridden, by id.
        weather: String,
    },
}

/// A dormant folk-figure woken onto the grid, carrying a Frith-load. Figures drift
/// by region while the load holds; a cell (a Hand on a Ground under a Weather) is a
/// casting call, and so the deck is the writers'-room situation-grammar (`D-0147`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Figure {
    /// The stable id, e.g. `the-pleacher`.
    pub id: String,
    /// The seat it rides.
    pub seat: FigureSeat,
}

impl Figure {
    fn on_hand(id: &str, role: HandRole) -> Self {
        Self {
            id: id.to_owned(),
            seat: FigureSeat::Hand { role },
        }
    }

    fn on_weather(id: &str, weather: &str) -> Self {
        Self {
            id: id.to_owned(),
            seat: FigureSeat::Weather {
                weather: weather.to_owned(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Leaves — the twenty-nine
// ---------------------------------------------------------------------------

/// The layer a leaf belongs to — the export's `layer` field.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    /// A Hand-on-Ground leaf.
    Hand,
    /// A Weather leaf.
    Weather,
    /// The Reckoner, the hub.
    Reckoner,
}

/// A **leaf**: a holdable premise, one of the deck's twenty-nine. A Hand leaf
/// fixes both its Frith-role and its Ground; a Weather leaf names a condition; the
/// Reckoner is the querent's own standpoint and is never dealt.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "layer", rename_all = "kebab-case")]
pub enum Leaf {
    /// The `role` Hand acting on the `ground` (both by id).
    Hand {
        /// The Frith-role.
        role: HandRole,
        /// The Ground it acts on, by id.
        ground: String,
    },
    /// The named Weather, by id.
    Weather {
        /// The Weather id.
        weather: String,
    },
    /// The Reckoner — the hub, the still centre that must choose which way to turn.
    Reckoner,
}

impl Leaf {
    /// The stable export id: `hand-<role>-<ground>`, `weather-<w>`, or `reckoner`.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::Hand { role, ground } => format!("hand-{}-{ground}", role.id()),
            Self::Weather { weather } => format!("weather-{weather}"),
            Self::Reckoner => "reckoner".to_owned(),
        }
    }

    /// The layer this leaf belongs to.
    #[must_use]
    pub const fn layer(&self) -> Layer {
        match self {
            Self::Hand { .. } => Layer::Hand,
            Self::Weather { .. } => Layer::Weather,
            Self::Reckoner => Layer::Reckoner,
        }
    }

    /// The Weather id this leaf names, if it is a Weather leaf.
    #[must_use]
    pub fn weather_id(&self) -> Option<&str> {
        match self {
            Self::Weather { weather } => Some(weather),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Layouts — the shapes of an argument
// ---------------------------------------------------------------------------

/// A **layout**: the shape of an argument, laid around the Reckoner. A layout
/// draws a fixed number of leaves; some layouts *require* a leaf be dealable — the
/// debt-square needs the Debt, so a keep-stamped deck that cut the Debt can no
/// longer compile it ([`Ruleset::available_layouts`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Layout {
    /// The layout name, e.g. `the-debt-square`.
    pub name: String,
    /// How many leaves it deals from the deck (the Reckoner, the hub, is never
    /// among them).
    pub leaves_dealt: u32,
    /// Weather ids the deck must still contain for this layout to be dealable.
    /// The debt-square requires `debt`; an empty list is an always-available
    /// layout.
    pub requires_weathers: Vec<String>,
}

impl Layout {
    fn new(name: &str, leaves_dealt: u32, requires: &[&str]) -> Self {
        Self {
            name: name.to_owned(),
            leaves_dealt,
            requires_weathers: requires.iter().map(|w| (*w).to_owned()).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// The ruleset — rules as data
// ---------------------------------------------------------------------------

/// The **Turning ruleset**: the deck and its rules, entirely as data. Holds no run
/// state, so the export lane renders it structurally and the deal battery reads it
/// without mutating it.
///
/// Construct the canonical lowlands deck with [`Ruleset::lowlands_honest`], the
/// keep-stamped rig with [`Ruleset::keep_stamped`], and the salt-fringe regional
/// deck with [`Ruleset::salt_fringe`]. Every constructor threads the same
/// [`Provenance`] the ratification path assigns.
///
/// ```
/// use dli_sim::turning::Ruleset;
///
/// let deck = Ruleset::lowlands_honest();
/// // A count from the world's own sky: 16 Hands + 12 Weathers + 1 Reckoner.
/// assert_eq!(deck.leaf_count(), 29);
/// assert!(deck.is_assertable()); // ratified in D-0148
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ruleset {
    /// The deck id, e.g. `turning-lowlands-honest`.
    pub deck_id: String,
    /// The region this deck serves, e.g. `lowlands`.
    pub region: String,
    /// The Grounds (suits), in canonical order.
    pub grounds: Vec<Ground>,
    /// The Weathers, in the closed weather-ring order.
    pub weathers: Vec<Weather>,
    /// The folk-figures woken onto the grid.
    pub figures: Vec<Figure>,
    /// The layouts the deck knows.
    pub layouts: Vec<Layout>,
    /// The follow-relation reach: a play may turn the leaf(29) wheel this many
    /// steps forward. A ratifiable parameter of the ruleset.
    pub reach: u32,
    /// Where this ruleset stands on the ratification path.
    pub provenance: Provenance,
}

impl Ruleset {
    /// The canonical lowlands honest deck, ratified in `D-0148` — the deck woken
    /// by the `D-0147` remap touchstone and then ratified. Defining it *as* that
    /// path is what makes the [fork/ratify replay](Ruleset::ratify) literal.
    #[must_use]
    pub fn lowlands_honest() -> Self {
        Self::remap_touchstone().ratify("D-0148")
    }

    /// The `D-0147` remap-method touchstone: the full lowlands anatomy, woken from
    /// the world's substrate but **only proposed** — structurally unassertable
    /// ([`is_assertable`](Ruleset::is_assertable) is `false`) until an authorial
    /// ratification. This is the fork end of the fork/ratify replay.
    #[must_use]
    pub fn remap_touchstone() -> Self {
        Self {
            deck_id: "turning-lowlands-honest".to_owned(),
            region: "lowlands".to_owned(),
            grounds: lowlands_grounds(),
            weathers: canonical_weathers(),
            figures: lowlands_figures(),
            layouts: canonical_layouts(),
            reach: 6,
            provenance: Provenance::proposed("D-0147"),
        }
    }

    /// Ratify this ruleset under a decision: lift its standing and record the
    /// decision, keeping its earlier citations. This is the ratify end of the
    /// replay — a proposed touchstone becomes ratified, assertable canon.
    #[must_use]
    pub fn ratify(mut self, decision: &str) -> Self {
        self.provenance = self.provenance.ratified_by(decision);
        self
    }

    /// Whether this ruleset may be asserted — false while it is merely
    /// [`Proposed`](Standing::Proposed).
    #[must_use]
    pub const fn is_assertable(&self) -> bool {
        self.provenance.standing.assertable()
    }

    /// The keep-stamped deck: the lowlands deck under the `D-0148`
    /// [keep-stamped variant](Variant::keep_stamped), which cuts the hard
    /// Weathers. A tool for the mind inverted into a cage — every reading soothes,
    /// and the debt-square can no longer be dealt.
    #[must_use]
    pub fn keep_stamped() -> Self {
        Self::lowlands_honest()
            .fork(&Variant::keep_stamped())
            .ratify("D-0148")
    }

    /// The salt-fringe regional deck: the lowlands deck under the
    /// [salt-fringe variant](Variant::salt_fringe), which swaps Grounds (the Tide
    /// and the Catch) and re-faces the Mender as the Net-mender. Still twenty-nine
    /// leaves — the moon-count holds across regions.
    #[must_use]
    pub fn salt_fringe() -> Self {
        Self::lowlands_honest()
            .fork(&Variant::salt_fringe())
            .ratify("D-0148")
    }

    /// The Hand-roles, always the four of the Frith-cycle.
    #[must_use]
    pub const fn hand_roles(&self) -> [HandRole; 4] {
        HandRole::ALL
    }

    /// The deck's leaves, in canonical leaf(29) wheel order: each Ground's four
    /// Hands, then the Weathers in ring order, then the Reckoner. This *is* the
    /// wheel the legal-move relation turns on ([`follows`](Ruleset::follows)).
    #[must_use]
    pub fn leaves(&self) -> Vec<Leaf> {
        let mut leaves = Vec::new();
        for ground in &self.grounds {
            for role in HandRole::ALL {
                leaves.push(Leaf::Hand {
                    role,
                    ground: ground.id.clone(),
                });
            }
        }
        for weather in &self.weathers {
            leaves.push(Leaf::Weather {
                weather: weather.id.clone(),
            });
        }
        leaves.push(Leaf::Reckoner);
        leaves
    }

    /// The number of leaves — the moon-count. Twenty-nine for the honest lowlands
    /// deck; twenty-six under the keep-stamped delta.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.grounds.len() * HandRole::ALL.len() + self.weathers.len() + 1
    }

    /// The figure that rides a given leaf, if any: a Hand leaf takes its role's
    /// figure, a Weather leaf its own.
    #[must_use]
    pub fn figure_for(&self, leaf: &Leaf) -> Option<&Figure> {
        self.figures.iter().find(|fig| match (&fig.seat, leaf) {
            (FigureSeat::Hand { role }, Leaf::Hand { role: r, .. }) => role == r,
            (FigureSeat::Weather { weather }, Leaf::Weather { weather: w }) => weather == w,
            _ => false,
        })
    }

    /// The layouts this deck can actually deal — those whose required Weathers the
    /// deck still contains. The debt-square drops out of a keep-stamped deck here,
    /// as a compile result rather than a hand-maintained list.
    #[must_use]
    pub fn available_layouts(&self) -> Vec<&Layout> {
        let present: BTreeSet<&str> = self.weathers.iter().map(|w| w.id.as_str()).collect();
        self.layouts
            .iter()
            .filter(|layout| {
                layout
                    .requires_weathers
                    .iter()
                    .all(|w| present.contains(w.as_str()))
            })
            .collect()
    }

    /// A named layout, if the deck knows it *and can deal it* (its requirements
    /// hold). [`None`] both when the layout is unknown and when it is foreclosed.
    #[must_use]
    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.available_layouts()
            .into_iter()
            .find(|layout| layout.name == name)
    }
}

// ---------------------------------------------------------------------------
// The legal-move relation and the wheel
// ---------------------------------------------------------------------------

impl Ruleset {
    /// The leaf ids in wheel order — the leaf(29) cyclic sort.
    fn wheel(&self) -> Vec<String> {
        self.leaves().iter().map(Leaf::id).collect()
    }

    /// Whether `next` legally follows `prev` on the wheel: it is within
    /// [`reach`](Ruleset::reach) steps *forward* (clockwise) of `prev`, and not
    /// `prev` itself. The relation is cyclic — the wheel has no highest card — and
    /// a pure function of the two leaves, so the export renders it as static
    /// successor edges.
    ///
    /// ```
    /// use dli_sim::turning::Ruleset;
    ///
    /// let deck = Ruleset::lowlands_honest();
    /// let leaves = deck.leaves();
    /// let a = leaves[0].id();
    /// let b = leaves[1].id();
    /// assert!(deck.follows(&a, &b));   // adjacent forward
    /// assert!(!deck.follows(&a, &a));  // never itself
    /// ```
    #[must_use]
    pub fn follows(&self, prev: &str, next: &str) -> bool {
        let wheel = self.wheel();
        let n = wheel.len();
        let (Some(i), Some(j)) = (
            wheel.iter().position(|id| id == prev),
            wheel.iter().position(|id| id == next),
        ) else {
            return false;
        };
        if n == 0 {
            return false;
        }
        let forward = (j + n - i) % n;
        forward >= 1 && forward <= usize::try_from(self.reach).unwrap_or(usize::MAX)
    }
}

// ---------------------------------------------------------------------------
// Readings — a laid spread, its legal moves, and its verdict
// ---------------------------------------------------------------------------

/// A **reading in progress**: the leaves laid so far, in the order laid, under a
/// chosen layout. The Reckoner is always the implicit hub and is never in `laid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Reading {
    /// The layout being laid, by name.
    pub layout: String,
    /// How many leaves the layout deals (carried so a reading reads without the
    /// ruleset).
    pub leaves_dealt: u32,
    /// The leaf ids laid so far, in order.
    pub laid: Vec<String>,
}

impl Reading {
    /// A fresh, empty reading of a layout.
    #[must_use]
    pub fn open(layout: &Layout) -> Self {
        Self {
            layout: layout.name.clone(),
            leaves_dealt: layout.leaves_dealt,
            laid: Vec::new(),
        }
    }

    /// Whether a leaf has already been laid (deck conservation: each leaf at most
    /// once).
    #[must_use]
    pub fn holds(&self, leaf_id: &str) -> bool {
        self.laid.iter().any(|id| id == leaf_id)
    }
}

/// A **reading's verdict** — counsel, never prophecy. The deck reveals a better
/// question, a contradiction to resolve, a missing premise, or (on a rigged deck)
/// a question it structurally cannot deal. It never returns a fixed future and
/// never a winner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// A complete, honest reading: it returns a better question.
    Question,
    /// The spread laid a reciprocal Weather pair on the wheel — the turn shows
    /// both directions at once, a contradiction to walk.
    Contradiction,
    /// The wheel jammed before the layout filled: a premise is missing, the
    /// reading cannot yet counsel.
    MissingPremise,
    /// The layout the querent needs cannot be dealt — the licensed deck cut the
    /// leaves it reads. The keep-stamped rig, felt as wrongness.
    Foreclosed,
}

impl Ruleset {
    /// The legal plays from a reading's current state, in wheel order.
    ///
    /// The opening play is free (any leaf may open a reading); every later play
    /// must [`follow`](Ruleset::follows) the last-laid leaf on the wheel and must
    /// not repeat a laid leaf (deck conservation). The Reckoner is the hub and is
    /// never a play.
    #[must_use]
    pub fn legal_moves(&self, reading: &Reading) -> Vec<String> {
        let playable = self
            .leaves()
            .into_iter()
            .map(|leaf| leaf.id())
            .filter(|id| id != "reckoner");
        match reading.laid.last() {
            None => playable.filter(|id| !reading.holds(id)).collect(),
            Some(last) => playable
                .filter(|id| !reading.holds(id) && self.follows(last, id))
                .collect(),
        }
    }

    /// Whether a reading has closed: the layout is filled, or the wheel jammed
    /// (no legal move remains). Every reading terminates — each play consumes a
    /// leaf from a finite deck — so there is no non-terminating reading.
    #[must_use]
    pub fn is_terminal(&self, reading: &Reading) -> bool {
        let filled = u32::try_from(reading.laid.len()).unwrap_or(u32::MAX) >= reading.leaves_dealt;
        filled || self.legal_moves(reading).is_empty()
    }

    /// The verdict of a reading. A filled reading returns a
    /// [`Contradiction`](Verdict::Contradiction) if it laid a reciprocal Weather
    /// pair, else a [`Question`](Verdict::Question); an unfilled (jammed) reading
    /// returns [`MissingPremise`](Verdict::MissingPremise). Foreclosure is a
    /// deck-level fact ([`available_layouts`](Ruleset::available_layouts)), not a
    /// laid-reading outcome.
    #[must_use]
    pub fn verdict(&self, reading: &Reading) -> Verdict {
        let filled = u32::try_from(reading.laid.len()).unwrap_or(u32::MAX) >= reading.leaves_dealt;
        if !filled {
            return Verdict::MissingPremise;
        }
        if self.holds_reciprocal_pair(reading) {
            Verdict::Contradiction
        } else {
            Verdict::Question
        }
    }

    /// Whether a reading laid both halves of some reciprocal Weather pair.
    fn holds_reciprocal_pair(&self, reading: &Reading) -> bool {
        self.weathers.iter().any(|w| {
            w.reciprocal.as_ref().is_some_and(|partner| {
                reading.holds(&format!("weather-{}", w.id))
                    && reading.holds(&format!("weather-{partner}"))
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Variants as deltas
// ---------------------------------------------------------------------------

/// One edit a variant makes to a base ruleset — a delta, never a rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum DeltaOp {
    /// Cut a Weather (and any layout that required it) from the deck, by id.
    CutWeather {
        /// The Weather id removed.
        weather: String,
    },
    /// Rename a Ground in place, keeping its wheel position — a regional re-suit.
    RenameGround {
        /// The Ground id to re-face.
        id: String,
        /// The new display name.
        to: String,
    },
    /// Re-face a Hand-role's figure (the Net-mender for the Pleacher on the fringe).
    RefaceHand {
        /// The Hand-role re-faced.
        role: HandRole,
        /// The new figure id.
        figure: String,
    },
}

/// A **variant**: a named delta over a base ruleset, carrying its own provenance.
/// A rule change is lore — a variant declaration — not a profile edit, so the
/// export compiles it rather than defining it (`DL-0007`, koan 10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Variant {
    /// The variant name, e.g. `turning/keep-stamped`.
    pub name: String,
    /// The deck id the variant produces, e.g. `turning-lowlands-keep-stamped`.
    pub deck_id: String,
    /// The delta operations, applied in order.
    pub ops: Vec<DeltaOp>,
    /// Where the variant came from.
    pub provenance: Provenance,
}

impl Variant {
    /// The keep-stamped variant (`D-0148`): cut the hard Weathers — the Debt, the
    /// Sundering, the Bourn — so a frightened person can never reach the
    /// debt-square. The deck-scale form of the false curriculum.
    #[must_use]
    pub fn keep_stamped() -> Self {
        Self {
            name: "turning/keep-stamped".to_owned(),
            deck_id: "turning-lowlands-keep-stamped".to_owned(),
            ops: vec![
                DeltaOp::CutWeather {
                    weather: "debt".to_owned(),
                },
                DeltaOp::CutWeather {
                    weather: "sundering".to_owned(),
                },
                DeltaOp::CutWeather {
                    weather: "bourn".to_owned(),
                },
            ],
            provenance: Provenance::proposed("D-0148"),
        }
    }

    /// The salt-fringe regional variant (`D-0148`): the Tide and the Catch replace
    /// Grounds and the Net-mender takes the Mender's face. The moon-count holds —
    /// a re-suit, not a cut.
    #[must_use]
    pub fn salt_fringe() -> Self {
        Self {
            name: "turning/salt-fringe".to_owned(),
            deck_id: "turning-salt-fringe-honest".to_owned(),
            ops: vec![
                DeltaOp::RenameGround {
                    id: "wood".to_owned(),
                    to: "The Tide".to_owned(),
                },
                DeltaOp::RenameGround {
                    id: "field".to_owned(),
                    to: "The Catch".to_owned(),
                },
                DeltaOp::RefaceHand {
                    role: HandRole::Mender,
                    figure: "the-net-mender".to_owned(),
                },
            ],
            provenance: Provenance::proposed("D-0148"),
        }
    }
}

impl Ruleset {
    /// Fork a variant off this ruleset: apply its delta and take the variant's
    /// (proposed) provenance and deck id. The result is a *proposed* ruleset —
    /// unassertable until ratified — exactly as a forked lore claim is.
    ///
    /// ```
    /// use dli_sim::turning::{Ruleset, Variant};
    ///
    /// let honest = Ruleset::lowlands_honest();
    /// let proposed = honest.fork(&Variant::keep_stamped());
    /// assert!(!proposed.is_assertable());           // a fork is only proposed
    /// assert_eq!(proposed.leaf_count(), 26);        // three hard Weathers cut
    /// let ratified = proposed.ratify("D-0148");
    /// assert!(ratified.is_assertable());
    /// ```
    #[must_use]
    pub fn fork(&self, variant: &Variant) -> Self {
        let mut next = self.clone();
        next.deck_id.clone_from(&variant.deck_id);
        next.provenance = variant.provenance.clone();
        for op in &variant.ops {
            next.apply(op);
        }
        next
    }

    /// Apply one delta op in place.
    fn apply(&mut self, op: &DeltaOp) {
        match op {
            DeltaOp::CutWeather { weather } => {
                self.weathers.retain(|w| &w.id != weather);
                // Cutting one half of a reciprocal pair leaves its partner a lone
                // survivor — the Mending stands without its Sundering — so heal
                // the now-dangling pointer rather than leave the ring unclosed.
                for w in &mut self.weathers {
                    if w.reciprocal.as_deref() == Some(weather.as_str()) {
                        w.reciprocal = None;
                        w.kind = WeatherKind::Passage;
                    }
                }
            }
            DeltaOp::RenameGround { id, to } => {
                if let Some(ground) = self.grounds.iter_mut().find(|g| &g.id == id) {
                    ground.name.clone_from(to);
                }
            }
            DeltaOp::RefaceHand { role, figure } => {
                self.figures
                    .retain(|fig| !matches!(&fig.seat, FigureSeat::Hand { role: r } if r == role));
                self.figures.push(Figure::on_hand(figure, *role));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Structural invariant enumeration
// ---------------------------------------------------------------------------

/// A structural invariant of the Turning ruleset — an exact, re-derivable
/// predicate over the deck itself (not a Monte-Carlo estimate), so every verdict
/// it yields grades [`checked`](dli_core::Grade::Checked).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StructuralInvariant {
    /// Every leaf id is unique — the deck is a set, not a multiset.
    DeckIsASet,
    /// The wheel is a single closed cycle covering every leaf (no highest card).
    ClosedRing,
    /// Every reciprocal Weather present has its partner present too.
    ReciprocityClosed,
    /// The moon-arithmetic and the distinct leaves the wheel enumerates agree —
    /// twenty-nine for the honest deck, twenty-six under the keep-stamped delta,
    /// always with no doubled or lost leaf.
    MoonCount,
}

impl StructuralInvariant {
    /// Every structural invariant, for a full enumeration sweep.
    pub const ALL: [StructuralInvariant; 4] = [
        Self::DeckIsASet,
        Self::ClosedRing,
        Self::ReciprocityClosed,
        Self::MoonCount,
    ];

    /// The invariant's stable name, carried onto its verdict.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeckIsASet => "deck-is-a-set",
            Self::ClosedRing => "closed-ring",
            Self::ReciprocityClosed => "reciprocity-closed",
            Self::MoonCount => "moon-count",
        }
    }

    /// Check this invariant against a ruleset, returning a breach reason if it
    /// fails, else [`None`]. Every check is exact and applies to any ruleset —
    /// honest or cut — so a variant's fewer leaves are legitimate as long as its
    /// count and its wheel agree.
    #[must_use]
    pub fn breach(self, ruleset: &Ruleset) -> Option<String> {
        match self {
            Self::DeckIsASet => breach_deck_is_a_set(ruleset),
            Self::ClosedRing => breach_closed_ring(ruleset),
            Self::ReciprocityClosed => breach_reciprocity(ruleset),
            Self::MoonCount => breach_moon_count(ruleset),
        }
    }
}

fn breach_deck_is_a_set(ruleset: &Ruleset) -> Option<String> {
    let leaves = ruleset.wheel();
    let unique: BTreeSet<&String> = leaves.iter().collect();
    (unique.len() != leaves.len()).then(|| {
        format!(
            "the deck has {} leaves but only {} distinct ids",
            leaves.len(),
            unique.len()
        )
    })
}

fn breach_closed_ring(ruleset: &Ruleset) -> Option<String> {
    // The wheel order is a permutation of a set (checked separately), so its
    // rotation is automatically a single cycle covering every leaf; it fails to
    // be a closed ring only in the degenerate empty deck.
    ruleset
        .wheel()
        .is_empty()
        .then(|| "an empty deck has no closed ring".to_owned())
}

fn breach_reciprocity(ruleset: &Ruleset) -> Option<String> {
    let present: BTreeSet<&str> = ruleset.weathers.iter().map(|w| w.id.as_str()).collect();
    ruleset.weathers.iter().find_map(|w| {
        w.reciprocal.as_ref().and_then(|partner| {
            (!present.contains(partner.as_str())).then(|| {
                format!(
                    "the Weather '{}' keeps its reciprocal '{partner}' but it was cut",
                    w.id
                )
            })
        })
    })
}

fn breach_moon_count(ruleset: &Ruleset) -> Option<String> {
    // The moon-arithmetic (grounds·4 + weathers + 1) and the distinct leaves the
    // wheel actually enumerates must agree: a deck that counts more leaves than it
    // holds distinct ids has drifted (a doubled Ground, a collision) and no longer
    // runs the moon.
    let distinct: BTreeSet<String> = ruleset.wheel().into_iter().collect();
    (distinct.len() != ruleset.leaf_count()).then(|| {
        format!(
            "the moon-count is {} but the wheel enumerates {} distinct leaves",
            ruleset.leaf_count(),
            distinct.len()
        )
    })
}

/// Enumerate every [`StructuralInvariant`] over a ruleset as exact
/// [`checked`](dli_core::Grade::Checked) verdicts — the invariant-enumeration
/// deliverable. An all-held result is a structurally sound deck.
///
/// ```
/// use dli_sim::turning::{enumerate_structural_invariants, Ruleset};
///
/// let verdicts = enumerate_structural_invariants(&Ruleset::lowlands_honest());
/// assert_eq!(verdicts.len(), 4);
/// assert!(verdicts.iter().all(dli_sim::InvariantVerdict::held));
/// ```
#[must_use]
pub fn enumerate_structural_invariants(ruleset: &Ruleset) -> Vec<InvariantVerdict> {
    StructuralInvariant::ALL
        .into_iter()
        .map(|inv| {
            let outcome = match inv.breach(ruleset) {
                None => InvariantOutcome::Held,
                Some(detail) => InvariantOutcome::Violated { trial: 0, detail },
            };
            InvariantVerdict::new(inv.name().to_owned(), outcome)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The deal battery — seeded readings through SIM-1's seams
// ---------------------------------------------------------------------------

/// A per-reading scalar the battery observes for a Monte-Carlo metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "observe", rename_all = "kebab-case")]
pub enum ReadingObservable {
    /// `1` if the named leaf id appears in the reading, else `0`.
    LeafAppears {
        /// The leaf id watched, e.g. `weather-debt`.
        leaf: String,
    },
    /// `1` if the reading's verdict is a contradiction, else `0`.
    ContradictionRate,
}

impl ReadingObservable {
    /// Observe a dealt reading under its ruleset.
    #[must_use]
    fn observe(&self, ruleset: &Ruleset, reading: &Reading) -> i64 {
        match self {
            Self::LeafAppears { leaf } => i64::from(reading.holds(leaf)),
            Self::ContradictionRate => {
                i64::from(ruleset.verdict(reading) == Verdict::Contradiction)
            }
        }
    }
}

/// A named Monte-Carlo metric over the deal battery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadingMetric {
    /// The metric name, carried onto its estimate.
    pub name: String,
    /// The per-reading value it aggregates.
    pub observable: ReadingObservable,
}

/// An exact predicate swept over every dealt reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "invariant", rename_all = "kebab-case")]
pub enum ReadingInvariant {
    /// The named leaf never appears in any reading (the keep-stamped Debt claim).
    LeafNeverAppears {
        /// The leaf id that must not appear.
        leaf: String,
    },
    /// Every reading conserves the deck: no leaf laid twice.
    DeckConserved,
    /// Every play in every reading was a legal move given the prefix laid.
    OnlyLegalPlays,
}

impl ReadingInvariant {
    /// The invariant's stable name.
    #[must_use]
    fn name(&self) -> String {
        match self {
            Self::LeafNeverAppears { leaf } => format!("{leaf}-never-appears"),
            Self::DeckConserved => "deck-conserved".to_owned(),
            Self::OnlyLegalPlays => "only-legal-plays".to_owned(),
        }
    }

    /// The breach reason if this reading violates the invariant, else [`None`].
    #[must_use]
    fn breach(&self, ruleset: &Ruleset, reading: &Reading, trial: u64) -> Option<String> {
        match self {
            Self::LeafNeverAppears { leaf } => reading
                .holds(leaf)
                .then(|| format!("'{leaf}' appeared at trial {trial}")),
            Self::DeckConserved => breach_conservation(reading, trial),
            Self::OnlyLegalPlays => breach_legality(ruleset, reading, trial),
        }
    }
}

fn breach_conservation(reading: &Reading, trial: u64) -> Option<String> {
    let unique: BTreeSet<&String> = reading.laid.iter().collect();
    (unique.len() != reading.laid.len()).then(|| format!("a leaf was laid twice at trial {trial}"))
}

fn breach_legality(ruleset: &Ruleset, reading: &Reading, trial: u64) -> Option<String> {
    let mut prefix = Reading {
        layout: reading.layout.clone(),
        leaves_dealt: reading.leaves_dealt,
        laid: Vec::new(),
    };
    for id in &reading.laid {
        if !ruleset.legal_moves(&prefix).contains(id) {
            return Some(format!("'{id}' was an illegal play at trial {trial}"));
        }
        prefix.laid.push(id.clone());
    }
    None
}

/// A **deal battery**: a ruleset, a layout, and the metrics and invariants to
/// grade over seeded readings. It reuses SIM-1's seams whole — the
/// [`SplitMix64`] draws and the honest
/// [`RunManifest`] — so a Turning study inherits the
/// same grade-honesty oracle: rates grade `tested`, exact sweeps grade `checked`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TurningBattery {
    /// The ruleset dealt from.
    pub ruleset: Ruleset,
    /// The layout name dealt.
    pub layout: String,
    /// The master seed every reading derives its sub-stream from.
    pub seed: u64,
    /// The number of readings to deal.
    pub trials: u64,
    /// The Monte-Carlo metrics to estimate.
    pub metrics: Vec<ReadingMetric>,
    /// The exact invariants to sweep.
    pub invariants: Vec<ReadingInvariant>,
}

impl TurningBattery {
    /// Deal reading `index`: play a legal reading from that reading's sub-stream,
    /// [`stream_value(seed, index)`](crate::rng::stream_value). Each play picks
    /// uniformly among the legal moves, so the reading is a pure function of
    /// `(seed, index)` — a battery replays byte-for-byte.
    ///
    /// A layout the deck cannot deal (a foreclosed debt-square) yields an empty
    /// reading; its [`verdict`](Ruleset::verdict) is
    /// [`MissingPremise`](Verdict::MissingPremise) and the study reads foreclosure
    /// from [`available_layouts`](Ruleset::available_layouts) instead.
    #[must_use]
    pub fn deal(&self, index: u64) -> Reading {
        let Some(layout) = self.ruleset.layout(&self.layout) else {
            return Reading {
                layout: self.layout.clone(),
                leaves_dealt: 0,
                laid: Vec::new(),
            };
        };
        let mut prng = SplitMix64::new(stream_value(self.seed, index));
        let mut reading = Reading::open(layout);
        while !self.ruleset.is_terminal(&reading) {
            let moves = self.ruleset.legal_moves(&reading);
            let Some(pick) = pick(&mut prng, moves.len()) else {
                break;
            };
            reading.laid.push(moves[pick].clone());
        }
        reading
    }

    /// Run the battery, folding every dealt reading into an honest
    /// [`RunManifest`]: each metric a `tested`
    /// estimate with exact counts, each invariant a `checked` sweep with its
    /// earliest witnessing trial. The returned manifest satisfies
    /// [`grades_are_honest`](crate::manifest::RunManifest::grades_are_honest).
    #[must_use]
    pub fn run(&self) -> RunManifest {
        let mut metric_stats = vec![(0i64, 0i64); self.metrics.len()];
        let mut breaches: Vec<Option<(u64, String)>> = vec![None; self.invariants.len()];

        for index in 0..self.trials {
            let reading = self.deal(index);
            for (slot, metric) in self.metrics.iter().enumerate() {
                let value = metric.observable.observe(&self.ruleset, &reading);
                metric_stats[slot].0 += value;
                metric_stats[slot].1 += value * value;
            }
            for (slot, inv) in self.invariants.iter().enumerate() {
                if breaches[slot].is_none()
                    && let Some(detail) = inv.breach(&self.ruleset, &reading, index)
                {
                    breaches[slot] = Some((index, detail));
                }
            }
        }

        let metrics = self
            .metrics
            .iter()
            .zip(metric_stats)
            .map(|(metric, (sum, sum_sq))| {
                MetricEstimate::from_counts(metric.name.clone(), self.trials, sum, sum_sq)
            })
            .collect();
        let invariants = self
            .invariants
            .iter()
            .zip(breaches)
            .map(|(inv, breach)| {
                let outcome = match breach {
                    None => InvariantOutcome::Held,
                    Some((trial, detail)) => InvariantOutcome::Violated { trial, detail },
                };
                InvariantVerdict::new(inv.name(), outcome)
            })
            .collect();

        RunManifest {
            battery_digest: self.digest(),
            seed: self.seed,
            trials: self.trials,
            metrics,
            invariants,
            checkpoint_lineage: if self.trials > 0 {
                vec![self.trials]
            } else {
                Vec::new()
            },
        }
    }

    /// A content digest of the battery's semantics — its deck id, provenance,
    /// layout, seed, trials, and specs — for pinning which battery a manifest came
    /// from.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_json(self)
    }
}

/// Pick an index uniformly in `[0, len)` from a generator; [`None`] when empty.
fn pick(prng: &mut SplitMix64, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let n = u64::try_from(len).unwrap_or(u64::MAX);
    usize::try_from(prng.below(n)).ok()
}

// ---------------------------------------------------------------------------
// The honest-versus-rigged study
// ---------------------------------------------------------------------------

/// The measurable difference between the honest deck and its keep-stamped rig —
/// the allegory shipped as data. Produced by [`honest_vs_rigged`].
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
    /// The number of honest readings in which the Debt appeared (a positive count
    /// — the deck can reach the question).
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
/// the measurable difference. This is the study `EXP-3`'s keep-stamped recompile
/// mirrors — the rig discovered as data, never announced.
///
/// ```
/// use dli_sim::turning::honest_vs_rigged;
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
        observable: ReadingObservable::LeafAppears {
            leaf: "weather-debt".to_owned(),
        },
    };
    let never_debt = ReadingInvariant::LeafNeverAppears {
        leaf: "weather-debt".to_owned(),
    };

    let battery = |ruleset: Ruleset| TurningBattery {
        ruleset,
        layout: "a-round".to_owned(),
        seed,
        trials,
        metrics: vec![debt.clone()],
        invariants: vec![never_debt.clone(), ReadingInvariant::DeckConserved],
    };

    let honest_deck = Ruleset::lowlands_honest();
    let rigged_deck = Ruleset::keep_stamped();
    let honest = battery(honest_deck.clone()).run();
    let rigged = battery(rigged_deck.clone()).run();

    StudyReport {
        debt_appearances_honest: honest.metrics[0].sum,
        debt_appearances_rigged: rigged.metrics[0].sum,
        debt_square_honest: honest_deck.layout("the-debt-square").is_some(),
        debt_square_rigged: rigged_deck.layout("the-debt-square").is_some(),
        honest,
        rigged,
    }
}

// ---------------------------------------------------------------------------
// Canonical deck data (D-0148)
// ---------------------------------------------------------------------------

/// The four lowlands Grounds, in canonical order.
fn lowlands_grounds() -> Vec<Ground> {
    vec![
        Ground::new("wood", "The Wood"),
        Ground::new("water", "The Water"),
        Ground::new("field", "The Field"),
        Ground::new("hearth", "The Hearth"),
    ]
}

/// A reciprocal Weather: half of a pair, cut by the keep-stamped rig iff `hard`.
fn recip(id: &str, name: &str, ring_next: &str, partner: &str, hard: bool) -> Weather {
    Weather {
        id: id.to_owned(),
        name: name.to_owned(),
        kind: WeatherKind::Reciprocal,
        ring_next: ring_next.to_owned(),
        reciprocal: Some(partner.to_owned()),
        hard,
    }
}

/// A passage Weather: a lone condition with no reciprocal, cut by the rig iff `hard`.
fn passage(id: &str, name: &str, ring_next: &str, hard: bool) -> Weather {
    Weather {
        id: id.to_owned(),
        name: name.to_owned(),
        kind: WeatherKind::Passage,
        ring_next: ring_next.to_owned(),
        reciprocal: None,
        hard,
    }
}

/// The twelve Weathers, in the closed weather-ring order, each carrying its ring
/// successor, its reciprocal (for the four pairs), and whether the keep-stamped
/// rig cuts it — the Debt, the Sundering, the Bourn.
fn canonical_weathers() -> Vec<Weather> {
    vec![
        recip("greening", "The Greening", "rot", "rot", false),
        recip("rot", "The Rot", "flood", "greening", false),
        recip("flood", "The Flood", "dearth", "dearth", false),
        recip("dearth", "The Dearth", "binding", "flood", false),
        recip("binding", "The Binding", "loosing", "loosing", false),
        recip("loosing", "The Loosing", "mending", "binding", false),
        recip("mending", "The Mending", "sundering", "sundering", false),
        recip("sundering", "The Sundering", "road", "mending", true),
        passage("road", "The Road", "bourn", false),
        passage("bourn", "The Bourn", "debt", true),
        passage("debt", "The Debt", "fallow", true),
        passage("fallow", "The Fallow", "greening", false),
    ]
}

/// The lowlands folk-figures: one per Hand-role, plus the few Weathers a figure
/// already fits (`D-0147`). The lowlands Mender is the Pleacher.
fn lowlands_figures() -> Vec<Figure> {
    vec![
        Figure::on_hand("the-reaper", HandRole::Taker),
        Figure::on_hand("the-drover", HandRole::Keeper),
        Figure::on_hand("the-host", HandRole::Giver),
        Figure::on_hand("the-pleacher", HandRole::Mender),
        Figure::on_weather("the-keener", "sundering"),
        Figure::on_weather("the-ferryman", "road"),
        Figure::on_weather("the-gleaner", "fallow"),
    ]
}

/// The canonical layouts. The debt-square requires the Debt to be dealable, so a
/// keep-stamped deck drops it in [`available_layouts`](Ruleset::available_layouts).
fn canonical_layouts() -> Vec<Layout> {
    vec![
        Layout::new("a-round", 3, &[]),
        Layout::new("the-ford", 2, &[]),
        Layout::new("the-whole-round", 7, &[]),
        Layout::new("the-debt-square", 4, &["debt"]),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn honest_deck_runs_the_moon() {
        let deck = Ruleset::lowlands_honest();
        assert_eq!(deck.leaf_count(), 29);
        assert_eq!(deck.leaves().len(), 29);
        // 16 Hands, 12 Weathers, 1 Reckoner.
        let hands = deck
            .leaves()
            .iter()
            .filter(|l| l.layer() == Layer::Hand)
            .count();
        let weathers = deck
            .leaves()
            .iter()
            .filter(|l| l.layer() == Layer::Weather)
            .count();
        assert_eq!(hands, 16);
        assert_eq!(weathers, 12);
    }

    #[test]
    fn leaf_ids_match_the_export_convention() {
        let leaf = Leaf::Hand {
            role: HandRole::Mender,
            ground: "field".to_owned(),
        };
        assert_eq!(leaf.id(), "hand-mender-field");
        assert_eq!(
            Leaf::Weather {
                weather: "debt".to_owned()
            }
            .id(),
            "weather-debt"
        );
        assert_eq!(Leaf::Reckoner.id(), "reckoner");
    }

    #[test]
    fn keep_stamped_cuts_three_weathers_and_the_debt_square() {
        let rig = Ruleset::keep_stamped();
        assert_eq!(rig.leaf_count(), 26);
        assert!(rig.weathers.iter().all(|w| w.id != "debt"));
        assert!(rig.layout("the-debt-square").is_none());
        // The honest deck keeps both.
        let honest = Ruleset::lowlands_honest();
        assert!(honest.layout("the-debt-square").is_some());
    }

    #[test]
    fn structural_invariants_all_hold_on_every_shipped_deck() {
        for deck in [
            Ruleset::lowlands_honest(),
            Ruleset::keep_stamped(),
            Ruleset::salt_fringe(),
        ] {
            for verdict in enumerate_structural_invariants(&deck) {
                assert!(verdict.held(), "{}: {:?}", deck.deck_id, verdict);
            }
        }
    }

    #[test]
    fn salt_fringe_re_suits_but_keeps_the_moon_count() {
        let fringe = Ruleset::salt_fringe();
        assert_eq!(fringe.leaf_count(), 29);
        assert!(fringe.grounds.iter().any(|g| g.name == "The Tide"));
        let mender = Leaf::Hand {
            role: HandRole::Mender,
            ground: "wood".to_owned(),
        };
        assert_eq!(fringe.figure_for(&mender).unwrap().id, "the-net-mender");
    }
}
