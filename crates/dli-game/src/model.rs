//! The generic ruleset model: a card game described entirely as data.
//!
//! A `Ruleset` is a plain description of a deck — declared axes, card families
//! (layers), a legal-move relation, terminal layouts, and structural invariants —
//! with no run state entangled in it. Three consumers read that one structure: the
//! play relation (`legal_moves`/`is_terminal`/`verdict`), the deal battery
//! (`crate::battery`), and the card/`GDL` export (`crate::export`). Because the
//! ruleset holds no simulator state, each of those is a straight structural read.
//!
//! The vocabulary here is deliberately neutral — axis, value, layer, card, hub,
//! wheel — so one engine spans a linear ranked deck (`MoveRule::Ascending`) and a
//! rankless cyclic wheel (`MoveRule::CyclicReach`) alike. Only two seams stay as
//! code a world supplies through `crate::binding::GameBindings`: a named verdict
//! predicate (`VerdictRule::Registered`) and a named structural invariant
//! (`StructuralInvariant::Registered`). Everything else is data loaded with
//! `Ruleset::from_rules`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_sim::manifest::{InvariantOutcome, InvariantVerdict};

use crate::binding::GameBindings;

// ---------------------------------------------------------------------------
// Provenance — the fork/ratify standing (generic, lifted unchanged)
// ---------------------------------------------------------------------------

/// A ruleset's standing on the ratification path.
///
/// A proposed ruleset is structurally *unassertable* — the same status-gate the
/// lore engine applies to proposed lore — and only an authorial ratification lifts
/// it.
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
    /// `Proposed`, which is shadow-only).
    #[must_use]
    pub const fn assertable(self) -> bool {
        !matches!(self, Self::Proposed)
    }
}

/// Where a ruleset (or a variant delta) came from: a standing and the decisions it
/// cites, in citation order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    /// The standing on the ratification path.
    pub standing: Standing,
    /// The decision ids cited, in the order added.
    pub cites: Vec<String>,
}

impl Provenance {
    /// A proposed provenance citing one decision (a woken-from touchstone).
    #[must_use]
    pub fn proposed(cite: &str) -> Self {
        Self {
            standing: Standing::Proposed,
            cites: vec![cite.to_owned()],
        }
    }

    /// Ratify: lift the standing to `Ratified` and record the ratifying decision,
    /// keeping the earlier citations.
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
// Attribute values — world facts carried as data
// ---------------------------------------------------------------------------

/// A world fact attached to an axis value, a decoration, or a rendered card.
///
/// Serialised untagged, so JSON carries the natural scalar (`true`, `5`,
/// `"text"`) with no wrapper. Deserialization tries `Flag`, then `Number`, then
/// `Text`, so a bool never lands as text and an integer never lands as a flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AttrValue {
    /// A boolean fact (e.g. a `hard` marker).
    Flag(bool),
    /// An integer fact (e.g. an ordinal or a count).
    Number(i64),
    /// A text fact (e.g. a partner id or a display label).
    Text(String),
}

impl AttrValue {
    /// A text attribute from a string slice.
    #[must_use]
    pub fn text(value: &str) -> Self {
        Self::Text(value.to_owned())
    }

    /// The text, if this is a `Text` attribute.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The flag, if this is a `Flag` attribute.
    #[must_use]
    pub const fn as_flag(&self) -> Option<bool> {
        match self {
            Self::Flag(flag) => Some(*flag),
            _ => None,
        }
    }

    /// The number, if this is a `Number` attribute.
    #[must_use]
    pub const fn as_number(&self) -> Option<i64> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Axes and their values
// ---------------------------------------------------------------------------

/// A named value on an axis: a stable id, a display name, and world-fact attrs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AxisValue {
    /// The stable id, used in card bindings and id templates.
    pub id: String,
    /// The display name.
    pub name: String,
    /// World facts carried as data (kind, partner, ordinal, hardness, …).
    #[serde(default)]
    pub attrs: BTreeMap<String, AttrValue>,
}

/// A named ordered set of values a card family ranges over. Order matters — a
/// ranked axis (`A < 2 < … < K`) reads its position from this order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Axis {
    /// The stable axis id (e.g. `rank`).
    pub id: String,
    /// The values, in declared order (the ranking, for a ranked axis).
    pub values: Vec<AxisValue>,
}

impl Axis {
    /// The value with this id, if any.
    #[must_use]
    pub fn value(&self, id: &str) -> Option<&AxisValue> {
        self.values.iter().find(|value| value.id == id)
    }

    /// The zero-based position of the value with this id in the declared order —
    /// its rank on a ranked axis.
    #[must_use]
    pub fn position(&self, id: &str) -> Option<usize> {
        self.values.iter().position(|value| value.id == id)
    }
}

