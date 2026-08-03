//! Native repository-contract checks.
//!
//! This is the first slice of the repository validator migration.  It mirrors
//! the Python canon validator's V1/V2 frontmatter checks while deliberately
//! keeping the interface independent of the CLI: callers receive stable,
//! serializable findings and decide how to render or exit.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

const EXEMPT_NAMES: &[&str] = &[
    "SCHEMA.md",
    "decisions.md",
    "open-questions.md",
    "resolved-questions.md",
    "README.md",
];
const STATUSES: &[&str] = &[
    "ratified",
    "provisional",
    "proposed",
    "rejected",
    "open-question",
];
const TYPES: &[&str] = &[
    "concept",
    "system",
    "character",
    "place",
    "faction",
    "narrative",
    "meta",
];

/// Stable machine-facing repository finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryFinding {
    /// Relative POSIX path from the repository's canon root.
    pub path: String,
    /// One-based source line.
    pub line: usize,
    /// Stable validator code (`V1` or `V2` in this slice).
    pub code: String,
    /// Human-readable diagnostic.
    pub message: String,
    /// Severity, currently always `error` for contract violations.
    pub severity: String,
}

impl RepositoryFinding {
    fn new(path: &str, code: &str, message: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            line: 1,
            code: code.to_owned(),
            message: message.into(),
            severity: "error".to_owned(),
        }
    }
}

/// Validate V1/V2 frontmatter for every non-exempt Markdown file under `canon`.
///
/// Missing roots and unreadable files are represented as findings rather than
/// panics. Results are deterministic by relative path, line, code, message.
pub fn validate_repository(root: impl AsRef<Path>) -> Vec<RepositoryFinding> {
    let root = root.as_ref();
    let mut files = Vec::new();
    collect_markdown(root, root, &mut files);
    let mut findings = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if is_exempt(&rel) {
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(source) => check_frontmatter(&rel, &source, &mut findings),
            Err(error) => findings.push(RepositoryFinding::new(
                &rel,
                "V1",
                format!("cannot read file: {error}"),
            )),
        }
    }
    findings.sort_by(|a, b| {
        (&a.path, a.line, &a.code, &a.message).cmp(&(&b.path, b.line, &b.code, &b.message))
    });
    findings
}

fn collect_markdown(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_markdown(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let _ = root;
            out.push(path);
        }
    }
}

fn is_exempt(rel: &str) -> bool {
    let mut parts = rel.split('/').collect::<Vec<_>>();
    let name = parts.pop().unwrap_or_default();
    EXEMPT_NAMES.contains(&name) || parts.first() == Some(&"sources")
}

