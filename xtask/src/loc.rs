//! The lines-of-code budget check.

use std::path::Path;

use anyhow::{Result, bail};
use walkdir::WalkDir;

const SOFT_CAP: usize = 25_000;
const HARD_CAP: usize = 40_000;

/// Count non-blank, non-comment lines under `crates/*/src` and enforce the caps.
pub fn run_loc(root: &Path) -> Result<()> {
    let crates_dir = root.join("crates");
    let mut total = 0usize;

    for entry in WalkDir::new(&crates_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let Ok(relative) = path.strip_prefix(&crates_dir) else {
            continue;
        };
        // relative == <crate>/src/... — only count files under a crate's src/.
        let components: Vec<String> = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        if components.get(1).map(String::as_str) != Some("src") {
            continue;
        }
        let text = std::fs::read_to_string(path)?;
        total += count_code_lines(&text);
    }

    println!("loc: {total} code lines under crates/*/src");
    if total > HARD_CAP {
        bail!("loc: {total} exceeds the hard cap of {HARD_CAP}");
    }
    if total > SOFT_CAP {
        eprintln!("loc: WARNING {total} exceeds the soft cap of {SOFT_CAP}");
    }
    Ok(())
}

fn count_code_lines(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("//")
        })
        .count()
}
