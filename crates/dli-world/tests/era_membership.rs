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
fn legacy_serialized_era_paths_are_recomputed_before_use() {
    let source = "scope storybook\nratify d-1\nera early ratified d-1\nera middle ratified d-1\nevent change during early ratified d-1\nevent bridge during early ratified d-1\nevent other during middle ratified d-1\nbefore early -> bridge ratified d-1\nbefore bridge -> middle ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let mut compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let point = |name| {
        dli_time::chronology::TimePoint(compiled.model.world.time_points.lookup(name).unwrap().0)
    };
    let early = point("early");
    let middle = point("middle");
    let change = point("change");
    let other = point("other");
    // Model the invalid cache produced by the former mixed event/era path.
    compiled
        .model
        .inferred_era_edges
        .push(dli_world::model::EraEdge {
            earlier: early,
            later: middle,
            ann: dli_core::ann::Ann::unconditional(),
        });
    let mut serialized = serde_json::to_value(&compiled).unwrap();
    serialized["model"]
        .as_object_mut()
        .unwrap()
        .remove("era_inference_revision");
    let restored: dli_world::CompiledWorld = serde_json::from_value(serialized).unwrap();
    let chronology = restored.model.chronology_with_era_membership();
    assert_eq!(
        chronology.precedes(change, middle),
        TemporalVerdict::Unordered
    );
    assert_eq!(
        chronology.precedes(change, other),
        TemporalVerdict::Unordered
    );
    assert_eq!(
        chronology.precedes(early, other),
        TemporalVerdict::Unordered
    );
}

#[test]
fn legacy_serialized_model_recovers_missing_era_cycle_and_citations() {
    let source = "scope storybook\nratify d-1\nratify d-2\nratify d-3\nera early < middle ratified d-1\nevent change during early ratified d-2\nbefore middle -> change ratified d-3\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let mut world = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    world.model.inferred_era_edges.clear();
    let mut serialized = serde_json::to_value(&world).unwrap();
    serialized["model"]
        .as_object_mut()
        .unwrap()
        .remove("era_inference_revision");
    let restored: dli_world::CompiledWorld = serde_json::from_value(serialized).unwrap();
    let finding = dli_world::check::run(&restored)
        .into_iter()
        .find(|f| f.code == "continuity.chronology-cycle")
        .expect("cycle after reload");
    for decision in ["d-1", "d-2", "d-3"] {
        assert!(
            finding.citations.contains(&decision.to_owned()),
            "{finding:?}"
        );
    }
}

#[test]
fn era_declarations_are_not_lost_when_a_name_has_another_sort() {
    let source = "scope storybook\nratify d-1\nperson middle \"Middle\" ratified d-1\nera early < middle < late ratified d-1\nevent change during early ratified d-1\nbefore late -> change ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let world = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    assert!(
        dli_world::check::run(&world)
            .iter()
            .any(|f| f.code == "continuity.chronology-cycle")
    );
    let clean = source.replace("before late -> change ratified d-1\n", "");
    let (document, _) = dli_syntax::parse_document(&clean);
    let clean = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let point = |name| {
        dli_time::chronology::TimePoint(clean.model.world.time_points.lookup(name).unwrap().0)
    };
    assert_eq!(
        clean
            .model
            .chronology_with_era_membership()
            .precedes(point("change"), point("late")),
        TemporalVerdict::Before
    );
}

#[test]
fn an_ordinary_era_fact_does_not_turn_an_event_into_an_era() {
    let source = "scope storybook\nratify d-1\nera early ratified d-1\nera late ratified d-1\nevent change during early ratified d-1\nevent bridge ratified d-1\nstate bridge.era ratified d-1\nbefore early -> bridge ratified d-1\nbefore bridge -> late ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let world = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let point = |name| {
        dli_time::chronology::TimePoint(world.model.world.time_points.lookup(name).unwrap().0)
    };
    assert_eq!(
        world
            .model
            .chronology_with_era_membership()
            .precedes(point("change"), point("late")),
        TemporalVerdict::Unordered
    );
}

#[test]
fn old_models_without_era_declaration_mapping_require_recompilation() {
    let source = "scope storybook\nratify d-1\nera early < middle ratified d-1\nevent change during early ratified d-1\n";
    let (document, _) = dli_syntax::parse_document(source);
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let mut serialized = serde_json::to_value(&compiled).unwrap();
    let model = serialized["model"].as_object_mut().unwrap();
    model.remove("era_inference_revision");
    model.remove("era_points");
    model.remove("era_points_complete");
    let restored: dli_world::CompiledWorld = serde_json::from_value(serialized).unwrap();
    let findings = dli_world::check::run(&restored);
    assert!(
        findings
            .iter()
            .any(|f| f.code == "continuity.stale-era-model" && f.message.contains("recompile")),
        "{findings:?}"
    );
    let at = dli_time::chronology::TimePoint(
        restored.model.world.time_points.lookup("middle").unwrap().0,
    );
    assert!(
        dli_world::check::slot_state_at(&restored, "home", "wren", at)
            .unwrap_err()
            .to_string()
            .contains("recompile")
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
fn event_against_its_era_is_a_cycle_with_its_full_support() {
    let source = "scope storybook\nratify d-0001\nratify d-0002\nratify d-0003\nera early < middle ratified d-0001\nevent change during early ratified d-0002\nbefore middle -> change ratified d-0003\nstate wren.home = new since change ratified d-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let mut world = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let finding = dli_world::check::run(&world)
        .into_iter()
        .find(|f| f.code == "continuity.chronology-cycle")
        .expect("cycle is reported");
    for decision in ["d-0001", "d-0002", "d-0003"] {
        assert!(
            finding.citations.contains(&decision.to_owned()),
            "{finding:?}"
        );
    }
    assert!(finding.span.is_some(), "{finding:?}");
    let membership = world.model.world.interner.lookup("d-0002").unwrap();
    world.model.world.valuation.set(membership, false);
    assert!(
        dli_world::check::run(&world)
            .iter()
            .all(|f| f.code != "continuity.chronology-cycle")
    );
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
    let state = dli_world::check::slot_state_at(&world, "home", "wren", at).unwrap();
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
fn a_single_membership_does_not_cache_every_era_pair() {
    let names: Vec<_> = (0..300).map(|n| format!("era-{n}")).collect();
    let source = format!(
        "scope storybook\nratify d-1\nera {} ratified d-1\nevent change during era-150 ratified d-1\n",
        names.join(" < ")
    );
    let (document, diagnostics) = dli_syntax::parse_document(&source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let world = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    assert_eq!(world.model.inferred_era_edges.len(), 299);
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