// ---------------------------------------------------------------------------
// Layers and cards
// ---------------------------------------------------------------------------

/// A card family: the product of zero or more axes with an id template. Zero axes
/// is a singleton (a hub card that is never dealt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Layer {
    /// The layer id (e.g. `card`, `hub`).
    pub id: String,
    /// The axes crossed to enumerate this layer, outer to inner.
    pub axes: Vec<String>,
    /// The id template, with `{axis}` placeholders (e.g. `{rank}-of-{suit}`).
    pub id_template: String,
    /// Whether cards in this layer are dealt (a hub layer is `false`).
    pub dealt: bool,
}

/// A single enumerated card: its rendered id, its layer, and its axis bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Card {
    /// The stable card id, rendered from the layer's id template.
    pub id: String,
    /// The layer this card belongs to.
    pub layer: String,
    /// The value bound on each of the layer's axes (axis id → value id).
    pub bindings: BTreeMap<String, String>,
}

/// The order the deck's cards enumerate in — the wheel the move relation turns on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Ordering {
    /// Layers in declaration order, each in axis-product order.
    LayerProduct,
}

// ---------------------------------------------------------------------------
// The move relation
// ---------------------------------------------------------------------------

/// The follow-predicate: a closed declared enum, extensible only by an engine
/// change. The opening play is always free; this governs every later `follows`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "rule", rename_all = "kebab-case")]
pub enum MoveRule {
    /// A rankless cyclic ring: `next` is within `reach` steps forward of `prev` on
    /// the wheel (and never `prev` itself).
    CyclicReach {
        /// How many wheel steps forward a play may turn.
        reach: u32,
    },
    /// A ranked run: `next` shares the `same` axis bindings and its `axis` position
    /// is `prev`'s plus `step` (or any strictly-greater position when `step` is
    /// absent).
    Ascending {
        /// The ranked axis whose position advances.
        axis: String,
        /// The exact step, or `None` for any strictly-ascending step.
        #[serde(default)]
        step: Option<u32>,
        /// Axes whose bindings `next` must share with `prev` (e.g. same suit).
        #[serde(default)]
        same: Vec<String>,
    },
    /// A pure draw: any not-yet-laid card follows.
    Free,
}

// ---------------------------------------------------------------------------
// Layouts, verdicts, invariants, decorations
// ---------------------------------------------------------------------------

/// A layout: the shape of a play. It deals a fixed number of cards; some layouts
/// require a card be dealable, so cutting that card forecloses the layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Layout {
    /// The layout name.
    pub name: String,
    /// How many cards it deals.
    pub cards_dealt: u32,
    /// Card ids the deck must still contain for this layout to be dealable.
    #[serde(default)]
    pub requires_cards: Vec<String>,
}

/// How a completed (or jammed) reading is judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "verdict", rename_all = "kebab-case")]
pub enum VerdictRule {
    /// The generic verdict: `completed` when filled, `incomplete` when jammed.
    Completed,
    /// A world verdict predicate, looked up by name in the game bindings.
    Registered {
        /// The registered predicate name.
        name: String,
    },
}

/// A structural invariant of the ruleset — an exact, re-derivable predicate over
/// the deck itself, so every verdict it yields grades `checked`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "invariant", rename_all = "kebab-case")]
pub enum StructuralInvariant {
    /// Every card id is unique — the deck is a set, not a multiset.
    DeckIsASet,
    /// The declared count (the sum of layer-product sizes) equals the distinct card
    /// ids the deck enumerates.
    CountMatches,
    /// The wheel is a single closed cycle covering every card (declared only for
    /// cyclic games).
    ClosedRing,
    /// A world structural invariant, looked up by name in the game bindings.
    Registered {
        /// The registered predicate name.
        name: String,
    },
}

impl StructuralInvariant {
    /// The invariant's stable name, carried onto its verdict.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::DeckIsASet => "deck-is-a-set".to_owned(),
            Self::CountMatches => "count-matches".to_owned(),
            Self::ClosedRing => "closed-ring".to_owned(),
            Self::Registered { name } => name.clone(),
        }
    }

    /// Check this invariant against a ruleset, returning a breach reason if it
    /// fails, else `None`. A `Registered` invariant with no bound predicate is a
    /// breach — a misconfiguration surfaces rather than silently holding.
    #[must_use]
    pub fn breach(&self, ruleset: &Ruleset, bindings: &GameBindings) -> Option<String> {
        match self {
            Self::DeckIsASet => breach_deck_is_a_set(ruleset),
            Self::CountMatches => breach_count_matches(ruleset),
            Self::ClosedRing => breach_closed_ring(ruleset),
            Self::Registered { name } => match bindings.invariant_fn(name) {
                Some(predicate) => predicate(ruleset),
                None => Some(format!("no invariant predicate registered for '{name}'")),
            },
        }
    }
}

