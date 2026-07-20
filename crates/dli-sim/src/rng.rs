//! A tiny owned deterministic PRNG — `splitmix64`.
//!
//! The determinism law (charter; the SIM-1 brief) is that **seeds are inputs**:
//! a battery draws every random choice from a `u64` seed and nothing else — no
//! wall-clock, no thread id, no `rand` crate. This module is that generator,
//! about ten lines of arithmetic, so a run is a pure function of its seed and
//! trial count and therefore replays byte-for-byte.
//!
//! `splitmix64` is chosen for one property beyond speed: its stream position is
//! a *closed form* of the seed. The i-th value a stream emits is
//! `mix(seed + (i+1)·GAMMA)` — so [`stream_value`] jumps to any position in O(1)
//! without advancing through the ones before it. That is what lets a checkpoint
//! resume at trial *k* and draw the identical sub-stream it would have drawn had
//! it never stopped ([`crate::checkpoint`]): the trial index *is* the stream
//! position, and the position is addressable.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `splitmix64` increment (the golden-ratio odd constant).
const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// The `splitmix64` finalizer: an avalanche mix of a 64-bit state word.
const fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The value at position `index` of the `splitmix64` stream started at `seed`.
///
/// Positions are zero-based: `stream_value(seed, 0)` is the first value a fresh
/// [`SplitMix64::new(seed)`](SplitMix64::new) yields, and `stream_value(seed, i)`
/// equals calling [`next_u64`](SplitMix64::next_u64) `i + 1` times — but computed
/// directly, so a checkpoint can address trial `k`'s sub-seed without replaying
/// trials `0..k`. This closed form is the whole reason resume is exact.
///
/// ```
/// use dli_sim::rng::{stream_value, SplitMix64};
///
/// // Position addressing agrees with sequential advancing.
/// let mut prng = SplitMix64::new(42);
/// for i in 0..8 {
///     assert_eq!(prng.next_u64(), stream_value(42, i));
/// }
/// ```
#[must_use]
pub fn stream_value(seed: u64, index: u64) -> u64 {
    mix(seed.wrapping_add(index.wrapping_add(1).wrapping_mul(GAMMA)))
}

/// A `splitmix64` generator: a single 64-bit state advanced by a fixed step.
///
/// Construct one per trial from that trial's sub-seed and draw the trial's whole
/// scene from it; never share one across trials, or resume equivalence breaks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SplitMix64 {
    /// The current state word; the next value mixes `state + GAMMA`.
    state: u64,
}

impl SplitMix64 {
    /// A generator seeded at `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64-bit value: advance the state by `GAMMA`, then finalize it.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        mix(self.state)
    }

    /// A value uniformly-ish in `[0, n)`; `0` when `n == 0`.
    ///
    /// Uses modulo reduction, whose bias is negligible for the small bounds a
    /// battery draws (step counts and magnitudes) and irrelevant to determinism
    /// — the whole point here is reproducibility, not statistical perfection.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// A signed step uniformly in `[-magnitude, +magnitude]`.
    ///
    /// The span `2·magnitude + 1` covers every integer from `-magnitude` to
    /// `+magnitude` inclusive; `magnitude == 0` yields `0` deterministically.
    /// A `magnitude` is a `u32` so the span always fits a `u64` and the shift to
    /// signed space is lossless.
    pub fn delta_in(&mut self, magnitude: u32) -> i64 {
        let span = 2 * u64::from(magnitude) + 1;
        let raw = self.below(span);
        // raw < span <= 2*u32::MAX + 1 < i64::MAX, so the conversion never fails;
        // the fallback keeps the fn total without an `unwrap`.
        i64::try_from(raw).unwrap_or(0) - i64::from(magnitude)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn stream_value_matches_sequential_advance() {
        for seed in [0, 1, 42, u64::MAX, 0x1234_5678_9abc_def0] {
            let mut prng = SplitMix64::new(seed);
            for i in 0..32 {
                assert_eq!(prng.next_u64(), stream_value(seed, i));
            }
        }
    }

    #[test]
    fn same_seed_same_sequence() {
        let mut a = SplitMix64::new(7);
        let mut b = SplitMix64::new(7);
        for _ in 0..64 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = SplitMix64::new(1);
        let mut b = SplitMix64::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn below_stays_in_range_and_zero_is_zero() {
        let mut prng = SplitMix64::new(99);
        assert_eq!(prng.below(0), 0);
        for _ in 0..1000 {
            assert!(prng.below(9) < 9);
        }
    }

    #[test]
    fn delta_in_stays_in_symmetric_range() {
        let mut prng = SplitMix64::new(3);
        assert_eq!(SplitMix64::new(3).delta_in(0), 0);
        for _ in 0..1000 {
            let d = prng.delta_in(5);
            assert!((-5..=5).contains(&d));
        }
    }
}
