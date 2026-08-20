//! Contract 1.13's atomic bulk decision recorder (GH #106).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};

const DLI: &str = "1.13.0";

fn req(id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: DLI.to_owned(),
        id: id.to_owned(),
        op,
        params,
        expected_digest: None,
        idempotency_key: None,
    }
}

fn result(response: dli_serve::Response) -> Value {
    assert!(response.ok, "request failed: {response:?}");
    response.result.expect("successful request has a result")
}

fn store_with_mara() -> (tempfile::TempDir, Store) {
    let root = tempfile::tempdir().expect("temporary store root");
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"record-decisions\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .expect("write project manifest");
    let (document, diagnostics) =
        dli_syntax::parse_document("scope demo\nperson mara \"Mara\" proposed seed\n");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error())
    );
    let pack = Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    };
    let mut store = Store::with_pack(pack).expect("base compiles");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .expect("attach lorestore");
    (root, store)
}

fn open(store: &mut Store) -> String {
    result(dispatch(
        &req("open", Op::AuthorOpen, json!({ "scope": "demo" })),
        store,
    ))["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned()
}

fn bulk(store: &mut Store, ws: &str, records: Value) -> Value {
    result(dispatch(
        &req(
            "bulk",
            Op::AuthorRecordDecisions,
            json!({ "workspace": ws, "records": records }),
        ),
        store,
    ))
}

fn head(store: &Store, ws: &str) -> dli_lorestore::ContentHash {
    store
        .workspace_lorestore(ws)
        .expect("workspace lorestore")
        .root_hash()
        .expect("read root")
        .expect("root exists")
}

#[test]
fn forward_and_cyclic_same_batch_citations_are_validated_as_a_whole() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let value = bulk(
        &mut store,
        &ws,
        json!([
            {"decision":"d-1", "provenance":{
                "kind":"cited", "decisions":["d-2"], "state":"ratified"}},
            {"decision":"d-2", "provenance":{
                "kind":"cited", "decisions":["d-1"], "state":"ratified"}}
        ]),
    );
    assert_eq!(value["findings"], json!([]));
    assert_eq!(value["requested"], 2);
    assert_eq!(value["already_known"], 0);
    assert_eq!(value["recorded"], 2);
    let decisions = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .decisions_at(&head(&store, &ws))
        .expect("decisions");
    assert!(decisions.ends_with(&["d-1".to_owned(), "d-2".to_owned()]));
}

#[test]
fn duplicate_and_unknown_citation_findings_are_atomic_and_identify_records() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let before = head(&store, &ws);
    let value = bulk(
        &mut store,
        &ws,
        json!([
            {"decision":"d-dup"},
            {"decision":"d-dup"},
            {"decision":"d-external", "provenance":{
                "kind":"cited", "decisions":["missing"], "state":"ratified"}}
        ]),
    );
    assert_eq!(value["recorded"], 0);
    let findings = value["findings"].as_array().expect("findings");
    assert!(findings.iter().any(|finding| {
        finding["code"] == "decision.duplicate-in-batch"
            && finding["data"]["record_index"] == 1
            && finding["data"]["first_index"] == 0
    }));
    assert!(findings.iter().any(|finding| {
        finding["code"] == "citation.unknown-decision"
            && finding["data"]["record_index"] == 2
            && finding["data"]["decision"] == "d-external"
    }));
    assert_eq!(head(&store, &ws), before);
    assert_eq!(store.base_update_stats().prepared_compiles, 0);
    assert_eq!(store.base_update_stats().prepared_installs, 0);
}

#[test]
fn mixed_known_and_new_records_preserve_order_and_all_known_is_zero_write() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    bulk(&mut store, &ws, json!([{"decision":"known"}]));
    let value = bulk(
        &mut store,
        &ws,
        json!([
            {"decision":"new-a"},
            {"decision":"known"},
            {"decision":"new-b"}
        ]),
    );
    assert_eq!(value["requested"], 3);
    assert_eq!(value["already_known"], 1);
    assert_eq!(value["recorded"], 2);
    let after_mixed = head(&store, &ws);
    let decisions = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .decisions_at(&after_mixed)
        .expect("decisions");
    assert!(decisions.ends_with(&["known".to_owned(), "new-a".to_owned(), "new-b".to_owned()]));

    let stats = store.base_update_stats();
    let all_known = bulk(
        &mut store,
        &ws,
        json!([
            {"decision":"new-b"},
            {"decision":"known"},
            {"decision":"new-a"}
        ]),
    );
    assert_eq!(all_known["already_known"], 3);
    assert_eq!(all_known["recorded"], 0);
    assert_eq!(head(&store, &ws), after_mixed);
    assert_eq!(store.base_update_stats(), stats);
}

