#![allow(clippy::print_stdout, clippy::print_stderr)]
//! Deloreis Lore Interchange build automation.
//!
//! `xtask` owns the gate (the canonical build/test/lint pipeline plus its
//! receipt), schema emission, the closed fixture registry check, the docs
//! oracle, the LOC budget, and the hooks check. The gate list here and the
//! CI workflow are kept in lockstep by `tests/ci_parity.rs`.

pub mod book;
pub mod boundary;
pub mod docgen;
pub mod docs_oracle;
pub mod docs_tokens;
pub mod finding_families;
pub mod fixtures;
pub mod gate;
pub mod hooks;
pub mod jsonval;
pub mod loc;
pub mod schema;
pub mod util;
