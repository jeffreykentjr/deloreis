#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke and law coverage for the time domain.

use dli_time::chronology::{Chronology, TimePoint};
use dli_time::wheel::{Wheel, WheelPos};
use proptest::prelude::*;

fn wheel_of(spokes: usize) -> Wheel {
    Wheel {
        name: "w".to_owned(),
        spokes: (0..spokes).map(|k| format!("s{k}")).collect(),
    }
}

fn pos(index: u32) -> WheelPos {
    WheelPos {
        wheel: "w".to_owned(),
        index,
    }
}

#[test]
fn wheel_succ_wraps_around() {
    let wheel = wheel_of(3);
    assert_eq!(wheel.succ(&pos(2)).index, 0);
    assert_eq!(wheel.pred(&pos(0)).index, 2);
    assert_eq!(wheel.distance(&pos(0), &pos(2)), 2);
}

proptest! {
    #[test]
    #[ignore = "enabled at TIME-1"]
    fn succ_then_pred_is_identity(n in 1usize..8, i in 0u32..8) {
        let wheel = wheel_of(n);
        prop_assert_eq!(wheel.pred(&wheel.succ(&pos(i))), wheel.succ(&wheel.pred(&pos(i))));
    }
}

proptest! {
    #[test]
    #[ignore = "enabled at TIME-1"]
    fn precedes_is_irreflexive(p in 0u32..100) {
        let chronology = Chronology::default();
        let point = TimePoint(p);
        prop_assert!(!chronology.precedes(point, point));
    }
}
