//! The evaluable world program.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::decl::{FactDecl, RuleDecl};
use dli_time::chronology::Chronology;
use dli_time::wheel::Wheel;

/// A lowered, evaluable program: ground facts, rules, and time structures.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct World {
    /// Ground facts.
    pub facts: Vec<FactDecl>,
    /// Derivation rules.
    pub rules: Vec<RuleDecl>,
    /// The precedence chronology.
    pub chronology: Chronology,
    /// The cyclic calendars.
    pub wheels: Vec<Wheel>,
}

impl World {
    /// An empty world.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
