//! Partial-order chronology over events.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A point on the timeline (an interned event index). Deliberately not `Ord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TimePoint(pub u32);

/// A precedence relation over time points, stored as directed edges. The
/// order it induces is partial, not linear.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Chronology {
    /// Directed `(before, after)` precedence edges.
    pub edges: Vec<(TimePoint, TimePoint)>,
}

impl Chronology {
    /// Whether `earlier` precedes `later` under transitive reachability.
    #[must_use]
    pub fn precedes(&self, _earlier: TimePoint, _later: TimePoint) -> bool {
        todo!("TIME-1: transitive reachability over the precedence DAG")
    }
}