#[test]
fn one_thousand_twenty_four_records_use_one_commit_and_one_prepared_compile() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let before = head(&store, &ws);
    let before_depth = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .walk_dag(&before, None)
        .expect("walk before")
        .len();
    let records = (0..1024)
        .map(|index| json!({"decision": format!("d-{index:04}")}))
        .collect::<Vec<_>>();
    let value = bulk(&mut store, &ws, Value::Array(records));
    assert_eq!(value["recorded"], 1024);
    let after = head(&store, &ws);
    let after_depth = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .walk_dag(&after, None)
        .expect("walk after")
        .len();
    assert_eq!(after_depth, before_depth + 1);
    let stats = store.base_update_stats();
    assert_eq!(stats.prepared_compiles, 1);
    assert_eq!(stats.prepared_installs, 1);
    assert_eq!(stats.reloads, 0);
}

#[test]
fn older_contract_rejects_bulk_without_moving_the_single_operation() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let mut request = req(
        "old",
        Op::AuthorRecordDecisions,
        json!({"workspace":ws, "records":[{"decision":"d-1"}]}),
    );
    request.dli = "1.12.0".to_owned();
    let line = serde_json::to_string(&request).expect("request json");
    let response = dli_serve::serve::respond(
        &line,
        &mut dli_serve::session::Sessions::default(),
        &mut store,
    )
    .expect("response");
    assert!(!response.ok, "pre-floor bulk op is refused");

    let mut single = req(
        "single",
        Op::AuthorRecordDecision,
        json!({"workspace":ws, "decision":"d-1"}),
    );
    single.dli = "1.12.0".to_owned();
    assert!(
        dispatch(&single, &mut store).ok,
        "single operation is unchanged"
    );
}

#[test]
fn branch_routing_and_idempotent_retry_preserve_isolation() {
    let (_root, mut store) = store_with_mara();
    let main_before = store
        .lorestore_for("demo")
        .expect("demo lorestore")
        .root_hash()
        .expect("main head")
        .expect("main exists");
    store
        .lorestore_for("demo")
        .expect("demo lorestore")
        .create_branch("side", "main")
        .expect("create side branch");
    let ws = result(dispatch(
        &req(
            "open-side",
            Op::AuthorOpen,
            json!({"scope":"demo", "branch":"side"}),
        ),
        &mut store,
    ))["workspace"]
        .as_str()
        .expect("workspace")
        .to_owned();
    let mut request = req(
        "bulk-side",
        Op::AuthorRecordDecisions,
        json!({"workspace":ws, "records":[{"decision":"d-side"}]}),
    );
    request.idempotency_key = Some("bulk-side-key".to_owned());
    let first = dispatch(&request, &mut store);
    assert!(first.ok, "first batch succeeds: {first:?}");
    let side_after = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .branch_ref("side")
        .expect("side ref")
        .expect("side exists")
        .commit;
    let replay = dispatch(&request, &mut store);
    assert_eq!(replay, first, "same idempotency key replays exact response");
    assert_eq!(
        store
            .workspace_lorestore(&ws)
            .expect("lorestore")
            .branch_ref("side")
            .expect("side ref")
            .expect("side exists")
            .commit,
        side_after
    );
    assert_eq!(
        store
            .lorestore_for("demo")
            .expect("demo lorestore")
            .root_hash()
            .expect("main head")
            .expect("main exists"),
        main_before,
        "side batch never advances main"
    );
}

/// Release-only scale evidence; correctness is pinned by the 1,024-record test.
#[test]
#[ignore = "release scale harness"]
fn sixteen_thousand_three_hundred_eighty_four_records_still_use_one_commit() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let before = head(&store, &ws);
    let records = (0..16_384)
        .map(|index| json!({"decision": format!("d-{index:05}")}))
        .collect::<Vec<_>>();
    let value = bulk(&mut store, &ws, Value::Array(records));
    assert_eq!(value["recorded"], 16_384);
    let after = head(&store, &ws);
    assert_eq!(
        store
            .workspace_lorestore(&ws)
            .expect("lorestore")
            .walk_dag(&after, Some(&before))
            .expect("walk delta")
            .len(),
        2
    );
    assert_eq!(store.base_update_stats().prepared_compiles, 1);
    assert_eq!(store.base_update_stats().reloads, 0);
}
