#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 2.0 retirement refusals.

use dli_serve::author::store::Store;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use serde_json::json;

#[cfg(feature = "syntax")]
#[test]
fn batch_check_keeps_structured_continuity_details() {
    let source = "scope storybook\nratify d-1\nfunctional home ratified d-1\nfact wren home shore ratified d-1\nfact wren home hill ratified d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let world = dli_world::compile(&dli_world::pack::Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let mut sessions = Sessions::new();
    sessions.scope("camelot").unwrap().world = world;
    let mut store = Store::default();
    let frame =
        json!({"dli":"2.0.0", "id":"check", "op":"dli.check", "params":{"document":"camelot"}});
    let response = respond(&frame.to_string(), &mut sessions, &mut store).unwrap();
    let result = response.result.unwrap();
    let finding = result["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "continuity.fd-violation")
        .unwrap();
    assert_eq!(finding["data"]["pred"], "home");
    assert_eq!(finding["data"]["subject"], "wren");
}

#[test]
fn retired_versions_and_operations_name_the_successor() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    for version in ["0.5.1", "0.6.0", "0.6.4", "1.0.0", "1.13.0", "1.99.0"] {
        let frame = json!({"dli": version, "id": "test", "op": "dli.version", "params": {}});
        let response = respond(&frame.to_string(), &mut sessions, &mut store).unwrap();
        let error = response.error.unwrap();
        assert_eq!(error.code.wire_name(), "killed-form", "{version}");
        assert!(error.message.contains(version), "{}", error.message);
        assert!(error.message.contains("2.0.0"), "{}", error.message);
    }
    for op in [
        "dli.check-draft",
        "dli.simulate.start",
        "dli.simulate.status",
        "dli.simulate.cancel",
        "dli.attestation-sweep",
        "dli.payload-submit",
    ] {
        let frame = json!({"dli": "2.0.0", "id": "test", "op": op, "params": {}});
        let response = respond(&frame.to_string(), &mut sessions, &mut store).unwrap();
        let error = response.error.unwrap();
        assert_eq!(error.code.wire_name(), "killed-form", "{op}");
        assert!(error.message.contains(op), "{}", error.message);
        assert!(error.message.contains("2.0.0"), "{}", error.message);
    }
    let frame = json!({"dli": "1.13.0", "id": "test", "op": "dli.check-draft", "params": {}});
    let response = respond(&frame.to_string(), &mut sessions, &mut store).unwrap();
    let error = response.error.unwrap();
    assert_eq!(error.code.wire_name(), "killed-form");
    assert!(error.message.contains("1.13.0"));
    assert!(error.message.contains("2.0.0"));
}

#[test]
fn current_contract_advertises_authoring_capabilities() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    let frame = json!({"dli": "2.0.0", "id": "test", "op": "dli.version", "params": {}});
    let response = respond(&frame.to_string(), &mut sessions, &mut store).unwrap();
    let result = response.result.unwrap();
    let capabilities = result["capabilities"].as_array().unwrap();
    for op in [
        "author.add_rule",
        "author.assert_same_as",
        "author.assert_predicate_same_as",
        "author.as_of",
        "author.declare_inertial",
        "author.declare_functional",
        "author.entity_candidates",
        "author.predicates",
    ] {
        assert!(capabilities.contains(&json!(op)), "missing {op}");
    }
}

#[test]
fn retired_string_fields_name_their_typed_successors() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    for (op, params, successor) in [
        (
            "dli.query",
            json!({"document": "camelot", "goal": "foo(bar)"}),
            "goal_atom",
        ),
        (
            "author.query",
            json!({"workspace": "missing", "goal": "foo(bar)"}),
            "goal_atom",
        ),
        (
            "dli.fork.add",
            json!({"fork": "missing", "declarations": ["foo(bar) proposed"]}),
            "additions",
        ),
    ] {
        let frame = json!({"dli": "2.0.0", "id": "test", "op": op, "params": params});
        let response = respond(&frame.to_string(), &mut sessions, &mut store).unwrap();
        let error = response.error.unwrap();
        assert_eq!(error.code.wire_name(), "killed-form", "{op}");
        assert!(error.message.contains(successor), "{}", error.message);
    }
}

#[test]
fn retired_and_unknown_operations_still_require_a_well_formed_envelope() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();
    for frame in [
        r#"{"dli":"1.13.0","id":"x","op":"dli.check-draft","op":"dli.check-draft","params":{}}"#,
        r#"{"dli":"2.0.0","id":"x","op":"unknown","op":"unknown","params":{}}"#,
        r#"{"dli":"1.13.0","op":"dli.check-draft","params":{}}"#,
    ] {
        let response = respond(frame, &mut sessions, &mut store).unwrap();
        assert_eq!(
            response.error.unwrap().code.wire_name(),
            "bad-frame",
            "{frame}"
        );
        if frame.contains(r#""id":"x""#) {
            assert_eq!(response.id, "x", "a unique correlation id is echoed");
        }
    }
    let mistyped_fence =
        r#"{"dli":"2.0.0","id":"fence","op":"unknown","params":{},"expected_digest":1}"#;
    let response = respond(mistyped_fence, &mut sessions, &mut store).unwrap();
    assert_eq!(response.id, "fence");
    assert_eq!(response.error.unwrap().code.wire_name(), "bad-frame");
    let duplicate_id = r#"{"dli":"2.0.0","id":"a","id":"b","op":"unknown","params":{}}"#;
    let response = respond(duplicate_id, &mut sessions, &mut store).unwrap();
    assert_eq!(response.id, "", "ambiguous ids are not echoed");
    assert_eq!(response.error.unwrap().code.wire_name(), "bad-frame");
    let retired_with_extra =
        json!({"dli":"1.13.0","id":"x","op":"dli.version","params":{},"extra":true});
    let response = respond(&retired_with_extra.to_string(), &mut sessions, &mut store).unwrap();
    let error = response.error.unwrap();
    assert_eq!(error.code.wire_name(), "killed-form");
    assert!(error.message.contains("1.13.0") && error.message.contains("2.0.0"));
}
