//! **To Pleach the Hedgeway** (`D-0143`) — the world crate.
//!
//! This is a *world* crate, not an engine crate: it deliberately drops the `dli-`
//! prefix to mark that it holds one world's content, layered *over* the generic
//! De Lore Is engine (`dli-sim`, `dli-export`, `dli-core`) and never depended on by
//! it. The engine stays world-agnostic; the Turning lives here.
//!
//! # The Turning ([`turning`])
//!
//! The world's oracular deck (`D-0148`, woken by the `D-0147` remap method): a
//! typed [`Ruleset`](turning::Ruleset) that is *data, not a simulator* — leaves, a
//! legal-move relation, terminal conditions, and variant deltas, with no run state.
//! It reads SIM-1's seams whole (`dli_sim::rng`, `dli_sim::manifest`) for its deal
//! battery, so a Turning study inherits the engine's grade-honesty oracle.
//!
//! # The card / Ludii export ([`export`])
//!
//! The world-side export dispatch ([`export::run`]): it compiles the same ruleset
//! to a card set ([`export::compile_cards`]) or a Ludii L-GDL program
//! ([`export::compile_ludii`]) — a *straight structural render* (the `DL-0007`
//! triviality bar). It builds the ruleset directly from the export options and pins
//! an engine [`ExportManifest`](dli_export::ExportManifest); it holds no
//! `CompiledWorld`, which is why the card/Ludii lane needed no coupling to the
//! engine's world model at all.

pub mod export;
pub mod turning;
