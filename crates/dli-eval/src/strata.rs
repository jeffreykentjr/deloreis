//! Stratification of the rule set.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Predicates partitioned into evaluation strata, lowest dependency first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Strata {
    /// Each level names the predicates evaluated together; earlier levels are
    /// saturated before later ones consume them.
    pub levels: Vec<Vec<String>>,
}
