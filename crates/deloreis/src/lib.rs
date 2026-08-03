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
#[derive(Debug, Subcommand)]
enum Command {
    /// Validate documents against the world model.
    Check(cmd::check::CheckArgs),
    /// Query a saturated world.
    Query(cmd::query::QueryArgs),
    /// Format documents into the one canonical surface.
    Fmt(cmd::fmt::FmtArgs),
    /// Inspect and evaluate forks over a workspace store.
    Fork(cmd::fork::ForkArgs),
    /// Run the ratification pre-flight over a fork.
    Preflight(cmd::fork::PreflightArgs),
    /// Read the timeline: chronology and the episode fold.
    Timeline(cmd::timeline::TimelineArgs),
    /// Explain the craft principles behind findings.
    Craft(cmd::craft::CraftArgs),
    /// Check a diegetic proof block against a practice.
    #[command(name = "check-proof")]
    CheckProof(cmd::drafts::CheckProofArgs),
    /// Run the merged battery over a fat draft payload.
    #[command(name = "check-draft")]
    CheckDraft(cmd::drafts::CheckDraftArgs),
    /// Simulate a practice or game.
    Simulate(cmd::drafts::SimulateArgs),
    /// Vet a name — the kernel, world-true half.
    Name(cmd::name::NameArgs),
    /// Export under a profile.
    Export(cmd::drafts::ExportArgs),
    /// Render the findings report.
    Report(cmd::report::ReportArgs),
    /// Validate Markdown canon repository frontmatter.
    #[command(name = "validate-repo")]
    ValidateRepo(cmd::repository::RepositoryValidateArgs),
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
        Command::Check(args) => cmd::check::run(&args),
        Command::Query(args) => cmd::query::run(&args),
        Command::Fmt(args) => cmd::fmt::run(&args),
        Command::Fork(args) => cmd::fork::run(&args),
        Command::Preflight(args) => cmd::fork::run_preflight(&args),
        Command::Timeline(args) => cmd::timeline::run(&args),
        Command::Craft(args) => cmd::craft::run(&args),
        Command::CheckProof(args) => cmd::drafts::run_check_proof(&args),
        Command::CheckDraft(args) => cmd::drafts::run_check_draft(&args),
        Command::Simulate(args) => cmd::drafts::run_simulate(&args),
        Command::Name(args) => cmd::name::run(&args),
        Command::Export(args) => cmd::drafts::run_export(&args),
        Command::Report(args) => cmd::report::run(&args),
        Command::ValidateRepo(args) => cmd::repository::run(&args),
        Command::Serve(args) => cmd::serve::run(&args),
        Command::Mcp(args) => cmd::serve::run_mcp(&args),
    }
}
