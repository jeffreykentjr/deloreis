//! CON-1 cold-replay oracle (R3/H5): every registered fixture is driven COLD
//! through the real `respond` path — from a fresh session, with its declared
//! `precondition` frames run first (setup, not asserted) — and its declared
//! `expected_outcome` is asserted. A fixture that pins a verdict the running
//! engine does not produce fails HERE, so the vendored manifest cannot silently
//! document a fiction (the gap R3 found: `serve_owned` skipped the gated arms and
//! the author fixtures were never replayed).
//!
//! Outcomes: `replay` (default) asserts each frame's response equals its `expect`
//! (goldens/framing) or its `expect_error` code (negatives); `gated` asserts a
//! capability-gated op answers `not-wired`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use dli_serve::author::store::Store;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_world::{KilledTokens, Pack};
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/dli-worker/fixtures")
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

fn fixture_author_store() -> Store {
    let source = "\
scope saltmarket
fork exploration
  option baseline
fork ep9-cast
  option baseline
";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "the strict fixture corpus parses: {diagnostics:?}"
    );
    Store::with_packs([(
        "saltmarket".to_owned(),
        Pack {
            name: "saltmarket".to_owned(),
            documents: vec![document],
        },
        KilledTokens::default(),
    )])
    .expect("the strict fixture corpus compiles")
}

#[test]
fn every_fixture_replays_cold() {
    let mut replayed = 0;
    for sub in ["golden", "negative", "framing"] {
        let dir = fixtures_dir().join(sub);
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect();
        files.sort();
        for path in files {
            let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
            let fixture: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

            // A fresh session per fixture: handles (ws-1, fork-1) are deterministic.
            let mut sessions = Sessions::new();
            let mut store = fixture_author_store();

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
                let send = fixture
                    .get("steps")
                    .and_then(|steps| steps.get(0))
                    .and_then(|step| step.get("send"))
                    .or_else(|| fixture.get("send"))
                    .unwrap();
                let response = drive(send, &mut sessions, &mut store);
                assert_eq!(
                    error_code(&response),
                    Some("not-wired"),
                    "{name}: a capability-gated op must answer not-wired, got {response}"
                );
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
    }
    assert!(
        replayed >= 40,
        "expected the full registered fixture set to replay, got {replayed}"
    );
}
