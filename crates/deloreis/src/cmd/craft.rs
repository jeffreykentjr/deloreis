//! `deloreis craft why <finding>` - explain the principles behind a finding.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use dli_world::craft::Kb;

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for `deloreis craft`.
#[derive(Debug, Args)]
pub(crate) struct CraftArgs {
    /// The craft knowledge-base operation.
    #[command(subcommand)]
    pub(crate) op: CraftOp,
}

/// Craft knowledge-base operations.
#[derive(Debug, Subcommand)]
pub(crate) enum CraftOp {
    /// Explain the principles that govern a craft or outline finding.
    Why(WhyArgs),
}

/// Arguments for `craft why`.
#[derive(Debug, Args)]
pub(crate) struct WhyArgs {
    /// The dotted finding code, such as `craft.accomplishment-earned`.
    #[arg(value_name = "FINDING")]
    pub(crate) finding: String,
    /// A caller-owned craft configuration directory.
    #[arg(long, value_name = "DIR")]
    pub(crate) config: Option<PathBuf>,
}

/// Run a craft knowledge-base operation.
pub(crate) fn run(args: &CraftArgs) -> CmdResult {
    match &args.op {
        CraftOp::Why(why) => run_why(why),
    }
}

fn run_why(args: &WhyArgs) -> CmdResult {
    if let Some(path) = &args.config {
        let (principles, _, _) = cmd::load_craft_config(path)?;
        Ok(render_why(Kb::new(principles.principles()), &args.finding))
    } else {
        Ok(render_why(Kb::builtin(), &args.finding))
    }
}

fn render_why(kb: Kb<'_>, finding: &str) -> i32 {
    let principles = kb.why(finding);
    if principles.is_empty() {
        outln!("no principle claims finding {finding}");
        return EXIT_FINDINGS;
    }
    for (index, principle) in principles.iter().enumerate() {
        if index > 0 {
            outln!();
        }
        outln!("{}", principle.explain());
    }
    EXIT_CLEAN
}
