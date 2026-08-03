//! Native repository-contract checks.
//!
//! This is the first slice of the repository validator migration.  It mirrors
//! the Python canon validator's V1/V2 frontmatter checks while deliberately
//! keeping the interface independent of the CLI: callers receive stable,
//! serializable findings and decide how to render or exit.

use std::collections::{BTreeMap, BTreeSet};
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
        Self::at_line(path, 1, code, message)
    }

    fn at_line(path: &str, line: usize, code: &str, message: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            line,
            code: code.to_owned(),
            message: message.into(),
            severity: "error".to_owned(),
        }
    }
}

/// Validate V1/V2 frontmatter and V3 section markers for every non-exempt
/// Markdown file under `canon`.
///
/// Missing roots and unreadable files are represented as findings rather than
/// panics. Results are deterministic by relative path, line, code, message.
pub fn validate_repository(root: impl AsRef<Path>) -> Vec<RepositoryFinding> {
    let root = root.as_ref();
    let mut files = Vec::new();
    collect_markdown(root, root, &mut files);
    let mut findings = Vec::new();
    let (decision_ids, question_status) = load_ledger_ids(root, &mut findings);
    let index = build_id_index(&files, root, &mut findings);
    check_structured_data(root, &mut findings);
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
            Ok(source) => {
                check_frontmatter(&rel, &source, &mut findings);
                check_citations(
                    &rel,
                    &source,
                    &decision_ids,
                    &question_status,
                    &mut findings,
                );
                check_links(&rel, &source, &index, &mut findings);
            }
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

fn check_structured_data(root: &Path, findings: &mut Vec<RepositoryFinding>) {
    let data = root.join("data");
    check_timeline(&data.join("timeline.yaml"), "data/timeline.yaml", findings);
    check_characters(
        &data.join("characters.yaml"),
        "data/characters.yaml",
        findings,
    );
}

fn read_yaml(path: &Path, label: &str, findings: &mut Vec<RepositoryFinding>) -> Option<Value> {
    if !path.exists() {
        return None;
    }
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            findings.push(RepositoryFinding::new(
                label,
                "V6",
                format!("cannot read YAML: {error}"),
            ));
            return None;
        }
    };
    match serde_yaml_ng::from_str(&source) {
        Ok(value) => Some(value),
        Err(error) => {
            findings.push(RepositoryFinding::new(
                label,
                "V6",
                format!("invalid YAML: {error}"),
            ));
            None
        }
    }
}

fn check_timeline(path: &Path, label: &str, findings: &mut Vec<RepositoryFinding>) {
    let Some(value) = read_yaml(path, label, findings) else {
        return;
    };
    let rel = label.to_owned();
    let Some(map) = value.as_mapping() else {
        findings.push(RepositoryFinding::new(
            &rel,
            "V6",
            "timeline must be a mapping",
        ));
        return;
    };
    let eras = map.get(Value::String("eras".to_owned()));
    let Some(eras) = eras.and_then(Value::as_sequence) else {
        findings.push(RepositoryFinding::new(
            &rel,
            "V6",
            "timeline eras must be a list",
        ));
        return;
    };
    let era_ids: BTreeSet<&str> = eras.iter().filter_map(Value::as_str).collect();
    let Some(events) = map
        .get(Value::String("events".to_owned()))
        .and_then(Value::as_sequence)
    else {
        return;
    };
    let mut ids = BTreeSet::new();
    for event in events {
        let Some(event) = event.as_mapping() else {
            findings.push(RepositoryFinding::new(
                &rel,
                "V6",
                "timeline event must be a mapping",
            ));
            continue;
        };
        let id = event
            .get(Value::String("id".to_owned()))
            .and_then(Value::as_str);
        if id.is_none() {
            findings.push(RepositoryFinding::new(
                &rel,
                "V6",
                "timeline event requires id",
            ));
        }
        if let Some(id) = id
            && !ids.insert(id)
        {
            findings.push(RepositoryFinding::new(
                &rel,
                "V6",
                format!("duplicate timeline event id {id}"),
            ));
        }
        let Some(era) = event
            .get(Value::String("era".to_owned()))
            .and_then(Value::as_str)
        else {
            findings.push(RepositoryFinding::new(
                &rel,
                "V6",
                "timeline event requires era",
            ));
            continue;
        };
        if !era_ids.contains(era) {
            findings.push(RepositoryFinding::new(
                &rel,
                "V6",
                format!("timeline event cites unknown era {era}"),
            ));
        }
        check_status(event, &rel, "timeline event", findings);
    }
}

