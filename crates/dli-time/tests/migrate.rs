#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Offset→before compiler coverage: adjacent-only edges, honestly-unordered
//! ties, and stable ordering of unsorted input.

use dli_time::migrate::{LegacyOffset, ProposedBefore, compile_offsets};

fn edges(befores: &[ProposedBefore]) -> Vec<(String, String)> {
    befores
        .iter()
        .map(|b| (b.earlier.clone(), b.later.clone()))
        .collect()
}

#[test]
fn distinct_offsets_become_an_adjacent_chain() {
    let legacy = vec![
        LegacyOffset::new("a", 10),
        LegacyOffset::new("b", 20),
        LegacyOffset::new("c", 30),
    ];
    let befores = compile_offsets(&legacy);
    // Only covering edges — a->c is left to transitivity.
    assert_eq!(
        edges(&befores),
        vec![
            ("a".to_owned(), "b".to_owned()),
            ("b".to_owned(), "c".to_owned()),
        ]
    );
    assert!(befores[0].citation.contains("10"));
    assert!(befores[0].citation.contains("20"));
    assert_eq!(befores[0].render(), "before a -> b");
}

#[test]
fn tied_offsets_stay_unordered_but_both_precede_the_next() {
    let legacy = vec![
        LegacyOffset::new("a", 10),
        LegacyOffset::new("b", 10),
        LegacyOffset::new("c", 20),
    ];
    let befores = compile_offsets(&legacy);
    // a->c and b->c; no edge within the tied pair, either direction.
    assert_eq!(befores.len(), 2);
    assert!(befores.iter().all(|e| e.later == "c"));
    assert!(!befores.iter().any(|e| e.earlier == "a" && e.later == "b"));
    assert!(!befores.iter().any(|e| e.earlier == "b" && e.later == "a"));
}

#[test]
fn a_middle_tie_fans_across_both_group_boundaries() {
    // 10 | 20,20 | 30 : a->b, a->c, then b->d, c->d.
    let legacy = vec![
        LegacyOffset::new("a", 10),
        LegacyOffset::new("b", 20),
        LegacyOffset::new("c", 20),
        LegacyOffset::new("d", 30),
    ];
    let befores = compile_offsets(&legacy);
    let set = edges(&befores);
    assert!(set.contains(&("a".to_owned(), "b".to_owned())));
    assert!(set.contains(&("a".to_owned(), "c".to_owned())));
    assert!(set.contains(&("b".to_owned(), "d".to_owned())));
    assert!(set.contains(&("c".to_owned(), "d".to_owned())));
    // b and c are tied — never ordered against each other.
    assert!(
        !set.iter()
            .any(|(x, y)| (x == "b" && y == "c") || (x == "c" && y == "b"))
    );
    assert_eq!(befores.len(), 4);
}

#[test]
fn all_equal_offsets_emit_no_ordering() {
    let legacy = vec![
        LegacyOffset::new("a", 5),
        LegacyOffset::new("b", 5),
        LegacyOffset::new("c", 5),
    ];
    assert!(compile_offsets(&legacy).is_empty());
}

#[test]
fn empty_and_singleton_inputs_emit_nothing() {
    assert!(compile_offsets(&[]).is_empty());
    assert!(compile_offsets(&[LegacyOffset::new("only", 1)]).is_empty());
}

#[test]
fn unsorted_input_is_ordered_by_offset() {
    let legacy = vec![
        LegacyOffset::new("late", 30),
        LegacyOffset::new("early", 10),
        LegacyOffset::new("mid", 20),
    ];
    let befores = compile_offsets(&legacy);
    assert_eq!(
        edges(&befores),
        vec![
            ("early".to_owned(), "mid".to_owned()),
            ("mid".to_owned(), "late".to_owned()),
        ]
    );
}

#[test]
fn negative_offsets_order_correctly() {
    let legacy = vec![
        LegacyOffset::new("before-zero", -5),
        LegacyOffset::new("zero", 0),
    ];
    let befores = compile_offsets(&legacy);
    assert_eq!(
        edges(&befores),
        vec![("before-zero".to_owned(), "zero".to_owned())]
    );
}
