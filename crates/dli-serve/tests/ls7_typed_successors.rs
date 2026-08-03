//! LS-7 typed successors: the additive-only proofs.
//!
//! The four text-on-wire remnants each gained an optional typed field beside
//! the string it replaces: `goal_atom`, `additions`, and a payload claim's
//! `atom_expr` at contract `0.6.3`, and `dli.check-draft`'s runtime-continuity
//! rows — `RuntimeFact::atom_expr`, `RuntimeChange::to_atom_expr` — at `0.6.4`.
//! The acceptance bar for that slice is *nothing else moved*, and it has two
//! halves:
//!
//! - **Byte-identity.** A value that does not carry the new field serializes to
//!   exactly the bytes it serialized to before, so every stored payload, every
//!   vendored fixture, and every prior-version response is untouched. Proven
//!   here at the type level; the whole-corpus proof is `fixture_replay`, where
//!   the 126 fixtures predating each bump replay byte-for-byte.
//! - **Version transparency.** Neither bump changed a response shape: the same
//!   request answered on every accepted version differs only in the echoed
//!   protocol tag (and, for `0.5.1`, the replay-context fields that version has
//!   always omitted).
//!
//! The field-level killed-form registry is dormant, which the `dli-serve` unit
//! tests prove directly; what is proven here is its consequence on the wire —
//! a string-form request is still answered, on every version.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use dli_serve::author::store::Store;
use dli_serve::contract::{ForkAddParams, QueryParams};
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_world::payload::{Claim, RuntimeChange, RuntimeFact};
use serde_json::{Value, json};

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let line = serde_json::to_string(frame).unwrap();
    let response = respond(&line, sessions, store).expect("a response for every request frame");
    serde_json::to_value(&response).unwrap()
}

fn fixture_project() -> (tempfile::TempDir, Sessions, Store) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/dli-worker/fixture-project");
    let temp = tempfile::tempdir().unwrap();
    copy_tree(&source, temp.path());
    let project = Arc::new(Project::load(temp.path()).unwrap());
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let store = Store::with_packs(packs).unwrap();
    let sessions = Sessions::from_project(Arc::clone(&project));
    (temp, sessions, store)
}

fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let to = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir(&to).unwrap();
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// A params value that omits the typed successor serializes without any trace
/// of it: no key, no null. This is what keeps a `0.5.1`/`0.6.x` request and
/// every payload already committed to the store byte-identical.
#[test]
fn omitted_typed_successors_leave_no_trace_in_the_bytes() {
    let query = QueryParams {
        document: "camelot".to_owned(),
        goal: "adjacent(camelot, the-lake)".to_owned(),
        goal_atom: None,
        layer: None,
        as_of: None,
        fork: None,
        why: false,
    };
    assert_eq!(
        serde_json::to_string(&query).unwrap(),
        r#"{"document":"camelot","goal":"adjacent(camelot, the-lake)","why":false}"#
    );

    let fork_add = ForkAddParams {
        fork: "fork-1".to_owned(),
        declarations: vec!["adjacent(camelot, avalon) proposed".to_owned()],
        additions: None,
    };
    assert_eq!(
        serde_json::to_string(&fork_add).unwrap(),
        r#"{"fork":"fork-1","declarations":["adjacent(camelot, avalon) proposed"]}"#
    );

    let claim = Claim {
        atom: "marker(\"control\")".to_owned(),
        atom_expr: None,
        quote: "the marker holds".to_owned(),
        line: 1,
    };
    assert_eq!(
        serde_json::to_string(&claim).unwrap(),
        r#"{"atom":"marker(\"control\")","quote":"the marker holds","line":1}"#
    );

    // 0.6.4: the runtime-continuity pair.
    let fact = RuntimeFact {
        predicate: "marker".to_owned(),
        subject: "baseline".to_owned(),
        value: "control".to_owned(),
        atom_expr: None,
    };
    assert_eq!(
        serde_json::to_string(&fact).unwrap(),
        r#"{"predicate":"marker","subject":"baseline","value":"control"}"#
    );

    let change = RuntimeChange {
        predicate: "marker".to_owned(),
        subject: "baseline".to_owned(),
        from_value: "draft".to_owned(),
        to_value: "control".to_owned(),
        to_atom_expr: None,
    };
    assert_eq!(
        serde_json::to_string(&change).unwrap(),
        r#"{"predicate":"marker","subject":"baseline","from_value":"draft","to_value":"control"}"#
    );
}

