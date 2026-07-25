//! The MCP adapter (`SERVE-1`): the same worker surface, reached over MCP.
//!
//! The MCP transport is a **thin wrapper, not a second protocol** (DL-0014;
//! `docs/spec/10-serving.md` §1). It speaks JSON-RPC 2.0 over stdio, registers
//! every `dli.*` and `author.*` op as an MCP tool (`tools/list`), and forwards a
//! `tools/call` payload **unchanged** through the very same [`crate::serve::respond`]
//! path the stdio worker uses. An MCP client and a stdio client therefore
//! exercise identical kernel paths and identical fixtures: a `tools/call` builds
//! the ordinary wire frame `{dli, id, op, params}` and routes it through the
//! shared dispatch, so killed forms, unknown ops, schema errors, and capability
//! gating all behave exactly as they do on the native surface.

use serde_json::{Value, json};
use std::sync::Arc;

use crate::author::store::Store;
use crate::ops::Op;
use crate::project::Project;
use crate::schema::contract_schemas;
use crate::session::Sessions;

/// Run the MCP JSON-RPC loop over stdio: one JSON-RPC message per line in, one
/// reply per line out (notifications get no reply). The session cache and Door T
/// store live for the life of the loop, exactly as the native worker's do.
pub fn run_mcp_demo() -> std::io::Result<()> {
    run_mcp_with(None)
}

/// Run MCP over an explicitly loaded caller project.
pub fn run_mcp_project(project: Project) -> std::io::Result<()> {
    run_mcp_with(Some(Arc::new(project)))
}

fn run_mcp_with(project: Option<Arc<Project>>) -> std::io::Result<()> {
    use std::io::{BufRead, Write};

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut out = stdout.lock();
    let mut sessions = project.as_ref().map_or_else(Sessions::new, |project| {
        Sessions::from_project(Arc::clone(project))
    });
    // The read-complete Door T store (base + staged), same as the stdio worker.
    let mut author = match project {
        Some(project) => crate::serve::project_author_store(&project)?,
        None => crate::serve::demo_author_store()?,
    };
    let mut buf = Vec::new();

    loop {
        buf.clear();
        // Raw bytes + lossy decode: a hostile non-UTF-8 byte must never abort the
        // loop (a `lines()` iterator would error out — a denial of service).
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break; // EOF
        }
        let line = String::from_utf8_lossy(&buf);
        if let Some(reply) = handle_rpc(&line, &mut sessions, &mut author) {
            writeln!(out, "{reply}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// Handle one JSON-RPC message, returning its reply line — or `None` for a
/// notification (a message with no `id`), which by protocol gets no reply. This
/// is the per-message seam the MCP tests drive.
#[must_use]
pub fn handle_rpc(line: &str, sessions: &mut Sessions, author: &mut Store) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let message: Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(error) => {
            return Some(rpc_error(
                &Value::Null,
                -32700,
                &format!("parse error: {error}"),
            ));
        }
    };

    // A JSON-RPC notification carries no `id` and gets no reply.
    let id = message.get("id").cloned()?;
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));

    let outcome = match method {
        "initialize" => Ok(initialize_result()),
        "tools/list" => Ok(json!({ "tools": tool_descriptors() })),
        "tools/call" => tools_call(&params, sessions, author),
        "ping" => Ok(json!({})),
        other => Err((-32601, format!("method '{other}' is not supported"))),
    };

    Some(match outcome {
        Ok(value) => rpc_result(&id, &value),
        Err((code, message)) => rpc_error(&id, code, &message),
    })
}

/// The `initialize` result: protocol version, the tools capability, and server
/// identity.
fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "deloreis", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// Every op as an MCP tool descriptor: its wire name, a one-line description, and
/// its request JSON Schema (the same schema the contract fixtures validate).
fn tool_descriptors() -> Vec<Value> {
    let schemas: std::collections::BTreeMap<String, Value> = contract_schemas()
        .into_iter()
        .filter_map(|(key, schema)| serde_json::to_value(schema).ok().map(|value| (key, value)))
        .collect();
    Op::ALL
        .iter()
        .map(|op| {
            let wire = wire_name(*op);
            let input_schema = schemas
                .get(&format!("{wire}.request"))
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object" }));
            json!({
                "name": wire,
                "description": op.summary(),
                "inputSchema": input_schema,
            })
        })
        .collect()
}

/// A `tools/call`: forward the tool name and arguments through the shared wire
/// dispatch and wrap the worker response as MCP tool-result content. A worker
/// error (a `false`-`ok` response) surfaces as `isError`, not a JSON-RPC error —
/// the call itself succeeded; the lore verdict is the content.
fn tools_call(
    params: &Value,
    sessions: &mut Sessions,
    author: &mut Store,
) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "tools/call requires a string `name`".to_owned()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let frame = json!({ "dli": "0.1.0", "id": "mcp", "op": name, "params": arguments });
    let response = crate::serve::respond(&frame.to_string(), sessions, author)
        .ok_or_else(|| (-32603, "the worker produced no response".to_owned()))?;
    let text = serde_json::to_string(&response).map_err(|e| (-32603, e.to_string()))?;

    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": !response.ok,
    }))
}

/// The op's wire identifier (its serde encoding).
fn wire_name(op: Op) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A JSON-RPC success reply.
fn rpc_result(id: &Value, result: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

/// A JSON-RPC error reply.
fn rpc_error(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}
