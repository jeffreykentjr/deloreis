//! Corrective wire acceptance for GH #113: materialize is a fenced,
//! idempotent mutation from contract 0.6.0 onward.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use dli_serve::author::store::Store;
use dli_serve::mcp::handle_rpc;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_world::pack::Pack;
use serde_json::{Value, json};

const CURRENT: &str = "2.0.0";

fn fixture() -> (tempfile::TempDir, Sessions, Store) {
    let root = tempfile::tempdir().expect("temporary project");
    let (document, diagnostics) =
        dli_syntax::parse_document("scope demo\nperson nara \"Nara\" proposed seed\n");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "fixture parses: {diagnostics:?}"
    );
    let mut store = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .expect("fixture compiles");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .expect("lorestore attaches");
    (root, Sessions::new(), store)
}

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let response = respond(&frame.to_string(), sessions, store).expect("request is answered");
    serde_json::to_value(response).expect("response serializes")
}

fn rpc(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let response = handle_rpc(&frame.to_string(), sessions, store).expect("RPC is answered");
    serde_json::from_str(&response).expect("RPC response is JSON")
}

fn open(branch: Option<&str>, sessions: &mut Sessions, store: &mut Store) -> (String, String) {
    let mut params = json!({"scope": "demo"});
    if let Some(branch) = branch {
        params["branch"] = json!(branch);
    }
    let opened = drive(
        &json!({"dli": CURRENT, "id": "open", "op": "author.open", "params": params}),
        sessions,
        store,
    );
    assert_eq!(opened["ok"], true, "workspace opens: {opened}");
    (
        opened["result"]["workspace"].as_str().unwrap().to_owned(),
        opened["result"]["workspace_digest"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn add_fact(
    workspace: &str,
    expected_digest: &str,
    key: &str,
    value: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) -> String {
    let response = drive(
        &json!({
            "dli": CURRENT,
            "id": format!("add-{key}"),
            "op": "author.add_fact",
            "params": {
                "workspace": workspace,
                "entity": "nara",
                "key": "rank",
                "value": value,
                "provenance": {"kind": "proposed", "origin": key}
            },
            "expected_digest": expected_digest,
            "idempotency_key": key
        }),
        sessions,
        store,
    );
    assert_eq!(response["ok"], true, "fact stages: {response}");
    response["result"]["workspace_digest"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn ref_digest(store: &Store, workspace: &str, branch: &str) -> String {
    store
        .workspace_lorestore(workspace)
        .expect("workspace has a lorestore")
        .branch_ref(branch)
        .expect("ref read succeeds")
        .expect("ref exists")
        .digest()
        .expect("ref digest")
        .to_string()
}

fn materialize_frame(
    workspace: &str,
    expected_digest: &str,
    idempotency_key: &str,
    rendered: bool,
) -> Value {
    json!({
        "dli": CURRENT,
        "id": format!("materialize-{idempotency_key}"),
        "op": "author.materialize",
        "params": {"workspace": workspace, "rendered": rendered},
        "expected_digest": expected_digest,
        "idempotency_key": idempotency_key
    })
}

fn assert_query_parity(workspace: &str, target: &str, sessions: &mut Sessions, store: &mut Store) {
    let goal = json!({"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "silver"}]});
    let live = drive(
        &json!({
            "dli": CURRENT, "id": "query-live", "op": "author.query",
            "params": {"workspace": workspace, "goal_atom": goal, "layer": "all", "why": true}
        }),
        sessions,
        store,
    );
    let (fresh, _) = open(Some(target), sessions, store);
    let reopened = drive(
        &json!({
            "dli": CURRENT, "id": "query-fresh", "op": "author.query",
            "params": {"workspace": fresh, "goal_atom": goal, "layer": "all", "why": true}
        }),
        sessions,
        store,
    );
    assert_eq!(live["result"]["rows"], reopened["result"]["rows"]);
}

fn assert_native_replay_and_conflict(
    frame: &Value,
    first: &Value,
    workspace: &str,
    target: &str,
    head: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) {
    let replay = drive(frame, sessions, store);
    assert_eq!(
        replay, *first,
        "an exact retry replays the original response"
    );
    assert_eq!(ref_digest(store, workspace, target), head);

    let mut moved_fence_replay = frame.clone();
    moved_fence_replay["id"] = json!("materialize-replay-with-moved-fence");
    moved_fence_replay["expected_digest"] = json!("jcs:sha256:now-stale");
    let moved_fence_replay = drive(&moved_fence_replay, sessions, store);
    assert_eq!(moved_fence_replay["result"], first["result"]);
    assert_eq!(ref_digest(store, workspace, target), head);

    let conflict = drive(
        &materialize_frame(
            workspace,
            frame["expected_digest"].as_str().unwrap(),
            "accept-gold",
            true,
        ),
        sessions,
        store,
    );
    assert_eq!(conflict["error"]["code"], "conflict", "{conflict}");
    assert_eq!(ref_digest(store, workspace, target), head);
}

fn exercise_target(branch: Option<&str>) {
    let (_root, mut sessions, mut store) = fixture();
    if let Some(branch) = branch {
        let (seed, _) = open(None, &mut sessions, &mut store);
        let created = drive(
            &json!({
                "dli": CURRENT,
                "id": "create-branch",
                "op": "author.branch",
                "params": {"workspace": seed, "action": "create", "name": branch, "from": "main"}
            }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(created["ok"], true, "branch is created: {created}");
    }

    let target = branch.unwrap_or("main");
    let (workspace, opened_digest) = open(branch, &mut sessions, &mut store);
    let main_before = branch.map(|_| ref_digest(&store, &workspace, "main"));
    let staged_digest = add_fact(
        &workspace,
        &opened_digest,
        "stage-gold",
        "gold",
        &mut sessions,
        &mut store,
    );
    let before = ref_digest(&store, &workspace, target);
    let frame = materialize_frame(&workspace, &staged_digest, "accept-gold", false);
    let first = drive(&frame, &mut sessions, &mut store);
    assert_eq!(first["ok"], true, "fenced materialize succeeds: {first}");
    let after = ref_digest(&store, &workspace, target);
    assert_ne!(after, before, "the first materialize advances its target");
    assert!(first["result"]["workspace_digest"].is_string());

    assert_native_replay_and_conflict(
        &frame,
        &first,
        &workspace,
        target,
        &after,
        &mut sessions,
        &mut store,
    );

    let accepted_digest = first["result"]["workspace_digest"].as_str().unwrap();
    let moved_digest = add_fact(
        &workspace,
        accepted_digest,
        "stage-silver",
        "silver",
        &mut sessions,
        &mut store,
    );
    let stale = drive(
        &materialize_frame(&workspace, accepted_digest, "stale-silver", false),
        &mut sessions,
        &mut store,
    );
    assert_eq!(stale["error"]["code"], "stale-fence", "{stale}");
    assert_eq!(
        ref_digest(&store, &workspace, target),
        after,
        "a stale envelope fence advances no durable ref"
    );
    assert_eq!(
        store.get(&workspace).expect("workspace remains").digest,
        moved_digest,
        "a stale materialize does not change live workspace state"
    );

    let accepted = drive(
        &materialize_frame(&workspace, &moved_digest, "accept-silver", false),
        &mut sessions,
        &mut store,
    );
    assert_eq!(accepted["ok"], true, "current fence succeeds: {accepted}");

    assert_query_parity(&workspace, target, &mut sessions, &mut store);
    if let Some(main_before) = main_before {
        assert_eq!(
            ref_digest(&store, &workspace, "main"),
            main_before,
            "branch-scoped materialize never advances main"
        );
    }
}

fn materialize_rpc(
    id: u64,
    workspace: &str,
    rendered: bool,
    expected_digest: Option<&str>,
    key: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) -> (Value, Value) {
    let mut arguments = json!({
        "workspace": workspace,
        "rendered": rendered,
        "idempotency_key": key
    });
    if let Some(digest) = expected_digest {
        arguments["expected_digest"] = json!(digest);
    }
    let response = rpc(
        &json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": "author.materialize", "arguments": arguments}
        }),
        sessions,
        store,
    );
    let worker = serde_json::from_str(
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("worker response text"),
    )
    .expect("worker response is JSON");
    (response, worker)
}

#[test]
fn native_main_materialize_is_fenced_and_idempotent() {
    exercise_target(None);
}

#[test]
fn native_branch_materialize_is_fenced_and_idempotent() {
    exercise_target(Some("rival"));
}

#[test]
fn retired_contracts_refuse_before_materialize_fencing() {
    let (_root, mut sessions, mut store) = fixture();
    let (workspace, _) = open(None, &mut sessions, &mut store);
    for (version, expected) in [
        ("0.5.1", "killed-form"),
        ("0.6.0", "killed-form"),
        ("1.12.0", "killed-form"),
        ("2.0.0", "stale-fence"),
    ] {
        let response = drive(
            &json!({
                "dli": version,
                "id": format!("floor-{version}"),
                "op": "author.materialize",
                "params": {"workspace": workspace},
                "expected_digest": "jcs:sha256:stale",
                "idempotency_key": format!("floor-{version}")
            }),
            &mut sessions,
            &mut store,
        );
        assert_eq!(response["error"]["code"], expected, "{version}: {response}");
    }
}

#[test]
fn mcp_advertises_and_replays_fenced_materialize() {
    let (_root, mut sessions, mut store) = fixture();
    let tools = rpc(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
        &mut sessions,
        &mut store,
    );
    let materialize = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "author.materialize")
        .expect("materialize tool is registered");
    assert!(
        materialize["inputSchema"]["properties"]["expected_digest"].is_object(),
        "MCP advertises the digest fence"
    );
    assert!(
        materialize["inputSchema"]["properties"]["idempotency_key"].is_object(),
        "MCP advertises the idempotency key"
    );

    let (workspace, opened_digest) = open(None, &mut sessions, &mut store);
    let staged_digest = add_fact(
        &workspace,
        &opened_digest,
        "mcp-stage",
        "bronze",
        &mut sessions,
        &mut store,
    );
    let (first, worker) = materialize_rpc(
        2,
        &workspace,
        false,
        Some(&staged_digest),
        "mcp-materialize",
        &mut sessions,
        &mut store,
    );
    assert_eq!(first["result"]["isError"], false, "{first}");
    assert_eq!(worker["ok"], true, "{worker}");
    let head = ref_digest(&store, &workspace, "main");

    let (replay, _) = materialize_rpc(
        2,
        &workspace,
        false,
        Some(&staged_digest),
        "mcp-materialize",
        &mut sessions,
        &mut store,
    );
    assert_eq!(replay["result"]["content"], first["result"]["content"]);
    assert_eq!(ref_digest(&store, &workspace, "main"), head);

    let (_, moved_worker) = materialize_rpc(
        3,
        &workspace,
        false,
        Some("jcs:sha256:stale-after-success"),
        "mcp-materialize",
        &mut sessions,
        &mut store,
    );
    assert_eq!(moved_worker["result"], worker["result"]);

    let (_, conflict_worker) = materialize_rpc(
        4,
        &workspace,
        true,
        None,
        "mcp-materialize",
        &mut sessions,
        &mut store,
    );
    assert_eq!(conflict_worker["error"]["code"], "conflict");
    assert_eq!(ref_digest(&store, &workspace, "main"), head);

    let (_, stale_worker) = materialize_rpc(
        5,
        &workspace,
        false,
        Some("jcs:sha256:fresh-key-stale-fence"),
        "mcp-stale-materialize",
        &mut sessions,
        &mut store,
    );
    assert_eq!(stale_worker["error"]["code"], "stale-fence");
    assert_eq!(ref_digest(&store, &workspace, "main"), head);
}
