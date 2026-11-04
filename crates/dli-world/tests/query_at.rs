//! Point-in-time query projection against the same kernel as `author.as_of`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_ast::{
    ident::Ident,
    term::{AtomExpr, Term},
};
use dli_eval::query::{Layer, Query, QueryOpts};
use dli_time::chronology::TimePoint;
use dli_world::pack::Pack;

fn run(source: &str, at: &str, pred: &str) -> Vec<String> {
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let point = TimePoint(compiled.model.world.time_points.lookup(at).unwrap().0);
    let world = dli_world::check::continuity::query_world_at(&compiled, point).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic(pred),
            args: vec![
                Term::Sym(Ident::synthetic("wren")),
                Term::Var(Ident::synthetic("where")),
            ],
        },
    };
    world
        .query_with(
            &query,
            &QueryOpts {
                layer: Layer::Certain,
                ..Default::default()
            },
        )
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.atom)
        .collect()
}

const WORLD: &str = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\ninertial home ratified d-0001\nstate wren.home = old since first ratified d-0001\nstate wren.home = new since second ratified d-0001\n";

#[test]
fn era_cut_orders_events_in_other_eras_but_not_its_own() {
    let source = "scope storybook\nratify d-0001\nera early < middle < late ratified d-0001\nevent change during early ratified d-0001\nstate wren.home = old until change ratified d-0001\nstate wren.home = new since change ratified d-0001\n";
    assert_eq!(run(source, "middle", "home"), vec!["home(wren, new)"]);

    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.iter().all(|d| !d.class.is_error()));
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let middle = TimePoint(compiled.model.world.time_points.lookup("middle").unwrap().0);
    let early = TimePoint(compiled.model.world.time_points.lookup("early").unwrap().0);
    let later = dli_world::check::slot_state_at(&compiled, "home", "wren", middle).unwrap();
    assert_eq!(later.holdings.len(), 1);
    assert_eq!(later.holdings[0].value, "new");
    assert_eq!(
        later.holdings[0].projection,
        dli_time::fluent::Projection::HoldsCertainly
    );
    let within = dli_world::check::slot_state_at(&compiled, "home", "wren", early).unwrap();
    assert_eq!(within.holdings.len(), 2);
    assert!(
        within
            .holdings
            .iter()
            .all(|h| h.projection == dli_time::fluent::Projection::OrderIndeterminate)
    );
    let future = source.replace("event change during early", "event change during late");
    assert_eq!(run(&future, "middle", "home"), vec!["home(wren, old)"]);

    let uncertain = source.replace(
        "event change during early ratified d-0001",
        "event change during early proposed",
    );
    let (document, _) = dli_syntax::parse_document(&uncertain);
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let middle = TimePoint(compiled.model.world.time_points.lookup("middle").unwrap().0);
    let result = dli_world::check::slot_state_at(&compiled, "home", "wren", middle).unwrap();
    assert!(
        result
            .holdings
            .iter()
            .all(|h| h.projection != dli_time::fluent::Projection::HoldsCertainly)
    );

    let defeated = source
        .replace("ratify d-0001\n", "ratify d-0001\nratify d-0002\n")
        .replace(
            "event change during early ratified d-0001",
            "event change during early ratified d-0002",
        );
    let (document, _) = dli_syntax::parse_document(&defeated);
    let mut compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let decision = compiled.model.world.interner.lookup("d-0002").unwrap();
    compiled.model.world.valuation.set(decision, false);
    let middle = TimePoint(compiled.model.world.time_points.lookup("middle").unwrap().0);
    let result = dli_world::check::slot_state_at(&compiled, "home", "wren", middle).unwrap();
    assert!(
        result
            .holdings
            .iter()
            .all(|h| h.projection != dli_time::fluent::Projection::HoldsCertainly)
    );
}

