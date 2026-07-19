//! Deloreis Lore Interchange practice & resource ledger.
//!
//! A **practice** is a declared system of laws — a magic system, a divination
//! game, a proof discipline — expressed entirely as data
//! (`docs/spec/04-practices.md`). This crate is the workbench's declaration
//! layer: it compiles a practice's declaration into a typed
//! [`PracticeDef`], checks it for well-formedness, and
//! makes the closed [`resource`] algebra its castings spend against *real* as
//! pure semantics over signed deltas. The generic ledger checker that runs a
//! single casting against a compiled practice is the [`ledger`] seam (PRAC-2).

pub mod ledger;
pub mod practice;
pub mod resource;

pub use ledger::{LedgerEntry, LedgerReport, LedgerViolation, check_ledger};
pub use practice::PracticeDef;
pub use resource::{Delta, GradeDirection, ResourceKind};
