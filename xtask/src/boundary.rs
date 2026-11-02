//! Repository-wide guard against retired private-world names.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Retired world tokens, matched case-insensitively at ASCII word boundaries.
pub const WORLD_TOKENS: &[&str] = &[
    "tpth",
    "enfys",
    "aalto",
    "the-debt-square",
    "weather-debt",
    "saltmarket",
    "the-shallows",
    "cymric",
    "the-turning",
    "lowlands",
    "reckoner",
    "concordance",
    "spoken-proof",
    "pleach",
    "hedgeway",
    "the-multitude",
    "gwern",
    "tide-bell",
    "mara",
    "breakwater",
    "turning",
    "proof-speech",
    "wyrd",
    "multitude",
    "tegwen",
    "kes",
    "branwen",
];

fn contains_word(text: &str, token: &str) -> bool {
    let text = text.to_ascii_lowercase();
    let variants = [
        token.to_owned(),
        token.replace('-', " "),
        token.replace('-', "_"),
    ];
    variants.iter().any(|variant| {
        text.match_indices(variant).any(|(start, _)| {
            let before = text.as_bytes().get(start.wrapping_sub(1));
            let after = text.as_bytes().get(start + variant.len());
            before.is_none_or(|c| !c.is_ascii_alphanumeric())
                && after.is_none_or(|c| !c.is_ascii_alphanumeric())
        })
    })
}

/// Check tracked paths and contents, excluding only this guard's token list.
pub fn run_boundary(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .context("listing tracked files for boundary guard")?;
    if !output.status.success() {
        bail!("boundary: git could not list tracked files");
    }
    let mut hits = Vec::new();
    for name in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
    {
        let name = std::str::from_utf8(name).context("non-UTF-8 tracked path")?;
        let path = root.join(name);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).with_context(|| format!("reading {name}")),
        };
        let contents = String::from_utf8_lossy(&bytes);
        let checked = if name == "xtask/src/boundary.rs" {
            let (before, tail) = contents
                .split_once("pub const WORLD_TOKENS: &[&str] = &[")
                .context("boundary: token list start is missing")?;
            let (_, after) = tail
                .split_once("];\n")
                .context("boundary: token list end is missing")?;
            format!("{before}{after}")
        } else {
            contents.into_owned()
        };
        for token in WORLD_TOKENS {
            if contains_word(name, token) || contains_word(&checked, token) {
                hits.push(format!("{name}: {token}"));
            }
        }
    }
    if !hits.is_empty() {
        bail!("boundary: retired world tokens found:\n{}", hits.join("\n"));
    }
    println!("boundary: tracked files contain no retired world tokens");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::contains_word;

    #[test]
    fn token_boundaries_are_case_insensitive_and_not_substrings() {
        assert!(contains_word("A BLUE-BELL rings", "blue-bell"));
        assert!(!contains_word("blue-bellflower", "blue-bell"));
        assert!(!contains_word("notblue-bell", "blue-bell"));
        assert!(contains_word("blue-bell's song", "blue-bell"));
        assert!(contains_word("a Blue Bell rings", "blue-bell"));
        assert!(contains_word("blue_bell rings", "blue-bell"));
        assert!(!contains_word("blue belling", "blue-bell"));
    }
}
