//! Slack: earliest/latest legal slots (`TIME-3`).
//!
//! The season checker (`docs/spec/03-time.md` §5) reports, for every episode,
//! the **earliest and latest legal production slot** it could occupy while every
//! ordering constraint still holds. The band between them is the episode's
//! *slack*: a zero-slack episode is pinned; a wide band is free to move. This is
//! the classic critical-path computation — a **forward pass** for earliest and a
//! **backward pass** for latest — over the partial order of episodes.
//!
//! Two kinds of constraint tighten the band:
//!
//! - **Production-order precedences** ([`Precedence`]) — episode A must reach the
//!   listener before episode B. These form a DAG; the passes run over its
//!   [topological layers](Chronology::topological_layers), so this module
//!   consumes the frozen chronology engine rather than re-deriving ordering.
//! - **Per-episode floors and ceilings** — a `slot >= N` order rule floors an
//!   episode; a `slot <= N` rule caps it.
//!
//! # Standing threads through slack (carry-item R2-G)
//!
//! A precedence carries the standing of the ruling that declared it. A *merely
//! proposed* ordering cannot harden the schedule — it would pin a slot on an
//! order no one has ratified — so [`slack`] uses only **certain** precedences
//! for the binding band, exactly as [`crate::order_rules::precedes_with_standing`]
//! refuses to let a proposed edge answer a hard `Before`. Possible precedences
//! are reported in [`SlackReport::possible_precedences`] as tightenings that
//! *would* apply once ratified, never folded silently into the band.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::ann::{Ann, Status, Theory};
use dli_core::valuation::Valuation;

use crate::chronology::{Chronology, TimePoint};

/// One production-order precedence: `before` must reach the listener strictly
/// before `after`, backed by the ruling that declared the ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Precedence {
    /// The episode that must come first in production order.
    pub before: String,
    /// The episode that must come later in production order.
    pub after: String,
    /// The standing of the ordering constraint. Only a certain precedence
    /// hardens the schedule.
    pub ann: Ann,
}

impl Precedence {
    /// A ratified (unconditional) precedence — the common case.
    #[must_use]
    pub fn certain(before: impl Into<String>, after: impl Into<String>) -> Self {
        Self {
            before: before.into(),
            after: after.into(),
            ann: Ann::unconditional(),
        }
    }

    /// A precedence with an explicit standing.
    #[must_use]
    pub fn new(before: impl Into<String>, after: impl Into<String>, ann: Ann) -> Self {
        Self {
            before: before.into(),
            after: after.into(),
            ann,
        }
    }
}

/// The inputs to a slack computation: the season's episodes, the ordering
/// constraints among them, and the per-episode slot floors/ceilings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlackInput {
    /// Every episode in the season, by id — the universe and the slot count `N`.
    pub episodes: Vec<String>,
    /// The production-order precedences among episodes.
    pub precedences: Vec<Precedence>,
    /// Per-episode earliest-slot floors (from `slot >= N` order rules).
    pub floors: BTreeMap<String, u32>,
    /// Per-episode latest-slot ceilings (from `slot <= N` order rules).
    pub ceilings: BTreeMap<String, u32>,
}

/// One episode's legal production-slot band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EpisodeSlack {
    /// The episode's id.
    pub episode: String,
    /// The earliest legal production slot (1-based).
    pub earliest: u32,
    /// The latest legal production slot (1-based).
    pub latest: u32,
    /// Whether the band is non-empty and within `1..=N` — a legal slot exists.
    pub feasible: bool,
}

impl EpisodeSlack {
    /// The width of the legal band: `latest - earliest`, or zero when
    /// infeasible. A slack of zero on a feasible episode means it is *pinned*.
    #[must_use]
    pub fn slack(&self) -> u32 {
        if self.feasible {
            self.latest.saturating_sub(self.earliest)
        } else {
            0
        }
    }

    /// Whether exactly one legal slot remains — the episode is pinned there.
    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.feasible && self.earliest == self.latest
    }
}

/// The result of a slack computation over a season.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlackReport {
    /// Per-episode legal bands, in the input's episode order. Empty when the
    /// certain precedences contradict (see [`cycle`](Self::cycle)).
    pub slots: Vec<EpisodeSlack>,
    /// A precedence cycle among the certain constraints, as the episode ids in
    /// the loop; `None` when the certain precedences are acyclic. A cycle makes
    /// the whole season unschedulable, so [`slots`](Self::slots) is left empty.
    pub cycle: Option<Vec<String>>,
    /// The precedences that were merely *possible* and so did not harden the
    /// band — the tightenings that would apply once their rulings land.
    pub possible_precedences: Vec<Precedence>,
}

impl SlackReport {
    /// The band for a named episode, if present.
    #[must_use]
    pub fn for_episode(&self, episode: &str) -> Option<&EpisodeSlack> {
        self.slots.iter().find(|s| s.episode == episode)
    }

    /// Whether every episode has a feasible band and no precedence cycle exists.
    #[must_use]
    pub fn is_feasible(&self) -> bool {
        self.cycle.is_none() && self.slots.iter().all(|s| s.feasible)
    }
}

