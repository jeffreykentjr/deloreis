//! **To Pleach the Hedgeway** (`D-0143`) — the world crate.
//!
//! This is a *world* crate, not an engine crate: it deliberately drops the `dli-`
//! prefix to mark that it holds one world's content, layered *over* the generic
//! De Lore Is engine (`dli-sim`, `dli-export`, `dli-core`) and never depended on by
//! it. The engine stays world-agnostic; the Turning lives here.
//!
//! # The Turning ([`turning`])
//!
//! The world's oracular deck (`D-0148`, woken by the `D-0147` remap method): **data
//! loaded by the generic [`dli_game`] engine plus a thin binding**. The structural
//! deck — axes, card families, the cyclic legal-move relation, layouts, variant
//! deltas, three of the four structural invariants — is authored in
//! `worlds/tpth/turning/*.json` and loaded with
//! [`Ruleset::from_rules`](turning::Ruleset::from_rules). Only the world's counsel
//! *verdict* and its *reciprocity invariant* stay as code, entering the engine
//! through [`turning_bindings`](turning::turning_bindings). The study
//! ([`honest_vs_rigged`](turning::honest_vs_rigged)) composes two generic
//! [`DealBattery`](dli_game::DealBattery)s over the loaded rulesets.
//!
//! # The card / Ludii export ([`export`])
//!
//! The world-side export dispatch ([`export::run`]) maps the region/variant
//! selectors onto a ruleset ([`export::ruleset_for`]) and delegates the whole
//! *straight structural render* (the `DL-0007` triviality bar) to
//! [`dli_game::export::run`] — a card set or a Ludii L-GDL program. It builds the
//! ruleset directly from the export options and needs no `CompiledWorld`, which is
//! why the card/Ludii lane needed no coupling to the engine's world model at all.

pub mod export;
pub mod turning;
