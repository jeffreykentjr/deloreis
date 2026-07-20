#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Slack coverage: the forward/backward passes, floors/ceilings, precedence
//! cycles, and the standing thread (only certain precedences bind).

use std::collections::BTreeMap;

use dli_core::ann::{Ann, Theory};
use dli_core::atom::Atom;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;
use dli_time::slack::{Precedence, SlackInput, slack};

fn ids(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| (*s).to_owned()).collect()
}

fn run(input: &SlackInput) -> dli_time::slack::SlackReport {
    slack(input, &Valuation::new(), &Theory::new())
}

fn band(report: &dli_time::slack::SlackReport, episode: &str) -> (u32, u32) {
    let s = report.for_episode(episode).expect("episode present");
    (s.earliest, s.latest)
}

#[test]
fn unconstrained_episodes_span_the_whole_season() {
    let input = SlackInput {
        episodes: ids(&["a", "b", "c"]),
        precedences: Vec::new(),
        floors: BTreeMap::new(),
        ceilings: BTreeMap::new(),
    };
    let report = run(&input);
    assert!(report.is_feasible());
    // Every episode is free to sit anywhere in 1..=3.
    for e in ["a", "b", "c"] {
        assert_eq!(band(&report, e), (1, 3));
        assert_eq!(report.for_episode(e).unwrap().slack(), 2);
    }
}

#[test]
fn a_chain_leaves_equal_slack_and_two_free_episodes_span_the_season() {
    // a -> b -> c, plus free d, e; N = 5.
    let input = SlackInput {
        episodes: ids(&["a", "b", "c", "d", "e"]),
        precedences: vec![Precedence::certain("a", "b"), Precedence::certain("b", "c")],
        floors: BTreeMap::new(),
        ceilings: BTreeMap::new(),
    };
    let report = run(&input);
    assert!(report.is_feasible());
    assert_eq!(band(&report, "a"), (1, 3));
    assert_eq!(band(&report, "b"), (2, 4));
    assert_eq!(band(&report, "c"), (3, 5));
    assert_eq!(band(&report, "d"), (1, 5));
    assert_eq!(band(&report, "e"), (1, 5));
}

#[test]
fn a_floor_raises_the_earliest_and_propagates_down_the_chain() {
    // a -> b -> c with a floored at slot 2, plus a free d (N = 4). The floor
    // forces the chain into slots 2,3,4 and pins it; d takes slot 1..4.
    let mut floors = BTreeMap::new();
    floors.insert("a".to_owned(), 2);
    let input = SlackInput {
        episodes: ids(&["a", "b", "c", "d"]),
        precedences: vec![Precedence::certain("a", "b"), Precedence::certain("b", "c")],
        floors,
        ceilings: BTreeMap::new(),
    };
    let report = run(&input);
    assert!(report.is_feasible());
    assert_eq!(band(&report, "a"), (2, 2));
    assert_eq!(band(&report, "b"), (3, 3));
    assert_eq!(band(&report, "c"), (4, 4));
    assert_eq!(band(&report, "d"), (1, 4));
    assert!(report.for_episode("a").unwrap().is_pinned());
}

#[test]
fn a_floor_above_its_ceiling_is_infeasible() {
    let mut floors = BTreeMap::new();
    floors.insert("x".to_owned(), 3);
    let mut ceilings = BTreeMap::new();
    ceilings.insert("x".to_owned(), 1);
    let input = SlackInput {
        episodes: ids(&["x", "y", "z"]),
        precedences: Vec::new(),
        floors,
        ceilings,
    };
    let report = run(&input);
    assert!(!report.is_feasible());
    let x = report.for_episode("x").unwrap();
    assert!(!x.feasible);
}

#[test]
fn a_precedence_cycle_is_reported_and_leaves_no_bands() {
    let input = SlackInput {
        episodes: ids(&["a", "b"]),
        precedences: vec![Precedence::certain("a", "b"), Precedence::certain("b", "a")],
        floors: BTreeMap::new(),
        ceilings: BTreeMap::new(),
    };
    let report = run(&input);
    assert!(report.cycle.is_some());
    assert!(report.slots.is_empty());
    assert!(!report.is_feasible());
}

#[test]
fn a_proposed_precedence_does_not_bind_the_schedule() {
    // a must-precede-b, but the ordering is only proposed (support on an open
    // event): it cannot pin a slot, so both stay free.
    let proposed = Ann::supported(PosBool::pos(Atom(7)));
    let input = SlackInput {
        episodes: ids(&["a", "b", "c"]),
        precedences: vec![Precedence::new("a", "b", proposed)],
        floors: BTreeMap::new(),
        ceilings: BTreeMap::new(),
    };
    let report = run(&input);
    assert!(report.is_feasible());
    // Nothing binds a before b.
    assert_eq!(band(&report, "a"), (1, 3));
    assert_eq!(band(&report, "b"), (1, 3));
    // The tightening is reported as possible, not folded in.
    assert_eq!(report.possible_precedences.len(), 1);
    assert_eq!(report.possible_precedences[0].before, "a");
}

#[test]
fn only_certain_links_in_a_mixed_chain_bind() {
    // a ->(certain) b ->(proposed) c: only a before b constrains the bands.
    let proposed = Ann::supported(PosBool::pos(Atom(7)));
    let input = SlackInput {
        episodes: ids(&["a", "b", "c"]),
        precedences: vec![
            Precedence::certain("a", "b"),
            Precedence::new("b", "c", proposed),
        ],
        floors: BTreeMap::new(),
        ceilings: BTreeMap::new(),
    };
    let report = run(&input);
    assert!(report.is_feasible());
    assert_eq!(band(&report, "a"), (1, 2));
    assert_eq!(band(&report, "b"), (2, 3));
    assert_eq!(band(&report, "c"), (1, 3));
    assert_eq!(report.possible_precedences.len(), 1);
}
