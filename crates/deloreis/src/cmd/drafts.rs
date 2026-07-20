//! The payload and simulation ops: `check-draft` (wired), plus `check-proof`,
//! `simulate`, and `export` (typed honest not-yet until their engines freeze).
//!
//! `check-draft` runs the merged fact-check battery (`dli_world::check_draft`)
//! over a `payload/v1` document. The other three name engines that have not
//! landed in this wave — the proof checker (PRAC-2), the practice simulator
//! (SIM-1), and the export profiles (EXP-1). Rather than call a `todo!` body and
//! panic, or guess an answer, each reports what it is waiting on and exits with a
//! usage code, so a script sees an honest "not yet" and the surface never lies.

use std::fs;
use std::path::PathBuf;

use clap::{Args, ValueEnum};
use dli_world::{DraftPayload, check_draft};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS, EXIT_USAGE};

/// Arguments for `deloreis check-draft`.
#[derive(Debug, Args)]
pub(crate) struct CheckDraftArgs {
    /// The `payload/v1` draft document (YAML) to fact-check.
    #[arg(value_name = "PAYLOAD")]
    pub(crate) payload: PathBuf,
    /// The `.dli` corpus (files or directories) to check the draft against.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
}

/// Fact-check a draft payload against the compiled world.
pub(crate) fn run_check_draft(args: &CheckDraftArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "check-draft")?;

    let text = fs::read_to_string(&args.payload)
        .map_err(|err| format!("cannot read {}: {err}", args.payload.display()))?;
    let draft: DraftPayload = serde_yaml_ng::from_str(&text).map_err(|err| {
        format!(
            "the draft payload {} is malformed: {err}",
            args.payload.display()
        )
    })?;

    let mut findings = check_draft(&world, &draft);
    findings.sort_by(|a, b| (&a.claim, &a.message).cmp(&(&b.claim, &b.message)));
    for finding in &findings {
        outln!(
            "error  {claim} — {message} ({grade})",
            claim = finding.claim,
            message = finding.message,
            grade = finding.grade,
        );
    }

    if findings.is_empty() {
        outln!("draft clean: {} claim(s) supported", draft.claims.len());
        Ok(EXIT_CLEAN)
    } else {
        outln!("draft: {} defect(s)", findings.len());
        Ok(EXIT_FINDINGS)
    }
}

/// Arguments for `deloreis check-proof`.
#[derive(Debug, Args)]
pub(crate) struct CheckProofArgs {
    /// The diegetic proof-block file to replay.
    #[arg(value_name = "PROOF")]
    pub(crate) proof: PathBuf,
    /// The practice the casting runs in.
    #[arg(long = "in", value_name = "PRACTICE")]
    pub(crate) practice: String,
}

/// Report that the proof checker has not yet frozen.
// Uniform `CmdResult` dispatch shape; grows fallible when PRAC-2 wires the real
// checker.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn run_check_proof(args: &CheckProofArgs) -> CmdResult {
    outln!(
        "check-proof is not wired yet: the proof checker (PRAC-2) has not frozen, so a proof \
         block cannot be replayed against practice '{}' offline. The command surface is stable; \
         the engine lands this wave.",
        args.practice
    );
    Ok(EXIT_USAGE)
}

/// Arguments for `deloreis simulate`.
#[derive(Debug, Args)]
pub(crate) struct SimulateArgs {
    /// The practice (or ruleset) to simulate.
    #[arg(value_name = "PRACTICE")]
    pub(crate) practice: String,
    /// The number of games to run.
    #[arg(long, value_name = "N")]
    pub(crate) games: Option<u64>,
    /// The random seed.
    #[arg(long, value_name = "SEED")]
    pub(crate) seed: Option<u64>,
}

/// Report that the practice simulator has not yet frozen.
// Uniform `CmdResult` dispatch shape; grows fallible when SIM-1 wires the sim.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn run_simulate(args: &SimulateArgs) -> CmdResult {
    outln!(
        "simulate is not wired yet: the practice simulator (SIM-1) has not frozen, so practice \
         '{}' cannot be run. The command surface is stable; the engine is a later wave.",
        args.practice
    );
    Ok(EXIT_USAGE)
}

/// An export target, as a CLI value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ProfileArg {
    /// A context pack for downstream generation.
    ContextPack,
    /// Printable reference cards.
    Cards,
    /// Ludii Game Description Language for the simulator.
    LudiiGdl,
}

/// Arguments for `deloreis export`.
#[derive(Debug, Args)]
pub(crate) struct ExportArgs {
    /// The export profile to render.
    #[arg(long, value_enum, value_name = "PROFILE")]
    pub(crate) profile: ProfileArg,
    /// The `.dli` corpus (files or directories) to export.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
}

/// Report that the export profiles have not yet frozen.
// Uniform `CmdResult` dispatch shape; grows fallible when EXP-1 wires export.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn run_export(_args: &ExportArgs) -> CmdResult {
    outln!(
        "export is not wired yet: the export profiles (EXP-1) have not frozen, so no profile can \
         be rendered. The command surface is stable; the engine is a later wave."
    );
    Ok(EXIT_USAGE)
}
