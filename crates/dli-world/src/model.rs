//! The compiled world model and its surrounding tables.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_eval::world::World;
use dli_ledger::{LedgerEntry, PracticeDef};
use dli_time::wheel::Wheel;

/// A diction register with its banned lexicon.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Register {
    /// The register name.
    pub name: String,
    /// Words banned in this register (the allegory is structural, not lexical).
    pub banned: Vec<String>,
}

/// The coined-term lexicon.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Lexicon {
    /// The coined terms.
    pub terms: Vec<String>,
}

/// A craft rule binding a principle to a register.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CraftRule {
    /// The craft principle.
    pub principle: String,
    /// The register it governs.
    pub register: String,
}

/// The season ledger: a seasonal wheel plus the practices tracked against it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SeasonLedger {
    /// The seasonal wheel, if one is defined.
    pub season_wheel: Option<Wheel>,
    /// Practice definitions checked this season.
    pub practices: Vec<PracticeDef>,
    /// Recorded ledger entries.
    pub entries: Vec<LedgerEntry>,
}

/// A compiled world model: the evaluable world plus its authoring tables.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorldModel {
    /// The evaluable world program.
    pub world: World,
    /// Diction registers.
    pub registers: Vec<Register>,
    /// The coined-term lexicon.
    pub lexicon: Lexicon,
    /// Craft rules.
    pub craft_rules: Vec<CraftRule>,
    /// The season ledger.
    pub season: SeasonLedger,
}
