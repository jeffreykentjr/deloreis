//! Deloreis Lore Interchange generic card-game engine (`GAME-1`).
//!
//! A world's card game reduces to **declared axes → card families → a positional or
//! facet move relation → declared layouts and invariants**, with only the world's
//! verdict predicate and any bespoke structural invariant remaining as code. This
//! crate is that generic machinery: a `model::Ruleset` that is *data, not a
//! simulator*, a generic `battery::DealBattery` atop the shared honesty substrate,
//! and a generic `export` that renders the ruleset to a card set or Ludii `L-GDL`.
//!
//! # The three consumers of one ruleset
//!
//! - the **play relation** — `model::Ruleset::legal_moves`,
//!   `model::Ruleset::is_terminal`, `model::Ruleset::verdict` — enumerates what
//!   plays follow what and how a reading closes;
//! - the **deal battery** (`battery::DealBattery`) deals seeded readings through the
//!   `dli_sim` honesty seams (`SplitMix64`, `RunManifest`), so structural sweeps
//!   grade `checked` and rates grade `tested`;
//! - the **card / `GDL` export** (`export::run`) compiles the same ruleset
//!   structurally, honoring a profile's declared render rules.
//!
//! # The neutral boundary
//!
//! This is an *engine* crate: it speaks only neutral vocabulary — axis, value,
//! layer, card, hub, wheel — and hardcodes no world's proper nouns. A world's
//! content (its axes, its layers, its verdict) enters as data through
//! `model::Ruleset::from_rules` and as named predicates through
//! `binding::GameBindings`. One engine spans a linear ranked deck
//! (`model::MoveRule::Ascending`) and a rankless cyclic wheel
//! (`model::MoveRule::CyclicReach`) alike.

pub mod battery;
pub mod binding;
pub mod export;
pub mod model;

#[cfg(test)]
mod tests;

pub use battery::{DealBattery, ReadingInvariant, ReadingMetric, ReadingObservable};
pub use binding::{GameBindings, InvariantFn, VerdictFn};
pub use export::{
    CardDeck, DeckDiff, DifferentialStatus, GameArtifact, GameExport, LudiiExport, RenderedCard,
};
pub use model::{
    AttrValue, Axis, AxisValue, Card, Decoration, DeltaOp, Layer, Layout, MoveRule, Ordering,
    Provenance, Reading, Ruleset, Standing, StructuralInvariant, Variant, Verdict, VerdictRule,
    enumerate_structural_invariants,
};
