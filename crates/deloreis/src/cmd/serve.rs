//! `deloreis serve` / `deloreis mcp` — delegate to the worker's two transports.
//!
//! The worker lives in `dli-serve` (the SERVE-1 milestone, owned by the server
//! lane) and exposes Door T over **both** transports the contract promises
//! (`10-serving.md` §1, `11-agent-authoring.md` §1): [`run`] is the JSON-over-
//! stdio worker loop, and [`run_mcp`] is the MCP JSON-RPC tool transport over
//! stdio. Both require an explicit caller project or explicit demo mode and map
//! the worker's `io::Result` to an exit code (clean EOF →
//! `0`, an I/O failure → usage). The MCP transport is a wrapper over the same
//! ops, not a second protocol, so both reach identical kernel paths.
//!
//! The loop never panics on any traffic. All 19 `author.*` tools are fully
//! implemented (`author::dispatch` routes through Door T). Batch operations
//! advertise only the capabilities implemented by this build; recognized
//! reservation rows for unfinished durable evidence and job behavior return a
//! typed `not-wired` fault.

use std::path::PathBuf;

use clap::{ArgGroup, Args};

use crate::cmd::{CmdResult, EXIT_CLEAN};

/// Project selection shared by the two worker transports.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("corpus")
        .required(true)
        .multiple(false)
        .args(["project", "demo"])
))]
pub(crate) struct ServeArgs {
    /// `deloreis.toml` or its containing directory.
    #[arg(long, value_name = "PATH")]
    project: Option<PathBuf>,
    /// Run only the bundled neutral fixtures.
    #[arg(long)]
    demo: bool,
}

/// Run the JSON-over-stdio worker loop.
pub(crate) fn run(args: &ServeArgs) -> CmdResult {
    let result = match load_project(args)? {
        Some(project) => dli_serve::serve::serve_project(project),
        None => dli_serve::serve::serve_demo(),
    };
    result
        .map(|()| EXIT_CLEAN)
        .map_err(|err| format!("the worker loop failed: {err}"))
}

/// Run the MCP JSON-RPC tool transport over stdio.
pub(crate) fn run_mcp(args: &ServeArgs) -> CmdResult {
    let result = match load_project(args)? {
        Some(project) => dli_serve::mcp::run_mcp_project(project),
        None => dli_serve::mcp::run_mcp_demo(),
    };
    result
        .map(|()| EXIT_CLEAN)
        .map_err(|err| format!("the MCP transport failed: {err}"))
}

fn load_project(args: &ServeArgs) -> Result<Option<dli_serve::project::Project>, String> {
    if args.demo {
        return Ok(None);
    }
    let path = args
        .project
        .as_ref()
        .ok_or_else(|| "pass `--project <path>` or explicit `--demo`".to_owned())?;
    dli_serve::project::Project::load(path)
        .map(Some)
        .map_err(|error| error.to_string())
}