#[test]
fn conflicting_era_and_event_order_refuses_both_projections() {
    let source = "scope storybook\nratify d-0001\nera early < middle < late ratified d-0001\nevent change during early ratified d-0001\nbefore middle -> change ratified d-0001\nstate wren.home = old until change ratified d-0001\nstate wren.home = new since change ratified d-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let middle = TimePoint(compiled.model.world.time_points.lookup("middle").unwrap().0);
    let slot = dli_world::check::slot_state_at(&compiled, "home", "wren", middle).unwrap_err();
    let query = dli_world::check::continuity::query_world_at(&compiled, middle).unwrap_err();
    assert!(slot.to_string().contains("cyclic chronology"), "{slot}");
    assert!(query.to_string().contains("cyclic chronology"), "{query}");
}

#[test]
fn a_path_through_an_event_does_not_order_entire_eras() {
    let source = "scope storybook\nratify d-1\nera early ratified d-1\nera middle ratified d-1\nevent change during early ratified d-1\nevent bridge during early ratified d-1\nbefore early -> bridge ratified d-1\nbefore bridge -> middle ratified d-1\nstate wren.home = new since change ratified d-1\n";
    assert!(run(source, "middle", "home").is_empty());
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let middle = TimePoint(compiled.model.world.time_points.lookup("middle").unwrap().0);
    let state = dli_world::check::slot_state_at(&compiled, "home", "wren", middle).unwrap();
    assert_eq!(state.holdings.len(), 1);
    assert_eq!(
        state.holdings[0].projection,
        dli_time::fluent::Projection::OrderIndeterminate
    );
}

#[test]
fn changing_fluent_projects_the_current_value() {
    assert_eq!(run(WORLD, "first", "home"), vec!["home(wren, old)"]);
    assert_eq!(run(WORLD, "second", "home"), vec!["home(wren, new)"]);
}

#[test]
fn independent_timeless_support_is_not_removed_with_a_temporal_span() {
    let source = format!("{WORLD}wren.home = old ratified d-0001\n");
    assert_eq!(
        run(&source, "second", "home"),
        vec!["home(wren, new)", "home(wren, old)"]
    );
}

#[test]
fn derived_rules_are_recomputed_from_the_temporal_projection() {
    let source = format!(
        "{WORLD}rule copy-home ratified d-0001\n  resides($who, $where) :- home($who, $where)\n"
    );
    assert_eq!(run(&source, "first", "resides"), vec!["resides(wren, old)"]);
    assert_eq!(
        run(&source, "second", "resides"),
        vec!["resides(wren, new)"]
    );
}

#[test]
fn an_unordered_anchor_stays_possible_instead_of_certain() {
    let source = WORLD
        .replace("before first -> second ratified d-0001\n", "")
        .replace("inertial home ratified d-0001\n", "");
    assert!(run(&source, "second", "home").contains(&"home(wren, new)".to_owned()));
    assert!(!run(&source, "second", "home").contains(&"home(wren, old)".to_owned()));
    let (document, _) = dli_syntax::parse_document(&source);
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let point = TimePoint(compiled.model.world.time_points.lookup("second").unwrap().0);
    let world = dli_world::check::continuity::query_world_at(&compiled, point).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("home"),
            args: vec![
                Term::Sym(Ident::synthetic("wren")),
                Term::Sym(Ident::synthetic("old")),
            ],
        },
    };
    let answer = world
        .query_with(
            &query,
            &QueryOpts {
                layer: Layer::Possible,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(answer.rows[0].status, dli_core::ann::Status::Possible);
}

#[test]
fn a_known_fluent_with_no_value_at_the_cutoff_returns_no_rows() {
    let source = WORLD.replace("event first ratified d-0001\n", "event before-all ratified d-0001\nevent first ratified d-0001\nbefore before-all -> first ratified d-0001\n");
    assert!(run(&source, "before-all", "home").is_empty());
}

#[test]
fn equal_values_in_disjoint_spans_keep_the_surviving_source() {
    let source = "scope storybook\nratify d-0001\nratify d-0002\nevent end ratified d-0001\nevent start ratified d-0001\nbefore end -> start ratified d-0001\nstate wren.home = old until end ratified d-0001\nstate wren.home = old since start ratified d-0002\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("start").unwrap().0);
    let state = dli_world::check::slot_state_at(&compiled, "home", "wren", at).unwrap();
    assert_eq!(state.holdings.len(), 1);
    assert_eq!(state.holdings[0].citations, vec!["d-0002"]);
    let projected = dli_world::check::continuity::query_world_at(&compiled, at).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("home"),
            args: vec![
                Term::Sym(Ident::synthetic("wren")),
                Term::Sym(Ident::synthetic("old")),
            ],
        },
    };
    let answer = projected
        .query_with(
            &query,
            &QueryOpts {
                why: true,
                ..Default::default()
            },
        )
        .unwrap();
    let why = &answer.rows[0].why.as_ref().unwrap().rendered;
    assert!(why.iter().any(|line| line.contains("d-0002")), "{why:?}");
    assert!(why.iter().all(|line| !line.contains("d-0001")), "{why:?}");
}

