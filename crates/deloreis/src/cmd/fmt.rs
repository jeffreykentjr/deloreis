//! `deloreis fmt <paths…>` — the canonical-surface migrator.
//!
//! The formatter is the one canonical surface (`01-syntax.md` §1, §7; DL-0003):
//! parse a document and render it back, idempotently. By default `fmt` writes
//! each file's canonical form in place (write-sloppy-save-canonical); with
//! `--check` it writes nothing and reports the files that are not already
//! canonical, exiting `1` — the shape the gate's `dli-fmt --check` step consumes.
//!
//! Formatting goes through the **safe** entry [`format_source`], never the raw
//! AST renderer: `format_source` re-parses and *refuses* (an `Err` of
//! diagnostics) on any malformed input rather than emit a lossy rewrite that
//! would silently delete a line the parser could not place (the H3 data-loss
//! class). A refused file is shown with its diagnostics, left untouched, and the
//! command exits `2` — a rewrite is never attempted on input it cannot round-trip.

use std::fs;
use std::path::PathBuf;

use clap::Args;
use dli_syntax::{format_source, ladder};

use crate::cmd::{self, CmdResult, EXIT_CLEAN, EXIT_FINDINGS, EXIT_USAGE};

/// Arguments for `deloreis fmt`.
#[derive(Debug, Args)]
pub(crate) struct FmtArgs {
    /// The `.dli` documents (or directories to walk) to format.
    #[arg(value_name = "PATH", required = true)]
    pub(crate) paths: Vec<PathBuf>,
    /// Report files that are not canonical instead of rewriting them.
    #[arg(long)]
    pub(crate) check: bool,
}

/// Format the given files, or (with `--check`) report the ones that would change.
pub(crate) fn run(args: &FmtArgs) -> CmdResult {
    let files = cmd::load_corpus(&args.paths)?;

    let mut dirty = 0usize;
    let mut blocked = 0usize;

    for file in &files {
        let path = file.path.display();
        match format_source(&file.source) {
            Ok(canonical) => {
                if canonical == file.source {
                    continue;
                }
                dirty += 1;
                if args.check {
                    outln!("{path}: not canonical (would reformat)");
                } else {
                    fs::write(&file.path, &canonical)
                        .map_err(|err| format!("cannot write {path}: {err}"))?;
                    outln!("{path}: formatted");
                }
            }
            Err(diagnostics) => {
                // The safe entry refused: rendering the partial AST would drop the
                // malformed line on save. Show the fault, write nothing.
                out!("{path}:\n{}", ladder::render(&file.source, &diagnostics));
                outln!(
                    "{path}: refusing to format — {} diagnostic(s); a rewrite would drop the \
                     malformed line",
                    diagnostics.len()
                );
                blocked += 1;
            }
        }
    }

    let count = files.len();
    if blocked > 0 {
        let verb = if args.check {
            "would reformat"
        } else {
            "changed"
        };
        outln!("fmt: {count} file(s) — {dirty} {verb}, {blocked} refused as malformed (untouched)");
        Ok(EXIT_USAGE)
    } else if args.check {
        if dirty == 0 {
            outln!("checked {count} file(s): all canonical");
            Ok(EXIT_CLEAN)
        } else {
            outln!("checked {count} file(s): {dirty} would reformat");
            Ok(EXIT_FINDINGS)
        }
    } else {
        outln!("formatted {count} file(s): {dirty} changed");
        Ok(EXIT_CLEAN)
    }
}