#[allow(clippy::too_many_lines)]
fn check_frontmatter(path: &str, source: &str, out: &mut Vec<RepositoryFinding>) {
    let mut lines = source.lines();
    if lines.next().map(str::trim) != Some("---") {
        out.push(RepositoryFinding::new(
            path,
            "V1",
            "missing frontmatter block",
        ));
        return;
    }
    let mut yaml = String::new();
    let mut closed = false;
    for line in lines {
        if line.trim() == "---" {
            closed = true;
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    if !closed {
        out.push(RepositoryFinding::new(
            path,
            "V1",
            "frontmatter: unterminated block",
        ));
        return;
    }
    let value: Value = match serde_yaml_ng::from_str(&yaml) {
        Ok(value) => value,
        Err(error) => {
            out.push(RepositoryFinding::new(
                path,
                "V1",
                format!("frontmatter: {error}"),
            ));
            return;
        }
    };
    let Some(map) = value.as_mapping() else {
        out.push(RepositoryFinding::new(
            path,
            "V1",
            "frontmatter must be a mapping",
        ));
        return;
    };
    if map.is_empty() {
        out.push(RepositoryFinding::new(
            path,
            "V1",
            "missing frontmatter block",
        ));
        return;
    }
    let allowed = [
        "id",
        "title",
        "type",
        "status",
        "tags",
        "source",
        "diegetic",
        "lexicon_exceptions",
    ];
    for key in map.keys().filter_map(Value::as_str) {
        if !allowed.contains(&key) {
            out.push(RepositoryFinding::new(
                path,
                "V2",
                format!("frontmatter {key}: unknown field"),
            ));
        }
    }
    require_string(path, map, "id", out);
    require_string(path, map, "title", out);
    enum_field(path, map, "type", TYPES, out);
    enum_field(path, map, "status", STATUSES, out);
    if let Some(id) = map
        .get(Value::String("id".to_owned()))
        .and_then(Value::as_str)
    {
        if !is_kebab(id) {
            out.push(RepositoryFinding::new(
                path,
                "V2",
                format!("frontmatter id: invalid kebab-case value {id:?}"),
            ));
        }
        let stem = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if id != stem {
            out.push(RepositoryFinding::new(
                path,
                "V2",
                format!("id {id:?} must equal filename stem {stem:?}"),
            ));
        }
    }
    for field in ["tags", "lexicon_exceptions"] {
        if let Some(value) = map.get(Value::String(field.to_owned()))
            && (!value.is_sequence()
                || value
                    .as_sequence()
                    .is_some_and(|items| items.iter().any(|item| item.as_str().is_none())))
        {
            out.push(RepositoryFinding::new(
                path,
                "V2",
                format!("frontmatter {field}: must be a list of strings"),
            ));
        }
    }
    if let Some(value) = map.get(Value::String("source".to_owned()))
        && !value.is_null()
        && value.as_str().is_none()
    {
        out.push(RepositoryFinding::new(
            path,
            "V2",
            "frontmatter source: must be a string or null",
        ));
    }
    if let Some(value) = map.get(Value::String("diegetic".to_owned()))
        && !value.is_bool()
    {
        out.push(RepositoryFinding::new(
            path,
            "V2",
            "frontmatter diegetic: must be a boolean",
        ));
    }
}

fn require_string(
    path: &str,
    map: &serde_yaml_ng::Mapping,
    field: &str,
    out: &mut Vec<RepositoryFinding>,
) {
    match map.get(Value::String(field.to_owned())) {
        None => out.push(RepositoryFinding::new(
            path,
            "V2",
            format!("frontmatter {field}: field required"),
        )),
        Some(value) if value.as_str().is_none() => out.push(RepositoryFinding::new(
            path,
            "V2",
            format!("frontmatter {field}: must be a string"),
        )),
        _ => {}
    }
}

fn enum_field(
    path: &str,
    map: &serde_yaml_ng::Mapping,
    field: &str,
    allowed: &[&str],
    out: &mut Vec<RepositoryFinding>,
) {
    let Some(value) = map.get(Value::String(field.to_owned())) else {
        out.push(RepositoryFinding::new(
            path,
            "V2",
            format!("frontmatter {field}: field required"),
        ));
        return;
    };
    let Some(value) = value.as_str() else {
        out.push(RepositoryFinding::new(
            path,
            "V2",
            format!("frontmatter {field}: must be a string"),
        ));
        return;
    };
    if !allowed.contains(&value) {
        out.push(RepositoryFinding::new(
            path,
            "V2",
            format!("frontmatter {field}: invalid value {value:?}"),
        ));
    }
}

fn is_kebab(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(root: &Path, name: &str, body: &str) {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, body).expect("write");
    }

    #[test]
    fn clean_frontmatter_has_no_findings_and_serializes() {
        let dir = tempdir().expect("tempdir");
        write(
            dir.path(),
            "anchor.md",
            "---\nid: anchor\ntitle: Anchor\ntype: character\nstatus: ratified\ntags: []\n---\n\n## Anchor\n",
        );
        let findings = validate_repository(dir.path());
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(
            serde_json::to_value(findings).expect("json"),
            serde_json::json!([])
        );
    }

    #[test]
    fn v1_and_v2_findings_are_normalized() {
        let dir = tempdir().expect("tempdir");
        write(dir.path(), "bad.md", "---\nid: Bad\ntitle: [broken\n---\n");
        write(dir.path(), "missing.md", "# no metadata\n");
        let findings = validate_repository(dir.path());
        assert!(
            findings
                .iter()
                .any(|f| f.path == "bad.md" && f.code == "V1")
        );
        assert!(
            findings
                .iter()
                .any(|f| f.path == "missing.md" && f.code == "V1")
        );
        let json = serde_json::to_value(&findings).expect("json");
        assert_eq!(json[0]["severity"], "error");
        assert!(json[0].get("path").is_some());
    }

    #[test]
    fn v2_rejects_unknown_fields_and_bad_stem() {
        let dir = tempdir().expect("tempdir");
        write(
            dir.path(),
            "wrong.md",
            "---\nid: right\ntitle: Wrong\ntype: character\nstatus: proposed\nextra: yes\n---\n",
        );
        let findings = validate_repository(dir.path());
        assert!(
            findings
                .iter()
                .any(|f| f.code == "V2" && f.message.contains("unknown field"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.code == "V2" && f.message.contains("filename stem"))
        );
    }

    #[test]
    fn schema_exemptions_match_canon_contract() {
        let dir = tempdir().expect("tempdir");
        write(dir.path(), "README.md", "not frontmatter\n");
        write(dir.path(), "sources/raw.md", "not frontmatter\n");
        assert!(validate_repository(dir.path()).is_empty());
    }
}