/// A pre-bump encoding still deserializes — the typed successor is optional in
/// both directions, so a vendored fixture or a stored payload written before the
/// field existed loads unchanged and round-trips to the same bytes.
#[test]
fn pre_typed_successor_encodings_round_trip_unchanged() {
    for encoded in [
        r#"{"document":"camelot","goal":"adjacent(camelot, the-lake)","why":false}"#,
        r#"{"document":"camelot","goal":"adjacent(camelot, ?where)","layer":"certain","why":true}"#,
    ] {
        let params: QueryParams = serde_json::from_str(encoded).unwrap();
        assert!(params.goal_atom.is_none());
        assert_eq!(serde_json::to_string(&params).unwrap(), encoded);
    }

    let encoded = r#"{"atom":"marker(\"control\")","quote":"the marker holds","line":1}"#;
    let claim: Claim = serde_json::from_str(encoded).unwrap();
    assert!(claim.atom_expr.is_none());
    assert_eq!(serde_json::to_string(&claim).unwrap(), encoded);

    let encoded = r#"{"predicate":"marker","subject":"baseline","value":"control"}"#;
    let fact: RuntimeFact = serde_json::from_str(encoded).unwrap();
    assert!(fact.atom_expr.is_none());
    assert_eq!(serde_json::to_string(&fact).unwrap(), encoded);

    let encoded =
        r#"{"predicate":"marker","subject":"baseline","from_value":"draft","to_value":"control"}"#;
    let change: RuntimeChange = serde_json::from_str(encoded).unwrap();
    assert!(change.to_atom_expr.is_none());
    assert_eq!(serde_json::to_string(&change).unwrap(), encoded);
}

/// Each typed-successor bump is a pure addition to the negotiation: the same
/// string-form request answered on every accepted version yields the same
/// response but for the echoed protocol tag. `0.5.1` is compared on its own
/// terms — it has always omitted the 0.6.0 replay context — so the assertion
/// there is that the rows themselves are unchanged.
#[test]
fn a_string_form_request_answers_the_same_on_every_accepted_version() {
    let (_temp, mut sessions, mut store) = fixture_project();
    let mut v2_results: Vec<Value> = Vec::new();
    for version in dli_serve::ACCEPTED_CONTRACT_VERSIONS {
        let response = drive(
            &json!({
                "dli": version,
                "id": "1",
                "op": "dli.query",
                "params": { "document": "saltmarket", "goal": "marker(baseline)" }
            }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(response["dli"], **version, "the tag is echoed: {response}");
        assert_eq!(response["ok"], Value::Bool(true), "{response}");
        assert_eq!(
            response["result"]["rows"][0]["atom"], "marker(baseline)",
            "every accepted version answers the same rows: {response}"
        );
        if dli_serve::speaks_v2(version) {
            v2_results.push(response["result"].clone());
        } else {
            assert!(
                response["result"].get("root_hash").is_none(),
                "0.5.1 has always omitted the replay context: {response}"
            );
        }
    }
    let first = v2_results.first().expect("a v2 version is accepted");
    for result in &v2_results {
        assert_eq!(
            result, first,
            "no typed-successor bump reshapes a v2 response"
        );
    }
}

/// The dormant field-level killed forms, seen from the wire: every registered
/// retired field is still accepted on every version this build speaks. A
/// refusal here would mean the registry had woken up early.
#[test]
fn retired_string_forms_are_still_accepted_on_every_version() {
    for version in dli_serve::ACCEPTED_CONTRACT_VERSIONS {
        let (_temp, mut sessions, mut store) = fixture_project();
        let query = drive(
            &json!({ "dli": version, "id": "q", "op": "dli.query",
                     "params": { "document": "saltmarket", "goal": "marker(baseline)" } }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(query["ok"], Value::Bool(true), "{version}: {query}");

        drive(
            &json!({ "dli": version, "id": "pre", "op": "dli.fork.new",
                     "params": { "document": "saltmarket" } }),
            &mut sessions,
            &mut store,
        );
        let fork_add = drive(
            &json!({ "dli": version, "id": "f", "op": "dli.fork.add",
                     "params": { "fork": "fork-1", "declarations": ["note(\"x\") proposed"] } }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(fork_add["ok"], Value::Bool(true), "{version}: {fork_add}");

        drive(
            &json!({ "dli": version, "id": "open", "op": "author.open",
                     "params": { "scope": "saltmarket" } }),
            &mut sessions,
            &mut store,
        );
        let author_query = drive(
            &json!({ "dli": version, "id": "aq", "op": "author.query",
                     "params": { "workspace": "ws-1", "goal": "marker(baseline)" } }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(
            author_query["ok"],
            Value::Bool(true),
            "{version}: {author_query}"
        );
    }
}
