//! Partial-order chronology over events.
//!
//! Story-time is *honestly partial* (`docs/spec/03-time.md` §1). Two events are
//! ordered only by an explicit `before`-fact or by the era spine lifted onto
//! them; absent that, they are **unordered**, and [`Chronology::precedes`]
//! answers [`TemporalVerdict::Unordered`] as a first-class verdict — never a
//! silent tie-break. Consistency is cycle detection, and a cycle is reported as
//! **blame**: the exact `before`-facts that close the loop
//! ([`Chronology::find_cycle`], rendered by [`crate::blame::CycleBlame`]).
//!
//! Nothing here is `Ord`. Linear comparison of two time points would fake a
//! total order no one ruled, so the type refuses it; graph bookkeeping keys on
//! the interned *index* ([`TimePoint`]`.0`), which is identity, not time.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::ann::Ann;

use crate::blame::{BlamedEdge, CycleBlame};

/// A point on the timeline (an interned event or era index). Deliberately not
/// `Ord`: time points are compared through [`Chronology::precedes`], which may
/// answer [`TemporalVerdict::Unordered`], not through `<`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TimePoint(pub u32);

/// The three-valued answer to *does one point precede another?*
///
/// `Unordered` is the honest answer for two events no `before`-fact and no era
/// lift has related — a real verdict a caller must handle, not a coin-flip
/// resolved silently (`docs/spec/03-time.md` §1; koan *unordered is an answer*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TemporalVerdict {
    /// The earlier point reaches the later one through declared precedence.
    Before,
    /// The later point reaches the earlier one — the query runs backwards.
    After,
    /// Neither reaches the other: no one has declared the order.
    Unordered,
}

/// A precedence relation over time points, stored as directed edges. The order
/// it induces is partial, not linear.
///
/// Each edge is a lowered `before`-fact or a consecutive era-order link. The
/// declaring rulings ([`Ann`]s) are *not* stored on the chronology — the loader
/// keeps them in parallel and hands them to [`find_cycle`](Chronology::find_cycle)
/// when it wants annotated blame, so this type stays a plain graph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Chronology {
    /// Directed `(before, after)` precedence edges, in declaration order.
    pub edges: Vec<(TimePoint, TimePoint)>,
}

/// A DFS coloring used only for cycle detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

impl Chronology {
    /// Whether `earlier` precedes `later`, or the two are unordered.
    ///
    /// The verdict is reachability over the precedence DAG: `Before` when
    /// `earlier` reaches `later`, `After` when `later` reaches `earlier`, and
    /// `Unordered` when neither does. A point never precedes itself in an
    /// acyclic chronology.
    ///
    /// This assumes the chronology is acyclic; run [`find_cycle`](Chronology::find_cycle)
    /// first if that is in doubt. Under a cycle, mutual reachability resolves to
    /// `Before` and the real answer is the blame report.
    ///
    /// ```
    /// use dli_time::chronology::{Chronology, TemporalVerdict, TimePoint};
    /// // a -> b -> c, and a standalone d.
    /// let chrono = Chronology {
    ///     edges: vec![(TimePoint(0), TimePoint(1)), (TimePoint(1), TimePoint(2))],
    /// };
    /// assert_eq!(chrono.precedes(TimePoint(0), TimePoint(2)), TemporalVerdict::Before);
    /// assert_eq!(chrono.precedes(TimePoint(2), TimePoint(0)), TemporalVerdict::After);
    /// // No fact relates 0 and 3 — that is a real answer, not a tie-break.
    /// assert_eq!(chrono.precedes(TimePoint(0), TimePoint(3)), TemporalVerdict::Unordered);
    /// // Nothing precedes itself.
    /// assert_eq!(chrono.precedes(TimePoint(0), TimePoint(0)), TemporalVerdict::Unordered);
    /// ```
    #[must_use]
    pub fn precedes(&self, earlier: TimePoint, later: TimePoint) -> TemporalVerdict {
        let adjacency = self.adjacency();
        if reaches(&adjacency, earlier.0, later.0) {
            TemporalVerdict::Before
        } else if reaches(&adjacency, later.0, earlier.0) {
            TemporalVerdict::After
        } else {
            TemporalVerdict::Unordered
        }
    }

