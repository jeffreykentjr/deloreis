//! `deloreis name check <name>` — the kernel, world-true half of the naming vet.
//!
//! This runs only the screens whose ground truth is a fact about the *story's*
//! world (`05-language-layer.md` §3): intra-canon uniqueness, fixed-token reuse,
//! aural distinctness, and banned-vocabulary-in-names, vetted against the settled
//! name-space compiled from the corpus. The factory-side screens (AI-tics,
//! pop-culture collisions, anachronism) are a fact about *our* world and stay in
//! the consuming repo — a candidate that fails only those is kernel-clean.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use dli_world::language::{vet_candidate, vet_candidate_in_dialect};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for `deloreis name`.
#[derive(Debug, Args)]
pub(crate) struct NameArgs {
    /// The naming operation.
    #[command(subcommand)]
    pub(crate) op: NameOp,
}

/// The naming operations. Only the kernel `check` half lives here.
#[derive(Debug, Subcommand)]
pub(crate) enum NameOp {
    /// Vet a candidate name against the settled name-space.
    Check(CheckArgs),
}

/// Arguments for `name check`.
#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// The candidate name to vet.
    #[arg(value_name = "NAME")]
    pub(crate) name: String,
    /// The dialect (register) whose name-space to vet in.
    #[arg(long, value_name = "DIALECT")]
    pub(crate) dialect: Option<String>,
    /// The `.dli` corpus (files or directories) carrying the name-space.
    #[arg(short = 'c', long = "corpus", value_name = "PATH", required = true)]
    pub(crate) corpus: Vec<PathBuf>,
}

/// Run a `deloreis name` operation.
pub(crate) fn run(args: &NameArgs) -> CmdResult {
    match &args.op {
        NameOp::Check(check) => run_check(check),
    }
}

fn run_check(args: &CheckArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.corpus)?;
    let world = cmd::compile_corpus(&files, "name")?;
    let model = &world.model.language;

    if let Some(dialect) = &args.dialect
        && model.register(dialect).is_none()
    {
        return Err(format!("unknown dialect '{dialect}' in this corpus"));
    }

    // The world-true screens always run (uniqueness, fixed-token reuse, aural
    // distinctness, banned-in-name). A `--dialect` additionally discharges the
    // one register-fit obligation a bare candidate can carry — the register's
    // phonotactic frame — while slot-sequence and role-membership stay unrun,
    // because they are claims about a composition the candidate does not have.
    let mut findings = vet_candidate(model, &args.name);
    if let Some(dialect) = &args.dialect {
        findings.extend(vet_candidate_in_dialect(model, &args.name, dialect));
    }

    if findings.is_empty() {
        outln!(
            "{}: clears the kernel screens — uniqueness, aural distinctness, \
             fixed-token reuse, banned-in-name",
            args.name
        );
        if let Some(dialect) = &args.dialect {
            outln!("  ran register-fit phonotactics for dialect {dialect}: clean");
            outln!("{}", coverage_note(dialect));
        }
        return Ok(EXIT_CLEAN);
    }
    for finding in &findings {
        outln!("{}", cmd::render_world_finding(&finding.to_world_finding()));
    }
    if let Some(dialect) = &args.dialect {
        outln!("{}", coverage_note(dialect));
    }
    outln!("{}: {} finding(s)", args.name, findings.len());
    Ok(EXIT_FINDINGS)
}

/// The honest coverage disclosure for a `--dialect` vet: name exactly which
/// register-fit obligations a bare candidate could not carry, so a clean line is
/// never read as full register-fit (spec 05 §3).
fn coverage_note(dialect: &str) -> String {
    format!(
        "  note: over a bare candidate, only the phonotactic frame of dialect \
         {dialect} is checked; the composition-dependent register-fit obligations \
         (slot-sequence, role-membership) were NOT run"
    )
}