/// Compute each episode's earliest/latest legal production slot.
///
/// Only **certain** precedences bind the band; possible ones are set aside in
/// [`SlackReport::possible_precedences`]. Earliest slots come from a forward
/// pass (an episode falls after everything that must precede it, and no earlier
/// than its floor); latest slots from a backward pass (before everything it must
/// precede, no later than its ceiling). A cycle in the certain precedences is
/// reported and leaves the bands empty.
///
/// ```
/// use std::collections::BTreeMap;
/// use dli_core::ann::Theory;
/// use dli_core::valuation::Valuation;
/// use dli_time::slack::{slack, Precedence, SlackInput};
///
/// // Three episodes, a must precede b precede c, all ratified. With N = 3 the
/// // chain pins every episode: a@1, b@2, c@3.
/// let input = SlackInput {
///     episodes: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
///     precedences: vec![Precedence::certain("a", "b"), Precedence::certain("b", "c")],
///     floors: BTreeMap::new(),
///     ceilings: BTreeMap::new(),
/// };
/// let report = slack(&input, &Valuation::new(), &Theory::new());
/// assert!(report.is_feasible());
/// let a = report.for_episode("a").unwrap();
/// assert_eq!((a.earliest, a.latest), (1, 1));
/// assert!(a.is_pinned());
/// let c = report.for_episode("c").unwrap();
/// assert_eq!((c.earliest, c.latest), (3, 3));
/// ```
#[must_use]
pub fn slack(input: &SlackInput, valuation: &Valuation, theory: &Theory) -> SlackReport {
    // The episode universe: the declared list, plus any id mentioned only in a
    // constraint (defensive — a floor on an unlisted episode still counts).
    let mut ids: Vec<String> = input.episodes.clone();
    for precedence in &input.precedences {
        push_unique(&mut ids, &precedence.before);
        push_unique(&mut ids, &precedence.after);
    }
    for id in input.floors.keys().chain(input.ceilings.keys()) {
        push_unique(&mut ids, id);
    }

    let n = u32::try_from(ids.len()).unwrap_or(u32::MAX);
    if n == 0 {
        return SlackReport::default();
    }

    let index: BTreeMap<&str, u32> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), u32::try_from(i).unwrap_or(0)))
        .collect();

    // Partition precedences by standing: only certain ones bind the schedule.
    let mut certain_edges: Vec<(TimePoint, TimePoint)> = Vec::new();
    let mut possible_precedences: Vec<Precedence> = Vec::new();
    for precedence in &input.precedences {
        let (Some(&before), Some(&after)) = (
            index.get(precedence.before.as_str()),
            index.get(precedence.after.as_str()),
        ) else {
            continue;
        };
        match precedence.ann.status(valuation, theory) {
            Status::Certain => certain_edges.push((TimePoint(before), TimePoint(after))),
            Status::Possible => possible_precedences.push(precedence.clone()),
            Status::Dead => {}
        }
    }

    let chronology = Chronology {
        edges: certain_edges,
    };
    let universe: Vec<TimePoint> = (0..n).map(TimePoint).collect();
    let layers = match chronology.topological_layers(&universe) {
        Ok(layers) => layers,
        Err(blame) => {
            let cycle = blame
                .cycle
                .iter()
                .map(|edge| ids[edge.before.0 as usize].clone())
                .collect();
            return SlackReport {
                slots: Vec::new(),
                cycle: Some(cycle),
                possible_precedences,
            };
        }
    };

    let predecessors = adjacency(&chronology, Direction::Into);
    let successors = adjacency(&chronology, Direction::OutOf);
    let floor_of = |id: &str| input.floors.get(id).copied().unwrap_or(1).max(1);
    let ceiling_of = |id: &str| input.ceilings.get(id).copied().unwrap_or(n).min(n);

    // Forward pass: earliest[e] respects its floor and sits after every
    // predecessor's earliest. Topological order means predecessors are done.
    let mut earliest: BTreeMap<u32, u32> = BTreeMap::new();
    for layer in &layers {
        for point in layer {
            let mut value = floor_of(&ids[point.0 as usize]);
            for &pred in predecessors.get(&point.0).into_iter().flatten() {
                let pred_earliest = earliest.get(&pred).copied().unwrap_or(1);
                value = value.max(pred_earliest.saturating_add(1));
            }
            earliest.insert(point.0, value);
        }
    }

    // Backward pass: latest[e] respects its ceiling and sits before every
    // successor's latest. Reverse topological order.
    let mut latest: BTreeMap<u32, u32> = BTreeMap::new();
    for layer in layers.iter().rev() {
        for point in layer {
            let mut value = ceiling_of(&ids[point.0 as usize]);
            for &succ in successors.get(&point.0).into_iter().flatten() {
                let succ_latest = latest.get(&succ).copied().unwrap_or(n);
                value = value.min(succ_latest.saturating_sub(1));
            }
            latest.insert(point.0, value);
        }
    }

    let slots = input
        .episodes
        .iter()
        .map(|id| {
            let point = index[id.as_str()];
            let e = earliest.get(&point).copied().unwrap_or(1);
            let l = latest.get(&point).copied().unwrap_or(n);
            EpisodeSlack {
                episode: id.clone(),
                earliest: e,
                latest: l,
                feasible: e <= l && e >= 1 && l <= n,
            }
        })
        .collect();

    SlackReport {
        slots,
        cycle: None,
        possible_precedences,
    }
}

/// Which way an adjacency edge points, for building predecessor/successor maps.
#[derive(Debug, Clone, Copy)]
enum Direction {
    /// Map each node to the nodes with an edge *into* it (its predecessors).
    Into,
    /// Map each node to the nodes it has an edge *out to* (its successors).
    OutOf,
}

/// Build a `node -> [neighbor]` adjacency in the requested direction.
fn adjacency(chronology: &Chronology, direction: Direction) -> BTreeMap<u32, Vec<u32>> {
    let mut map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &(from, to) in &chronology.edges {
        match direction {
            Direction::Into => map.entry(to.0).or_default().push(from.0),
            Direction::OutOf => map.entry(from.0).or_default().push(to.0),
        }
    }
    map
}

/// Append `id` to `ids` if not already present (small `N`, so linear is fine).
fn push_unique(ids: &mut Vec<String>, id: &str) {
    if !ids.iter().any(|existing| existing == id) {
        ids.push(id.to_owned());
    }
}
