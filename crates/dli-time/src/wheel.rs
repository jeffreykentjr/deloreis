//! Cyclic calendars and modular navigation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A named cyclic calendar: an ordered ring of spokes.
///
/// Navigation is modular — `succ` and `pred` wrap around the ring:
///
/// ```
/// use dli_time::{Wheel, WheelPos};
/// let week = Wheel {
///     name: "week".to_owned(),
///     spokes: vec!["mon".to_owned(), "tue".to_owned(), "wed".to_owned()],
/// };
/// let mon = WheelPos { wheel: "week".to_owned(), index: 0 };
/// assert_eq!(week.succ(&mon).index, 1);
/// // pred of the first spoke wraps to the last.
/// assert_eq!(week.pred(&mon).index, 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Wheel {
    /// The wheel's name.
    pub name: String,
    /// The spoke labels, in cyclic order.
    pub spokes: Vec<String>,
}

/// A position on a wheel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WheelPos {
    /// The wheel this position belongs to.
    pub wheel: String,
    /// The spoke index.
    pub index: u32,
}

impl Wheel {
    /// The number of spokes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spokes.len()
    }

    /// Whether the wheel has no spokes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spokes.is_empty()
    }

    fn offset(&self, pos: &WheelPos, by: usize) -> WheelPos {
        let n = self.spokes.len();
        if n == 0 {
            return pos.clone();
        }
        let index = (pos.index as usize + by) % n;
        WheelPos {
            wheel: self.name.clone(),
            index: u32::try_from(index).unwrap_or(0),
        }
    }

    /// The next spoke, wrapping around.
    #[must_use]
    pub fn succ(&self, pos: &WheelPos) -> WheelPos {
        self.offset(pos, 1)
    }

    /// The previous spoke, wrapping around.
    #[must_use]
    pub fn pred(&self, pos: &WheelPos) -> WheelPos {
        let n = self.spokes.len();
        if n == 0 {
            return pos.clone();
        }
        self.offset(pos, n - 1)
    }

    /// The spoke half a turn away.
    #[must_use]
    pub fn across(&self, pos: &WheelPos) -> WheelPos {
        self.offset(pos, self.spokes.len() / 2)
    }

    /// The forward (positive) modular distance from `from` to `to`.
    #[must_use]
    pub fn distance(&self, from: &WheelPos, to: &WheelPos) -> u32 {
        let n = self.spokes.len();
        if n == 0 {
            return 0;
        }
        let from_index = from.index as usize % n;
        let to_index = to.index as usize % n;
        let forward = (to_index + n - from_index) % n;
        u32::try_from(forward).unwrap_or(0)
    }
}