    /// The first precedence cycle, blamed on the `before`-facts that close it.
    ///
    /// `anns[i]` is taken to be the ruling that declared `edges[i]` — the loader
    /// keeps that parallel table. Pass `&[]` when the rulings are not to hand;
    /// the blamed edges then carry `None` for their [`Ann`]. Returns `None` when
    /// the chronology is acyclic.
    ///
    /// ```
    /// use dli_time::chronology::{Chronology, TimePoint};
    /// // a -> b -> a is a contradiction; blame names both closing edges.
    /// let chrono = Chronology {
    ///     edges: vec![(TimePoint(0), TimePoint(1)), (TimePoint(1), TimePoint(0))],
    /// };
    /// let blame = chrono.find_cycle(&[]).expect("cycle");
    /// assert_eq!(blame.cycle.len(), 2);
    /// assert!(Chronology::default().find_cycle(&[]).is_none());
    /// ```
    #[must_use]
    pub fn find_cycle(&self, anns: &[Ann]) -> Option<CycleBlame> {
        let adjacency = self.adjacency();
        let mut color: BTreeMap<u32, Color> = BTreeMap::new();
        for start in self.node_set() {
            if color_of(&color, start) == Color::White {
                let mut path_nodes = vec![start];
                let mut path_edges = vec![usize::MAX];
                if let Some(cycle) = dfs_find_cycle(
                    start,
                    &adjacency,
                    &mut color,
                    &mut path_nodes,
                    &mut path_edges,
                ) {
                    return Some(self.blame(&cycle, anns));
                }
            }
        }
        None
    }

    /// The events partitioned into topological layers, least-constrained first:
    /// layer 0 holds every point no declared edge points *into*, and each later
    /// layer holds points whose predecessors are all already placed.
    ///
    /// `universe` names the full set of points to lay out — pass the time-point
    /// table so events with no `before`-facts still appear (they land in layer
    /// 0). Points referenced only by edges are included defensively. On a cycle
    /// the layering is impossible and the blamed loop is returned instead (its
    /// edges carry no [`Ann`]; call [`find_cycle`](Chronology::find_cycle) with
    /// the ruling table for annotated blame).
    ///
    /// ```
    /// use dli_time::chronology::{Chronology, TimePoint};
    /// let chrono = Chronology {
    ///     edges: vec![(TimePoint(0), TimePoint(1)), (TimePoint(0), TimePoint(2))],
    /// };
    /// let layers = chrono.topological_layers(&[]).expect("acyclic");
    /// assert_eq!(layers, vec![vec![TimePoint(0)], vec![TimePoint(1), TimePoint(2)]]);
    /// ```
    pub fn topological_layers(
        &self,
        universe: &[TimePoint],
    ) -> Result<Vec<Vec<TimePoint>>, CycleBlame> {
        let mut nodes: BTreeSet<u32> = universe.iter().map(|point| point.0).collect();
        for (from, to) in &self.edges {
            nodes.insert(from.0);
            nodes.insert(to.0);
        }

        let mut unique_edges: BTreeSet<(u32, u32)> = BTreeSet::new();
        for (from, to) in &self.edges {
            unique_edges.insert((from.0, to.0));
        }

        let mut indegree: BTreeMap<u32, usize> = nodes.iter().map(|&node| (node, 0)).collect();
        let mut outgoing: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for &(from, to) in &unique_edges {
            *indegree.entry(to).or_default() += 1;
            outgoing.entry(from).or_default().push(to);
        }

        let mut layers: Vec<Vec<TimePoint>> = Vec::new();
        let mut current: Vec<u32> = nodes
            .iter()
            .copied()
            .filter(|node| indegree.get(node).copied().unwrap_or(0) == 0)
            .collect();
        let mut placed = 0usize;
        while !current.is_empty() {
            current.sort_unstable();
            placed += current.len();
            let mut next: Vec<u32> = Vec::new();
            for node in &current {
                if let Some(neighbors) = outgoing.get(node) {
                    for &downstream in neighbors {
                        if let Some(count) = indegree.get_mut(&downstream) {
                            *count -= 1;
                            if *count == 0 {
                                next.push(downstream);
                            }
                        }
                    }
                }
            }
            layers.push(current.iter().map(|&node| TimePoint(node)).collect());
            current = next;
        }

        if placed < nodes.len() {
            return Err(self
                .find_cycle(&[])
                .unwrap_or_else(|| CycleBlame { cycle: Vec::new() }));
        }
        Ok(layers)
    }

