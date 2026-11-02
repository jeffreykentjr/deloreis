//! Event ordering inferred from ruled era memberships.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_time::chronology::TemporalVerdict;
use dli_world::pack::Pack;

#[test]
fn event_order_uses_only_certain_memberships_and_era_order() {
    let source = "scope storybook\nratify d-0001\nratify d-0002\nratify d-0003\nera early < late ratified d-0001\nevent first during early ratified d-0003\nevent second during late ratified d-0002\nevent rumor during late proposed\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let mut world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let point = |name| world.model.world.time_points.lookup(name).unwrap().0;
    let first = dli_time::chronology::TimePoint(point("first"));
    let second = dli_time::chronology::TimePoint(point("second"));
    let rumor = dli_time::chronology::TimePoint(point("rumor"));
    assert_eq!(
        world
            .model
            .chronology_with_era_membership()
            .precedes(first, second),
        TemporalVerdict::Before
    );
    assert_eq!(
        world
            .model
            .chronology_with_era_membership()
            .precedes(first, rumor),
        TemporalVerdict::Unordered
    );
    let (possible_graph, possible_anns) = world.model.annotated_chronology();
    assert_eq!(
        dli_time::order_rules::precedes_with_standing(
            &possible_graph,
            &possible_anns,
            &world.model.world.valuation,
            &world.model.world.theory,
            first,
            rumor
        ),
        dli_time::order_rules::StandingOrder::PossiblyBefore
    );
    let d2 = world.model.world.interner.lookup("d-0002").unwrap();
    world.model.world.valuation.set(d2, false);
    assert_eq!(
        world
            .model
            .chronology_with_era_membership()
            .precedes(first, second),
        TemporalVerdict::Unordered
    );
    let d1 = world.model.world.interner.lookup("d-0001").unwrap();
    world.model.world.valuation.set(d1, false);
    world.model.world.valuation.set(d2, true);
    assert_eq!(
        world
            .model
            .chronology_with_era_membership()
            .precedes(first, second),
        TemporalVerdict::Unordered
    );
}

#[test]
fn inferred_cycle_is_rejected_and_names_its_supporting_decisions() {
    let source = "scope storybook\nratify d-0001\nratify d-0002\nratify d-0003\nera early < late ratified d-0001\nevent first during early ratified d-0002\nevent second during late ratified d-0002\nbefore second -> first ratified d-0003\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let finding = dli_world::check::run(&world)
        .into_iter()
        .find(|f| f.code == "continuity.chronology-cycle")
        .expect("inferred cycle is reported");
    for decision in ["d-0001", "d-0002", "d-0003"] {
        assert!(
            finding.citations.contains(&decision.to_owned()),
            "{finding:?}"
        );
    }
}

#[test]
fn exclusive_membership_support_cannot_invent_a_possible_order() {
    let source = "scope storybook\nratify d-0001\nera early < late ratified d-0001\nquestion witnesses \"Whose account?\"\n  option witness-one \"One\"\n  option witness-two \"Two\"\nevent first during early proposed assumes witness-one\nevent second during late proposed assumes witness-two\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let point = |name| {
        dli_time::chronology::TimePoint(world.model.world.time_points.lookup(name).unwrap().0)
    };
    let (graph, anns) = world.model.annotated_chronology();
    assert_eq!(
        dli_time::order_rules::precedes_with_standing(
            &graph,
            &anns,
            &world.model.world.valuation,
            &world.model.world.theory,
            point("first"),
            point("second")
        ),
        dli_time::order_rules::StandingOrder::Unordered
    );
}

#[test]
fn proposed_orders_neither_create_a_certain_cycle_nor_harden_a_fluent() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second proposed\nbefore second -> first proposed\nstate wren.home = village since first ratified d-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    assert!(
        dli_world::check::run(&world)
            .iter()
            .all(|f| f.code != "continuity.chronology-cycle")
    );
    let at =
        dli_time::chronology::TimePoint(world.model.world.time_points.lookup("second").unwrap().0);
    let state = dli_world::check::slot_state_at(&world, "home", "wren", at);
    assert_eq!(
        state.holdings[0].temporal,
        dli_time::fluent::TemporalCover::OrderIndeterminate
    );
}

#[test]
fn mixed_inferred_and_authored_path_requires_jointly_possible_support() {
    let source = "scope storybook\nratify d-0001\nquestion q \"which?\"\n  option alpha\n  option beta\nera early < late ratified d-0001\nevent middle during late ratified d-0001\nevent last during early ratified d-0001\nbefore middle -> last proposed assumes beta\nevent other during early proposed assumes alpha\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let point = |name| {
        dli_time::chronology::TimePoint(world.model.world.time_points.lookup(name).unwrap().0)
    };
    let (graph, anns) = world.model.annotated_chronology();
    assert_eq!(
        dli_time::order_rules::precedes_with_standing(
            &graph,
            &anns,
            &world.model.world.valuation,
            &world.model.world.theory,
            point("other"),
            point("last")
        ),
        dli_time::order_rules::StandingOrder::Unordered
    );
}

#[test]
fn stored_inference_scales_with_eras_not_event_pairs() {
    use std::fmt::Write as _;
    let mut source =
        String::from("scope storybook\nratify d-0001\nera early < late ratified d-0001\n");
    for index in 0..400 {
        let era = if index < 200 { "early" } else { "late" };
        writeln!(source, "event event-{index} during {era} ratified d-0001").unwrap();
    }
    let (document, diagnostics) = dli_syntax::parse_document(&source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let world = dli_world::compile(&Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    assert_eq!(world.model.era_memberships.len(), 400);
    assert_eq!(world.model.inferred_era_edges.len(), 1);
}

#[test]
fn cycle_blame_excludes_defeated_alternative_order_support() {
    let source = "scope storybook\nratify d-0001\nratify d-0002\nratify d-0003\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\nbefore first -> second ratified d-0002\nbefore second -> first ratified d-0003\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let mut world = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let defeated = world.model.world.interner.lookup("d-0002").unwrap();
    world.model.world.valuation.set(defeated, false);
    let finding = dli_world::check::run(&world)
        .into_iter()
        .find(|f| f.code == "continuity.chronology-cycle")
        .unwrap();
    assert!(
        finding.citations.contains(&"d-0001".to_owned()),
        "{finding:?}"
    );
    assert!(
        finding.citations.contains(&"d-0003".to_owned()),
        "{finding:?}"
    );
    assert!(
        !finding.citations.contains(&"d-0002".to_owned()),
        "{finding:?}"
    );
}
