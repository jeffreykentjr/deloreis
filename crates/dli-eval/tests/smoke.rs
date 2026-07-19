#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the evaluator.

use dli_eval::world::World;

#[test]
fn empty_world_constructs() {
    let world = World::new();
    assert!(world.facts.is_empty());
    assert!(world.rules.is_empty());
    assert!(world.wheels.is_empty());
    assert_eq!(world, World::default());
}

#[test]
#[ignore = "enabled at EVAL-1"]
fn golden_query_answer_matches() {
    // Once lowering and query land, assert a lowered saltmarket world answers
    // adjacency queries with the committed support formula.
    todo!("EVAL-1: golden lower + query over saltmarket")
}