fn check_characters(path: &Path, label: &str, findings: &mut Vec<RepositoryFinding>) {
    let Some(value) = read_yaml(path, label, findings) else {
        return;
    };
    let rel = label.to_owned();
    let Some(map) = value.as_mapping() else {
        findings.push(RepositoryFinding::new(
            &rel,
            "V6",
            "characters must be a mapping",
        ));
        return;
    };
    let Some(characters) = map
        .get(Value::String("characters".to_owned()))
        .and_then(Value::as_sequence)
    else {
        findings.push(RepositoryFinding::new(
            &rel,
            "V6",
            "characters must contain a list",
        ));
        return;
    };
    let mut ids = BTreeSet::new();
    for character in characters {
        let Some(character) = character.as_mapping() else {
            findings.push(RepositoryFinding::new(
                &rel,
                "V6",
                "character entry must be a mapping",
            ));
            continue;
        };
        let id = character
            .get(Value::String("id".to_owned()))
            .and_then(Value::as_str);
        if id.is_none() {
            findings.push(RepositoryFinding::new(&rel, "V6", "character requires id"));
        }
        if let Some(id) = id
            && !ids.insert(id)
        {
            findings.push(RepositoryFinding::new(
                &rel,
                "V6",
                format!("duplicate character id {id}"),
            ));
        }
        for field in ["name", "role"] {
            if character
                .get(Value::String(field.to_owned()))
                .and_then(Value::as_str)
                .is_none()
            {
                findings.push(RepositoryFinding::new(
                    &rel,
                    "V6",
                    format!("character requires string {field}"),
                ));
            }
        }
        check_status(character, &rel, "character", findings);
    }
}

fn check_status(
    map: &serde_yaml_ng::Mapping,
    path: &str,
    label: &str,
    findings: &mut Vec<RepositoryFinding>,
) {
    let Some(status) = map
        .get(Value::String("status".to_owned()))
        .and_then(Value::as_str)
    else {
        findings.push(RepositoryFinding::new(
            path,
            "V6",
            format!("{label} requires status"),
        ));
        return;
    };
    if !STATUSES.contains(&status) {
        findings.push(RepositoryFinding::new(
            path,
            "V6",
            format!("{label} has invalid status {status:?}"),
        ));
    }
}

fn build_id_index(
    files: &[PathBuf],
    root: &Path,
    findings: &mut Vec<RepositoryFinding>,
) -> BTreeSet<String> {
    let mut index = BTreeSet::new();
    let mut seen = BTreeMap::<String, String>::new();
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if is_exempt(&rel) {
            continue;
        }
        let id = frontmatter_id(path).unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned()
        });
        if let Some(previous) = seen.insert(id.clone(), rel.clone()) {
            findings.push(RepositoryFinding::new(
                &rel,
                "V5",
                format!("duplicate id {id:?} (also in {previous})"),
            ));
        } else {
            index.insert(id);
        }
    }
    index
}

fn frontmatter_id(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    let mut lines = source.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut yaml = String::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    let value: Value = serde_yaml_ng::from_str(&yaml).ok()?;
    value
        .as_mapping()?
        .get(Value::String("id".to_owned()))?
        .as_str()
        .map(ToOwned::to_owned)
}

fn check_links(
    path: &str,
    source: &str,
    index: &BTreeSet<String>,
    findings: &mut Vec<RepositoryFinding>,
) {
    let lines: Vec<&str> = source.lines().collect();
    let mut in_frontmatter = false;
    let mut frontmatter_closed = false;
    let mut in_fence = false;
    for (line_no, line) in lines.iter().enumerate() {
        if line_no == 0 && line.trim() == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
                frontmatter_closed = true;
            }
            continue;
        }
        if !frontmatter_closed {
            continue;
        }
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut cursor = 0;
        while let Some(start_rel) = line[cursor..].find("[[") {
            let start = cursor + start_rel + 2;
            let Some(end_rel) = line[start..].find("]]") else {
                break;
            };
            let end = start + end_rel;
            let target = line[start..end]
                .split('|')
                .next()
                .unwrap_or_default()
                .trim();
            if !target.is_empty() && !index.contains(target) {
                findings.push(RepositoryFinding::at_line(
                    path,
                    line_no + 1,
                    "V5",
                    format!("[[{target}]] does not resolve in the id index"),
                ));
            }
            cursor = end + 2;
            if cursor >= line.len() {
                break;
            }
        }
    }
}

