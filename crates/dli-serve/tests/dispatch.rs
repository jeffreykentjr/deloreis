//! SERVE-1 dispatch tests: the dli-worker fixtures are the oracle.
//!
//! Every golden fixture whose ops SERVE-1 dispatches replays through the real
//! `respond` path (framing + routing + engine), threading session state across a
//! multi-step flow; every negative fixture yields its typed error code; the raw
//! frames exercise the framing discipline. Fixtures whose ops belong to another
//! lane (author.*, door-t) or whose engine has not landed (fork.add's declaration
//! parser, timeline.*, and the killed `check-ledger` form) are covered elsewhere
//! or are capability-gated, and are skipped here by ownership. check-proof,
//! export.run, and simulate.* are wired and replay here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use dli_serve::author::store::Store;
use dli_serve::envelope::Response;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/dli-worker/fixtures")
}

/// The ops whose golden fixture replays in v0. Excluded: author.* (door-t) and
/// the killed `check-ledger` form (covered by the killed-op test). Every `dli.*`
/// engine has now landed — fork.add's declaration parser and the `timeline.*`
/// chronology/episode reads were the last gated ops — so they replay here too.
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
        let ours = !steps.is_empty()
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
        &serde_json::json!({"dli":"0.1.0","id":"1","op":"dli.fork.new","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(new.result.unwrap()["fork"], "fork-1");

    let suppose = send(
        &serde_json::json!({"dli":"0.1.0","id":"2","op":"dli.fork.suppose","params":{"fork":"fork-1","assume":["h.camelot#0"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let delta = suppose.result.unwrap();
    assert_eq!(
        delta["newly_certain"],
        serde_json::json!(["kept-at(excalibur, camelot)"])
    );

    // The suppose persisted on the fork: a later diff sees the same consequence.
    let diff = send(
        &serde_json::json!({"dli":"0.1.0","id":"3","op":"dli.fork.diff","params":{"fork":"fork-1"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(
        diff.result.unwrap()["newly_certain"],
        serde_json::json!(["kept-at(excalibur, camelot)"])
    );
}

#[test]
fn unknown_document_is_unknown_doc() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let response = send(
        &serde_json::json!({"dli":"0.1.0","id":"1","op":"dli.check","params":{"document":"nowhere"}}),
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
            &serde_json::json!({"dli":"0.1.0","id":"1","op":op,"params":{}}),
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
fn timeline_and_fork_add_are_wired() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    // A corpus with no chronology reads the honest empty state, never `not-wired`.
    let bare = send(
        &serde_json::json!({"dli":"0.1.0","id":"1","op":"dli.timeline.show","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(bare.ok, "timeline.show is wired: {bare:?}");
    assert_eq!(
        bare.result.unwrap()["findings"],
        serde_json::json!(["no chronology in this corpus"])
    );

    // The neutral round-table world carries a real era spine, so show lays it out.
    let laid = send(
        &serde_json::json!({"dli":"0.1.0","id":"2","op":"dli.timeline.show","params":{"document":"round-table"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let findings = laid.result.unwrap()["findings"].clone();
    assert!(
        findings[0].as_str().unwrap().starts_with("layer 0:"),
        "chronology is laid out in layers: {findings}"
    );

    // fork.add parses a declaration line and adds its lowered fact to the fork.
    send(
        &serde_json::json!({"dli":"0.1.0","id":"3","op":"dli.fork.new","params":{"document":"round-table"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let added = send(
        &serde_json::json!({"dli":"0.1.0","id":"4","op":"dli.fork.add","params":{"fork":"fork-1","declarations":["adjacent(camelot, avalon) proposed"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(added.ok, "fork.add is wired: {added:?}");
    assert_eq!(
        added.result.unwrap()["delta"]["newly_certain"],
        serde_json::json!(["adjacent(camelot, avalon)"])
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
        &serde_json::json!({"dli":"0.1.0","id":"1","op":"dli.check-proof","params":{"document":"warding-vows"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert!(wired.ok, "check-proof is wired: {wired:?}");
    let report = &wired.result.unwrap()["report"];
    assert_eq!(report["checks"], serde_json::json!(true));
    assert_eq!(report["verdict"], serde_json::json!("holds"));

    let malformed = send(
        &serde_json::json!({"dli":"0.1.0","id":"2","op":"dli.check-proof","params":{}}),
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
    input.extend_from_slice(br#"{"dli":"0.1.0","id":"1","op":"dli.version","params":{}}"#);
    input.push(b'\n');
    input.push(0xff); // an invalid UTF-8 byte on its own line
    input.push(b'\n');
    input.extend_from_slice(br#"{"dli":"0.1.0","id":"2","op":"dli.version","params":{}}"#);
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
        &serde_json::json!({"dli":"0.1.0","id":"1","op":"dli.version","params":{},"zzz":1}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(stray.error.unwrap().code.wire_name(), "schema-violation");

    // A mistyped `expected_digest` (typo'd key) is a stray field, caught before
    // dispatch — otherwise it would silently defeat the stale-fence.
    let typo = send(
        &serde_json::json!({"dli":"0.1.0","id":"2","op":"author.materialize","params":{"workspace":"ws-1"},"expcted_digest":"jcs:x"}),
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
            "dli":"0.1.0", "id":"1", "op":"author.status",
            "params":{"workspace":"ws-1"}, "expected_digest":"jcs:x"
        }),
        serde_json::json!({
            "dli":"0.1.0", "id":"2", "op":"dli.check",
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
fn query_modifiers_layer_and_fork() {
    // H9: --layer filters by standing floor; --fork evaluates under a fork's
    // valuation overlay.
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    // kept-at is merely POSSIBLE in camelot (gated by the open hypothesis); the
    // certain layer drops that shadow row → empty answer, FALSE support.
    let certain = send(
        &serde_json::json!({"dli":"0.1.0","id":"1","op":"dli.query","params":{"document":"camelot","goal":"kept-at(excalibur, camelot)","layer":"certain"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(
        certain.result.unwrap()["support"],
        serde_json::json!({"monomials": []})
    );

    // Under a fork that supposes the gate, kept-at is certain — the --fork overlay
    // makes the certain-layer query return it.
    send(
        &serde_json::json!({"dli":"0.1.0","id":"2","op":"dli.fork.new","params":{"document":"camelot"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    send(
        &serde_json::json!({"dli":"0.1.0","id":"3","op":"dli.fork.suppose","params":{"fork":"fork-1","assume":["h.camelot#0"]}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    let forked = send(
        &serde_json::json!({"dli":"0.1.0","id":"4","op":"dli.query","params":{"document":"camelot","goal":"kept-at(excalibur, camelot)","layer":"certain","fork":"fork-1"}}),
        &mut sessions,
        &mut store,
    )
    .unwrap();
    assert_eq!(
        forked.result.unwrap()["support"],
        serde_json::json!({"monomials": [["+2"]]})
    );
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
        &serde_json::json!({"dli":"0.1.0","id":"1","op":"author.open","params":{"scope":"camelot"}}),
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
        &serde_json::json!({"dli":"0.1.0","id":"2","op":"author.query","params":{"workspace":workspace,"goal":"adjacent(camelot, the-lake)","layer":"certain"}}),
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
