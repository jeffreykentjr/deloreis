//! The LS-9 no-parse gate — the acceptance bar for expert-tier structuring.
//!
//! `dli-world`, `dli-ledger`, and `dli-time` must not depend on `dli-syntax` on
//! the **default build path**. A normal `[dependencies]` entry links the parser
//! into the library, which is exactly the coupling LS-9 retires so LS-7 can
//! feature-gate `dli_syntax` out of the default build
//! (`docs/design-notes/lorestore-roadmap.md` §LS-9, §LS-7). `dli-syntax` under
//! `[dev-dependencies]` is fine — tests and doc examples still build `.dli`
//! text through the front end, and a one-off v1→v2 store upcast at the load
//! boundary is allowed to (that lives in `dli-lorestore`, not a target crate).
//!
//! Rust enforces the deeper property for free: a crate's non-test code cannot
//! `use dli_syntax::…` unless `dli-syntax` is a normal dependency. So the
//! manifest classification is both necessary and sufficient — checking it is the
//! objective truth about whether the parser is on a crate's default path.
//!
//! This gate is a **ratchet**. `STILL_COUPLED` is the honest, tracked set of
//! target crates LS-9 has not yet fully converted; the goal is the empty set,
//! at which point LS-7 is unblocked. A crate that is cleared must be removed
//! from `STILL_COUPLED` in the same change; a crate that regresses (re-adds
//! the normal dependency) fails the gate. It "tells the truth" — it never
//! claims the bar is met when it is not.

use std::path::Path;

use anyhow::{Result, bail};

/// The engine crates whose default build LS-9 is retiring the parser from.
const TARGET_CRATES: &[&str] = &["dli-world", "dli-ledger", "dli-time"];

/// Target crates that still declare `dli-syntax` as a normal `[dependencies]`
/// entry — tracked LS-9 debt, retired as each crate's expert-tier declaration
/// interiors are structured. The acceptance bar is met when this is empty.
///
/// - `dli-world`: `language/dialect.rs`, `language/name.rs`, `payload.rs` (proof)
///   still re-parse stored clause text.
/// - `dli-ledger`: `practice.rs`, `ledger_check.rs` still re-parse stored text.
///
/// `dli-time` is **not** here: its only parse site (`order_rules.rs`) was
/// retired by the `RuleDecl` structuring, and `dli-syntax` moved to its
/// `[dev-dependencies]`.
const STILL_COUPLED: &[&str] = &["dli-world", "dli-ledger"];

/// Run the LS-9 no-parse gate over the workspace at `root`.
pub fn run_syntax_free(root: &Path) -> Result<()> {
    let mut coupled: Vec<&str> = Vec::new();
    for crate_name in TARGET_CRATES {
        let manifest = root.join("crates").join(crate_name).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", manifest.display()))?;
        if normal_deps_include_dli_syntax(&text) {
            coupled.push(crate_name);
        }
    }

    let regressed: Vec<&str> = coupled
        .iter()
        .copied()
        .filter(|c| !STILL_COUPLED.contains(c))
        .collect();
    let cleared: Vec<&str> = STILL_COUPLED
        .iter()
        .copied()
        .filter(|c| !coupled.contains(c))
        .collect();

    if !regressed.is_empty() {
        bail!(
            "LS-9 no-parse gate: {regressed:?} newly declare `dli-syntax` as a normal \
             dependency — the parser must not link into the default build of a target \
             crate. Structure the interior and repoint the reader to typed AST."
        );
    }
    if !cleared.is_empty() {
        bail!(
            "LS-9 no-parse gate: {cleared:?} no longer depend on `dli-syntax` on the \
             default path — remove them from STILL_COUPLED so the ratchet advances \
             toward the empty (LS-7-ready) set."
        );
    }

    if coupled.is_empty() {
        println!(
            "LS-9 no-parse gate: all {} target crates are syntax-free on the default \
             path — the acceptance bar is met and LS-7 is unblocked.",
            TARGET_CRATES.len()
        );
    } else {
        println!(
            "LS-9 no-parse gate: {}/{} target crates syntax-free; still coupled \
             (tracked debt): {coupled:?}.",
            TARGET_CRATES.len() - coupled.len(),
            TARGET_CRATES.len()
        );
    }
    Ok(())
}

/// Whether the `[dependencies]` table (and not `[dev-dependencies]`) of a
/// manifest names `dli-syntax`.
fn normal_deps_include_dli_syntax(manifest: &str) -> bool {
    let mut in_normal_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            // A new table header ends the previous table's scope.
            in_normal_deps = rest.starts_with("dependencies]");
            continue;
        }
        if in_normal_deps && trimmed.starts_with("dli-syntax") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::normal_deps_include_dli_syntax;

    #[test]
    fn normal_dependency_is_detected() {
        let manifest = "[dependencies]\ndli-core.workspace = true\ndli-syntax.workspace = true\n";
        assert!(normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn dev_dependency_is_not_the_default_path() {
        let manifest = "[dependencies]\ndli-ast.workspace = true\n\n[dev-dependencies]\ndli-syntax.workspace = true\n";
        assert!(!normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn commented_out_dependency_does_not_count() {
        let manifest = "[dependencies]\n# dli-syntax.workspace = true\ndli-ast.workspace = true\n";
        assert!(!normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn absent_dependency_is_clear() {
        let manifest = "[dependencies]\ndli-core.workspace = true\n\n[lints]\nworkspace = true\n";
        assert!(!normal_deps_include_dli_syntax(manifest));
    }
}
