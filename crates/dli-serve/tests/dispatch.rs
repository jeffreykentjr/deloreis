//! SERVE-1 dispatch tests: the dli-worker fixtures are the oracle.
//!
//! Every implemented golden fixture owned by SERVE-1 replays through the real
//! `respond` path (framing + routing + engine), threading session state across a
//! multi-step flow; every negative fixture yields its typed error code; the raw
//! frames exercise the framing discipline. Fixtures owned by Door T, killed
//! forms, and capability-gated runtimes are covered elsewhere.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use dli_serve::ErrorCode;
use dli_serve::author::store::Store;
use dli_serve::envelope::Response;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/dli-worker/fixtures")
}

/// The ops whose implemented golden fixture replays in v0. Excluded:
/// `author.*` (Door T), the killed `check-ledger` form, and fixtures explicitly
/// marked as capability-gated.
fn serve_owned(op: &str) -> bool {
    op.starts_with("dli.") && !["dli.check-ledger"].contains(&op)
}

fn json_fixtures(sub: &str) -> Vec<(String, Value)> {
    let dir = fixtures_dir().join(sub);
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        out.push((name, value));
    }
    out
}

/// Feed one request `send` value through the framing path as a wire line.
fn send(value: &Value, sessions: &mut Sessions, store: &mut Store) -> Option<Response> {
    let line = serde_json::to_string(value).unwrap();
    respond(&line, sessions, store)
}

#[test]
fn golden_fixtures_replay_through_dispatch() {
    let mut replayed = 0;
    for (name, fixture) in json_fixtures("golden") {
        let steps = fixture["steps"].as_array().cloned().unwrap_or_default();
        let ours = fixture.get("project").and_then(Value::as_bool) != Some(true)
            && fixture["expected_outcome"].as_str() != Some("gated")
            && !steps.is_empty()
            && steps
                .iter()
                .all(|step| step["send"]["op"].as_str().is_some_and(serve_owned));
        if !ours {
            continue;
        }
        // A fresh session per fixture: fork handles are then deterministic
        // (`fork-1`, `fork-2`, …) and no state leaks between flows.
        let mut sessions = Sessions::new();
        let mut store = Store::default();
        for step in &steps {
            let response = send(&step["send"], &mut sessions, &mut store)
                .unwrap_or_else(|| panic!("{name}: a request produced no response"));
            let got = serde_json::to_value(&response).unwrap();
            assert_eq!(
                got, step["expect"],
                "golden {name} @ id {}",
                step["send"]["id"]
            );
        }
        replayed += 1;
    }
    assert!(
        replayed >= 5,
        "expected the dli.* golden fixtures to replay, replayed {replayed}"
    );
}

#[test]
fn negative_fixtures_yield_their_error_code() {
    let mut checked = 0;
    for (name, fixture) in json_fixtures("negative") {
        // Single-frame negatives with a `send` + `expect_error`. Step-based
        // framing fixtures (author-framing-attack) are golden-shaped and owned
        // by door-t.
        let (Some(request), Some(expect_error)) =
            (fixture.get("send"), fixture.get("expect_error"))
        else {
            continue;
        };
        let op = request["op"].as_str().unwrap_or_default();
        if op.starts_with("author.") {
            continue; // door-t's Door T seam
        }
        // This oracle drives a projectless session, so it can only reach
        // refusals that fire above the project seam. A fixture that declares
        // `project: true` needs the loaded fixture project to reach its verdict
        // at all; `fixture_replay::every_fixture_replays_cold` is where those
        // are driven, project-backed and cold.
        if fixture.get("project").and_then(serde_json::Value::as_bool) == Some(true) {
            continue;
        }
        let expected = expect_error["code"].as_str().unwrap();
        let mut sessions = Sessions::new();
        let mut store = Store::default();
        let response = send(request, &mut sessions, &mut store).expect("a frame is answered");
        assert!(!response.ok, "negative {name} should not be ok");
        let code = response.error.as_ref().unwrap().code.wire_name();
        assert_eq!(code, expected, "negative {name}");
        checked += 1;
    }
    assert!(checked >= 4, "expected dli.* negatives, checked {checked}");
}