#[test]
fn opposing_bounds_cannot_produce_an_impossible_derived_fact() {
    let source = "scope storybook\nratify d-0001\nevent end ratified d-0001\nevent cutoff ratified d-0001\nstate wren.home = old until end ratified d-0001\nstate wren.home = new since end ratified d-0001\nrule impossible ratified d-0001\n  impossible($who) :- home($who, old), home($who, new)\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("cutoff").unwrap().0);
    let projected = dli_world::check::continuity::query_world_at(&compiled, at).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("impossible"),
            args: vec![Term::Sym(Ident::synthetic("wren"))],
        },
    };
    let answer = projected
        .query_with(
            &query,
            &QueryOpts {
                layer: Layer::Possible,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(answer.rows.is_empty(), "{answer:?}");
}

#[test]
fn uncertain_inertial_projection_refuses_instead_of_guessing() {
    let source = WORLD.replace("before first -> second ratified d-0001\n", "");
    let (document, _) = dli_syntax::parse_document(&source);
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("second").unwrap().0);
    let error = dli_world::check::continuity::query_world_at(&compiled, at).unwrap_err();
    assert!(
        error.to_string().contains("inertial") && error.to_string().contains("unresolved"),
        "{error}"
    );
}

#[test]
fn unordered_successor_cannot_leave_an_old_inertial_value_certain() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nevent cutoff ratified d-0001\nbefore first -> second ratified d-0001\nbefore first -> cutoff ratified d-0001\ninertial home ratified d-0001\nstate wren.home = old since first ratified d-0001\nstate wren.home = new since second ratified d-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("cutoff").unwrap().0);
    let state = dli_world::check::slot_state_at(&compiled, "home", "wren", at).unwrap();
    assert!(
        state
            .holdings
            .iter()
            .all(|h| h.projection != dli_time::fluent::Projection::HoldsCertainly),
        "{state:?}"
    );
    assert!(dli_world::check::continuity::query_world_at(&compiled, at).is_err());
}

#[test]
fn unary_states_project_their_temporal_bounds() {
    for state in [
        "state wren.awake since second",
        "state awake = wren since second",
    ] {
        let source = format!(
            "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\n{state} ratified d-0001\n"
        );
        let (document, diagnostics) = dli_syntax::parse_document(&source);
        assert!(
            diagnostics.iter().all(|d| !d.class.is_error()),
            "{diagnostics:?}"
        );
        let compiled = dli_world::compile(&Pack {
            name: "storybook".into(),
            documents: vec![document],
        })
        .unwrap();
        let query = Query {
            goal: AtomExpr {
                pred: Ident::synthetic("awake"),
                args: vec![Term::Sym(Ident::synthetic("wren"))],
            },
        };
        for (point, count) in [("first", 0), ("second", 1)] {
            let at = TimePoint(compiled.model.world.time_points.lookup(point).unwrap().0);
            let projected = dli_world::check::continuity::query_world_at(&compiled, at).unwrap();
            let answer = projected
                .query_with(
                    &query,
                    &QueryOpts {
                        layer: Layer::Certain,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(answer.rows.len(), count, "{state}: {point}: {answer:?}");
        }
    }
}

#[test]
fn cyclic_chronology_is_refused_even_without_unordered_anchors() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\nbefore second -> first ratified d-0001\nstate wren.home = old since second ratified d-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("first").unwrap().0);
    assert!(matches!(
        dli_world::check::continuity::query_world_at(&compiled, at),
        Err(dli_world::check::continuity::TemporalQueryError::Indeterminate(_))
    ));
}

