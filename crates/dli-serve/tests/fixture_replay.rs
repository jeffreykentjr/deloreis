//! CON-1 cold-replay oracle (R3/H5): every registered fixture is driven COLD
//! through the real `respond` path — from a fresh session, with its declared
//! `precondition` frames run first (setup, not asserted) — and its declared
//! `expected_outcome` is asserted. A fixture that pins a verdict the running
//! engine does not produce fails HERE, so the vendored manifest cannot silently
//! document a fiction (the gap R3 found: `serve_owned` skipped the gated arms and
//! the author fixtures were never replayed).
//!
//! Outcomes: `replay` (default) asserts each frame's response equals its `expect`
//! (goldens/framing) or its `expect_error` code (negatives); `gated` executes
//! every declared step, asserts its exact error response, and additionally
//! requires `not-wired`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_world::{KilledTokens, Pack};
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/dli-worker/fixtures")
}

fn registered_json_fixtures() -> Vec<PathBuf> {
    let fixtures = fixtures_dir();
    let contract = fixtures.parent().expect("contract directory");
    let registry: serde_yaml_ng::Value = serde_yaml_ng::from_str(
        &fs::read_to_string(contract.join("registry.yaml")).expect("read fixture registry"),
    )
    .expect("parse fixture registry");
    let mut paths: Vec<PathBuf> = registry["fixtures"]
        .as_sequence()
        .expect("fixture registry list")
        .iter()
        .map(|entry| PathBuf::from(entry["path"].as_str().expect("registered fixture path")))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    paths.sort();
    paths
}

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let line = serde_json::to_string(frame).unwrap();
    let response =
        respond(&line, sessions, store).unwrap_or_else(|| panic!("no response for frame {frame}"));
    serde_json::to_value(&response).unwrap()
}

fn error_code(response: &Value) -> Option<&str> {
    response
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
}

fn run_precondition(case: &Value, sessions: &mut Sessions, store: &mut Store, name: &str) {
    run_precondition_field(case, "precondition", sessions, store, name);
}

fn run_precondition_field(
    case: &Value,
    field: &str,
    sessions: &mut Sessions,
    store: &mut Store,
    name: &str,
) {
    if let Some(precondition) = case.get(field).and_then(Value::as_array) {
        for frame in precondition {
            let response = drive(frame, sessions, store);
            assert_eq!(
                response.get("ok"),
                Some(&Value::Bool(true)),
                "{name}: paired {field} failed: {response}"
            );
        }
    }
}

fn assert_pointer_pairs_equal(control: &Value, attack: &Value, paths: &[Value], name: &str) {
    for path in paths {
        let path = path.as_str().expect("validated pointer");
        assert!(
            control.pointer(path).is_some() && attack.pointer(path).is_some(),
            "{name}: safety projection path {path} is absent from a response"
        );
        assert_eq!(
            control.pointer(path),
            attack.pointer(path),
            "{name}: safety projection differs at {path}\ncontrol={control}\nattack={attack}"
        );
    }
}

fn marker_paths(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::String(text) if text.contains("__DLI_FRAME_ATTACK__") => {
            out.push(path.to_owned());
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                marker_paths(item, &format!("{path}/{index}"), out);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                marker_paths(
                    item,
                    &format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                    out,
                );
            }
        }
        _ => {}
    }
}