/// A card-selector to attribute overlay — the generalization of a folk-figure. A
/// card matches when its bindings contain every `(axis, value)` pair in `selector`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Decoration {
    /// The card selector: `(axis id → value id)` constraints, all of which must
    /// hold. An empty selector matches nothing.
    pub selector: BTreeMap<String, String>,
    /// The attributes overlaid onto matching cards (e.g. an emblem id).
    pub attrs: BTreeMap<String, AttrValue>,
}

impl Decoration {
    /// Whether this decoration covers a card.
    #[must_use]
    pub fn matches(&self, card: &Card) -> bool {
        !self.selector.is_empty()
            && self
                .selector
                .iter()
                .all(|(axis, value)| card.bindings.get(axis).map(String::as_str) == Some(value))
    }
}

// ---------------------------------------------------------------------------
// Variants as deltas
// ---------------------------------------------------------------------------

/// One edit a variant makes to a base ruleset — a delta, never a rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum DeltaOp {
    /// Cut an axis value: its bound cards stop enumerating and any layout that
    /// required one of them is foreclosed.
    CutAxisValue {
        /// The axis edited.
        axis: String,
        /// The value id removed.
        value: String,
    },
    /// Rename an axis value's display name in place, keeping its id (and so its
    /// wheel position) — a regional re-facing.
    RenameAxisValue {
        /// The axis edited.
        axis: String,
        /// The value id re-faced.
        value: String,
        /// The new display name.
        to: String,
    },
    /// Set (add or replace) a decoration keyed by its selector.
    SetDecoration {
        /// The card selector the decoration applies to.
        selector: BTreeMap<String, String>,
        /// The attributes overlaid onto matching cards.
        attrs: BTreeMap<String, AttrValue>,
    },
}

/// A named delta over a base ruleset, carrying its own provenance. A rule change is
/// data — a variant declaration — not a profile edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Variant {
    /// The variant name.
    pub name: String,
    /// The game id the variant produces.
    pub game_id: String,
    /// The delta operations, applied in order.
    pub ops: Vec<DeltaOp>,
    /// Where the variant came from.
    pub provenance: Provenance,
}

// ---------------------------------------------------------------------------
// Readings and verdicts
// ---------------------------------------------------------------------------

/// A reading in progress: the cards laid so far, in order, under a chosen layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Reading {
    /// The layout being laid, by name.
    pub layout: String,
    /// How many cards the layout deals (carried so a reading reads without the
    /// ruleset).
    pub cards_dealt: u32,
    /// The card ids laid so far, in order.
    pub laid: Vec<String>,
}

impl Reading {
    /// A fresh, empty reading of a layout.
    #[must_use]
    pub fn open(layout: &Layout) -> Self {
        Self {
            layout: layout.name.clone(),
            cards_dealt: layout.cards_dealt,
            laid: Vec::new(),
        }
    }

    /// Whether a card has already been laid (deck conservation).
    #[must_use]
    pub fn holds(&self, card_id: &str) -> bool {
        self.laid.iter().any(|id| id == card_id)
    }

    /// Whether the layout is filled.
    #[must_use]
    pub fn is_filled(&self) -> bool {
        u32::try_from(self.laid.len()).unwrap_or(u32::MAX) >= self.cards_dealt
    }
}

/// A reading's verdict — a declared string, generic (`completed`/`incomplete`) or a
/// world verdict (`contradiction`, `question`, …) from a registered predicate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    /// The verdict id.
    pub id: String,
}

impl Verdict {
    /// The generic verdict for a filled reading.
    pub const COMPLETED: &'static str = "completed";
    /// The generic verdict for a jammed (unfilled) reading.
    pub const INCOMPLETE: &'static str = "incomplete";
    /// The verdict a `Registered` rule yields when no predicate is bound.
    pub const UNBOUND: &'static str = "unbound";

    /// A verdict with this id.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self { id: id.to_owned() }
    }

    /// The generic `completed` verdict.
    #[must_use]
    pub fn completed() -> Self {
        Self::new(Self::COMPLETED)
    }

    /// The generic `incomplete` verdict.
    #[must_use]
    pub fn incomplete() -> Self {
        Self::new(Self::INCOMPLETE)
    }
}

// ---------------------------------------------------------------------------
// The ruleset — rules as data
// ---------------------------------------------------------------------------

