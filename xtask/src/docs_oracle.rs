//! The docs oracle: every fenced example in the docs is machine-checked.
//!
//! Two front ends, one law. Fenced `dli` blocks run through the real parser
//! ([`dli_syntax::parse_document`]); fenced `json` blocks run through
//! `serde_json` and, when tagged, through the LS-1 stored-object schemas
//! ([`dli_lorestore::schemas`]). A block that produces any error-class
//! diagnostic — a parse error, a schema violation, a malformed transcript —
//! fails the gate, so documentation cannot rot into showing forms the system
//! no longer accepts (spec `01-syntax.md` §7).
//!
//! The `json` path is the lorestore pivot's re-aiming of the same law
//! (DL-0041, roadmap §LS-7): as `.dli` recedes from the canonical surface, the
//! stored fact objects become what the docs teach, and they are checked
//! against the schema that defines them. Nothing is silently unchecked —
//! **every** fenced `json` block falls into one of three classes, and the
//! class is written in the fence itself:
//!
//! | fence | check |
//! |---|---|
//! | ` ```json <schema-name> ` | parses **and** validates against that LS-1 schema |
//! | ` ```json ` | parses as well-formed JSON |
//! | ` ```json transcript ` | an MCP request/response sketch: `→`/`←`-led segments, each bracket-balanced |
//!
//! `<schema-name>` is one of the names [`dli_lorestore::schemas`] publishes —
//! `fact-object-v1`, `commit-v1`, `ref-v1`, `evidence-envelope-v1`,
//! `lineage-edge-v1`. An unrecognized tag is a hard failure rather than a
//! silent downgrade, so a typo can never quietly disable the check.
//!
//! The `transcript` class exists because the wire-protocol examples are
//! deliberately elided sketches (`{…}`, `"…": "…"`) prefixed with `→`/`←`
//! markers, and are not JSON documents at all. Marking them is a visible,
//! greppable decision, and the marker is itself checked: a `transcript` block
//! with no arrow markers, or with content before its first marker, fails. It
//! cannot be used as a blanket escape hatch for arbitrary JSON.

use std::path::Path;

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

const MIN_SKILL_BLOCKS: usize = 3;
const MIN_TOTAL_UNITS: usize = 5;
/// The schema path must never quietly become a no-op: at least this many
/// fenced `json` blocks must validate against an LS-1 stored-object schema.
const MIN_SCHEMA_BLOCKS: usize = 3;

/// The fence tag that marks an elided MCP request/response sketch.
const TRANSCRIPT_TAG: &str = "transcript";

/// One fenced code block, with the source position of its opening fence.
struct Fence {
    /// `path:line` of the opening fence — a clickable label for diagnostics.
    label: String,
    /// The first word of the info string (`dli`, `json`, `text`, …).
    lang: String,
    /// The remainder of the info string, if any — the check class for `json`.
    tag: Option<String>,
    /// The block's contents, newline-terminated.
    body: String,
}

/// How a fenced `json` block is checked, decided by its fence tag.
enum JsonCheck {
    /// ` ```json <name> ` — parse, then validate against that LS-1 schema.
    Schema(String),
    /// ` ```json ` — parse; the block must be a well-formed JSON document.
    WellFormed,
    /// ` ```json transcript ` — an arrow-marked, elided wire sketch.
    Transcript,
}

/// Extract every fenced block from every markdown source, then probe each one
/// through the front end its fence names.
pub fn run_docs_oracle(root: &Path) -> Result<()> {
    let sources = markdown_sources(root)?;

    let mut dli_units: Vec<(String, String)> = Vec::new();
    let mut json_blocks: Vec<(String, JsonCheck, String)> = Vec::new();
    let schemas = stored_object_schemas()?;
    let mut skill_dli = 0usize;
    let mut tag_failures: Vec<String> = Vec::new();

    for (origin, text) in &sources {
        let mut dli_index = 0usize;
        for fence in extract_fences(origin, text) {
            match fence.lang.as_str() {
                "dli" => {
                    dli_index += 1;
                    if origin == "SKILL.md" {
                        skill_dli += 1;
                    }
                    dli_units.push((format!("{origin} #{dli_index}"), fence.body));
                }
                "json" => match classify_json(fence.tag.as_deref(), &schemas) {
                    Ok(check) => json_blocks.push((fence.label, check, fence.body)),
                    Err(message) => tag_failures.push(format!("{}: {message}", fence.label)),
                },
                _ => {}
            }
        }
    }

    if !tag_failures.is_empty() {
        bail!(
            "docs-oracle: {} json fence tag(s) are not a recognized check class:\n{}",
            tag_failures.len(),
            tag_failures.join("\n")
        );
    }

    if skill_dli < MIN_SKILL_BLOCKS {
        bail!("docs-oracle: SKILL.md has {skill_dli} dli blocks, need at least {MIN_SKILL_BLOCKS}");
    }

    for path in example_paths(root) {
        let text = std::fs::read_to_string(&path)?;
        dli_units.push((path.display().to_string(), text));
    }

    if dli_units.len() < MIN_TOTAL_UNITS {
        bail!(
            "docs-oracle: {} units, need at least {MIN_TOTAL_UNITS}",
            dli_units.len()
        );
    }

    check_dli_units(&dli_units)?;

    let census = check_json_blocks(&json_blocks, &schemas)?;
    if census.schema < MIN_SCHEMA_BLOCKS {
        bail!(
            "docs-oracle: {} schema-tagged json block(s), need at least {MIN_SCHEMA_BLOCKS} \
             — the LS-1 schema path must stay exercised",
            census.schema
        );
    }

    println!(
        "docs-oracle: {} dli units parse without errors; {} json blocks checked \
         ({} schema-validated, {} well-formed, {} transcripts)",
        dli_units.len(),
        json_blocks.len(),
        census.schema,
        census.well_formed,
        census.transcript
    );
    Ok(())
}

