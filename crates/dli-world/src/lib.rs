//! Deloreis Lore Interchange world model.
//!
//! A [`Pack`] of documents [`compile`]s into a [`WorldModel`]: the evaluable
//! world plus registers, lexicon, craft rules, and the season ledger. Drafts
//! arrive as [`DraftPayload`]s and are fact-checked with [`check_draft`];
//! [`Attestation`]s are recomputed for staleness with [`sweep`].

pub mod attest;
pub mod check;
pub mod draft;
pub mod episode;
pub mod language;
pub mod loader;
pub mod model;
pub mod pack;

pub use attest::{Attestation, AttestationStore};
pub use draft::{DraftFinding, DraftMeta, DraftPayload};
pub use model::{CraftRule, Lexicon, Register, SeasonLedger, WorldModel};
pub use pack::Pack;

use dli_eval::error::EvalError;

/// Compile a pack into a status-gated world model.
pub fn compile(_pack: &Pack) -> Result<WorldModel, EvalError> {
    todo!("FRONT-1: compile documents, gating on ratification status")
}

/// Recompute which stored attestations are stale against the current model.
pub fn sweep(_store: &AttestationStore, _model: &WorldModel) -> Vec<Attestation> {
    todo!("ATT-1: return attestations whose sweep fingerprint no longer matches")
}

/// Fact-check a draft payload against the world model.
pub fn check_draft(_model: &WorldModel, _draft: &DraftPayload) -> Vec<DraftFinding> {
    todo!("CHECK-1: classify each draft claim as supported/contradicted/unsupported")
}