fn assert_taint_confined(response: &Value, case: &Value, name: &str) {
    let allowed: Vec<&str> = case
        .get("allow_taint_paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut found = Vec::new();
    marker_paths(response, "", &mut found);
    for path in found {
        assert!(
            path == "/id" || allowed.iter().any(|prefix| path.starts_with(prefix)),
            "{name}: attack marker escaped to undeclared response path {path}"
        );
    }
}

fn assert_changed_parameter_was_reached(
    attack: &Value,
    attack_probe: Option<&Value>,
    case: &Value,
    name: &str,
) {
    let mut markers = Vec::new();
    marker_paths(attack, "", &mut markers);
    if let Some(probe) = attack_probe {
        marker_paths(probe, "", &mut markers);
    }
    let asserted_post_state = case
        .get("probe_expect")
        .and_then(Value::as_object)
        .is_some_and(|expected| !expected.is_empty());
    assert!(
        !markers.is_empty() || asserted_post_state,
        "{name}: the attack marker is absent from the response and no post-state effect is \
         asserted; the handler may have ignored the changed parameter\nattack={attack}"
    );
}

fn assert_probe_expect(response: &Value, case: &Value, name: &str) {
    let Some(expected) = case.get("probe_expect").and_then(Value::as_object) else {
        return;
    };
    for (path, value) in expected {
        assert_eq!(
            response.pointer(path),
            Some(value),
            "{name}: post-state probe did not reach expected value at {path}\nprobe={response}"
        );
    }
}

fn assert_materialized_reparses(response: &Value, name: &str) {
    let Some(rendered) = response
        .pointer("/result/rendered")
        .and_then(Value::as_array)
    else {
        return;
    };
    if rendered
        .first()
        .and_then(Value::as_str)
        .is_none_or(|line| !line.starts_with("scope "))
    {
        return;
    }
    let source = rendered
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    let (_, diagnostics) = dli_syntax::parse_document(&source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{name}: materialized attack output does not reparse: {diagnostics:?}\n{source}"
    );
}

fn replay_framing_case(case: &Value, name: &str) {
    let (mut control_sessions, mut control_store) = fixture_stores();
    let (mut attack_sessions, mut attack_store) = fixture_stores();
    run_precondition(case, &mut control_sessions, &mut control_store, name);
    run_precondition(case, &mut attack_sessions, &mut attack_store, name);
    run_precondition_field(
        case,
        "control_precondition",
        &mut control_sessions,
        &mut control_store,
        name,
    );
    run_precondition_field(
        case,
        "attack_precondition",
        &mut attack_sessions,
        &mut attack_store,
        name,
    );

    let control = drive(&case["control"], &mut control_sessions, &mut control_store);
    let attack = drive(&case["attack"], &mut attack_sessions, &mut attack_store);
    let compare = case["compare_paths"].as_array().expect("validated paths");
    assert_pointer_pairs_equal(&control, &attack, compare, name);
    assert_taint_confined(&attack, case, name);
    if case.get("expected_outcome").and_then(Value::as_str) == Some("gated") {
        assert_eq!(error_code(&control), Some("not-wired"), "{name}: {control}");
        assert_eq!(error_code(&attack), Some("not-wired"), "{name}: {attack}");
        return;
    }
    assert_materialized_reparses(&attack, name);

    let probes = case.get("probe").map(|probe| {
        let control_probe = drive(probe, &mut control_sessions, &mut control_store);
        let attack_probe = drive(probe, &mut attack_sessions, &mut attack_store);
        let probe_paths = case
            .get("probe_compare_paths")
            .and_then(Value::as_array)
            .expect("a probe carries probe_compare_paths");
        assert_pointer_pairs_equal(&control_probe, &attack_probe, probe_paths, name);
        assert_eq!(
            control_probe
                .pointer("/result/rendered")
                .and_then(Value::as_array)
                .map(Vec::len),
            attack_probe
                .pointer("/result/rendered")
                .and_then(Value::as_array)
                .map(Vec::len),
            "{name}: control and attack changed different numbers of declarations"
        );
        assert_taint_confined(&attack_probe, case, name);
        assert_materialized_reparses(&attack_probe, name);
        assert_probe_expect(&control_probe, case, name);
        assert_probe_expect(&attack_probe, case, name);
        (control_probe, attack_probe)
    });
    assert_changed_parameter_was_reached(
        &attack,
        probes.as_ref().map(|(_, attack)| attack),
        case,
        name,
    );
}

fn fixture_stores() -> (Sessions, Store) {
    let root = fixtures_dir()
        .parent()
        .expect("contract directory")
        .join("fixture-project");
    let project = Arc::new(Project::load(root).expect("the committed fixture project loads"));
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    (
        Sessions::from_project(Arc::clone(&project)),
        Store::with_packs(packs).expect("the committed fixture project compiles"),
    )
}

fn legacy_fixture_author_store() -> Store {
    let source = "\
scope saltmarket
ratify d-1
era first-tides decided d-1
fork exploration
  option baseline
fork ep9-cast
  option baseline
";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error())
    );
    Store::with_packs([(
        "saltmarket".to_owned(),
        Pack {
            name: "saltmarket".to_owned(),
            documents: vec![document],
        },
        KilledTokens::default(),
    )])
    .expect("legacy fixture corpus compiles")
}