/// The generic ruleset: a deck and its rules, entirely as data. Holds no run
/// state, so the export renders it structurally and the deal battery reads it
/// without mutating it.
///
/// ```
/// use dli_game::model::Ruleset;
///
/// let rules = r#"{
///   "game_id": "mini", "region": "sample",
///   "axes": [{ "id": "rank", "values": [
///     { "id": "a", "name": "A" }, { "id": "b", "name": "B" }] }],
///   "layers": [{ "id": "card", "axes": ["rank"], "id_template": "card-{rank}", "dealt": true }],
///   "ordering": "layer-product",
///   "moves": { "rule": "ascending", "axis": "rank", "step": 1, "same": [] },
///   "layouts": [{ "name": "pair", "cards_dealt": 2 }],
///   "verdict": { "verdict": "completed" },
///   "invariants": [{ "invariant": "deck-is-a-set" }, { "invariant": "count-matches" }],
///   "decorations": [],
///   "provenance": { "standing": "ratified", "cites": ["D-0001"] }
/// }"#;
/// let deck = Ruleset::from_rules(rules).expect("valid rules");
/// assert_eq!(deck.cards().len(), 2);
/// assert!(deck.follows("card-a", "card-b"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ruleset {
    /// The game id.
    pub game_id: String,
    /// The region this ruleset serves.
    pub region: String,
    /// The axes card families range over.
    pub axes: Vec<Axis>,
    /// The card families.
    pub layers: Vec<Layer>,
    /// The order the deck enumerates in — the wheel.
    pub ordering: Ordering,
    /// Named relations export profiles may select from this ruleset.
    #[serde(default)]
    pub relations: Vec<String>,
    /// The move relation.
    pub moves: MoveRule,
    /// The layouts the deck knows.
    pub layouts: Vec<Layout>,
    /// How a reading is judged.
    pub verdict: VerdictRule,
    /// The structural invariants declared over the deck.
    pub invariants: Vec<StructuralInvariant>,
    /// The decorations (card-selector attribute overlays).
    #[serde(default)]
    pub decorations: Vec<Decoration>,
    /// Where this ruleset stands on the ratification path.
    pub provenance: Provenance,
}

impl Ruleset {
    /// Deserialize a ruleset from its JSON description — the north star: a world's
    /// whole game loads from data.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_json` error if the JSON is malformed or does
    /// not match the ruleset schema.
    pub fn from_rules(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The axis with this id, if any.
    #[must_use]
    pub fn axis(&self, id: &str) -> Option<&Axis> {
        self.axes.iter().find(|axis| axis.id == id)
    }

    /// Every card the deck enumerates: for each layer, the product of its axes'
    /// values, rendered through the layer's id template. A zero-axis layer yields
    /// one card (its literal template).
    #[must_use]
    pub fn cards(&self) -> Vec<Card> {
        let mut out = Vec::new();
        for layer in &self.layers {
            for bindings in self.enumerate_bindings(&layer.axes) {
                let id = render_template(&layer.id_template, &bindings);
                out.push(Card {
                    id,
                    layer: layer.id.clone(),
                    bindings,
                });
            }
        }
        out
    }

    /// The deck's cards in `Ordering` order — the wheel the move relation turns on.
    #[must_use]
    pub fn leaves(&self) -> Vec<Card> {
        match self.ordering {
            Ordering::LayerProduct => self.cards(),
        }
    }

    /// The card ids in wheel order.
    #[must_use]
    pub fn card_ids(&self) -> Vec<String> {
        self.leaves().into_iter().map(|card| card.id).collect()
    }

    /// The declared card count — the sum of each layer's axis-product size. This is
    /// what `CountMatches` compares against the distinct card ids.
    #[must_use]
    pub fn card_count(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| self.layer_product_size(&layer.axes))
            .sum()
    }

    /// Whether `next` legally follows `prev`, dispatched on the move relation. A
    /// pure function of the two cards, so the export renders it as static successor
    /// edges.
    #[must_use]
    pub fn follows(&self, prev: &str, next: &str) -> bool {
        match &self.moves {
            MoveRule::Free => true,
            MoveRule::CyclicReach { reach } => self.cyclic_follows(prev, next, *reach),
            MoveRule::Ascending { axis, step, same } => {
                self.ascending_follows(prev, next, axis, *step, same)
            }
        }
    }

