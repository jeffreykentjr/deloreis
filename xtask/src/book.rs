//! The mdBook build: renders `book/` into `out/book`.
//!
//! This step is **not** part of the offline gate — it shells out to `mdbook`,
//! an external tool that need not be installed for the gate to pass. When
//! `mdbook` is absent the build degrades gracefully (a warning, exit 0); CI
//! installs `mdbook` and the build is expected to succeed there.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Build the book with `mdbook`, or skip with a warning if it is not installed.
pub fn run_book(root: &Path) -> Result<()> {
    if !mdbook_available() {
        eprintln!(
            "book: mdbook not installed — skipping (install with `cargo install mdbook`). \
             This step is non-gating; CI installs mdbook and builds the book."
        );
        return Ok(());
    }

    let book_dir = root.join("book");
    let dest = root.join("out/book");
    let status = Command::new("mdbook")
        .arg("build")
        .arg(&book_dir)
        .arg("-d")
        .arg(&dest)
        .status()
        .with_context(|| format!("spawning mdbook build {}", book_dir.display()))?;
    if !status.success() {
        bail!("book: mdbook build failed (exit {:?})", status.code());
    }
    println!("book: built {}", dest.display());
    Ok(())
}

/// Whether an `mdbook` binary is on `PATH` and answers `--version`.
fn mdbook_available() -> bool {
    Command::new("mdbook")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}
