//! SERVE-1 MCP adapter: a scripted JSON-RPC session over the shared dispatch.
//!
//! The MCP transport is a thin wrapper: `tools/call` forwards the payload through
//! the same `respond` path the stdio worker uses, so a batch op and an author
//! tool both round-trip identically to their native surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_serve::author::store::Store;
use dli_serve::mcp::{ToolProfile, handle_rpc, handle_rpc_with_profile};
use dli_serve::session::Sessions;
use serde_json::Value;

fn rpc(line: &str, sessions: &mut Sessions, author: &mut Store) -> Value {
    let reply = handle_rpc(line, sessions, author).expect("a reply for a request");
    serde_json::from_str(&reply).unwrap()
}

#[test]
fn mcp_session_registers_tools_and_forwards_calls() {
    let mut sessions = Sessions::new();
    let mut author = Store::default();

    // initialize handshake
    let init = rpc(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &mut sessions,
        &mut author,
    );
    assert_eq!(init["jsonrpc"], "2.0");
    assert_eq!(init["result"]["serverInfo"]["name"], "deloreis");
    assert!(init["result"]["capabilities"]["tools"].is_object());

    // the `initialized` notification carries no id and gets no reply
    assert!(
        handle_rpc(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &mut sessions,
            &mut author,
        )
        .is_none()
    );

    // tools/list registers every op — a batch op and an author tool, each with a
    // request schema
    let list = rpc(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        &mut sessions,
        &mut author,
    );
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"dli.check"), "batch op registered");
    assert!(names.contains(&"author.open"), "author tool registered");
    assert!(
        tools.iter().all(|t| t["inputSchema"].is_object()),
        "every tool carries an input schema"
    );
    let add_entity = tools
        .iter()
        .find(|tool| tool["name"] == "author.add_entity")
        .expect("mutating author tool");
    assert!(
        add_entity["inputSchema"]["properties"]["expected_digest"].is_object(),
        "MCP exposes the digest fence as a standard tool argument"
    );
    assert!(
        add_entity["inputSchema"]["properties"]["idempotency_key"].is_object(),
        "MCP exposes the retry key as a standard tool argument"
    );

    // tools/call a batch op — identical kernel path to the stdio worker
    let call = rpc(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"dli.check","arguments":{"document":"camelot"}}}"#,
        &mut sessions,
        &mut author,
    );
    assert_eq!(call["result"]["isError"], false);
    let response: Value =
        serde_json::from_str(call["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(response["ok"], true);
    assert_eq!(
        response["result"],
        serde_json::json!({"ok": true, "grade": "checked", "findings": []})
    );

    // tools/call an author.* tool — routes through the authoring seam
    let author_call = rpc(
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"author.open","arguments":{"scope":"camelot"}}}"#,
        &mut sessions,
        &mut author,
    );
    assert_eq!(author_call["result"]["isError"], false);
    let author_response: Value = serde_json::from_str(
        author_call["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(author_response["ok"], true);
    assert!(author_response["result"]["workspace"].is_string());

    let stale = rpc(
        r#"{"jsonrpc":"2.0","id":41,"method":"tools/call","params":{"name":"author.add_entity","arguments":{"workspace":"ws-1","kind":"person","id":"sif","name":"Sif","provenance":{"kind":"proposed","origin":"probe"},"expected_digest":"jcs:sha256:deadbeef","idempotency_key":"entity-sif"}}}"#,
        &mut sessions,
        &mut author,
    );
    assert_eq!(stale["result"]["isError"], true);
    let stale_response: Value =
        serde_json::from_str(stale["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(stale_response["error"]["code"], "stale-fence");

    let digest = author_response["result"]["workspace_digest"]
        .as_str()
        .expect("open digest");
    let add_line = format!(
        r#"{{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{{"name":"author.add_entity","arguments":{{"workspace":"ws-1","kind":"person","id":"sif","name":"Sif","provenance":{{"kind":"proposed","origin":"probe"}},"expected_digest":"{digest}","idempotency_key":"entity-sif"}}}}}}"#
    );
    let first = rpc(&add_line, &mut sessions, &mut author);
    let replay = rpc(&add_line, &mut sessions, &mut author);
    assert_eq!(first["result"]["content"], replay["result"]["content"]);

    // an unknown tool routes to unknown-op, surfaced as isError (not a transport error)
    let unknown_tool = rpc(
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"dli.frobnicate","arguments":{}}}"#,
        &mut sessions,
        &mut author,
    );
    assert_eq!(unknown_tool["result"]["isError"], true);

    // an unknown JSON-RPC method is a method-not-found transport error
    let unknown_method = rpc(
        r#"{"jsonrpc":"2.0","id":6,"method":"nope","params":{}}"#,
        &mut sessions,
        &mut author,
    );
    assert_eq!(unknown_method["error"]["code"], -32601);
}

#[test]
fn mcp_only_lifts_fences_for_author_mutations() {
    let mut sessions = Sessions::new();
    let mut author = Store::default();
    let fence_on_read = rpc(
        r#"{"jsonrpc":"2.0","id":43,"method":"tools/call","params":{"name":"author.status","arguments":{"workspace":"ws-1","expected_digest":"jcs:sha256:deadbeef"}}}"#,
        &mut sessions,
        &mut author,
    );

    assert_eq!(fence_on_read["result"]["isError"], true);
    let response: Value = serde_json::from_str(
        fence_on_read["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        response["error"]["code"], "schema-violation",
        "shared dispatch refuses fence fields on read-only tools"
    );

    let fence_on_batch = rpc(
        r#"{"jsonrpc":"2.0","id":44,"method":"tools/call","params":{"name":"dli.check","arguments":{"document":"camelot","idempotency_key":"not-a-mutation"}}}"#,
        &mut sessions,
        &mut author,
    );
    let response: Value = serde_json::from_str(
        fence_on_batch["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["error"]["code"], "schema-violation");
}

#[test]
fn discovery_profiles_advertise_core_or_all() {
    let mut sessions = Sessions::new();
    let mut author = Store::default();
    let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let core =
        handle_rpc_with_profile(request, &mut sessions, &mut author, ToolProfile::Core).unwrap();
    let all =
        handle_rpc_with_profile(request, &mut sessions, &mut author, ToolProfile::All).unwrap();
    let core_tools: Value = serde_json::from_str(&core).unwrap();
    let all_tools: Value = serde_json::from_str(&all).unwrap();
    let core_tools = core_tools["result"]["tools"].as_array().unwrap();
    let all_tools = all_tools["result"]["tools"].as_array().unwrap();
    assert!(
        core.len() < 60_000,
        "core tool list was {} bytes",
        core.len()
    );
    assert!(core_tools.len() < all_tools.len());
    assert_eq!(all_tools.len(), dli_serve::ops::Op::ALL.len());
    for name in [
        "author.query",
        "author.add_relation",
        "author.findings",
        "author.predicates",
        "author.add_state",
        "author.as_of",
        "author.add_hypothesis",
        "author.suppose",
    ] {
        assert!(
            core_tools.iter().any(|tool| tool["name"] == name),
            "missing {name}"
        );
    }
    assert!(
        !core_tools
            .iter()
            .any(|tool| tool["name"] == "dli.export.run")
    );
    assert!(
        all_tools
            .iter()
            .any(|tool| tool["name"] == "dli.export.run")
    );
}