    /// Like `follows`, but dispatched against a precomputed deck so a batch of
    /// candidate checks (as in `legal_moves`) reuses one deck build instead of
    /// rebuilding it per call. Behaviour is identical to `follows`.
    fn follows_in(&self, prev: &str, next: &str, deck: &[Card]) -> bool {
        match &self.moves {
            MoveRule::Free => true,
            MoveRule::CyclicReach { reach } => Self::cyclic_follows_in(prev, next, *reach, deck),
            MoveRule::Ascending { axis, step, same } => {
                self.ascending_follows_in(prev, next, axis, *step, same, deck)
            }
        }
    }

    /// The legal plays from a reading's current state, in wheel order. The opening
    /// play is free; every later play must `follows` the last-laid card and must
    /// not repeat a laid card. Undealt (hub) cards are never plays.
    #[must_use]
    pub fn legal_moves(&self, reading: &Reading) -> Vec<String> {
        let dealt: BTreeSet<&str> = self
            .layers
            .iter()
            .filter(|layer| layer.dealt)
            .map(|layer| layer.id.as_str())
            .collect();
        // Build the deck once and thread it through every candidate's follow
        // check. `follows` rebuilds the whole deck on each call, so checking it
        // per playable card would rebuild the deck O(n) times per query; the
        // hoisted `follows_in` reuses this single build.
        let deck = self.leaves();
        let playable: Vec<String> = deck
            .iter()
            .filter(|card| dealt.contains(card.layer.as_str()))
            .map(|card| card.id.clone())
            .collect();
        match reading.laid.last() {
            None => playable
                .into_iter()
                .filter(|id| !reading.holds(id))
                .collect(),
            Some(last) => playable
                .into_iter()
                .filter(|id| !reading.holds(id) && self.follows_in(last, id, &deck))
                .collect(),
        }
    }

    /// Whether a reading has closed: the layout is filled, or the wheel jammed (no
    /// legal move remains). Every reading terminates — each play consumes a card
    /// from a finite deck.
    #[must_use]
    pub fn is_terminal(&self, reading: &Reading) -> bool {
        reading.is_filled() || self.legal_moves(reading).is_empty()
    }

    /// The verdict of a reading, per the ruleset's `VerdictRule`. A `Registered`
    /// rule with no bound predicate yields the `unbound` verdict.
    #[must_use]
    pub fn verdict(&self, reading: &Reading, bindings: &GameBindings) -> Verdict {
        match &self.verdict {
            VerdictRule::Completed => {
                if reading.is_filled() {
                    Verdict::completed()
                } else {
                    Verdict::incomplete()
                }
            }
            VerdictRule::Registered { name } => match bindings.verdict_fn(name) {
                Some(predicate) => predicate(self, reading),
                None => Verdict::new(Verdict::UNBOUND),
            },
        }
    }

    /// The layouts this deck can actually deal — those whose required cards the deck
    /// still enumerates. A cut card forecloses the layouts that read it, as a
    /// compile result rather than a list kept by hand.
    #[must_use]
    pub fn available_layouts(&self) -> Vec<&Layout> {
        let present: BTreeSet<String> = self.card_ids().into_iter().collect();
        self.layouts
            .iter()
            .filter(|layout| {
                layout
                    .requires_cards
                    .iter()
                    .all(|card| present.contains(card))
            })
            .collect()
    }

