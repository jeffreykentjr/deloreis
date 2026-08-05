//! `deloreis validate-repo <canon-root>` — the native repository checks,
//! `V1`–`V8` and `V10`–`V12` (DL-0057; there is no `V9`).

use std::fs;
use std::path::PathBuf;

use clap::Args;
use dli_world::validate_repository;

use crate::cmd::{CmdResult, EXIT_CLEAN, EXIT_FINDINGS};

/// Arguments for the repository validator.
#[derive(Debug, Args)]
pub(crate) struct RepositoryValidateArgs {
    /// The canon root to scan (the directory containing Markdown authority files).
    #[arg(value_name = "CANON_ROOT")]
    pub(crate) root: PathBuf,
}

/// Run the native repository validator and emit its stable JSON findings.
pub(crate) fn run(args: &RepositoryValidateArgs) -> CmdResult {
    let metadata = fs::metadata(&args.root)
        .map_err(|error| format!("cannot read canon root {}: {error}", args.root.display()))?;
    if !metadata.is_dir() {
        return Err(format!(
            "canon root is not a directory: {}",
            args.root.display()
        ));
    }
    let findings = validate_repository(&args.root);
    let json = serde_json::to_string(&findings)
        .map_err(|error| format!("cannot serialize repository findings: {error}"))?;
    outln!("{}", json);
    if findings.is_empty() {
        Ok(EXIT_CLEAN)
    } else {
        Ok(EXIT_FINDINGS)
    }
}
