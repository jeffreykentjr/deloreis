#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Partial-order coverage: precedence verdicts, cycle blame, topological layers.

use dli_core::ann::Ann;
use dli_core::atom::{Atom, Interner};
use dli_core::posbool::PosBool;
use dli_time::chronology::{Chronology, TemporalVerdict, TimePoint};
use proptest::prelude::*;

fn tp(index: u32) -> TimePoint {
    TimePoint(index)
}

/// A chronology mirroring the vendored spine fragment: an era chain lifted onto
/// events plus intra-era before-facts.
fn creep_chain() -> Chronology {
    // breaking-of-the-stones(0) -> tithe-of-minds-sweeps(1) -> protagonists-taking(2)
    Chronology {
        edges: vec![(tp(0), tp(1)), (tp(1), tp(2))],
    }
}

#[test]
fn precedence_is_transitive_and_directional() {
    let chrono = creep_chain();
    assert_eq!(chrono.precedes(tp(0), tp(2)), TemporalVerdict::Before);
    assert_eq!(chrono.precedes(tp(2), tp(0)), TemporalVerdict::After);
    assert_eq!(chrono.precedes(tp(1), tp(2)), TemporalVerdict::Before);
}

#[test]
fn an_undeclared_pair_is_unordered_not_a_tie_break() {
    // Two events in the same era with no before-fact between them: the honest
    // answer is Unordered, both directions, every time.
    let chrono = Chronology {
        edges: vec![(tp(0), tp(9)), (tp(1), tp(9))], // 0 and 1 both precede 9, never each other
    };
    assert_eq!(chrono.precedes(tp(0), tp(1)), TemporalVerdict::Unordered);
    assert_eq!(chrono.precedes(tp(1), tp(0)), TemporalVerdict::Unordered);
}

#[test]
fn a_point_never_precedes_itself() {
    let chrono = creep_chain();
    assert_eq!(chrono.precedes(tp(1), tp(1)), TemporalVerdict::Unordered);
}

#[test]
fn a_before_cycle_is_reported_as_blame_over_its_rulings() {
    // Three before-facts closing a loop, each declared by a distinct ruling.
    let chrono = Chronology {
        edges: vec![(tp(0), tp(1)), (tp(1), tp(2)), (tp(2), tp(0))],
    };
    let anns = vec![
        Ann::supported(PosBool::pos(Atom(10))), // ruling behind edge 0
        Ann::supported(PosBool::pos(Atom(11))), // ruling behind edge 1
        Ann::supported(PosBool::pos(Atom(12))), // ruling behind edge 2
    ];
    let blame = chrono
        .find_cycle(&anns)
        .expect("the loop is a contradiction");

    assert_eq!(blame.len(), 3);
    assert!(blame.is_fully_attributed());
    // The blamed edges are exactly the loop's, each carrying its declaring ann.
    let mut blamed: Vec<usize> = blame.cycle.iter().map(|edge| edge.edge_index).collect();
    blamed.sort_unstable();
    assert_eq!(blamed, vec![0, 1, 2]);
    for edge in &blame.cycle {
        assert_eq!(edge.ann.as_ref(), anns.get(edge.edge_index));
    }
}

#[test]
fn blame_renders_with_event_names() {
    let mut names = Interner::new();
    names.intern("breaking-of-the-stones"); // TimePoint(0)
    names.intern("tithe-of-minds-sweeps"); // TimePoint(1)
    let chrono = Chronology {
        edges: vec![(tp(0), tp(1)), (tp(1), tp(0))],
    };
    let blame = chrono.find_cycle(&[]).expect("cycle");
    let text = blame.render(&names);
    assert!(text.contains("breaking-of-the-stones"));
    assert!(text.contains("tithe-of-minds-sweeps"));
    // No ruling table was supplied, so nothing is marked as a declared order.
    assert!(!blame.is_fully_attributed());
}

#[test]
fn topological_layers_place_the_spine_in_depth_order() {
    // 0 -> {1, 2}; 1 -> 3; 2 -> 3. Layers: [0], [1, 2], [3].
    let chrono = Chronology {
        edges: vec![
            (tp(0), tp(1)),
            (tp(0), tp(2)),
            (tp(1), tp(3)),
            (tp(2), tp(3)),
        ],
    };
    let layers = chrono.topological_layers(&[]).expect("acyclic");
    assert_eq!(layers, vec![vec![tp(0)], vec![tp(1), tp(2)], vec![tp(3)]]);
}

#[test]
fn topological_layers_include_isolated_points_from_the_universe() {
    // Event 5 has no before-facts at all: it must still appear, in layer 0.
    let chrono = creep_chain();
    let universe = vec![tp(0), tp(1), tp(2), tp(5)];
    let layers = chrono.topological_layers(&universe).expect("acyclic");
    assert!(layers[0].contains(&tp(0)));
    assert!(layers[0].contains(&tp(5)));
    let total: usize = layers.iter().map(Vec::len).sum();
    assert_eq!(total, 4);
}

#[test]
fn topological_layers_return_blame_on_a_cycle() {
    let chrono = Chronology {
        edges: vec![(tp(0), tp(1)), (tp(1), tp(0))],
    };
    let outcome = chrono.topological_layers(&[]);
    let blame = outcome.expect_err("a cycle has no layering");
    assert_eq!(blame.len(), 2);
}

proptest! {
    #[test]
    fn precedes_is_irreflexive_on_an_acyclic_chain(p in 0u32..64) {
        // A pure forward chain has no self-reachability, so nothing precedes itself.
        let edges: Vec<(TimePoint, TimePoint)> =
            (0..63u32).map(|i| (tp(i), tp(i + 1))).collect();
        let chrono = Chronology { edges };
        prop_assert_eq!(chrono.precedes(tp(p), tp(p)), TemporalVerdict::Unordered);
    }
}
