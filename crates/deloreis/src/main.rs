#![allow(clippy::print_stdout, clippy::print_stderr)]
//! The `deloreis` command-line toolchain.
//!
//! Every subcommand is wired to the real engine crates (`CLI-1`): [`cmd::check`]
//! parses, compiles, and runs the world-check battery; [`cmd::query`] answers a
//! goal over the compiled world with optional `--why`; [`cmd::fmt`] is the
//! write-sloppy-save-canonical migrator; [`cmd::fork`] drives the fork store over
//! a workspace cache; [`cmd::timeline`] reads the chronology and the episode
//! fold; [`cmd::name`] runs the kernel vet half; [`cmd::report`] renders the
//! findings report; [`cmd::serve`] delegates to the JSON worker loop and MCP
//! transport. The engines
//! that are not yet frozen (`check-proof`, `simulate`, `export`) answer with a
//! typed honest not-yet rather than a guess or a panic.
//!
//! Exit codes are verdict-reflecting and uniform across the surface: `0` clean,
//! `1` findings (a diagnosed problem the caller acts on — including a query that
//! matched nothing), `2` a usage or operational error (bad arguments, a missing
//! file, an engine that has not landed).

mod cmd;

use clap::{Parser, Subcommand};

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
    /// Check a diegetic proof block against a practice.
    #[command(name = "check-proof")]
    CheckProof(cmd::drafts::CheckProofArgs),
    /// Fact-check a draft payload against the world model.
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
    /// Run the JSON-over-stdio worker.
    Serve,
    /// Run the MCP tool transport over stdio.
    Mcp,
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
        Command::CheckProof(args) => cmd::drafts::run_check_proof(&args),
        Command::CheckDraft(args) => cmd::drafts::run_check_draft(&args),
        Command::Simulate(args) => cmd::drafts::run_simulate(&args),
        Command::Name(args) => cmd::name::run(&args),
        Command::Export(args) => cmd::drafts::run_export(&args),
        Command::Report(args) => cmd::report::run(&args),
        Command::Serve => cmd::serve::run(),
        Command::Mcp => cmd::serve::run_mcp(),
    }
}

fn main() {
    let cli = Cli::parse();
    let code = match run(cli.command) {
        Ok(code) => code,
        Err(message) => {
            cmd::emit_err(&format!("deloreis: {message}"));
            cmd::EXIT_USAGE
        }
    };
    std::process::exit(code);
}