    /// A named layout, if the deck knows it *and can deal it*. `None` both when the
    /// layout is unknown and when it is foreclosed.
    #[must_use]
    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.available_layouts()
            .into_iter()
            .find(|layout| layout.name == name)
    }

    /// Whether this ruleset may be asserted — false while it is merely `Proposed`.
    #[must_use]
    pub const fn is_assertable(&self) -> bool {
        self.provenance.standing.assertable()
    }

    /// Fork a variant off this ruleset: apply its delta and take the variant's
    /// (proposed) provenance and game id. The result is a *proposed* ruleset,
    /// unassertable until ratified.
    #[must_use]
    pub fn fork(&self, variant: &Variant) -> Self {
        let mut next = self.clone();
        next.game_id.clone_from(&variant.game_id);
        next.provenance = variant.provenance.clone();
        for op in &variant.ops {
            next.apply(op);
        }
        next
    }

    /// Apply one delta op in place.
    pub fn apply(&mut self, op: &DeltaOp) {
        match op {
            DeltaOp::CutAxisValue { axis, value } => {
                if let Some(target) = self.axes.iter_mut().find(|candidate| &candidate.id == axis) {
                    target.values.retain(|entry| &entry.id != value);
                }
            }
            DeltaOp::RenameAxisValue { axis, value, to } => {
                if let Some(target) = self.axes.iter_mut().find(|candidate| &candidate.id == axis)
                    && let Some(entry) = target.values.iter_mut().find(|entry| &entry.id == value)
                {
                    entry.name.clone_from(to);
                }
            }
            DeltaOp::SetDecoration { selector, attrs } => {
                self.decorations
                    .retain(|decoration| &decoration.selector != selector);
                self.decorations.push(Decoration {
                    selector: selector.clone(),
                    attrs: attrs.clone(),
                });
            }
        }
    }

    /// Ratify this ruleset under a decision: lift its standing and record the
    /// decision, keeping its earlier citations.
    #[must_use]
    pub fn ratify(mut self, decision: &str) -> Self {
        self.provenance = self.provenance.ratified_by(decision);
        self
    }

    /// The merged field namespace a card render rule may keep from: its axis
    /// bindings (axis id → value id), the bound values' attrs and display `name`
    /// (innermost axis wins), and any matching decoration's attrs (which override).
    #[must_use]
    pub fn render_fields(&self, card: &Card) -> BTreeMap<String, AttrValue> {
        let mut fields: BTreeMap<String, AttrValue> = BTreeMap::new();
        if let Some(layer) = self.layers.iter().find(|layer| layer.id == card.layer) {
            for axis_id in &layer.axes {
                let Some(value_id) = card.bindings.get(axis_id) else {
                    continue;
                };
                fields.insert(axis_id.clone(), AttrValue::Text(value_id.clone()));
                if let Some(axis) = self.axis(axis_id)
                    && let Some(value) = axis.value(value_id)
                {
                    for (key, attr) in &value.attrs {
                        fields.insert(key.clone(), attr.clone());
                    }
                    fields.insert("name".to_owned(), AttrValue::Text(value.name.clone()));
                }
            }
        }
        for decoration in &self.decorations {
            if decoration.matches(card) {
                for (key, attr) in &decoration.attrs {
                    fields.insert(key.clone(), attr.clone());
                }
            }
        }
        fields
    }

    // -- internals ----------------------------------------------------------

    /// The Cartesian product of the named axes' values as binding maps, in
    /// outer-to-inner order. An unknown axis yields no cards (fail closed).
    fn enumerate_bindings(&self, axis_ids: &[String]) -> Vec<BTreeMap<String, String>> {
        let mut acc: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];
        for axis_id in axis_ids {
            let Some(axis) = self.axis(axis_id) else {
                return Vec::new();
            };
            let mut next = Vec::with_capacity(acc.len() * axis.values.len());
            for base in &acc {
                for value in &axis.values {
                    let mut bindings = base.clone();
                    bindings.insert(axis_id.clone(), value.id.clone());
                    next.push(bindings);
                }
            }
            acc = next;
        }
        acc
    }

    /// The number of cards a layer's axis product enumerates (`1` for a hub).
    fn layer_product_size(&self, axis_ids: &[String]) -> usize {
        axis_ids
            .iter()
            .map(|id| self.axis(id).map_or(0, |axis| axis.values.len()))
            .product()
    }

    /// `CyclicReach`: `next` is within `reach` steps forward of `prev` on the wheel.
    fn cyclic_follows(&self, prev: &str, next: &str, reach: u32) -> bool {
        Self::cyclic_follows_in(prev, next, reach, &self.leaves())
    }

    /// `CyclicReach` against a precomputed deck. The wheel is the deck's ids in
    /// enumeration order, so position lookups match `card_ids()` exactly.
    fn cyclic_follows_in(prev: &str, next: &str, reach: u32, deck: &[Card]) -> bool {
        let count = deck.len();
        if count == 0 {
            return false;
        }
        let (Some(from), Some(to)) = (
            deck.iter().position(|card| card.id == prev),
            deck.iter().position(|card| card.id == next),
        ) else {
            return false;
        };
        let forward = (to + count - from) % count;
        forward >= 1 && forward <= usize::try_from(reach).unwrap_or(usize::MAX)
    }

    /// `Ascending`: `next` shares the `same` axis bindings and its `axis` position
    /// advances by `step` (or any strictly-greater position when `step` is absent).
    fn ascending_follows(
        &self,
        prev: &str,
        next: &str,
        axis_id: &str,
        step: Option<u32>,
        same: &[String],
    ) -> bool {
        self.ascending_follows_in(prev, next, axis_id, step, same, &self.cards())
    }

    /// `Ascending` against a precomputed deck, so a batch of candidate checks
    /// reuses one `cards()` build. Behaviour is identical to `ascending_follows`.
    fn ascending_follows_in(
        &self,
        prev: &str,
        next: &str,
        axis_id: &str,
        step: Option<u32>,
        same: &[String],
        cards: &[Card],
    ) -> bool {
        let (Some(prev_card), Some(next_card)) = (
            cards.iter().find(|card| card.id == prev),
            cards.iter().find(|card| card.id == next),
        ) else {
            return false;
        };
        for shared in same {
            match (
                prev_card.bindings.get(shared),
                next_card.bindings.get(shared),
            ) {
                (Some(left), Some(right)) if left == right => {}
                _ => return false,
            }
        }
        let Some(axis) = self.axis(axis_id) else {
            return false;
        };
        let (Some(prev_value), Some(next_value)) = (
            prev_card.bindings.get(axis_id),
            next_card.bindings.get(axis_id),
        ) else {
            return false;
        };
        let (Some(prev_pos), Some(next_pos)) =
            (axis.position(prev_value), axis.position(next_value))
        else {
            return false;
        };
        match step {
            Some(step) => usize::try_from(step)
                .ok()
                .and_then(|step| prev_pos.checked_add(step))
                .is_some_and(|target| next_pos == target),
            None => next_pos > prev_pos,
        }
    }
}