#[test]
fn unrelated_uncertainty_does_not_block_a_projected_fluent_or_its_rules() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent cutoff ratified d-0001\nevent other proposed\nbefore first -> cutoff ratified d-0001\nbefore other -> cutoff proposed\nstate wren.home = burrow since first ratified d-0001\nstate fox.hat = blue since other proposed\nrule copy-home ratified d-0001\n  resides($who, $where) :- home($who, $where)\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("cutoff").unwrap().0);
    assert!(dli_world::check::continuity::query_world_at(&compiled, at).is_err());
    for pred in ["home", "resides"] {
        let world =
            dli_world::check::continuity::query_world_at_for_goal(&compiled, at, pred).unwrap();
        let query = Query {
            goal: AtomExpr {
                pred: Ident::synthetic(pred),
                args: vec![
                    Term::Sym(Ident::synthetic("wren")),
                    Term::Sym(Ident::synthetic("burrow")),
                ],
            },
        };
        let answer = world
            .query_with(
                &query,
                &QueryOpts {
                    layer: Layer::Certain,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(answer.rows.len(), 1, "{pred}: {answer:?}");
    }
    assert!(dli_world::check::continuity::query_world_at_for_goal(&compiled, at, "hat").is_err());
}

#[test]
fn unrelated_fluents_do_not_block_queries_but_rule_dependencies_do() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nevent cutoff ratified d-0001\nbefore first -> second ratified d-0001\nbefore first -> cutoff ratified d-0001\ninertial home ratified d-0001\nstate wren.home = old since first ratified d-0001\nstate wren.home = new since second ratified d-0001\nfact wren friend fox ratified d-0001\nrule derived ratified d-0001\n  resides($who, $where) :- home($who, $where)\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("cutoff").unwrap().0);
    assert!(dli_world::check::continuity::query_world_at_for_goal(&compiled, at, "friend").is_ok());
    assert!(
        dli_world::check::continuity::query_world_at_for_goal(&compiled, at, "resides").is_err()
    );
}

#[test]
fn legacy_world_without_temporal_mapping_must_be_recompiled() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\nstate wren.awake since second ratified d-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let mut compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("first").unwrap().0);
    compiled.model.temporal_sources_complete = false;
    compiled.model.standalone_temporal_facts.clear();
    compiled.model.temporal_atom_facts.clear();
    assert!(matches!(
        dli_world::check::continuity::query_world_at_for_goal(&compiled, at, "awake"),
        Err(dli_world::check::continuity::TemporalQueryError::Internal(
            _
        ))
    ));
}

#[test]
fn cutoff_constraints_respect_known_order_between_distinct_anchors() {
    let source = "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nevent cutoff ratified d-0001\nbefore first -> second ratified d-0001\nstate wren.home = old until first ratified d-0001\nstate wren.home = new since second ratified d-0001\nrule impossible ratified d-0001\n  impossible($who) :- home($who, old), home($who, new)\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let compiled = dli_world::compile(&Pack {
        name: "storybook".into(),
        documents: vec![document],
    })
    .unwrap();
    let at = TimePoint(compiled.model.world.time_points.lookup("cutoff").unwrap().0);
    let projected = dli_world::check::continuity::query_world_at(&compiled, at).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("impossible"),
            args: vec![Term::Sym(Ident::synthetic("wren"))],
        },
    };
    let answer = projected
        .query_with(
            &query,
            &QueryOpts {
                layer: Layer::Possible,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(answer.rows.is_empty(), "{answer:?}");
}
