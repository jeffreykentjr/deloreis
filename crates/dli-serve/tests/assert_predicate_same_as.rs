#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Contract 1.12.0's predicate identity wire lane. The declaration is typed
//! carriage only: it stages no `.dli` line, but reaches the compiled world,
//! FD checking, structured support, and the existing defeat machinery.

use serde_json::{Value, json};

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;

fn req(dli: &str, id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: dli.to_owned(),
        id: id.to_owned(),
        op,
        params,
        expected_digest: None,
        idempotency_key: None,
    }
}

fn result(response: &dli_serve::Response) -> &Value {
    assert!(response.ok, "expected ok, got error: {:?}", response.error);
    response.result.as_ref().expect("successful result")
}

fn cited(decision: &str) -> Value {
    json!({"kind": "cited", "decisions": [decision], "state": "ratified"})
}

fn proposed(origin: &str) -> Value {
    json!({"kind": "proposed", "origin": origin})
}

fn base() -> Store {
    let source = concat!(
        "scope tpth\n",
        "ratify D-1\n",
        "ratify D-3\n",
        "person mara \"Mara\" ratified D-1\n",
        "functional current-work ratified D-1\n",
        "current-work(mara, software) ratified D-1\n",
        "current-job(mara, employment) ratified D-1\n",
    );
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "base parses: {diagnostics:?}");
    Store::with_pack(Pack {
        name: "tpth".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles")
}

fn base_surface_only() -> Store {
    let source = concat!(
        "scope tpth\n",
        "ratify D-1\n",
        "ratify D-3\n",
        "person mara \"Mara\" ratified D-1\n",
        "current-work(mara, software) ratified D-1\n",
    );
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.is_empty(),
        "surface base parses: {diagnostics:?}"
    );
    Store::with_pack(Pack {
        name: "tpth".to_owned(),
        documents: vec![document],
    })
    .expect("surface base compiles")
}

