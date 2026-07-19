#![allow(clippy::print_stdout, clippy::print_stderr)]
//! The `deloreis` command-line toolchain.
//!
//! This is the spike scaffold: the subcommand surface and help text are real,
//! but every handler reports that it is unimplemented and exits 2. The library
//! crates it depends on carry the real types the handlers will wire up.

use clap::{Parser, Subcommand};

/// The Deloreis Lore Interchange toolchain.
#[derive(Debug, Parser)]
#[command(name = "deloreis", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The toolchain subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a document against the world model.
    Check,
    /// Query a saturated world.
    Query,
    /// Format a document.
    Fmt,
    /// Inspect and evaluate forks.
    Fork,
    /// Run the gentle-subset preflight probe.
    Preflight,
    /// Check a resource ledger.
    CheckLedger,
    /// Fact-check a draft.
    CheckDraft,
    /// Simulate a practice or game.
    Simulate,
    /// Render the timeline.
    Timeline,
    /// Export under a profile.
    Export,
    /// Run the wire server.
    Serve,
    /// Render the author review report.
    Report,
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Command::Check => "check",
            Command::Query => "query",
            Command::Fmt => "fmt",
            Command::Fork => "fork",
            Command::Preflight => "preflight",
            Command::CheckLedger => "check-ledger",
            Command::CheckDraft => "check-draft",
            Command::Simulate => "simulate",
            Command::Timeline => "timeline",
            Command::Export => "export",
            Command::Serve => "serve",
            Command::Report => "report",
        }
    }
}

fn main() {
    let cli = Cli::parse();
    eprintln!(
        "deloreis: {} not implemented in the spike scaffold",
        cli.command.name()
    );
    std::process::exit(2);
}