/// Every markdown source the oracle reads: `SKILL.md` plus `docs/**/*.md`,
/// each paired with the origin label its diagnostics carry.
fn markdown_sources(root: &Path) -> Result<Vec<(String, String)>> {
    let mut sources: Vec<(String, String)> = Vec::new();

    let skill_path = root.join("SKILL.md");
    let skill_text = std::fs::read_to_string(&skill_path)
        .with_context(|| format!("reading {}", skill_path.display()))?;
    sources.push(("SKILL.md".to_owned(), skill_text));

    let docs_dir = root.join("docs");
    if docs_dir.is_dir() {
        let mut doc_paths: Vec<_> = WalkDir::new(&docs_dir)
            .into_iter()
            .filter_map(Result::ok)
            .map(walkdir::DirEntry::into_path)
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
            .collect();
        doc_paths.sort();
        for path in doc_paths {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let label = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            sources.push((label, text));
        }
    }
    Ok(sources)
}

/// The committed `.dli` example files, in a stable order.
fn example_paths(root: &Path) -> Vec<std::path::PathBuf> {
    let examples_dir = root.join("examples");
    if !examples_dir.is_dir() {
        return Vec::new();
    }
    let mut paths: Vec<_> = WalkDir::new(&examples_dir)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("dli"))
        .collect();
    paths.sort();
    paths
}