    /// Build `from -> [(edge index, to)]` adjacency once for a traversal.
    fn adjacency(&self) -> BTreeMap<u32, Vec<(usize, u32)>> {
        let mut adjacency: BTreeMap<u32, Vec<(usize, u32)>> = BTreeMap::new();
        for (index, (from, to)) in self.edges.iter().enumerate() {
            adjacency.entry(from.0).or_default().push((index, to.0));
        }
        adjacency
    }

    /// Every point that appears as an endpoint of some edge.
    fn node_set(&self) -> BTreeSet<u32> {
        let mut nodes = BTreeSet::new();
        for (from, to) in &self.edges {
            nodes.insert(from.0);
            nodes.insert(to.0);
        }
        nodes
    }

    /// Turn a run of cycle edge indices into a blamed loop, attaching each
    /// edge's ruling from the parallel `anns` table when present.
    fn blame(&self, edge_indices: &[usize], anns: &[Ann]) -> CycleBlame {
        let cycle = edge_indices
            .iter()
            .filter_map(|&index| {
                let &(before, after) = self.edges.get(index)?;
                Some(BlamedEdge {
                    edge_index: index,
                    before,
                    after,
                    ann: anns.get(index).cloned(),
                })
            })
            .collect();
        CycleBlame { cycle }
    }
}

/// A point's color, defaulting to `White` for points not yet visited.
fn color_of(color: &BTreeMap<u32, Color>, node: u32) -> Color {
    color.get(&node).copied().unwrap_or(Color::White)
}

/// Whether `dst` is reachable from `src` over a path of at least one edge.
fn reaches(adjacency: &BTreeMap<u32, Vec<(usize, u32)>>, src: u32, dst: u32) -> bool {
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut queue: VecDeque<u32> = VecDeque::new();
    if let Some(neighbors) = adjacency.get(&src) {
        for &(_, next) in neighbors {
            queue.push_back(next);
        }
    }
    while let Some(node) = queue.pop_front() {
        if node == dst {
            return true;
        }
        if !seen.insert(node) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&node) {
            for &(_, next) in neighbors {
                queue.push_back(next);
            }
        }
    }
    false
}

/// Depth-first search for a back edge, returning the cycle's edge indices in
/// loop order. `path_nodes[k]` is entered by `path_edges[k]` (index 0 is a
/// sentinel root with no entering edge).
fn dfs_find_cycle(
    node: u32,
    adjacency: &BTreeMap<u32, Vec<(usize, u32)>>,
    color: &mut BTreeMap<u32, Color>,
    path_nodes: &mut Vec<u32>,
    path_edges: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    color.insert(node, Color::Gray);
    if let Some(neighbors) = adjacency.get(&node) {
        for &(edge_index, next) in neighbors {
            match color_of(color, next) {
                Color::Gray => {
                    let start = path_nodes.iter().position(|&n| n == next).unwrap_or(0);
                    let mut cycle: Vec<usize> = path_edges[start + 1..].to_vec();
                    cycle.push(edge_index);
                    return Some(cycle);
                }
                Color::White => {
                    path_nodes.push(next);
                    path_edges.push(edge_index);
                    if let Some(cycle) =
                        dfs_find_cycle(next, adjacency, color, path_nodes, path_edges)
                    {
                        return Some(cycle);
                    }
                    path_nodes.pop();
                    path_edges.pop();
                }
                Color::Black => {}
            }
        }
    }
    color.insert(node, Color::Black);
    None
}