#[test]
fn every_fixture_replays_cold() {
    let mut replayed = 0;
    let registered = registered_json_fixtures();
    for relative in &registered {
        let path = fixtures_dir().join(relative);
        let name = relative.to_string_lossy().into_owned();
        let fixture: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

        // A fresh session per fixture: handles (ws-1, fork-1) are deterministic.
        if let Some(cases) = fixture.get("cases").and_then(Value::as_array) {
            for (index, case) in cases.iter().enumerate() {
                replay_framing_case(case, &format!("{name} case {index}"));
            }
            replayed += 1;
            continue;
        }

        let mut sessions = Sessions::new();
        let mut store = legacy_fixture_author_store();

        // Precondition frames set up state (open a workspace, stage a finding)
        // and must succeed; they are not asserted against any expect.
        if let Some(pre) = fixture.get("precondition").and_then(Value::as_array) {
            for frame in pre {
                let response = drive(frame, &mut sessions, &mut store);
                assert_eq!(
                    response.get("ok"),
                    Some(&Value::Bool(true)),
                    "{name}: precondition frame did not succeed: {response}"
                );
            }
        }

        let outcome = fixture
            .get("expected_outcome")
            .and_then(Value::as_str)
            .unwrap_or("replay");

        if outcome == "gated" {
            let steps = fixture
                .get("steps")
                .and_then(Value::as_array)
                .expect("a gated golden carries explicit steps");
            for step in steps {
                let response = drive(&step["send"], &mut sessions, &mut store);
                assert_eq!(
                    error_code(&response),
                    Some("not-wired"),
                    "{name}: a capability-gated op must answer not-wired, got {response}"
                );
                assert_eq!(
                    response, step["expect"],
                    "{name} @ id {}",
                    step["send"]["id"]
                );
            }
        } else if let Some(steps) = fixture.get("steps").and_then(Value::as_array) {
            for step in steps {
                let response = drive(&step["send"], &mut sessions, &mut store);
                assert_eq!(
                    response, step["expect"],
                    "{name} @ id {}",
                    step["send"]["id"]
                );
            }
        } else if let Some(expect_error) = fixture.get("expect_error") {
            let response = drive(&fixture["send"], &mut sessions, &mut store);
            assert_eq!(
                response.get("ok"),
                Some(&Value::Bool(false)),
                "{name}: a negative must not succeed: {response}"
            );
            assert_eq!(
                error_code(&response),
                expect_error.get("code").and_then(Value::as_str),
                "{name}: wrong error code: {response}"
            );
        }
        replayed += 1;
    }
    assert_eq!(
        replayed,
        registered.len(),
        "every registered JSON fixture must replay exactly once"
    );
}

#[test]
#[should_panic(expected = "handler may have ignored the changed parameter")]
fn successful_framing_case_without_an_observable_effect_fails() {
    let response = serde_json::json!({"ok": true, "result": {"accepted": true}});
    assert_changed_parameter_was_reached(&response, None, &serde_json::json!({}), "shadowed-field");
}

#[test]
#[should_panic(expected = "post-state probe did not reach expected value")]
fn mutation_probe_rejects_a_no_op_post_state() {
    let response = serde_json::json!({"ok": true, "result": {"forks": [{"local_atoms": 0}]}});
    let case = serde_json::json!({
        "probe_expect": {"/result/forks/0/local_atoms": 1}
    });
    assert_probe_expect(&response, &case, "no-op-mutation");
}
