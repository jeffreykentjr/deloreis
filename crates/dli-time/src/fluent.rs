//! Fluent timelines: how a fluent's valuation changes over time.

use serde::{Deserialize, Serialize};

use dli_core::valuation::Valuation;

use crate::chronology::TimePoint;

/// A record of a fluent's value at successive time points (SURF/TIME-1 stub).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FluentTimeline {
    /// Value snapshots keyed by time point, in insertion order.
    pub entries: Vec<(TimePoint, Valuation)>,
}

impl FluentTimeline {
    /// The valuation in effect at a given time point.
    #[must_use]
    pub fn value_at(&self, _point: TimePoint) -> Option<&Valuation> {
        todo!("TIME-1: last-write-wins lookup along the precedence order")
    }
}