fn load_ledger_ids(
    root: &Path,
    findings: &mut Vec<RepositoryFinding>,
) -> (std::collections::BTreeSet<String>, BTreeMap<String, bool>) {
    let mut decisions = std::collections::BTreeSet::new();
    let mut questions = BTreeMap::new();
    let decision_path = root.join("decisions.md");
    if let Ok(source) = fs::read_to_string(&decision_path) {
        for (line_no, line) in source.lines().enumerate() {
            if let Some(id) = heading_id(line, "D-") {
                if !decisions.insert(id.clone()) {
                    findings.push(RepositoryFinding::new(
                        "decisions.md",
                        "V4",
                        format!("duplicate ledger id {id}"),
                    ));
                }
                let _ = line_no;
            }
        }
    }
    for name in ["open-questions.md", "resolved-questions.md"] {
        let path = root.join(name);
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let lines: Vec<&str> = source.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let Some(id) = heading_id(line, "Q-") else {
                continue;
            };
            let end = lines[index + 1..]
                .iter()
                .position(|candidate| candidate.starts_with("## "))
                .map_or(lines.len(), |offset| index + 1 + offset);
            let open = lines[index..end]
                .iter()
                .any(|candidate| candidate.trim() == "- status: open");
            questions.insert(id, open);
        }
    }
    (decisions, questions)
}

fn heading_id(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix("## ")?;
    let id = rest.split_whitespace().next()?;
    if id.starts_with(prefix) && valid_ref(id) {
        Some(id.to_owned())
    } else {
        None
    }
}

fn check_citations(
    path: &str,
    source: &str,
    decisions: &std::collections::BTreeSet<String>,
    questions: &BTreeMap<String, bool>,
    findings: &mut Vec<RepositoryFinding>,
) {
    for (line, heading, refs) in section_citations(source) {
        for reference in refs {
            if reference.starts_with("D-") && !decisions.contains(reference) {
                findings.push(RepositoryFinding::at_line(
                    path,
                    line,
                    "V4",
                    format!("section '{heading}' cites unknown {reference}"),
                ));
            }
            if reference.starts_with("Q-") {
                match questions.get(reference) {
                    None => findings.push(RepositoryFinding::at_line(
                        path,
                        line,
                        "V4",
                        format!("section '{heading}' cites unknown {reference}"),
                    )),
                    Some(true) => {}
                    Some(false) => findings.push(RepositoryFinding::at_line(
                        path,
                        line,
                        "V4",
                        format!("section '{heading}' cites {reference} which is not open"),
                    )),
                }
            }
        }
    }
}

fn section_citations(source: &str) -> Vec<(usize, String, Vec<&str>)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with("## ") || line.starts_with("### ") {
            continue;
        }
        let heading = line.trim_start_matches("## ").to_owned();
        let mut cursor = index + 1;
        while cursor < lines.len() && lines[cursor].trim().is_empty() {
            cursor += 1;
        }
        if cursor < lines.len()
            && let Some((_, refs)) = lines[cursor].split_once("status: ")
            && let Some(refs) = refs.split_once(' ').map(|(_, value)| value)
            && let Some(refs) = refs.strip_prefix('(').and_then(|v| v.strip_suffix(')'))
        {
            out.push((index + 1, heading, refs.split(", ").collect()));
        }
    }
    out
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
    check_sections(path, source, out);
}

fn check_sections(path: &str, source: &str, out: &mut Vec<RepositoryFinding>) {
    let mut in_frontmatter = false;
    let mut frontmatter_closed = false;
    let lines: Vec<&str> = source.lines().collect();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if index == 0 && line.trim() == "---" {
            in_frontmatter = true;
            index += 1;
            continue;
        }
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
                frontmatter_closed = true;
            }
            index += 1;
            continue;
        }
        if !frontmatter_closed || !line.starts_with("## ") || line.starts_with("### ") {
            index += 1;
            continue;
        }
        let heading = line.trim_start_matches("## ");
        let mut marker = None;
        let mut cursor = index + 1;
        while cursor < lines.len() {
            let candidate = lines[cursor];
            if candidate.starts_with("## ") {
                break;
            }
            if !candidate.trim().is_empty() {
                marker = Some((candidate, cursor + 1));
                break;
            }
            cursor += 1;
        }
        match marker
            .and_then(|(value, line)| parse_status_marker(value).map(|parsed| (parsed, line)))
        {
            None => out.push(RepositoryFinding::at_line(
                path,
                index + 1,
                "V3",
                format!("section '{heading}' has no well-formed status marker"),
            )),
            Some(((status, citations), _marker_line)) => {
                check_marker_shape(path, index + 1, heading, status, &citations, out);
            }
        }
        index += 1;
    }
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with("### ") {
            continue;
        }
        let heading = line.trim_start_matches("### ");
        let mut cursor = index + 1;
        while cursor < lines.len() && lines[cursor].trim().is_empty() {
            cursor += 1;
        }
        let Some(candidate) = lines.get(cursor).copied() else {
            continue;
        };
        if !candidate.trim_start().starts_with("> status:") {
            continue;
        }
        match parse_status_marker(candidate) {
            Some((status, citations)) => {
                check_marker_shape(path, index + 1, heading, status, &citations, out);
            }
            None => out.push(RepositoryFinding::at_line(
                path,
                index + 1,
                "V3",
                format!("subsection '{heading}' has a malformed status override"),
            )),
        }
    }
}

