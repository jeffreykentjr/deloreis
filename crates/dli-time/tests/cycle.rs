#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Cycle law coverage: the pinned M6 edge semantics and their invariants.

use dli_time::cycle::{Cycle, CyclePos};
use proptest::prelude::*;

fn wheel_of(spokes: usize) -> Cycle {
    Cycle {
        name: "w".to_owned(),
        spokes: (0..spokes).map(|k| format!("s{k}")).collect(),
    }
}

fn pos(index: u32) -> CyclePos {
    CyclePos {
        cycle: "w".to_owned(),
        index,
    }
}

#[test]
fn succ_and_pred_wrap_around() {
    let cycle = wheel_of(3);
    assert_eq!(cycle.succ(&pos(2)).index, 0);
    assert_eq!(cycle.pred(&pos(0)).index, 2);
    assert_eq!(cycle.distance(&pos(0), &pos(2)), 2);
}

#[test]
fn across_is_an_involution_on_even_wheels() {
    // Even cycle: a true antipode. across(across(p)) == p, every spoke.
    let tide = wheel_of(4);
    for i in 0..4 {
        let p = pos(i);
        assert_eq!(tide.across(&p).index, (i + 2) % 4);
        assert_eq!(tide.across(&tide.across(&p)), tide.normalized(&p));
    }
}

#[test]
fn across_is_pred_squared_on_odd_wheels() {
    // Odd cycle: no exact antipode. across is floor(n/2) forward, so
    // across(across(p)) == pred(p) — deliberately non-involutive.
    let moon = wheel_of(29);
    for i in 0..29 {
        let p = pos(i);
        assert_eq!(moon.across(&p).index, (i + 14) % 29);
        assert_eq!(moon.across(&moon.across(&p)), moon.pred(&p));
        assert_ne!(moon.across(&moon.across(&p)), moon.normalized(&p));
    }
}

#[test]
fn distance_is_directional_and_sums_to_len() {
    let tide = wheel_of(4);
    assert_eq!(tide.distance(&pos(0), &pos(3)), 3);
    assert_eq!(tide.distance(&pos(3), &pos(0)), 1);
    for a in 0..4 {
        for b in 0..4 {
            if a != b {
                assert_eq!(
                    tide.distance(&pos(a), &pos(b)) + tide.distance(&pos(b), &pos(a)),
                    4
                );
            }
        }
    }
}

#[test]
fn separation_is_symmetric_and_the_shorter_arc() {
    let tide = wheel_of(4);
    assert_eq!(tide.separation(&pos(0), &pos(3)), 1);
    assert_eq!(tide.separation(&pos(0), &pos(2)), 2); // the diameter
    for a in 0..4 {
        for b in 0..4 {
            assert_eq!(
                tide.separation(&pos(a), &pos(b)),
                tide.separation(&pos(b), &pos(a))
            );
            assert!(tide.separation(&pos(a), &pos(b)) <= 2);
        }
    }
}

#[test]
fn out_of_range_index_normalizes_on_navigation() {
    // A position past the end is a congruence class, canonicalized by any walk.
    let tide = wheel_of(4);
    assert_eq!(tide.normalized(&pos(6)).index, 2);
    assert_eq!(tide.succ(&pos(7)).index, 0); // 7 mod 4 = 3, succ -> 0
    assert_eq!(tide.distance(&pos(4), &pos(6)), 2); // reads 0 -> 2
}

#[test]
fn at_and_in_range_are_the_strict_boundary() {
    let tide = wheel_of(4);
    assert!(tide.at(3).is_some());
    assert!(tide.at(4).is_none());
    assert!(tide.in_range(&pos(3)));
    assert!(!tide.in_range(&pos(4)));
    assert_eq!(tide.spoke(&pos(2)), Some("s2"));
}

#[test]
fn empty_wheel_navigation_is_total() {
    let empty = wheel_of(0);
    assert!(empty.is_empty());
    assert_eq!(empty.succ(&pos(0)), pos(0));
    assert_eq!(empty.distance(&pos(0), &pos(1)), 0);
    assert_eq!(empty.separation(&pos(0), &pos(1)), 0);
    assert!(empty.at(0).is_none());
    assert_eq!(empty.spoke(&pos(0)), None);
}

proptest! {
    #[test]
    fn succ_then_pred_commutes(n in 1usize..12, i in 0u32..64) {
        let cycle = wheel_of(n);
        prop_assert_eq!(cycle.pred(&cycle.succ(&pos(i))), cycle.succ(&cycle.pred(&pos(i))));
    }

    #[test]
    fn succ_and_pred_are_inverse(n in 1usize..12, i in 0u32..64) {
        let cycle = wheel_of(n);
        prop_assert_eq!(cycle.pred(&cycle.succ(&pos(i))), cycle.normalized(&pos(i)));
        prop_assert_eq!(cycle.succ(&cycle.pred(&pos(i))), cycle.normalized(&pos(i)));
    }

    #[test]
    fn across_involution_matches_parity(n in 1usize..12, i in 0u32..64) {
        let cycle = wheel_of(n);
        let twice = cycle.across(&cycle.across(&pos(i)));
        if n % 2 == 0 {
            prop_assert_eq!(twice, cycle.normalized(&pos(i)));
        } else {
            prop_assert_eq!(twice, cycle.pred(&pos(i)));
        }
    }

    #[test]
    fn separation_never_exceeds_half(n in 1usize..12, i in 0u32..64, j in 0u32..64) {
        let cycle = wheel_of(n);
        let sep = cycle.separation(&pos(i), &pos(j));
        prop_assert_eq!(sep, cycle.separation(&pos(j), &pos(i)));
        prop_assert!(sep as usize <= n / 2);
    }
}