/// Run every `.dli` unit through the real front end.
///
/// The gate fails on hard errors only. Advisory warnings (e.g. a marker-less
/// claim, koan 5's proposed-first authoring) are surfaced to authors by the
/// parser but never block the build.
fn check_dli_units(units: &[(String, String)]) -> Result<()> {
    let mut failures: Vec<String> = Vec::new();
    for (label, content) in units {
        let (_document, diagnostics) = dli_syntax::parse_document(content);
        for diag in diagnostics {
            if diag.class.is_error() {
                failures.push(format!("{label}: [{}] {}", diag.class.code(), diag.message));
            }
        }
    }
    if !failures.is_empty() {
        bail!(
            "docs-oracle: the parser rejected {} line(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
    Ok(())
}

/// How many json blocks were checked in each class.
#[derive(Default)]
struct JsonCensus {
    schema: usize,
    well_formed: usize,
    transcript: usize,
}

/// The stored-object schemas [`dli_lorestore::schemas`] publishes, each paired
/// with its name and rendered as a `serde_json::Value` for the validator.
fn stored_object_schemas() -> Result<Vec<(String, serde_json::Value)>> {
    dli_lorestore::schemas()
        .into_iter()
        .map(|(name, schema)| {
            let value = serde_json::to_value(&schema)
                .with_context(|| format!("rendering the `{name}` schema as JSON"))?;
            Ok((name.to_owned(), value))
        })
        .collect()
}

/// Decide which check a fenced `json` block gets from its fence tag.
fn classify_json(
    tag: Option<&str>,
    schemas: &[(String, serde_json::Value)],
) -> Result<JsonCheck, String> {
    let Some(tag) = tag else {
        return Ok(JsonCheck::WellFormed);
    };
    if tag == TRANSCRIPT_TAG {
        return Ok(JsonCheck::Transcript);
    }
    if schemas.iter().any(|(name, _)| name == tag) {
        return Ok(JsonCheck::Schema(tag.to_owned()));
    }
    let known: Vec<&str> = schemas
        .iter()
        .map(|(name, _)| name.as_str())
        .chain(std::iter::once(TRANSCRIPT_TAG))
        .collect();
    Err(format!(
        "`json {tag}` names no check class; use a bare ```json fence, or one of: {}",
        known.join(", ")
    ))
}

/// Run every fenced `json` block through its class's check, collecting all
/// failures into one report.
fn check_json_blocks(
    blocks: &[(String, JsonCheck, String)],
    schemas: &[(String, serde_json::Value)],
) -> Result<JsonCensus> {
    let mut census = JsonCensus::default();
    let mut failures: Vec<String> = Vec::new();
    for (label, check, body) in blocks {
        let outcome = match check {
            JsonCheck::Schema(name) => {
                census.schema += 1;
                check_against_schema(name, body, schemas)
            }
            JsonCheck::WellFormed => {
                census.well_formed += 1;
                serde_json::from_str::<serde_json::Value>(body)
                    .map(|_| ())
                    .map_err(|error| {
                        format!(
                            "is not well-formed JSON: {error}\n  \
                             (an elided wire sketch belongs in a ```json {TRANSCRIPT_TAG} fence)"
                        )
                    })
            }
            JsonCheck::Transcript => {
                census.transcript += 1;
                check_transcript(body)
            }
        };
        if let Err(message) = outcome {
            failures.push(format!("{label}: {message}"));
        }
    }
    if !failures.is_empty() {
        bail!(
            "docs-oracle: {} fenced json block(s) failed their check:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
    Ok(census)
}

/// Parse a block and validate it against the named LS-1 stored-object schema.
fn check_against_schema(
    name: &str,
    body: &str,
    schemas: &[(String, serde_json::Value)],
) -> Result<(), String> {
    let instance: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("is not well-formed JSON: {error}"))?;
    let schema = schemas
        .iter()
        .find(|(schema_name, _)| schema_name == name)
        .map(|(_, schema)| schema)
        .ok_or_else(|| format!("no schema named `{name}`"))?;
    crate::jsonval::validate_json(schema, &instance)
        .map_err(|error| format!("does not validate against `{name}`: {error}"))
}

/// Check an elided wire sketch: the block is a sequence of `→`/`←`-led
/// segments, each bracket-balanced outside string literals.
///
/// This is deliberately weaker than a JSON parse — the segments carry `…`
/// elisions and are not JSON documents. What it buys is that the tag cannot
/// hide a plain JSON document from the parser (a `transcript` block with no
/// markers fails), and that a truncated or unbalanced sketch is caught.
fn check_transcript(body: &str) -> Result<(), String> {
    let mut segments = 0usize;
    let mut current = String::new();
    let mut started = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('→') || trimmed.starts_with('←') {
            if started && !balanced(&current) {
                return Err(format!(
                    "transcript segment {segments} is not bracket-balanced"
                ));
            }
            segments += 1;
            started = true;
            current.clear();
            current.push_str(trimmed.trim_start_matches(['→', '←']));
        } else if started {
            current.push('\n');
            current.push_str(line);
        } else if !trimmed.is_empty() {
            return Err(format!(
                "transcript block has content before its first → / ← marker; \
                 drop the `{TRANSCRIPT_TAG}` tag if this is a plain JSON document"
            ));
        }
    }
    if segments == 0 {
        return Err(format!(
            "a ```json {TRANSCRIPT_TAG} block must carry at least one → / ← marker; \
             drop the tag if this is a plain JSON document"
        ));
    }
    if !balanced(&current) {
        return Err(format!(
            "transcript segment {segments} is not bracket-balanced"
        ));
    }
    Ok(())
}

/// Whether braces and brackets nest and close, ignoring string literals.
fn balanced(text: &str) -> bool {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for ch in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0 && !in_string
}

/// Scan `text` for fenced code blocks, tagging each with `origin:line`.
///
/// Every fence opens a block, whatever its info string, so a `json` example
/// nested inside a `text` block cannot be mistaken for an example in its own
/// right. An unclosed final fence is dropped, as before.
fn extract_fences(origin: &str, text: &str) -> Vec<Fence> {
    let mut fences: Vec<Fence> = Vec::new();
    let mut current = String::new();
    let mut open: Option<(usize, String, Option<String>)> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some((start, lang, tag)) = open.take() {
            if trimmed.starts_with("```") {
                fences.push(Fence {
                    label: format!("{origin}:{start}"),
                    lang,
                    tag,
                    body: std::mem::take(&mut current),
                });
            } else {
                current.push_str(line);
                current.push('\n');
                open = Some((start, lang, tag));
            }
        } else if let Some(info) = trimmed.strip_prefix("```") {
            let mut words = info.split_whitespace();
            let lang = words.next().unwrap_or_default().to_owned();
            let rest = words.collect::<Vec<_>>().join(" ");
            let tag = (!rest.is_empty()).then_some(rest);
            open = Some((index + 1, lang, tag));
            current.clear();
        }
    }
    fences
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{
        JsonCheck, balanced, check_against_schema, check_transcript, classify_json, extract_fences,
        stored_object_schemas,
    };

    /// A fact object exactly as `dli_lorestore` serializes one.
    const FACT: &str = r#"{
      "schema_version": 2,
      "authority": null,
      "claim_id": "sha256:4fcac73d",
      "atom": { "pred": "located_in", "args": ["tegwen", "gwern"] },
      "support": { "monomials": [["+0"]] },
      "defeaters": { "monomials": [] },
      "grade": "asserted"
    }"#;

    #[test]
    fn schema_tag_validates_a_real_fact_object() {
        let schemas = stored_object_schemas().unwrap();
        check_against_schema("fact-object-v1", FACT, &schemas).unwrap();
    }

    #[test]
    fn schema_tag_rejects_a_drifted_fact_object() {
        let schemas = stored_object_schemas().unwrap();
        // A `parent` pointer inside a fact is exactly the shape the three-way
        // identity split forbids; `additionalProperties: false` catches it.
        let drifted = FACT.replace(
            r#""grade": "asserted""#,
            r#""grade": "asserted", "parent": "sha256:beef""#,
        );
        assert!(check_against_schema("fact-object-v1", &drifted, &schemas).is_err());
        // A grade outside the lattice.
        let bad_grade = FACT.replace(r#""asserted""#, r#""probably""#);
        assert!(check_against_schema("fact-object-v1", &bad_grade, &schemas).is_err());
        // A missing required field.
        let missing = FACT.replace(r#""schema_version": 2,"#, "");
        assert!(check_against_schema("fact-object-v1", &missing, &schemas).is_err());
    }

    #[test]
    fn every_published_schema_name_is_a_usable_tag() {
        let schemas = stored_object_schemas().unwrap();
        assert!(!schemas.is_empty());
        for (name, _) in &schemas {
            assert!(matches!(
                classify_json(Some(name), &schemas),
                Ok(JsonCheck::Schema(_))
            ));
        }
    }

    #[test]
    fn an_unknown_tag_fails_rather_than_downgrading() {
        let schemas = stored_object_schemas().unwrap();
        // A typo must not silently fall back to the well-formed-only check.
        assert!(classify_json(Some("fact-object-v2"), &schemas).is_err());
        assert!(matches!(
            classify_json(None, &schemas),
            Ok(JsonCheck::WellFormed)
        ));
        assert!(matches!(
            classify_json(Some("transcript"), &schemas),
            Ok(JsonCheck::Transcript)
        ));
    }

    #[test]
    fn transcript_tag_is_not_an_escape_hatch() {
        // Arrow-marked sketches pass, elisions and all.
        check_transcript("→ {\"op\": \"author.open\"}\n← {\"ok\": true, \"result\": {…}}\n")
            .unwrap();
        // A plain JSON document wearing the tag does not.
        assert!(check_transcript("{\"ok\": true}\n").is_err());
        assert!(check_transcript("\n").is_err());
        // Nor does a truncated segment.
        assert!(check_transcript("→ {\"op\": \"author.open\"\n").is_err());
    }

    #[test]
    fn balance_ignores_brackets_inside_strings() {
        assert!(balanced(r#"{"a": "}{", "b": ["]"]}"#));
        assert!(balanced(r#"{"a": "\""}"#));
        assert!(!balanced(r#"{"a": 1"#));
        assert!(!balanced(r#"{"a": "unterminated}"#));
    }

    #[test]
    fn fences_carry_their_language_tag_and_line() {
        let text = "prose\n```json fact-object-v1\n{}\n```\n\n```dli\nscope x\n```\n";
        let fences = extract_fences("doc.md", text);
        assert_eq!(fences.len(), 2);
        assert_eq!(fences[0].label, "doc.md:2");
        assert_eq!(fences[0].lang, "json");
        assert_eq!(fences[0].tag.as_deref(), Some("fact-object-v1"));
        assert_eq!(fences[1].lang, "dli");
        assert_eq!(fences[1].tag, None);
    }

    #[test]
    fn a_nested_fence_is_not_a_second_example() {
        // A `json` fence shown *inside* a `text` block is documentation about
        // fences, not an example to check.
        let text = "```text\n```json\n{ not json\n```\n";
        let fences = extract_fences("doc.md", text);
        assert_eq!(fences.len(), 1);
        assert_eq!(fences[0].lang, "text");
    }
}