#[test]
fn raw_frames_behave_per_file() {
    let dir = fixtures_dir().join("raw");
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    // A log-noise line is skipped (no answer); the following real frame is
    // answered — the stream never desyncs.
    let noise = fs::read_to_string(dir.join("stdout-noise.raw")).unwrap();
    let answered: Vec<Response> = noise
        .lines()
        .filter_map(|line| respond(line, &mut sessions, &mut store))
        .collect();
    assert_eq!(answered.len(), 1, "one frame answered after the noise line");
    assert!(answered[0].ok, "the trailing dli.version frame succeeds");

    // A truncated frame is a bad frame.
    let partial = fs::read_to_string(dir.join("partial-frame.raw")).unwrap();
    let response = respond(partial.lines().next().unwrap(), &mut sessions, &mut store).unwrap();
    assert_eq!(response.error.unwrap().code.wire_name(), "bad-frame");

    // A duplicate key is a bad frame (the strict decode rejects it).
    let dup = fs::read_to_string(dir.join("duplicate-key.raw")).unwrap();
    let response = respond(dup.lines().next().unwrap(), &mut sessions, &mut store).unwrap();
    assert_eq!(response.error.unwrap().code.wire_name(), "bad-frame");
}

#[test]
fn fork_flow_threads_session_state() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    let new = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.fork.new","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(new.result.unwrap()["fork"], "fork-1");

    let suppose = send(
        &serde_json::json!({"dli":"0.5.1","id":"2","op":"dli.fork.suppose","params":{"fork":"fork-1","assume":["h.camelot#0"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let delta = suppose.result.unwrap();
    assert_eq!(
        delta["newly_certain"][0]["atom"],
        serde_json::json!({"pred": "kept-at", "args": ["excalibur", "camelot"]})
    );
    assert_eq!(delta["newly_certain"][0]["before"]["status"], "possible");
    assert_eq!(delta["newly_certain"][0]["after"]["status"], "certain");

    let listed = send(
        &serde_json::json!({"dli":"0.5.1","id":"list","op":"dli.fork.list","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    assert_eq!(listed["forks"][0]["fork"], "fork-1");
    assert_eq!(listed["forks"][0]["assumes"]["h.camelot#0"], true);
    assert_eq!(listed["forks"][0]["local_atoms"], 0);
    assert_eq!(listed["forks"][0]["restrictions"], 0);

    let preflight = send(
        &serde_json::json!({"dli":"0.5.1","id":"preflight","op":"dli.fork.preflight","params":{"fork":"fork-1"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    assert_eq!(preflight["candidate"]["h.camelot#0"], true);
    assert!(
        preflight["candidate"]
            .as_object()
            .is_some_and(|candidate| candidate.keys().all(|name| !name.starts_with("var#"))),
        "candidate valuation exposes stable names"
    );

    let promoted = send(
        &serde_json::json!({"dli":"0.5.1","id":"promote","op":"dli.fork.promote","params":{"fork":"fork-1"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    assert_eq!(promoted["rq_draft"]["slug"], "fork-1");
    assert!(
        !promoted["rq_draft"]["ask"]
            .as_str()
            .unwrap()
            .contains("fork-0"),
        "promotion never leaks the internal zero-based id"
    );

    // The suppose persisted on the fork: a later diff sees the same consequence.
    let diff = send(
        &serde_json::json!({"dli":"0.5.1","id":"3","op":"dli.fork.diff","params":{"fork":"fork-1"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(
        diff.result.unwrap()["newly_certain"][0]["atom"],
        serde_json::json!({"pred": "kept-at", "args": ["excalibur", "camelot"]})
    );
}

#[test]
fn rejected_fork_suppose_batch_has_no_partial_side_effect() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.fork.new","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();

    let rejected = send(
        &serde_json::json!({"dli":"0.5.1","id":"2","op":"dli.fork.suppose","params":{"fork":"fork-1","assume":["h.camelot#0","missing-target"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(!rejected.ok);

    let listed = send(
        &serde_json::json!({"dli":"0.5.1","id":"3","op":"dli.fork.list","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    assert_eq!(listed["forks"][0]["assumes"], serde_json::json!({}));
    let diff = send(
        &serde_json::json!({"dli":"0.5.1","id":"4","op":"dli.fork.diff","params":{"fork":"fork-1"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    assert_eq!(diff["newly_certain"], serde_json::json!([]));
}

#[test]
fn unknown_document_is_unknown_doc() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let response = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.check","params":{"document":"nowhere"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(response.error.unwrap().code.wire_name(), "unknown-doc");
}

#[test]
fn killed_and_unknown_ops_are_refused() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    // A registered killed form is `killed-form` naming its successor; a truly
    // unknown op is `unknown-op` (M1).
    for (op, expected, successor) in [
        ("dli.suppose", "killed-form", Some("dli.fork.suppose")),
        ("dli.check-ledger", "killed-form", Some("dli.check-proof")),
        ("dli.frobnicate", "unknown-op", None),
    ] {
        let response = send(
            &serde_json::json!({"dli":"0.5.1","id":"1","op":op,"params":{}}),
            &mut sessions,
            &mut store,
        )
        .unwrap();
        let error = response.error.unwrap();
        assert_eq!(error.code.wire_name(), expected, "op {op}");
        if let Some(successor) = successor {
            assert!(
                error.message.contains(successor),
                "op {op} names its successor"
            );
        }
    }
}

#[test]
fn empty_param_structs_still_require_json_objects() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    for op in ["dli.version", "author.list_workspaces"] {
        let response = send(
            &serde_json::json!({"dli":"0.5.1","id":"shape","op":op,"params":[]}),
            &mut sessions,
            &mut store,
        )
        .unwrap();
        assert_eq!(
            response.error.unwrap().code.wire_name(),
            "schema-violation",
            "{op} accepted array params despite its object schema"
        );
    }
}

#[test]
fn timeline_and_fork_add_are_wired() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    // A corpus with no chronology reads the honest empty state, never `not-wired`.
    let bare = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.timeline.show","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(bare.ok, "timeline.show is wired: {bare:?}");
    let bare = bare.result.unwrap();
    assert_eq!(bare["layers"], serde_json::json!([]));
    assert_eq!(bare["cycle"], serde_json::json!([]));
    assert_eq!(bare["placements"], serde_json::json!([]));

    // The neutral round-table world carries a real era spine, so show lays it out.
    let laid = send(
        &serde_json::json!({"dli":"0.5.1","id":"2","op":"dli.timeline.show","params":{"document":"round-table"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let layers = laid.result.unwrap()["layers"].clone();
    assert!(
        layers.as_array().is_some_and(|layers| !layers.is_empty())
            && layers[0]["rank"] == 0
            && layers[0]["points"]
                .as_array()
                .is_some_and(|points| !points.is_empty()),
        "chronology is laid out in typed layers: {layers}"
    );
    let cursor = send(
        &serde_json::json!({"dli":"0.5.1","id":"cursor","op":"dli.timeline.show","params":{"document":"round-table","at":"arthurs-reign"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    assert_eq!(
        cursor["cursor"]["before"],
        serde_json::json!(["uthers-time"])
    );
    assert_eq!(
        cursor["cursor"]["after"],
        serde_json::json!(["after-camlann"])
    );

    // fork.add parses a declaration line and adds its lowered fact to the fork.
    send(
        &serde_json::json!({"dli":"0.5.1","id":"3","op":"dli.fork.new","params":{"document":"round-table"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let added = send(
        &serde_json::json!({"dli":"0.5.1","id":"4","op":"dli.fork.add","params":{"fork":"fork-1","declarations":["adjacent(camelot, avalon) proposed"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(added.ok, "fork.add is wired: {added:?}");
    assert_eq!(
        added.result.unwrap()["delta"]["newly_certain"][0]["atom"],
        serde_json::json!({"pred": "adjacent", "args": ["camelot", "avalon"]})
    );

    let slot = send(
        &serde_json::json!({"dli":"0.5.1","id":"slot","op":"dli.fork.add","params":{"fork":"fork-1","declarations":["camelot.role = \"island court\" proposed"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(
        slot.result.unwrap()["delta"]["newly_certain"]
            .as_array()
            .is_some_and(|changes| changes.iter().any(|change| {
                change["atom"]
                    == serde_json::json!({"pred": "role", "args": ["camelot", "island court"]})
            })),
        "slot declarations become fork-local facts"
    );

    let unsupported = send(
        &serde_json::json!({"dli":"0.5.1","id":"entity","op":"dli.fork.add","params":{"fork":"fork-1","declarations":["place avalon proposed"]}}),
        &mut sessions,
        &mut store,
    )
    .expect("non-fact declarations receive a response");
    assert!(!unsupported.ok);
    assert_eq!(
        unsupported.error.expect("typed error").code,
        dli_serve::ErrorCode::SchemaViolation
    );
}

#[test]
fn check_proof_is_wired_and_validates_params() {
    // check-proof is wired: it replays the document's proof block against its
    // practice and the certain-layer scene projection, returning the rich
    // ProofReport. A malformed request is still a schema error, caught before the
    // handler ever runs.
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    let wired = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.check-proof","params":{"document":"warding-vows","proof":"e01-s04"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(wired.ok, "check-proof is wired: {wired:?}");
    let report = &wired.result.unwrap()["report"];
    assert_eq!(report["checks"], serde_json::json!(true));
    assert_eq!(report["verdict"], serde_json::json!("holds"));

    let malformed = send(
        &serde_json::json!({"dli":"0.5.1","id":"2","op":"dli.check-proof","params":{}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(
        malformed.error.unwrap().code.wire_name(),
        "schema-violation"
    );
}

#[test]
fn serve_loop_survives_non_utf8_bytes() {
    // R3 C2: a single hostile non-UTF-8 byte between two valid frames must not
    // abort the loop — both valid frames are answered, the bad line skipped.
    let mut input: Vec<u8> = Vec::new();
    input.extend_from_slice(br#"{"dli":"0.5.1","id":"1","op":"dli.version","params":{}}"#);
    input.push(b'\n');
    input.push(0xff); // an invalid UTF-8 byte on its own line
    input.push(b'\n');
    input.extend_from_slice(br#"{"dli":"0.5.1","id":"2","op":"dli.version","params":{}}"#);
    input.push(b'\n');

    let mut reader = std::io::Cursor::new(input);
    let mut output: Vec<u8> = Vec::new();
    dli_serve::serve::serve_io(&mut reader, &mut output).expect("the loop never aborts on input");

    let answered: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        answered.len(),
        2,
        "both valid frames answered, noise skipped"
    );
    assert_eq!(answered[0]["id"], "1");
    assert_eq!(answered[1]["id"], "2");
    assert!(answered.iter().all(|r| r["ok"] == true));
}

#[test]
fn unknown_envelope_field_is_schema_violation() {
    // R3 H1: the envelope must reject unknown/mistyped top-level fields, so a
    // stray field is never silently dropped (contract §2).
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    let stray = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.version","params":{},"zzz":1}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(stray.error.unwrap().code.wire_name(), "schema-violation");

    // A mistyped `expected_digest` (typo'd key) is a stray field, caught before
    // dispatch — otherwise it would silently defeat the stale-fence.
    let typo = send(
        &serde_json::json!({"dli":"0.5.1","id":"2","op":"author.materialize","params":{"workspace":"ws-1"},"expcted_digest":"jcs:x"}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(typo.error.unwrap().code.wire_name(), "schema-violation");
}

#[test]
fn fences_are_refused_on_native_read_and_batch_operations() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    for request in [
        serde_json::json!({
            "dli":"0.5.1", "id":"1", "op":"author.status",
            "params":{"workspace":"ws-1"}, "expected_digest":"jcs:x"
        }),
        serde_json::json!({
            "dli":"0.5.1", "id":"2", "op":"dli.check",
            "params":{"document":"camelot"}, "idempotency_key":"not-a-mutation"
        }),
    ] {
        let response = send(&request, &mut sessions, &mut store).expect("frame is answered");
        assert_eq!(
            response.error.expect("fence is refused").code.wire_name(),
            "schema-violation"
        );
    }
}

#[test]
fn wrong_contract_version_is_version_mismatch() {
    // M1: a well-formed frame declaring a contract this worker does not speak is
    // a `version-mismatch`, not a bad frame.
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let response = send(
        &serde_json::json!({"dli":"9.9.9","id":"1","op":"dli.version","params":{}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(response.error.unwrap().code.wire_name(), "version-mismatch");
}

#[test]
fn null_valued_unknown_fields_are_rejected_recursively() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    for frame in [
        serde_json::json!({
            "dli": "0.5.1",
            "id": "envelope",
            "op": "dli.version",
            "params": {},
            "unknown": null
        }),
        serde_json::json!({
            "dli": "0.5.1",
            "id": "nested",
            "op": "dli.check-draft",
            "params": {
                "document": "camelot",
                "source": "draft",
                "payload": {
                    "meta": {
                        "source": {
                            "sha256": "invalid"
                        },
                        "unknown": null
                    }
                }
            }
        }),
    ] {
        let response = send(&frame, &mut sessions, &mut store).expect("frame is answered");
        assert_eq!(
            response.error.as_ref().map(|error| error.code.wire_name()),
            Some("schema-violation")
        );
    }
}

#[test]
fn capabilities_include_durable_simulation_jobs() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let version = send(
        &serde_json::json!({
            "dli": "0.5.1",
            "id": "version",
            "op": "dli.version",
            "params": {}
        }),
        &mut sessions,
        &mut store,
    )
    .expect("frame is answered");
    let capabilities = version
        .result
        .as_ref()
        .and_then(|result| result["capabilities"].as_array())
        .expect("version capabilities");
    for implemented in [
        "dli.simulate.start",
        "dli.simulate.status",
        "dli.simulate.cancel",
    ] {
        assert!(
            capabilities
                .iter()
                .any(|capability| capability == implemented),
            "{implemented} must be advertised with its durable semantics"
        );
    }
}

#[test]
fn old_contract_mutation_is_refused_without_side_effects() {
    use dli_serve::CONTRACT_VERSION;

    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let opened = send(
        &serde_json::json!({
            "dli": CONTRACT_VERSION,
            "id": "open",
            "op": "author.open",
            "params": {"scope": "camelot"}
        }),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let initial_digest = opened.result.unwrap()["workspace_digest"].clone();

    let rejected = send(
        &serde_json::json!({
            "dli": "0.1.0",
            "id": "old-mutation",
            "op": "author.add_entity",
            "params": {
                "workspace": "ws-1",
                "kind": "person",
                "id": "intruder",
                "name": "Intruder",
                "provenance": {"kind": "proposed", "origin": "old-client"}
            }
        }),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(rejected.error.unwrap().code.wire_name(), "version-mismatch");

    let status = send(
        &serde_json::json!({
            "dli": CONTRACT_VERSION,
            "id": "status",
            "op": "author.status",
            "params": {"workspace": "ws-1"}
        }),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    assert_eq!(status["workspace_digest"], initial_digest);
    assert_eq!(status["rendered"], serde_json::json!([]));
}

#[test]
fn query_modifiers_layer_and_fork() {
    // H9: --layer filters by standing floor; --fork evaluates under a fork's
    // valuation overlay.
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    let all = send(
        &serde_json::json!({"dli":"0.5.1","id":"all","op":"dli.query","params":{"document":"round-table","goal":"adjacent(camelot, ?place)","layer":"certain"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap()
    .result
    .unwrap();
    let rows = all["rows"].as_array().expect("typed query rows");
    assert_eq!(rows.len(), 3, "the wire returns every matching row");
    assert!(
        rows.iter().all(|row| {
            row["bindings"]["place"].is_string()
                && row["status"] == "certain"
                && row["support"]["monomials"].is_array()
        }),
        "each row retains bindings and its verdict"
    );

    // kept-at is merely POSSIBLE in camelot (gated by the open hypothesis); the
    // certain layer drops that shadow row → empty answer, FALSE support.
    let certain = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.query","params":{"document":"camelot","goal":"kept-at(excalibur, camelot)","layer":"certain"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(certain.result.unwrap()["rows"], serde_json::json!([]));

    // Under a fork that supposes the gate, kept-at is certain — the --fork overlay
    // makes the certain-layer query return it.
    send(
        &serde_json::json!({"dli":"0.5.1","id":"2","op":"dli.fork.new","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    send(
        &serde_json::json!({"dli":"0.5.1","id":"3","op":"dli.fork.suppose","params":{"fork":"fork-1","assume":["h.camelot#0"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let forked = send(
        &serde_json::json!({"dli":"0.5.1","id":"4","op":"dli.query","params":{"document":"camelot","goal":"kept-at(excalibur, camelot)","layer":"certain","fork":"fork-1"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let forked = forked.result.unwrap();
    assert_eq!(forked["rows"].as_array().map(Vec::len), Some(1));
    assert_eq!(forked["rows"][0]["atom"], "kept-at(excalibur, camelot)");
    assert_eq!(forked["rows"][0]["status"], "certain");
    assert_eq!(
        forked["rows"][0]["support"],
        serde_json::json!({"monomials": [["+2"]]})
    );
}

#[test]
fn export_returns_the_target_shaped_emission() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let response = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"dli.export.run","params":{"document":"french-deck","profile":"cards"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let result = response.result.unwrap();
    assert_eq!(result["emission"]["profile"], "cards");
    assert_eq!(
        result["emission"]["artifact"]["profile"],
        "cards/french-deck@1"
    );
    assert!(
        result["emission"]["artifact"]["cards"]
            .as_array()
            .is_some_and(|cards| !cards.is_empty())
    );
    assert_eq!(result["manifest"]["layer"], "certain");
    let rules = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/games/french-deck/rules.json"),
    )
    .unwrap();
    assert_eq!(
        result["manifest"]["commit"],
        dli_export::source_fingerprint([rules.as_slice()])
    );
}

#[test]
fn export_returns_all_generic_lore_emissions() {
    for (profile, expected_field) in [
        ("context-pack", "text"),
        ("articy", "entities"),
        ("csv", "content"),
        ("static-wiki", "pages"),
    ] {
        let mut sessions = Sessions::new();
        let mut store = Store::default();
        let response = send(
            &serde_json::json!({
                "dli": "0.5.1",
                "id": profile,
                "op": "dli.export.run",
                "params": {"document": "camelot", "profile": profile}
            }),
            &mut sessions,
            &mut store,
        )
        .unwrap();
        let result = response.result.unwrap();
        assert_eq!(result["emission"]["profile"], profile);
        assert!(
            result["emission"]["artifact"][expected_field].is_array()
                || result["emission"]["artifact"][expected_field].is_string(),
            "{profile}: {result}"
        );
    }
}

#[test]
fn export_rejects_a_profile_for_the_wrong_target() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let response = send(
        &serde_json::json!({
            "dli": "0.5.1",
            "id": "wrong",
            "op": "dli.export.run",
            "params": {
                "document": "camelot",
                "profile": "csv",
                "profile_name": "wiki/static@1"
            }
        }),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(response.error.unwrap().code, ErrorCode::SchemaViolation);
}

#[test]
fn author_reads_see_the_base_pack_not_just_staged() {
    // Item 1: the Door T store is built with_pack, so a read tool evaluates
    // base + staged — an agent that has staged nothing still sees the BASE facts
    // of the pack it opened, which a staged-only (with_base/default) store would
    // hide.
    let pack = dli_serve::session::builtin_pack("camelot").expect("built-in corpus");
    let mut store = Store::with_pack(pack).expect("camelot compiles");
    let mut sessions = Sessions::new();

    let open = send(
        &serde_json::json!({"dli":"0.5.1","id":"1","op":"author.open","params":{"scope":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(open.ok, "author.open over the camelot pack");
    let workspace = open.result.unwrap()["workspace"]
        .as_str()
        .unwrap()
        .to_owned();

    // Nothing staged; the ratified base fact must still be visible to the read.
    let query = send(
        &serde_json::json!({"dli":"0.5.1","id":"2","op":"author.query","params":{"workspace":workspace,"goal":"adjacent(camelot, the-lake)","layer":"certain"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(query.ok, "author.query answers over base + staged");
    let rows = query.result.unwrap()["rows"].as_array().unwrap().clone();
    assert!(
        rows.iter()
            .any(|row| row["atom"] == "adjacent(camelot, the-lake)"),
        "the base pack's fact is visible to the read tool, not just staged decls: {rows:?}"
    );
}
