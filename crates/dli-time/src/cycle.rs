//! Cyclic calendars and modular navigation — the cycles (`docs/spec/03-time.md`
//! §1 iv).
//!
//! Tide, moon, and season are small **modular sorts**, orthogonal to the linear
//! spine. A cycle carries **succ, pred, across, distance** and refuses `<`, sort,
//! and max: asking for the "greatest" season is a category error, not a silent
//! tie-break, so [`CyclePos`] is deliberately not `Ord` (koan *the wheel refuses
//! rank*). There is **no epoch** — positions are local declarations, so a
//! position's index is read *modulo* the cycle's length.
//!
//! # Pinned edge semantics (ruling M6)
//!
//! Three edge cases were pinned so the cycle's laws are stated, not accidental:
//!
//! - **`across` is the half-turn forward step, `floor(n/2)`.** It is an
//!   involution exactly on **even** cycles (`across ∘ across = id`); on **odd**
//!   cycles it is deliberately non-involutive (`across ∘ across = pred`), because
//!   an odd ring has *no* exact antipode and inventing one would be a lie. See
//!   [`Cycle::across`].
//! - **`distance` is directional** — the count of `succ` steps from `from` to
//!   `to`. For the shorter arc regardless of direction, use the symmetric
//!   [`Cycle::separation`].
//! - **A [`CyclePos`] index `>= len` normalizes modulo `len`** on every
//!   navigation (documented, not an error), since positions are congruence
//!   classes with no epoch. Callers wanting strictness use the checked
//!   constructor [`Cycle::at`] or the guard [`Cycle::in_range`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A named cyclic calendar: an ordered ring of spokes.
///
/// Navigation is modular — `succ` and `pred` wrap around the ring:
///
/// ```
/// use dli_time::{Cycle, CyclePos};
/// let week = Cycle {
///     name: "week".to_owned(),
///     spokes: vec!["mon".to_owned(), "tue".to_owned(), "wed".to_owned()],
/// };
/// let mon = CyclePos { cycle: "week".to_owned(), index: 0 };
/// assert_eq!(week.succ(&mon).index, 1);
/// // pred of the first spoke wraps to the last.
/// assert_eq!(week.pred(&mon).index, 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Cycle {
    /// The cycle's name.
    pub name: String,
    /// The spoke labels, in cyclic order.
    pub spokes: Vec<String>,
}

/// A position on a cycle.
///
/// Not `Ord`: a cycle refuses rank, so two positions are never `<`-compared. The
/// `index` is a congruence class modulo the cycle's length — an out-of-range
/// value is normalized by navigation, never rejected mid-walk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CyclePos {
    /// The cycle this position belongs to.
    pub cycle: String,
    /// The spoke index (read modulo the cycle's length).
    pub index: u32,
}

