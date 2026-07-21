//! Loading craft principles from data (design §2.2, §2.4, §4).
//!
//! Step 2 of the craft-separation DAG makes the [`Principle`] record
//! **data-loadable**: with `Cow<'static, str>` fields and `Deserialize` in place,
//! the compiled [`PRINCIPLES`](super::kb::PRINCIPLES) corpus now round-trips
//! through a serialized record format ([`PrinciplePack`]). This module carries the
//! record-level (de)serialization plus a minimal file loader; the higher-level
//! doctrine-pack and world-config assembly are later steps (see the TODOs below).
//!
//! The Step-2 green criterion is the round-trip in this module's tests: serialize
//! the builtin 107 principles, deserialize, and assert the result equals the
//! compiled builtin — the proof that the data format faithfully carries the
//! compiled doctrine.
//!
//! ## Not yet here (later steps)
//!
//! - **Step 4 — shipped generic packs.** `doctrines() -> Vec<DoctrinePack>` over
//!   the world-agnostic `DoctrinePrinciple` records embedded from
//!   `craft-doctrines/*.toml` (design §1.1/§4). This module deliberately loads only
//!   the *runtime* [`Principle`] record today; the shipped `DoctrinePrinciple`
//!   type and the pack TOML land in Step 4.
//! - **Step 5 — world config.** `load_config(world_dir: &Path) -> Result<CraftConfig>`
//!   (serde over `worlds/<world>/craft/*.toml`), `apply(cfg, packs) -> Vec<Principle>`,
//!   and `validate(cfg, packs) -> Vec<ConfigError>` (design §2.4/§4). These depend on
//!   the `CraftConfig` / `DoctrinePack` / `PrincipleOverlay` types, which do not
//!   exist yet.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::kb::{Kb, Principle};

/// A loadable pack of runtime [`Principle`] records — the data form of a craft
/// corpus (design §2.2).
///
/// This is the Step-2 record format: it carries the *runtime* `Principle` (the
/// type today's [`PRINCIPLES`](super::kb::PRINCIPLES) table is built from), so the
/// compiled builtin can be emitted to data and loaded back identically. Step 4
/// introduces the world-agnostic `DoctrinePrinciple` pack + overlays that compose
/// *into* runtime principles; Step 5 folds those into a `CraftConfig`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PrinciplePack {
    /// The principles this pack carries, in corpus order.
    pub principles: Vec<Principle>,
}

impl PrinciplePack {
    /// A pack snapshotting the compiled builtin corpus
    /// ([`PRINCIPLES`](super::kb::PRINCIPLES)).
    ///
    /// The clone turns the const table's `Cow::Borrowed` fields into owned records;
    /// value-equality with the builtin is preserved (a `Cow` compares by content).
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            principles: Kb::builtin().principles().to_vec(),
        }
    }

    /// A borrow-generic [`Kb`] query handle over this pack's principles.
    #[must_use]
    pub fn kb(&self) -> Kb<'_> {
        Kb::new(&self.principles)
    }
}

/// Parse a [`PrinciplePack`] from its serialized record form (the minimal loader).
///
/// The record format is JSON here; the shipped generic packs move to embedded TOML
/// in Step 4 (design §1.1). The format is an implementation detail of the loader —
/// what Step 2 pins is that the [`Principle`] record deserializes faithfully.
///
/// # Errors
/// Returns [`LoadError::Parse`] when the input is not a valid pack record.
pub fn parse_pack(json: &str) -> Result<PrinciplePack, LoadError> {
    serde_json::from_str(json).map_err(LoadError::from)
}

/// Render a [`PrinciplePack`] to its serialized record form.
///
/// # Errors
/// Returns [`LoadError::Parse`] when the pack cannot be serialized.
pub fn render_pack(pack: &PrinciplePack) -> Result<String, LoadError> {
    serde_json::to_string_pretty(pack).map_err(LoadError::from)
}

/// Load a [`PrinciplePack`] from a record file on disk.
///
/// # Errors
/// Returns [`LoadError::Io`] when the file cannot be read and [`LoadError::Parse`]
/// when its contents are not a valid pack record.
pub fn load_pack(path: &Path) -> Result<PrinciplePack, LoadError> {
    let text = std::fs::read_to_string(path)?;
    parse_pack(&text)
}

/// A failure loading a craft pack from data.
#[derive(Debug)]
pub enum LoadError {
    /// Reading the pack file from disk failed.
    Io(std::io::Error),
    /// Deserializing (or serializing) the pack record format failed.
    Parse(serde_json::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "reading craft pack: {err}"),
            Self::Parse(err) => write!(f, "parsing craft pack: {err}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Parse(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for LoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(err: serde_json::Error) -> Self {
        Self::Parse(err)
    }
}

#[cfg(test)]
mod tests {
    use super::{PrinciplePack, parse_pack, render_pack};
    use crate::craft::kb;

    /// Step-2 green criterion (design §5): serialize the builtin 107 principles →
    /// deserialize → assert the result equals the compiled builtin, proving the
    /// data record format carries the compiled doctrine losslessly.
    #[test]
    fn builtin_pack_round_trips_through_the_record_format() {
        let builtin = PrinciplePack::builtin();
        assert_eq!(
            builtin.principles.len(),
            107,
            "the pack snapshots the compiled 107-principle corpus"
        );

        let json = render_pack(&builtin).expect("the builtin corpus serializes");
        let loaded = parse_pack(&json).expect("the record format deserializes");

        // Whole-pack value equality: the record round-trips.
        assert_eq!(loaded, builtin, "the pack round-trips by value");

        // The loaded corpus equals the *compiled* const table field-for-field —
        // `Cow::Owned` from data compares equal to `Cow::Borrowed` in the literal.
        assert_eq!(
            loaded.principles.as_slice(),
            kb::PRINCIPLES,
            "deserialized principles equal the compiled builtin"
        );
    }
}
