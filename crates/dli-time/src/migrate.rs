//! The offset-to-before migration compiler (`TIME-3`).
//!
//! The legacy corpus faked a total order with a per-event integer `offset`
//! (`docs/spec/03-time.md` §1). The new model forbids it: within an era, events
//! are ordered only by explicit `before`-facts, and an undeclared pair is
//! honestly unordered. This module is the mechanism that carries the old data
//! across — a **pure function** from legacy `(event, offset)` pairs to the
//! `before`-facts that reconstruct the order the offsets implied, each carrying a
//! citation back to the offsets it came from.
//!
//! Two properties make the migration honest rather than lossy:
//!
//! - **Only adjacent edges are emitted.** Between one offset value and the next
//!   there is a covering `before`-fact; the rest of the order follows by
//!   transitivity. No `O(n²)` closure, no redundant facts.
//! - **Ties stay unordered.** Two events sharing an offset were never really
//!   ordered — the integer only pretended they were — so no `before`-fact is
//!   emitted *between* them. Both still precede the next offset's events.
//!
//! Emitting the facts is all this module does. Filing them as a ratification
//! batch (an `RQ`) — giving them a real ruling — is a separate, out-of-scope
//! authorial act; every [`ProposedBefore`] enters as proposed lore, never
//! ratified by the compiler.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One legacy event with its faked total-order offset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LegacyOffset {
    /// The event's name.
    pub event: String,
    /// The legacy integer offset that faked its position in a total order.
    pub offset: i64,
}

impl LegacyOffset {
    /// A legacy `(event, offset)` pair.
    #[must_use]
    pub fn new(event: impl Into<String>, offset: i64) -> Self {
        Self {
            event: event.into(),
            offset,
        }
    }
}

/// A `before`-fact the compiler proposes, with a citation back to the adjacent
/// offsets that justify it. Proposed, never ratified — see the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProposedBefore {
    /// The event the fact places earlier.
    pub earlier: String,
    /// The event the fact places later.
    pub later: String,
    /// Where the ordering came from: the two adjacent legacy offsets.
    pub citation: String,
}

impl ProposedBefore {
    /// Render the fact as canonical `.dli` surface: `before <earlier> -> <later>`.
    /// The proposed standing and the citation are carried separately (a filer
    /// attaches them); the covering order is the surface line.
    #[must_use]
    pub fn render(&self) -> String {
        format!("before {} -> {}", self.earlier, self.later)
    }
}

/// Compile legacy `(event, offset)` pairs into the covering `before`-facts that
/// reconstruct the order the offsets implied.
///
/// Events are grouped by offset and the groups ordered ascending. Between each
/// group and the next, one `before`-fact is emitted from every member of the
/// lower group to every member of the higher group — the covering relation of
/// the order. Members of one group (equal offsets) are left unordered.
///
/// ```
/// use dli_time::migrate::{compile_offsets, LegacyOffset};
///
/// // Distinct offsets: a clean chain, adjacent edges only.
/// let legacy = vec![
///     LegacyOffset::new("breaking-of-the-stones", 10),
///     LegacyOffset::new("tithe-of-minds-sweeps", 20),
///     LegacyOffset::new("protagonists-taking", 30),
/// ];
/// let befores = compile_offsets(&legacy);
/// assert_eq!(befores.len(), 2); // 10->20, 20->30; 10->30 is left to transitivity
/// assert_eq!(befores[0].render(), "before breaking-of-the-stones -> tithe-of-minds-sweeps");
/// assert!(befores[0].citation.contains("10"));
/// ```
///
/// Ties are honestly unordered — no fact is emitted between equal-offset events,
/// but both still precede the next offset:
///
/// ```
/// use dli_time::migrate::{compile_offsets, LegacyOffset};
///
/// let legacy = vec![
///     LegacyOffset::new("a", 10),
///     LegacyOffset::new("b", 10), // tied with a — never ordered against it
///     LegacyOffset::new("c", 20),
/// ];
/// let befores = compile_offsets(&legacy);
/// // a->c and b->c, but neither a->b nor b->a.
/// assert_eq!(befores.len(), 2);
/// assert!(befores.iter().all(|edge| edge.later == "c"));
/// assert!(!befores.iter().any(|edge| edge.earlier == "a" && edge.later == "b"));
/// assert!(!befores.iter().any(|edge| edge.earlier == "b" && edge.later == "a"));
/// ```
#[must_use]
pub fn compile_offsets(legacy: &[LegacyOffset]) -> Vec<ProposedBefore> {
    // Order indices by offset, stably (input order breaks ties), then group
    // consecutive equal offsets. `sort_by_key` is stable.
    let mut order: Vec<usize> = (0..legacy.len()).collect();
    order.sort_by_key(|&i| legacy[i].offset);

    let groups = group_by_offset(legacy, &order);

    let mut befores = Vec::new();
    for pair in groups.windows(2) {
        let [lower, upper] = pair else { continue };
        let lo = legacy[lower[0]].offset;
        let hi = legacy[upper[0]].offset;
        let citation = format!("migrated: offset {lo} immediately precedes offset {hi}");
        for &earlier in lower {
            for &later in upper {
                befores.push(ProposedBefore {
                    earlier: legacy[earlier].event.clone(),
                    later: legacy[later].event.clone(),
                    citation: citation.clone(),
                });
            }
        }
    }
    befores
}

/// Partition the offset-sorted indices into runs of equal offset.
fn group_by_offset(legacy: &[LegacyOffset], order: &[usize]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &i in order {
        match groups.last_mut() {
            Some(group) if legacy[group[0]].offset == legacy[i].offset => group.push(i),
            _ => groups.push(vec![i]),
        }
    }
    groups
}
