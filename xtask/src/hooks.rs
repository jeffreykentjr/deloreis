//! The git hooks-path check.

use std::process::Command;

use anyhow::{Context, Result, bail};

/// Assert `core.hooksPath` points at `.githooks`.
pub fn run_hooks_check() -> Result<()> {
    let output = Command::new("git")
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .context("running git config --get core.hooksPath")?;
    let value = String::from_utf8_lossy(&output.stdout);
    let value = value.trim();
    if value == ".githooks" {
        println!("hooks-check: core.hooksPath = .githooks");
        Ok(())
    } else {
        bail!(
            "hooks-check: core.hooksPath is '{value}', expected .githooks; run scripts/install-hooks.sh"
        )
    }
}