impl Cycle {
    /// The number of spokes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spokes.len()
    }

    /// Whether the cycle has no spokes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spokes.is_empty()
    }

    /// A checked position on this cycle: `Some` when `index` is in range,
    /// `None` when it is `>= len` (or the cycle is empty). Use this at the
    /// boundary where a raw index enters; navigation itself normalizes.
    ///
    /// ```
    /// use dli_time::{Cycle, CyclePos};
    /// let tide = Cycle { name: "tide".to_owned(), spokes: vec![
    ///     "ebb".to_owned(), "slack".to_owned(), "flood".to_owned(), "high".to_owned(),
    /// ] };
    /// assert!(tide.at(2).is_some());
    /// assert!(tide.at(4).is_none()); // out of range — caught, not silently wrapped
    /// ```
    #[must_use]
    pub fn at(&self, index: u32) -> Option<CyclePos> {
        if (index as usize) < self.spokes.len() {
            Some(CyclePos {
                cycle: self.name.clone(),
                index,
            })
        } else {
            None
        }
    }

    /// Whether a position's index is already in range for this cycle.
    #[must_use]
    pub fn in_range(&self, pos: &CyclePos) -> bool {
        (pos.index as usize) < self.spokes.len()
    }

    /// The canonical form of a position: its index taken modulo the cycle's
    /// length. Idempotent, and the identity on already-in-range positions.
    #[must_use]
    pub fn normalized(&self, pos: &CyclePos) -> CyclePos {
        self.offset(pos, 0)
    }

    /// The spoke label at a position (normalizing the index), or `None` for an
    /// empty cycle.
    #[must_use]
    pub fn spoke(&self, pos: &CyclePos) -> Option<&str> {
        let n = self.spokes.len();
        if n == 0 {
            return None;
        }
        let index = (pos.index as usize) % n;
        self.spokes.get(index).map(String::as_str)
    }

    /// A new position `by` spokes forward of `pos`, normalized modulo `len`. The
    /// input index is read modulo `len` too, so an out-of-range position is
    /// canonicalized rather than rejected.
    fn offset(&self, pos: &CyclePos, by: usize) -> CyclePos {
        let n = self.spokes.len();
        if n == 0 {
            return pos.clone();
        }
        let index = ((pos.index as usize) % n + by % n) % n;
        CyclePos {
            cycle: self.name.clone(),
            index: u32::try_from(index).unwrap_or(0),
        }
    }

    /// The next spoke, wrapping around.
    #[must_use]
    pub fn succ(&self, pos: &CyclePos) -> CyclePos {
        self.offset(pos, 1)
    }

    /// The previous spoke, wrapping around.
    #[must_use]
    pub fn pred(&self, pos: &CyclePos) -> CyclePos {
        let n = self.spokes.len();
        if n == 0 {
            return pos.clone();
        }
        self.offset(pos, n - 1)
    }

    /// The spoke a half-turn (`floor(n/2)` steps) forward — the ring's opposite.
    ///
    /// On an **even** cycle this is a true antipode and an involution:
    /// `across(across(p)) == p`. On an **odd** cycle there is no exact opposite,
    /// so `across` steps `floor(n/2)` forward and is deliberately *not* an
    /// involution — instead `across(across(p)) == pred(p)`. The asymmetry is
    /// pinned and honest: the cycle does not fake a spoke that isn't there.
    ///
    /// ```
    /// use dli_time::{Cycle, CyclePos};
    /// // Even cycle (tide, 4): across is a true antipode and involutive.
    /// let tide = Cycle { name: "tide".to_owned(), spokes: vec![
    ///     "ebb".to_owned(), "slack".to_owned(), "flood".to_owned(), "high".to_owned(),
    /// ] };
    /// let ebb = CyclePos { cycle: "tide".to_owned(), index: 0 };
    /// assert_eq!(tide.across(&ebb).index, 2);          // ebb <-> flood
    /// assert_eq!(tide.across(&tide.across(&ebb)), ebb); // involutive
    ///
    /// // Odd cycle (3): across is floor(3/2)=1 and across∘across == pred.
    /// let three = Cycle { name: "w".to_owned(), spokes: vec![
    ///     "a".to_owned(), "b".to_owned(), "c".to_owned(),
    /// ] };
    /// let a = CyclePos { cycle: "w".to_owned(), index: 0 };
    /// assert_eq!(three.across(&three.across(&a)), three.pred(&a));
    /// ```
    #[must_use]
    pub fn across(&self, pos: &CyclePos) -> CyclePos {
        self.offset(pos, self.spokes.len() / 2)
    }

    /// The **directional** distance from `from` to `to`: the number of `succ`
    /// steps that walk `from` onto `to`, in `0..len`.
    ///
    /// This is not symmetric — `distance(a, b) + distance(b, a) == len` for
    /// `a != b`. For the shorter arc regardless of direction, use
    /// [`separation`](Cycle::separation).
    ///
    /// ```
    /// use dli_time::{Cycle, CyclePos};
    /// let tide = Cycle { name: "tide".to_owned(), spokes: vec![
    ///     "ebb".to_owned(), "slack".to_owned(), "flood".to_owned(), "high".to_owned(),
    /// ] };
    /// let ebb = CyclePos { cycle: "tide".to_owned(), index: 0 };
    /// let high = CyclePos { cycle: "tide".to_owned(), index: 3 };
    /// assert_eq!(tide.distance(&ebb, &high), 3); // ebb -> ... -> high, forward
    /// assert_eq!(tide.distance(&high, &ebb), 1); // high -> ebb, one step forward
    /// ```
    #[must_use]
    pub fn distance(&self, from: &CyclePos, to: &CyclePos) -> u32 {
        let n = self.spokes.len();
        if n == 0 {
            return 0;
        }
        let from_index = (from.index as usize) % n;
        let to_index = (to.index as usize) % n;
        let forward = (to_index + n - from_index) % n;
        u32::try_from(forward).unwrap_or(0)
    }

    /// The **symmetric** separation of two positions: the shorter of the two
    /// directional arcs, in `0..=len/2`. Order-independent —
    /// `separation(a, b) == separation(b, a)` — this is the "how close on the
    /// ring" a timing check wants (is the moon within `k` nights), where
    /// [`distance`](Cycle::distance) answers "how many `succ` steps".
    ///
    /// ```
    /// use dli_time::{Cycle, CyclePos};
    /// let tide = Cycle { name: "tide".to_owned(), spokes: vec![
    ///     "ebb".to_owned(), "slack".to_owned(), "flood".to_owned(), "high".to_owned(),
    /// ] };
    /// let ebb = CyclePos { cycle: "tide".to_owned(), index: 0 };
    /// let high = CyclePos { cycle: "tide".to_owned(), index: 3 };
    /// assert_eq!(tide.separation(&ebb, &high), 1); // one step apart the short way
    /// assert_eq!(tide.separation(&ebb, &high), tide.separation(&high, &ebb));
    /// ```
    #[must_use]
    pub fn separation(&self, from: &CyclePos, to: &CyclePos) -> u32 {
        let n = self.spokes.len();
        if n == 0 {
            return 0;
        }
        let forward = self.distance(from, to);
        let backward = u32::try_from(n).unwrap_or(u32::MAX) - forward;
        forward.min(backward)
    }
}
