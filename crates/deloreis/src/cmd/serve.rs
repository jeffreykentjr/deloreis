//! `deloreis serve` / `deloreis mcp` — delegate to the worker's two transports.
//!
//! The worker lives in `dli-serve` (the SERVE-1 milestone, owned by the server
//! lane) and exposes Door T over **both** transports the contract promises
//! (`10-serving.md` §1, `11-agent-authoring.md` §1): [`run`] is the JSON-over-
//! stdio worker loop (`dli_serve::serve::serve`), and [`run_mcp`] is the MCP
//! JSON-RPC tool transport over stdio (`dli_serve::mcp::run_mcp`). Both are thin
//! delegations that map the worker's `io::Result` to an exit code (clean EOF →
//! `0`, an I/O failure → usage). The MCP transport is a wrapper over the same
//! ops, not a second protocol, so both reach identical kernel paths.
//!
//! The loop never panics on any traffic. All 19 `author.*` tools are fully
//! implemented (`author::dispatch` routes through Door T); all `dli.*` ops are
//! wired. No capability-gated or unimplemented paths remain — the DL-0021 and
//! ec92efd wiring waves completed the dispatch table.

use crate::cmd::{CmdResult, EXIT_CLEAN};

/// Run the JSON-over-stdio worker loop.
pub(crate) fn run() -> CmdResult {
    dli_serve::serve::serve()
        .map(|()| EXIT_CLEAN)
        .map_err(|err| format!("the worker loop failed: {err}"))
}

/// Run the MCP JSON-RPC tool transport over stdio.
pub(crate) fn run_mcp() -> CmdResult {
    dli_serve::mcp::run_mcp()
        .map(|()| EXIT_CLEAN)
        .map_err(|err| format!("the MCP transport failed: {err}"))
}