// ---------------------------------------------------------------------------
// Structural invariant enumeration
// ---------------------------------------------------------------------------

/// Enumerate every declared `StructuralInvariant` over a ruleset as exact
/// `checked` verdicts. An all-held result is a structurally sound deck.
///
/// ```
/// use dli_game::binding::GameBindings;
/// use dli_game::model::{enumerate_structural_invariants, Ruleset};
///
/// let rules = r#"{
///   "game_id": "mini", "region": "sample",
///   "axes": [{ "id": "rank", "values": [
///     { "id": "a", "name": "A" }, { "id": "b", "name": "B" }] }],
///   "layers": [{ "id": "card", "axes": ["rank"], "id_template": "card-{rank}", "dealt": true }],
///   "ordering": "layer-product",
///   "moves": { "rule": "free" },
///   "layouts": [{ "name": "pair", "cards_dealt": 2 }],
///   "verdict": { "verdict": "completed" },
///   "invariants": [{ "invariant": "deck-is-a-set" }, { "invariant": "count-matches" }],
///   "decorations": [],
///   "provenance": { "standing": "ratified", "cites": ["D-0001"] }
/// }"#;
/// let deck = Ruleset::from_rules(rules).expect("valid rules");
/// let verdicts = enumerate_structural_invariants(&deck, &GameBindings::new());
/// assert_eq!(verdicts.len(), 2);
/// assert!(verdicts.iter().all(dli_sim::InvariantVerdict::held));
/// ```
#[must_use]
pub fn enumerate_structural_invariants(
    ruleset: &Ruleset,
    bindings: &GameBindings,
) -> Vec<InvariantVerdict> {
    ruleset
        .invariants
        .iter()
        .map(|invariant| {
            let outcome = match invariant.breach(ruleset, bindings) {
                None => InvariantOutcome::Held,
                Some(detail) => InvariantOutcome::Violated { trial: 0, detail },
            };
            InvariantVerdict::new(invariant.name(), outcome)
        })
        .collect()
}

fn breach_deck_is_a_set(ruleset: &Ruleset) -> Option<String> {
    let ids = ruleset.card_ids();
    let unique: BTreeSet<&String> = ids.iter().collect();
    (unique.len() != ids.len()).then(|| {
        format!(
            "the deck has {} cards but only {} distinct ids",
            ids.len(),
            unique.len()
        )
    })
}

fn breach_count_matches(ruleset: &Ruleset) -> Option<String> {
    let distinct: BTreeSet<String> = ruleset.card_ids().into_iter().collect();
    let declared = ruleset.card_count();
    (distinct.len() != declared).then(|| {
        format!(
            "the declared count is {declared} but the deck enumerates {} distinct cards",
            distinct.len()
        )
    })
}

fn breach_closed_ring(ruleset: &Ruleset) -> Option<String> {
    ruleset
        .card_ids()
        .is_empty()
        .then(|| "an empty deck has no closed ring".to_owned())
}

// ---------------------------------------------------------------------------
// Template rendering
// ---------------------------------------------------------------------------

