//! `deloreis check <paths…>` — parse, compile, and run the world-check battery.
//!
//! The battery is three seams over one corpus: the syntax diagnostics from
//! parsing (rendered through the three-rung ladder), the loader's fail-closed
//! findings plus the continuity classes (`dli_world::check::run`), and the
//! language layer's world-true naming findings (`dli_world::language::check`).
//! A clean corpus exits `0`; any finding exits `1` with the finding lines and,
//! for a syntax fault, the ladder block that teaches the repair.

use std::path::PathBuf;

use clap::Args;
use dli_syntax::ladder;
use dli_world::{check, language};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for `deloreis check`.
#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// The `.dli` documents (or directories to walk) to validate.
    #[arg(value_name = "PATH", required = true)]
    pub(crate) paths: Vec<PathBuf>,
}

/// Validate the given corpus and report every finding.
pub(crate) fn run(args: &CheckArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.paths)?;

    let mut findings = 0usize;

    // 1. Syntax diagnostics, rendered with their graded ladders under a caret
    //    block, per file.
    for file in &files {
        if !file.diagnostics.is_empty() {
            findings += file.diagnostics.len();
            out!(
                "{}:\n{}",
                file.path.display(),
                ladder::render(&file.source, &file.diagnostics)
            );
        }
    }

    // 2. Semantic findings: load-time, continuity, and language, collected into
    //    one list sorted for a deterministic, diffable report.
    let mut lines: Vec<String> = Vec::new();
    match cmd::compile_corpus(&files, "check") {
        Ok(world) => {
            for finding in world.model.findings() {
                lines.push(cmd::render_load_finding(finding));
            }
            for finding in check::run(&world) {
                lines.push(cmd::render_world_finding(&finding));
            }
            for finding in language::check(&world.model.language) {
                lines.push(cmd::render_world_finding(&finding));
            }
        }
        Err(message) => lines.push(format!("error  compile.unstratifiable — {message}")),
    }
    lines.sort();
    findings += lines.len();
    for line in &lines {
        outln!("{line}");
    }

    let count = files.len();
    if findings == 0 {
        outln!("checked {count} document(s): clean");
        Ok(EXIT_CLEAN)
    } else {
        outln!("checked {count} document(s): {findings} finding(s)");
        Ok(EXIT_FINDINGS)
    }
}