fn check_marker_shape(
    path: &str,
    line: usize,
    heading: &str,
    status: &str,
    citations: &[&str],
    out: &mut Vec<RepositoryFinding>,
) {
    let d_refs = citations
        .iter()
        .filter(|ref_id| ref_id.starts_with("D-"))
        .count();
    let q_refs = citations
        .iter()
        .filter(|ref_id| ref_id.starts_with("Q-"))
        .count();
    let valid = match status {
        "ratified" | "provisional" | "rejected" => d_refs > 0 && q_refs == 0,
        "open-question" => d_refs == 0 && q_refs == 1,
        "proposed" => citations.is_empty(),
        _ => false,
    };
    if !valid {
        let requirement = match status {
            "open-question" => "open-question needs one Q-ref",
            "proposed" => "proposed must cite nothing",
            _ => "status needs >=1 D-ref",
        };
        out.push(RepositoryFinding::at_line(
            path,
            line,
            "V3",
            format!("section '{heading}' {requirement}"),
        ));
    }
}

fn parse_status_marker(line: &str) -> Option<(&str, Vec<&str>)> {
    let rest = line.strip_prefix("> status: ")?;
    let (status, citation_text) = match rest.split_once(' ') {
        Some((status, refs)) => (status, Some(refs)),
        None => (rest, None),
    };
    if !STATUSES.contains(&status) {
        return None;
    }
    let citations = match citation_text {
        None => Vec::new(),
        Some(refs) => {
            let refs = refs.strip_prefix('(')?.strip_suffix(')')?;
            if refs.is_empty() {
                return None;
            }
            let values: Vec<&str> = refs.split(", ").collect();
            if values.iter().any(|value| !valid_ref(value)) {
                return None;
            }
            values
        }
    };
    Some((status, citations))
}

fn valid_ref(value: &str) -> bool {
    (value.starts_with("D-") || value.starts_with("Q-"))
        && value.len() == 6
        && value[2..].chars().all(|ch| ch.is_ascii_digit())
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
            "---\nid: anchor\ntitle: Anchor\ntype: character\nstatus: ratified\ntags: []\n---\n\n## Anchor\n> status: ratified (D-0001)\n",
        );
        write(
            dir.path(),
            "decisions.md",
            "## D-0001 — test\n- status: ratified\n",
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
    fn v3_enforces_marker_citation_shape() {
        let dir = tempdir().expect("tempdir");
        write(
            dir.path(),
            "markers.md",
            "---\nid: markers\ntitle: Markers\ntype: narrative\nstatus: proposed\n---\n\n## Ratified\n> status: ratified (Q-0001)\n\n### Override\n> status: ratified (Q-0001)\n\n## Question\n> status: open-question (D-0001)\n\n### Broken override\n> status: propsed\n",
        );
        let findings = validate_repository(dir.path());
        assert_eq!(findings.iter().filter(|f| f.code == "V3").count(), 4);
        assert!(
            findings
                .iter()
                .filter(|f| f.code == "V3")
                .all(|f| f.line > 1)
        );
    }

    #[test]
    fn v5_resolves_links_and_rejects_duplicate_ids() {
        let dir = tempdir().expect("tempdir");
        write(
            dir.path(),
            "one.md",
            "---\nid: one\ntitle: One\ntype: place\nstatus: proposed\n---\n\n## One\n> status: proposed\nSee [[two]] and [[missing]].\n",
        );
        write(
            dir.path(),
            "two.md",
            "---\nid: one\ntitle: Two\ntype: place\nstatus: proposed\n---\n\n## Two\n> status: proposed\n",
        );
        let findings = validate_repository(dir.path());
        assert!(
            findings
                .iter()
                .any(|f| f.code == "V5" && f.message.contains("duplicate id"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.code == "V5" && f.message.contains("missing"))
        );
    }

    #[test]
    fn v6_validates_structured_timeline_and_characters() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("data")).expect("data");
        write(
            dir.path(),
            "data/timeline.yaml",
            "eras: [before]\nevents:\n  - id: arrival\n    name: Arrival\n    era: absent\n    status: unknown\n",
        );
        write(
            dir.path(),
            "data/characters.yaml",
            "characters:\n  - id: anchor\n    name: Anchor\n    role: person\n    status: proposed\n  - id: anchor\n    name: Duplicate\n    role: person\n    status: proposed\n",
        );
        let findings = validate_repository(dir.path());
        assert!(
            findings
                .iter()
                .any(|f| f.code == "V6" && f.message.contains("unknown era"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.code == "V6" && f.message.contains("invalid status"))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.code == "V6" && f.message.contains("duplicate character"))
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