/// Substitute `{axis}` placeholders in a template with the card's bindings. An
/// unknown placeholder is left literal; a template with no placeholders is copied.
#[must_use]
pub(crate) fn render_template(template: &str, bindings: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        if let Some(close) = after.find('}') {
            let key = &after[..close];
            if let Some(value) = bindings.get(key) {
                out.push_str(value);
            } else {
                out.push('{');
                out.push_str(key);
                out.push('}');
            }
            rest = &after[close + 1..];
        } else {
            out.push_str(&rest[open..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod hoist_equivalence {
    //! Proof that hoisting the deck build out of `legal_moves`' inner loop is
    //! behaviour-identical: the private `follows_in` (deck threaded in) must
    //! agree with the public `follows` (deck rebuilt per call) for every ordered
    //! pair of card ids, on both an `Ascending` and a `CyclicReach` deck. Being a
    //! child of `model`, this module can reach the private helper directly.

    use super::{Reading, Ruleset};

    const ASCENDING_RULES: &str = r#"{
      "game_id": "asc", "region": "sample",
      "axes": [
        { "id": "suit", "values": [ { "id": "s", "name": "S" }, { "id": "t", "name": "T" } ] },
        { "id": "rank", "values": [ { "id": "a", "name": "A" }, { "id": "b", "name": "B" } ] }
      ],
      "layers": [{ "id": "card", "axes": ["suit", "rank"], "id_template": "{rank}-{suit}", "dealt": true }],
      "ordering": "layer-product",
      "moves": { "rule": "ascending", "axis": "rank", "step": 1, "same": ["suit"] },
      "layouts": [{ "name": "run", "cards_dealt": 2 }],
      "verdict": { "verdict": "completed" },
      "invariants": [{ "invariant": "deck-is-a-set" }, { "invariant": "count-matches" }],
      "decorations": [],
      "provenance": { "standing": "ratified", "cites": ["D-0001"] }
    }"#;

    const CYCLIC_RULES: &str = r#"{
      "game_id": "cyc", "region": "sample",
      "axes": [{ "id": "pos", "values": [
        { "id": "x", "name": "X" }, { "id": "y", "name": "Y" }, { "id": "z", "name": "Z" }] }],
      "layers": [{ "id": "card", "axes": ["pos"], "id_template": "card-{pos}", "dealt": true }],
      "ordering": "layer-product",
      "moves": { "rule": "cyclic-reach", "reach": 1 },
      "layouts": [{ "name": "ring", "cards_dealt": 3 }],
      "verdict": { "verdict": "completed" },
      "invariants": [{ "invariant": "deck-is-a-set" }, { "invariant": "count-matches" }, { "invariant": "closed-ring" }],
      "decorations": [],
      "provenance": { "standing": "ratified", "cites": ["D-0001"] }
    }"#;

    /// `follows_in` (hoisted deck) matches `follows` (per-call rebuild) over every
    /// ordered pair — the exhaustive behaviour-identity check.
    fn assert_follows_agree(rules: &str) {
        let deck = Ruleset::from_rules(rules).expect("rules parse");
        let cards = deck.cards();
        let hoisted = deck.leaves();
        for prev in &cards {
            for next in &cards {
                assert_eq!(
                    deck.follows(&prev.id, &next.id),
                    deck.follows_in(&prev.id, &next.id, &hoisted),
                    "follows/follows_in disagree on {} -> {}",
                    prev.id,
                    next.id,
                );
            }
        }
    }

    fn reading(layout: &str, cards_dealt: u32, laid: &[&str]) -> Reading {
        Reading {
            layout: layout.to_owned(),
            cards_dealt,
            laid: laid.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    #[test]
    fn follows_in_matches_follows_ascending() {
        assert_follows_agree(ASCENDING_RULES);
    }

    #[test]
    fn follows_in_matches_follows_cyclic() {
        assert_follows_agree(CYCLIC_RULES);
    }

    #[test]
    fn ascending_legal_moves_and_terminal_unchanged() {
        let deck = Ruleset::from_rules(ASCENDING_RULES).expect("rules parse");
        assert_eq!(
            deck.cards()
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>(),
            vec!["a-s", "b-s", "a-t", "b-t"],
        );
        let open = reading("run", 2, &[]);
        assert_eq!(deck.legal_moves(&open).len(), 4);
        assert!(!deck.is_terminal(&open));

        let after_as = reading("run", 2, &["a-s"]);
        assert_eq!(deck.legal_moves(&after_as), vec!["b-s".to_owned()]);
        assert!(!deck.is_terminal(&after_as));

        let filled = reading("run", 2, &["a-s", "b-s"]);
        assert!(deck.is_terminal(&filled));
    }

    #[test]
    fn cyclic_legal_moves_and_terminal_unchanged() {
        let deck = Ruleset::from_rules(CYCLIC_RULES).expect("rules parse");
        let open = reading("ring", 3, &[]);
        assert_eq!(deck.legal_moves(&open).len(), 3);

        let after_x = reading("ring", 3, &["card-x"]);
        assert_eq!(deck.legal_moves(&after_x), vec!["card-y".to_owned()]);

        // Reach 1 wraps: from card-z the only forward step is card-x.
        let after_z = reading("ring", 3, &["card-z"]);
        assert_eq!(deck.legal_moves(&after_z), vec!["card-x".to_owned()]);
    }
}
