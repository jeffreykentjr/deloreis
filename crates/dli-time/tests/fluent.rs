#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Fluent projection coverage: the cautious three-valued reading, the factorial
//! composition with support, and the D-0114 water-status oracle.

use dli_core::ann::{Ann, Status, Theory};
use dli_core::atom::Atom;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;
use dli_time::chronology::{Chronology, TimePoint};
use dli_time::fluent::{FluentTimeline, Projection, Span, TemporalCover};

fn tp(index: u32) -> TimePoint {
    TimePoint(index)
}

/// flashback(0) -> enclosure-of-the-parish-water(1) -> story-present(2).
fn enclosure_spine() -> Chronology {
    Chronology {
        edges: vec![(tp(0), tp(1)), (tp(1), tp(2))],
    }
}

/// The D-0114 fluent: the parish water was held in common *until* the enclosure
/// and enclosed *since* it. Both anchoring facts are ratified (unconditional).
fn water_status() -> FluentTimeline<String> {
    let mut timeline = FluentTimeline::new();
    timeline.push(Span::until(
        "commons".to_owned(),
        tp(1),
        Ann::unconditional(),
    ));
    timeline.push(Span::since(
        "enclosed".to_owned(),
        tp(1),
        Ann::unconditional(),
    ));
    timeline
}

#[test]
fn d0114_the_flashback_era_reads_clean() {
    let chrono = enclosure_spine();
    let water = water_status();
    let (nu, theory) = (Valuation::new(), Theory::new());

    // At the pre-enclosure flashback, the water is commons — and only commons.
    let past = water.state_at(&chrono, &nu, &theory, tp(0));
    assert_eq!(past.certain_values(), vec![&"commons".to_owned()]);
    assert!(!past.has_contradiction());
    // The enclosed span is certainly absent here — not a suppressed conflict.
    assert_eq!(past.holdings.len(), 1);
}

#[test]
fn d0114_the_story_present_era_reads_clean() {
    let chrono = enclosure_spine();
    let water = water_status();
    let (nu, theory) = (Valuation::new(), Theory::new());

    // At story-present, the water is enclosed — and only enclosed.
    let present = water.state_at(&chrono, &nu, &theory, tp(2));
    assert_eq!(present.certain_values(), vec![&"enclosed".to_owned()]);
    assert!(!present.has_contradiction());
    assert_eq!(present.holdings.len(), 1);
}

#[test]
fn d0114_the_two_eras_never_contradict() {
    // The whole point: the flashback's "commons" and story-present's "enclosed"
    // hold over disjoint spans, so no query point sees both at once.
    let chrono = enclosure_spine();
    let water = water_status();
    let (nu, theory) = (Valuation::new(), Theory::new());
    for at in [tp(0), tp(1), tp(2)] {
        let state = water.state_at(&chrono, &nu, &theory, at);
        assert!(
            !state.has_contradiction(),
            "point {at:?} saw a false contradiction"
        );
        assert!(state.certain_values().len() <= 1);
    }
    // At the enclosure event itself, `since` is inclusive and `until` exclusive,
    // so enclosed holds and commons has already ended — exactly one value.
    let at_enclosure = water.state_at(&chrono, &nu, &theory, tp(1));
    assert_eq!(at_enclosure.certain_values(), vec![&"enclosed".to_owned()]);
}

#[test]
fn a_possible_anchor_projects_possibly_not_certainly() {
    // The factorial verdict: temporal cover is settled (Covers) but the anchoring
    // ruling is merely Possible (an unratified order, gated on an open variable),
    // so the fluent projects holds-possibly — support cannot lift it to certain.
    let chrono = enclosure_spine();
    let mut water: FluentTimeline<String> = FluentTimeline::new();
    // Anchoring fact rests on unruled event 7: Status::Possible under the empty ν.
    let unratified = Ann::supported(PosBool::pos(Atom(7)));
    water.push(Span::since("enclosed".to_owned(), tp(1), unratified));

    let state = water.state_at(&chrono, &Valuation::new(), &Theory::new(), tp(2));
    assert_eq!(state.holdings.len(), 1);
    let holding = &state.holdings[0];
    // The pair keeps the two channels apart...
    assert_eq!(holding.temporal, TemporalCover::Covers);
    assert_eq!(holding.support, Status::Possible);
    // ...and the collapsed reading is possibly, never certainly.
    assert_eq!(holding.projection(), Projection::HoldsPossibly);
    assert!(state.certain_values().is_empty());
}

#[test]
fn ruling_the_anchor_true_lifts_the_projection_to_certain() {
    // Same fluent, but now the ledger rules event 7 true: support becomes Certain
    // and the very same temporal cover reads holds-certainly. No re-projection of
    // time — only the support channel moved.
    let chrono = enclosure_spine();
    let mut water: FluentTimeline<String> = FluentTimeline::new();
    water.push(Span::since(
        "enclosed".to_owned(),
        tp(1),
        Ann::supported(PosBool::pos(Atom(7))),
    ));

    let mut nu = Valuation::new();
    nu.set(Atom(7), true);
    let state = water.state_at(&chrono, &nu, &Theory::new(), tp(2));
    assert_eq!(state.holdings[0].projection(), Projection::HoldsCertainly);
    assert_eq!(state.certain_values(), vec![&"enclosed".to_owned()]);
}

#[test]
fn an_unordered_query_point_is_order_indeterminate() {
    // The query point 5 is unordered against the anchor 1: whether the span
    // covers it depends on an order no one has declared.
    let chrono = enclosure_spine(); // knows nothing about point 5
    let mut water: FluentTimeline<String> = FluentTimeline::new();
    water.push(Span::since(
        "enclosed".to_owned(),
        tp(1),
        Ann::unconditional(),
    ));

    let state = water.state_at(&chrono, &Valuation::new(), &Theory::new(), tp(5));
    assert_eq!(state.holdings.len(), 1);
    assert_eq!(
        state.holdings[0].temporal,
        TemporalCover::OrderIndeterminate
    );
    assert_eq!(
        state.holdings[0].projection(),
        Projection::OrderIndeterminate
    );
    // Order-indeterminate is not a holding-certainly.
    assert!(state.certain_values().is_empty());
}

#[test]
fn a_dead_anchor_contributes_nothing() {
    // A fail-closed fact (support identically false) is dropped entirely — it is
    // neither a holding nor a phantom contradiction.
    let chrono = enclosure_spine();
    let mut water: FluentTimeline<String> = FluentTimeline::new();
    water.push(Span::since(
        "enclosed".to_owned(),
        tp(1),
        Ann::supported(PosBool::FALSE),
    ));
    let state = water.state_at(&chrono, &Valuation::new(), &Theory::new(), tp(2));
    assert!(state.holdings.is_empty());
}

#[test]
fn two_distinct_certain_values_are_a_real_contradiction() {
    // A malformed fluent: two overlapping open spans with different values both
    // hold certainly at the same point. That IS a contradiction, and the state
    // reports it — the disjoint water-status spans are what make D-0114 clean by
    // contrast, not a blanket suppression.
    let chrono = enclosure_spine();
    let mut broken: FluentTimeline<String> = FluentTimeline::new();
    broken.push(Span::since(
        "commons".to_owned(),
        tp(0),
        Ann::unconditional(),
    ));
    broken.push(Span::since(
        "enclosed".to_owned(),
        tp(0),
        Ann::unconditional(),
    ));
    let state = broken.state_at(&chrono, &Valuation::new(), &Theory::new(), tp(2));
    assert_eq!(state.certain_values().len(), 2);
    assert!(state.has_contradiction());
}
