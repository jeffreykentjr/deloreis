#![allow(clippy::print_stdout, clippy::print_stderr)]
//! The `deloreis` command-line toolchain.
//!
//! Every subcommand is wired to the real engine crates (`CLI-1`). The public
//! [`command`] seam exposes the exact Clap graph to documentation and test
//! tooling; [`run_cli`] parses the process arguments and dispatches through the
//! same graph.

mod cmd;

use clap::{CommandFactory, Parser, Subcommand};

/// The Deloreis Lore Interchange toolchain.
#[derive(Debug, Parser)]
#[command(name = "deloreis", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The toolchain subcommands. Each variant carries the argument struct owned by
/// its `cmd` submodule, so the parse surface and the handler stay together.
///
/// Text commands are included by default through `syntax`. Disabling default
/// features retains the structured worker and MCP transports.
#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new lorestore-backed project.
    #[cfg(feature = "syntax")]
    Init(cmd::project::InitArgs),
    /// Import a .dli file into a new project.
    #[cfg(feature = "syntax")]
    Import(cmd::project::ImportArgs),
    /// Validate documents against the world model.
    #[cfg(feature = "syntax")]
    Check(cmd::check::CheckArgs),
    /// Query a saturated world.
    #[cfg(feature = "syntax")]
    Query(cmd::query::QueryArgs),
    /// Format documents into the one canonical surface.
    #[cfg(feature = "syntax")]
    Fmt(cmd::fmt::FmtArgs),
    /// Inspect and evaluate forks over a workspace store.
    #[cfg(feature = "syntax")]
    Fork(cmd::fork::ForkArgs),
    /// Run the ratification pre-flight over a fork.
    #[cfg(feature = "syntax")]
    Preflight(cmd::fork::PreflightArgs),
    /// Read the timeline: chronology and the episode fold.
    #[cfg(feature = "syntax")]
    Timeline(cmd::timeline::TimelineArgs),
    /// Check a diegetic proof block against a practice.
    #[cfg(feature = "syntax")]
    #[command(name = "check-proof")]
    CheckProof(cmd::drafts::CheckProofArgs),
    /// Vet a name — the kernel, world-true half.
    #[cfg(feature = "syntax")]
    Name(cmd::name::NameArgs),
    /// Export under a profile.
    #[cfg(feature = "syntax")]
    Export(cmd::drafts::ExportArgs),
    /// Render the findings report.
    #[cfg(feature = "syntax")]
    Report(cmd::report::ReportArgs),
    /// Run JSON-over-stdio with `--project <path>` or explicit `--demo`.
    Serve(cmd::serve::ServeArgs),
    /// Run MCP over stdio with `--project <path>` or explicit `--demo`.
    Mcp(cmd::serve::ServeArgs),
}

/// Return the executable's exact Clap command graph.
///
/// Documentation generators and command-surface tests use this rather than a
/// second, hand-maintained command inventory.
#[must_use]
pub fn command() -> clap::Command {
    let mut command = Cli::command();
    command.build();
    command
}

/// Parse the process arguments, run the selected command, and return its exit
/// code.
///
/// Clap handles help, version, and parse errors directly. Handler failures map
/// to the uniform operational-error code `2`.
#[must_use]
pub fn run_cli() -> i32 {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(code) => code,
        Err(message) => {
            cmd::emit_err(&format!("deloreis: {message}"));
            cmd::EXIT_USAGE
        }
    }
}

/// Route a parsed command to its handler, returning the verdict exit code.
fn run(command: Command) -> cmd::CmdResult {
    match command {
        #[cfg(feature = "syntax")]
        Command::Init(args) => cmd::project::init(&args),
        #[cfg(feature = "syntax")]
        Command::Import(args) => cmd::project::import(&args),
        #[cfg(feature = "syntax")]
        Command::Check(args) => cmd::check::run(&args),
        #[cfg(feature = "syntax")]
        Command::Query(args) => cmd::query::run(&args),
        #[cfg(feature = "syntax")]
        Command::Fmt(args) => cmd::fmt::run(&args),
        #[cfg(feature = "syntax")]
        Command::Fork(args) => cmd::fork::run(&args),
        #[cfg(feature = "syntax")]
        Command::Preflight(args) => cmd::fork::run_preflight(&args),
        #[cfg(feature = "syntax")]
        Command::Timeline(args) => cmd::timeline::run(&args),
        #[cfg(feature = "syntax")]
        Command::CheckProof(args) => cmd::drafts::run_check_proof(&args),
        #[cfg(feature = "syntax")]
        Command::Name(args) => cmd::name::run(&args),
        #[cfg(feature = "syntax")]
        Command::Export(args) => cmd::drafts::run_export(&args),
        #[cfg(feature = "syntax")]
        Command::Report(args) => cmd::report::run(&args),
        Command::Serve(args) => cmd::serve::run(&args),
        Command::Mcp(args) => cmd::serve::run_mcp(&args),
    }
}
