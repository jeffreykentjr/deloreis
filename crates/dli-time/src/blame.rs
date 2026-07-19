//! Cycle blame: which `before`-facts close a precedence loop.
//!
//! A precedence cycle is a contradiction in story-time — a run of `before`-facts
//! that says an event precedes itself. The repair is authorial (retract one of
//! the rulings), so the report names the **exact edges** that close the loop and,
//! when the loader supplies them, the [`Ann`]s that declared each one: declaring
//! an order is a ruling (`docs/spec/03-time.md` §3), so a cycle is a set of
//! rulings in conflict, not a nameless graph error.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::ann::Ann;
use dli_core::atom::{Atom, Interner};

use crate::chronology::TimePoint;

/// One `before`-fact implicated in a precedence cycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BlamedEdge {
    /// The edge's index in [`Chronology::edges`](crate::chronology::Chronology::edges).
    pub edge_index: usize,
    /// The point the fact places earlier.
    pub before: TimePoint,
    /// The point the fact places later.
    pub after: TimePoint,
    /// The ruling that declared this order, when the loader supplied its
    /// parallel `anns` table; `None` when blame was computed without rulings.
    pub ann: Option<Ann>,
}

/// A precedence cycle, reported as the closed loop of `before`-facts.
///
/// The edges are in loop order — each edge's `after` is the next edge's `before`,
/// and the last edge's `after` is the first edge's `before`. An empty cycle is a
/// defensive placeholder that never arises from real detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CycleBlame {
    /// The `before`-facts that close the loop, in order.
    pub cycle: Vec<BlamedEdge>,
}

impl CycleBlame {
    /// The number of edges in the cycle.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cycle.len()
    }

    /// Whether the report names no edges (never true for a detected cycle).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cycle.is_empty()
    }

    /// Whether every blamed edge carries the ruling that declared it.
    #[must_use]
    pub fn is_fully_attributed(&self) -> bool {
        !self.cycle.is_empty() && self.cycle.iter().all(|edge| edge.ann.is_some())
    }

    /// Render the cycle for a reader, naming the points through the time-point
    /// table (`World::time_points`). Edges backed by a ruling are marked, so a
    /// human sees which declared orders must be reconciled.
    ///
    /// ```
    /// use dli_core::atom::Interner;
    /// use dli_time::chronology::{Chronology, TimePoint};
    /// let mut names = Interner::new();
    /// names.intern("breaking-of-the-stones");
    /// names.intern("tithe-of-minds-sweeps");
    /// let chrono = Chronology {
    ///     edges: vec![(TimePoint(0), TimePoint(1)), (TimePoint(1), TimePoint(0))],
    /// };
    /// let blame = chrono.find_cycle(&[]).expect("cycle");
    /// let text = blame.render(&names);
    /// assert!(text.contains("breaking-of-the-stones"));
    /// assert!(text.contains("tithe-of-minds-sweeps"));
    /// ```
    #[must_use]
    pub fn render(&self, time_points: &Interner) -> String {
        if self.cycle.is_empty() {
            return "no precedence cycle".to_owned();
        }
        let steps: Vec<String> = self
            .cycle
            .iter()
            .map(|edge| {
                let before = name_of(time_points, edge.before);
                let after = name_of(time_points, edge.after);
                let ruling = if edge.ann.is_some() {
                    " [declared by a ruling]"
                } else {
                    ""
                };
                format!("before({before} -> {after}){ruling}")
            })
            .collect();
        format!("precedence cycle: {}", steps.join("; "))
    }
}

/// Resolve a point to its interned name, falling back to `#index` when the name
/// table does not know it.
fn name_of(time_points: &Interner, point: TimePoint) -> String {
    time_points
        .resolve(Atom(point.0))
        .map_or_else(|| format!("#{}", point.0), ToOwned::to_owned)
}
