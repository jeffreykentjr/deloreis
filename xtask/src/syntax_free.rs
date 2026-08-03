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
/// - `dli-world`: `payload.rs` still parses non-declaration payload-claim /
///   runtime-fact atoms via `dli_syntax::parse_clause_atom` — a load-boundary
///   parse that LS-7 feature-gates, not an expert-tier declaration body
///   (every eleven-decl-type parser was retired by LS-9).
///
/// `dli-time` is **not** here: its only parse site (`order_rules.rs`) was
/// retired by the `RuleDecl` structuring, and `dli-syntax` moved to its
/// `[dev-dependencies]`.
const STILL_COUPLED: &[&str] = &["dli-world"];

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

/// Whether the normal-dependencies table (and never `[dev-dependencies]`) of a
/// manifest names `dli-syntax`, in either form TOML allows:
///
/// - flat table with a key row — `[dependencies]` … `dli-syntax.workspace = true`;
/// - dotted sub-table header — `[dependencies.dli-syntax]` (`workspace = true`
///   in its body), including target-scoped `[target.'cfg(…)'.dependencies…]`.
///
/// A table is "normal dependencies" when `dependencies` is one of its dotted
/// header segments (so `dev-dependencies` / `build-dependencies` are excluded,
/// being distinct single segments). The dotted sub-table form is caught by the
/// header naming both `dependencies` and `dli-syntax`; the flat form is caught
/// by scanning the body of a table whose final segment is `dependencies`.
fn normal_deps_include_dli_syntax(manifest: &str) -> bool {
    let mut scanning_normal_body = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            let Some(segments) = table_header_segments(trimmed) else {
                scanning_normal_body = false;
                continue;
            };
            let is_normal = segments.iter().any(|s| s == "dependencies");
            // Dotted-sub-table form: the header itself names the dependency, e.g.
            // `[dependencies.dli-syntax]` — this is the hole the flat scan missed.
            if is_normal && segments.iter().any(|s| s == "dli-syntax") {
                return true;
            }
            // Only a table whose final segment is `dependencies` carries ordinary
            // `key = value` dependency rows worth scanning (a `[dependencies.foo]`
            // body holds `foo`'s fields, not sibling dependencies).
            scanning_normal_body =
                is_normal && segments.last().map(String::as_str) == Some("dependencies");
            continue;
        }
        if scanning_normal_body && line_declares_dli_syntax(trimmed) {
            return true;
        }
    }
    false
}

/// Split a TOML table header line (`[a.b.c]`) into its dotted segments, trimming
/// whitespace and surrounding quotes from each. `None` if it is not a `[…]`
/// header.
fn table_header_segments(header: &str) -> Option<Vec<String>> {
    let inner = header.trim().strip_prefix('[')?.strip_suffix(']')?;
    Some(
        inner
            .split('.')
            .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').to_owned())
            .collect(),
    )
}

/// Whether a `key = value` body line declares `dli-syntax` (as `dli-syntax = …`
/// or `dli-syntax.workspace = …`), ignoring comment lines.
fn line_declares_dli_syntax(line: &str) -> bool {
    if line.starts_with('#') {
        return false;
    }
    // The dependency name is the token before the first `=` or `.`.
    let key = line
        .split(['=', '.'])
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(|c| c == '"' || c == '\'');
    key == "dli-syntax"
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

    #[test]
    fn dotted_normal_subtable_is_detected() {
        // The dotted sub-table form the flat scan used to miss — a false
        // negative in the acceptance gate itself.
        let manifest = "[dependencies.dli-syntax]\nworkspace = true\n";
        assert!(normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn dotted_normal_subtable_among_others_is_detected() {
        let manifest = "[dependencies]\ndli-core.workspace = true\n\n\
                        [dependencies.dli-syntax]\nworkspace = true\nfeatures = []\n";
        assert!(normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn dotted_dev_subtable_is_ignored() {
        // The dev-dependencies dotted form must still be ignored.
        let manifest = "[dependencies]\ndli-ast.workspace = true\n\n\
                        [dev-dependencies.dli-syntax]\nworkspace = true\n";
        assert!(!normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn dotted_other_subtable_body_is_not_scanned() {
        // A `[dependencies.serde]` body holds serde's fields, not sibling deps.
        let manifest = "[dependencies.serde]\nversion = \"1\"\nfeatures = [\"derive\"]\n";
        assert!(!normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn target_scoped_dotted_normal_subtable_is_detected() {
        let manifest = "[target.'cfg(unix)'.dependencies.dli-syntax]\nworkspace = true\n";
        assert!(normal_deps_include_dli_syntax(manifest));
    }
}