fn open(store: &mut Store) -> String {
    let response = dispatch(
        &req("1.12.0", "open", Op::AuthorOpen, json!({"scope": "tpth"})),
        store,
    );
    result(&response)["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned()
}

fn assert_predicate(store: &mut Store, workspace: &str, provenance: Value) -> dli_serve::Response {
    dispatch(
        &req(
            "1.12.0",
            "predicate",
            Op::AuthorAssertPredicateSameAs,
            json!({
                "workspace": workspace,
                "left": "current-work",
                "right": "current-job",
                "provenance": provenance,
            }),
        ),
        store,
    )
}

fn check(store: &mut Store, workspace: &str) -> dli_serve::Response {
    dispatch(
        &req(
            "1.12.0",
            "check",
            Op::AuthorCheck,
            json!({"workspace": workspace}),
        ),
        store,
    )
}

fn codes(value: &Value) -> Vec<String> {
    value["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["code"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn wire_only_predicate_identity_reaches_fd_check_and_has_no_dli_rendering() {
    let mut store = base();
    let workspace = open(&mut store);
    let staged = assert_predicate(&mut store, &workspace, cited("D-3"));
    let staged_result = result(&staged);
    assert!(
        staged_result["rendered"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert_eq!(staged_result["findings"], json!([]));

    let status = dispatch(
        &req(
            "1.12.0",
            "status",
            Op::AuthorStatus,
            json!({"workspace": workspace}),
        ),
        &mut store,
    );
    let row = result(&status)["staged"]
        .as_array()
        .expect("staged rows")
        .iter()
        .find(|row| row["kind"] == "predicate-same-as")
        .expect("predicate identity structured row");
    assert_eq!(row["rendered"], "");

    let checked = check(&mut store, &workspace);
    let checked_result = result(&checked);
    assert!(codes(checked_result).contains(&"continuity.fd-violation".to_owned()));
    let finding = checked_result["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "continuity.fd-violation")
        .unwrap();
    let identity = finding["data"]["predicate_identity"]
        .as_array()
        .expect("structured identity support");
    assert_eq!(identity[0]["left"], "current-job");
    assert_eq!(identity[0]["right"], "current-work");
}

#[test]
fn proposed_identity_is_previewable_but_does_not_create_a_certain_conflict() {
    let mut store = base();
    let workspace = open(&mut store);
    let staged = assert_predicate(&mut store, &workspace, proposed("mention-1"));
    assert!(result(&staged)["findings"].as_array().unwrap().is_empty());
    let checked = check(&mut store, &workspace);
    assert!(!codes(result(&checked)).contains(&"continuity.fd-violation".to_owned()));
}

#[test]
fn predicate_identity_guard_census_covers_text_annotation_and_decl_kind() {
    let mut store = base();
    let workspace = open(&mut store);

    let control_char = dispatch(
        &req(
            "1.12.0",
            "control",
            Op::AuthorAssertPredicateSameAs,
            json!({
                "workspace": workspace,
                "left": "current\nwork",
                "right": "current-job",
                "provenance": proposed("control")
            }),
        ),
        &mut store,
    );
    assert!(codes(result(&control_char)).contains(&"field.control-char".to_owned()));

    let unknown_citation = assert_predicate(&mut store, &workspace, cited("D-unknown"));
    assert!(codes(result(&unknown_citation)).contains(&"citation.unknown-decision".to_owned()));

    let status = dispatch(
        &req(
            "1.12.0",
            "status",
            Op::AuthorStatus,
            json!({"workspace": workspace}),
        ),
        &mut store,
    );
    assert!(
        result(&status)["staged"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "predicate-same-as")
    );
}

#[test]
fn durable_defeat_unmerges_predicates_and_keeps_the_source_fact() {
    let root = tempfile::tempdir().expect("temporary store root");
    let mut store = base_surface_only();
    store
        .attach_lorestore(root.path().join("tpth"))
        .expect("attach lorestore");
    let workspace = open(&mut store);
    let staged = assert_predicate(&mut store, &workspace, cited("D-3"));
    assert!(result(&staged)["findings"].as_array().unwrap().is_empty());

    let materialized = dispatch(
        &req(
            "1.12.0",
            "materialize",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false}),
        ),
        &mut store,
    );
    let facts = result(&materialized)["facts"]
        .as_array()
        .expect("fact objects");
    let identity = facts
        .iter()
        .find(|fact| fact["expert_body"]["name"] == "current-work=current-job")
        .unwrap_or_else(|| panic!("stored predicate identity carriage: {facts:?}"));
    let claim_id = identity["claim_id"].as_str().expect("claim id").to_owned();

    let before = dispatch(
        &req(
            "1.12.0",
            "before",
            Op::AuthorQuery,
            json!({"workspace": workspace, "goal": "current-job(mara, software)", "layer": "all"}),
        ),
        &mut store,
    );
    assert!(!result(&before)["rows"].as_array().unwrap().is_empty());

    let defeated = dispatch(
        &req(
            "1.12.0",
            "defeat",
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": claim_id,
                "decision": "D-2",
                "defeated_decision": "D-3"
            }),
        ),
        &mut store,
    );
    let defeated_result = result(&defeated);
    assert!(defeated_result["findings"].as_array().unwrap().is_empty());

    let after = dispatch(
        &req(
            "1.12.0",
            "after",
            Op::AuthorQuery,
            json!({"workspace": workspace, "goal": "current-job(mara, software)", "layer": "all"}),
        ),
        &mut store,
    );
    assert!(result(&after)["rows"].as_array().unwrap().is_empty());
    let source = dispatch(
        &req(
            "1.12.0",
            "source",
            Op::AuthorQuery,
            json!({"workspace": workspace, "goal": "current-work(mara, software)", "layer": "all"}),
        ),
        &mut store,
    );
    assert!(!result(&source)["rows"].as_array().unwrap().is_empty());
}

#[test]
fn operation_is_unknown_before_1_12_and_advertised_at_the_floor() {
    let mut sessions = Sessions::new();
    let mut store = base();
    let request = |dli: &str| {
        serde_json::to_string(&json!({
            "dli": dli,
            "id": "version",
            "op": "dli.version",
            "params": {}
        }))
        .expect("version request")
    };
    let old = serde_json::to_value(
        respond(&request("1.11.0"), &mut sessions, &mut store).expect("response"),
    )
    .expect("old response");
    let new = serde_json::to_value(
        respond(&request("1.12.0"), &mut sessions, &mut store).expect("response"),
    )
    .expect("new response");
    assert!(
        !old["result"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op == "author.assert_predicate_same_as")
    );
    assert!(
        new["result"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op == "author.assert_predicate_same_as")
    );

    let old_op = respond(
        &serde_json::to_string(&json!({
            "dli": "1.11.0",
            "id": "old-op",
            "op": "author.assert_predicate_same_as",
            "params": {"workspace": "ws-1", "left": "a", "right": "b", "provenance": proposed("x")}
        }))
        .unwrap(),
        &mut sessions,
        &mut store,
    )
    .expect("old operation response");
    assert_eq!(
        old_op.error.expect("unknown operation").code,
        dli_serve::ErrorCode::UnknownOp
    );
}
