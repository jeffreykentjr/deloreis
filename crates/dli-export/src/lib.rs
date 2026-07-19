//! Deloreis Lore Interchange export profiles.
//!
//! A compiled model can be rendered out under an [`ExportProfile`]: a
//! context pack for downstream generation, printable cards, or Ludii GDL for the
//! game simulator. Every export carries an [`ExportManifest`] pinning the commit,
//! model layer, valuation, and a hash of the profile so an artifact is traceable
//! back to exactly what produced it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::valuation::Valuation;
use dli_world::model::WorldModel;

/// The export target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExportProfile {
    /// A context pack for downstream generation.
    ContextPack,
    /// Printable reference cards.
    Cards,
    /// Ludii Game Description Language for the simulator.
    LudiiGdl,
}

/// Provenance for an exported artifact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportManifest {
    /// The source commit the export was built from.
    pub commit: String,
    /// The model layer exported (e.g. ratified-only).
    pub layer: String,
    /// A fingerprint of the valuation the export was taken under.
    pub valuation: String,
    /// A hash of the export profile and its options.
    pub profile_hash: String,
}

/// Render a model out under a profile, returning its manifest.
pub fn export(
    _model: &WorldModel,
    _profile: ExportProfile,
    _valuation: &Valuation,
) -> ExportManifest {
    todo!("EXP-1: render the profile and pin a reproducible manifest")
}
