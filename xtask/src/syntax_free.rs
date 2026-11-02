//! Verify that the kernel and optional parser-free builds stay independent of
//! the `.dli` parser. The shipped default builds include `syntax`; this lane
//! deliberately checks the artifacts with `--no-default-features`.
//!
//! The manifest ratchet covers the core crates, and `cargo tree` checks the
//! complete dependency graphs of the optional parser-free artifacts.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The engine crates whose default build LS-9 retired the parser from.
const TARGET_CRATES: &[&str] = &["dli-world", "dli-ledger", "dli-time"];

/// The shipped artifacts whose parser-free dependency graph must be free of
/// `dli-syntax` — the LS-7 acceptance oracle.
const DEFAULT_BUILD_ROOTS: &[&str] = &["dli-serve", "deloreis"];

/// Target crates that still declare `dli-syntax` as a non-optional normal
/// `[dependencies]` entry — tracked LS-9 debt. The acceptance bar is met when
/// this is empty, and it is: LS-9 retired every expert-tier declaration parser,
/// and LS-7 made the one remaining load-boundary parse in `dli-world`
/// (`payload.rs`, a payload claim's surface `atom` string) reachable only
/// through the off-by-default `syntax` feature.
const STILL_COUPLED: &[&str] = &[];

/// Run the no-parse gate over the workspace at `root`: the LS-9 manifest
/// ratchet, then the LS-7 default-graph oracle.
pub fn run_syntax_free(root: &Path) -> Result<()> {
    run_manifest_ratchet(root)?;
    run_default_graph_oracle(root)
}

/// With default features disabled, neither shipped artifact's
/// normal dependency graph may contain `dli-syntax`.
///
/// `cargo tree -e normal` is the whole truth — it resolves the real graph rather
/// than reading one manifest at a time, so a parser pulled in three crates down
/// is caught. `-p <root>` alone would let cargo unify features across every
/// workspace member (`xtask` enables `deloreis/syntax` for the docgen step), so
/// the query runs against the root's own manifest, which selects that package
/// and nothing else.
fn run_default_graph_oracle(root: &Path) -> Result<()> {
    for artifact in DEFAULT_BUILD_ROOTS {
        let manifest = root.join("crates").join(artifact).join("Cargo.toml");
        let output = Command::new("cargo")
            .arg("tree")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(&manifest)
            .args(["--edges", "normal", "--no-default-features"])
            .output()
            .with_context(|| format!("running cargo tree for {artifact}"))?;
        if !output.status.success() {
            bail!(
                "parser-free graph oracle: `cargo tree` failed for {artifact}:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let tree = String::from_utf8_lossy(&output.stdout);
        let hits: Vec<&str> = tree
            .lines()
            .filter(|line| line.contains("dli-syntax"))
            .collect();
        if !hits.is_empty() {
            bail!(
                "parser-free graph oracle: the parser-free build of `{artifact}` links the parser — \
                 `cargo tree --edges normal` names dli-syntax at:\n{}\n\
                 The no-default-features build must exclude the parser. Put the edge behind the crate's \
                 `syntax` feature (`dli-syntax = {{ workspace = true, optional = \
                 true }}`) and `#[cfg(feature = \"syntax\")]` the call sites.",
                hits.join("\n")
            );
        }
    }
    println!(
        "parser-free graph oracle: {} shipped artifact(s) resolve with no `dli-syntax` when \
         default features are disabled.",
        DEFAULT_BUILD_ROOTS.len()
    );
    Ok(())
}

/// The LS-9 manifest ratchet over [`TARGET_CRATES`].
fn run_manifest_ratchet(root: &Path) -> Result<()> {
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
/// manifest names `dli-syntax` **unconditionally**, in either form TOML allows:
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
///
/// An entry marked `optional = true` does **not** count. An optional dependency
/// links only when a feature turns it on, and LS-7's features are all
/// off-by-default, so such an entry is not on the default build path — which is
/// the only thing this gate is about. The whole-graph oracle
/// ([`run_default_graph_oracle`]) is what proves those features stay off.
fn normal_deps_include_dli_syntax(manifest: &str) -> bool {
    let mut scanning_normal_body = false;
    let mut in_dli_syntax_subtable = false;
    let mut subtable_is_optional = false;
    let mut saw_unconditional_subtable = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_dli_syntax_subtable && !subtable_is_optional {
                saw_unconditional_subtable = true;
            }
            let Some(segments) = table_header_segments(trimmed) else {
                scanning_normal_body = false;
                in_dli_syntax_subtable = false;
                continue;
            };
            let is_normal = segments.iter().any(|s| s == "dependencies");
            // Dotted-sub-table form: the header itself names the dependency, e.g.
            // `[dependencies.dli-syntax]` — this is the hole the flat scan missed.
            // Its `optional` flag lives in the body, so the verdict waits for the
            // next header (or end of file).
            in_dli_syntax_subtable = is_normal && segments.iter().any(|s| s == "dli-syntax");
            subtable_is_optional = false;
            // Only a table whose final segment is `dependencies` carries ordinary
            // `key = value` dependency rows worth scanning (a `[dependencies.foo]`
            // body holds `foo`'s fields, not sibling dependencies).
            scanning_normal_body =
                is_normal && segments.last().map(String::as_str) == Some("dependencies");
            continue;
        }
        if in_dli_syntax_subtable && declares_optional(trimmed) {
            subtable_is_optional = true;
        }
        if scanning_normal_body && line_declares_dli_syntax(trimmed) && !declares_optional(trimmed)
        {
            return true;
        }
    }
    saw_unconditional_subtable || (in_dli_syntax_subtable && !subtable_is_optional)
}

/// Whether a manifest line carries `optional = true`, ignoring comments. Covers
/// both the inline-table row (`dli-syntax = { …, optional = true }`) and the
/// sub-table body row (`optional = true` under `[dependencies.dli-syntax]`).
fn declares_optional(line: &str) -> bool {
    if line.starts_with('#') {
        return false;
    }
    let Some(rest) = line.split("optional").nth(1) else {
        return false;
    };
    rest.trim_start()
        .strip_prefix('=')
        .is_some_and(|value| value.trim_start().starts_with("true"))
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

    #[test]
    fn optional_inline_dependency_is_not_the_default_path() {
        // The LS-7 shape: reachable only through an off-by-default feature.
        let manifest = "[features]\nsyntax = [\"dep:dli-syntax\"]\n\n[dependencies]\n\
                        dli-syntax = { workspace = true, optional = true }\n";
        assert!(!normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn optional_dotted_subtable_is_not_the_default_path() {
        let manifest = "[dependencies.dli-syntax]\nworkspace = true\noptional = true\n";
        assert!(!normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn dotted_subtable_without_optional_is_still_detected() {
        // The `optional` verdict is decided at the *end* of the sub-table body,
        // so a body that never says `optional` must still count.
        let manifest = "[dependencies.dli-syntax]\nworkspace = true\n\n[lints]\nworkspace = true\n";
        assert!(normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn optional_false_is_still_the_default_path() {
        let manifest = "[dependencies]\ndli-syntax = { workspace = true, optional = false }\n";
        assert!(normal_deps_include_dli_syntax(manifest));
    }

    #[test]
    fn optional_on_a_sibling_dependency_does_not_excuse_dli_syntax() {
        let manifest = "[dependencies]\nserde = { version = \"1\", optional = true }\n\
                        dli-syntax.workspace = true\n";
        assert!(normal_deps_include_dli_syntax(manifest));
    }
}
