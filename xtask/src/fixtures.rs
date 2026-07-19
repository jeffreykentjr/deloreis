//! The closed fixture registry check.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use walkdir::WalkDir;

use dli_serve::ErrorCode;

use crate::jsonval::validate_json;

/// Verify the fixture registry against `contract_dir`, validating JSON fixtures
/// against the emitted schemas in `schemas_dir`. With `update_digests`, rewrite
/// the registry's `sha256` fields instead of checking them.
pub fn run_fixtures(contract_dir: &Path, schemas_dir: &Path, update_digests: bool) -> Result<()> {
    let registry_path = contract_dir.join("registry.yaml");
    let text = std::fs::read_to_string(&registry_path)
        .with_context(|| format!("reading {}", registry_path.display()))?;
    let registry: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).context("parsing registry.yaml")?;
    let fixtures_dir = contract_dir.join("fixtures");

    let entries = registry
        .get("fixtures")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .ok_or_else(|| anyhow!("registry.yaml is missing a `fixtures` list"))?;

    let mut registered: BTreeSet<String> = BTreeSet::new();
    for entry in entries {
        let path = entry
            .get("path")
            .and_then(serde_yaml_ng::Value::as_str)
            .ok_or_else(|| anyhow!("a fixture entry is missing `path`"))?;
        registered.insert(path.to_owned());
    }

    let found = discover_fixture_files(&fixtures_dir);

    if update_digests {
        update_registry_digests(&registry_path, &text, &fixtures_dir)?;
        println!("fixtures: updated digests in {}", registry_path.display());
        return Ok(());
    }

    let unknown: Vec<String> = found.difference(&registered).cloned().collect();
    if !unknown.is_empty() {
        bail!(
            "fixtures: files under fixtures/ are not in the registry: {}",
            unknown.join(", ")
        );
    }
    let missing: Vec<String> = registered.difference(&found).cloned().collect();
    if !missing.is_empty() {
        bail!(
            "fixtures: registry lists files that do not exist: {}",
            missing.join(", ")
        );
    }

    for entry in entries {
        let path = entry
            .get("path")
            .and_then(serde_yaml_ng::Value::as_str)
            .expect("path present (checked above)");
        let want = entry
            .get("sha256")
            .and_then(serde_yaml_ng::Value::as_str)
            .ok_or_else(|| anyhow!("fixture {path} is missing `sha256`"))?;
        let bytes = std::fs::read(fixtures_dir.join(path))
            .with_context(|| format!("reading fixture {path}"))?;
        let got = hex_digest(&bytes);
        if got != want {
            bail!("fixtures: digest mismatch for {path}: registry {want}, actual {got}");
        }
    }

    for path in &found {
        if Path::new(path).extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(fixtures_dir.join(path))?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).with_context(|| format!("parsing {path}"))?;
        validate_fixture(path, &value, schemas_dir)
            .with_context(|| format!("validating fixture {path}"))?;
    }

    println!("fixtures: {} files verified", found.len());
    Ok(())
}

fn discover_fixture_files(fixtures_dir: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for entry in WalkDir::new(fixtures_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|ext| ext.to_str());
        if ext != Some("json") && ext != Some("raw") {
            continue;
        }
        if let Ok(relative) = path.strip_prefix(fixtures_dir) {
            found.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    found
}

fn validate_fixture(path: &str, value: &serde_json::Value, schemas_dir: &Path) -> Result<()> {
    if let Some(steps) = value.get("steps").and_then(serde_json::Value::as_array) {
        for (index, step) in steps.iter().enumerate() {
            validate_golden_step(step, schemas_dir).with_context(|| format!("step {index}"))?;
        }
    } else if let Some(error) = value.get("expect_error") {
        let code = error
            .get("code")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("{path}: expect_error is missing `code`"))?;
        // The closed code set is derived from the contract, never hand-copied.
        if !ErrorCode::ALL.iter().any(|known| known.wire_name() == code) {
            bail!("{path}: unknown error code '{code}'");
        }
        if value.get("send").is_none() {
            bail!("{path}: negative fixture is missing `send`");
        }
    } else {
        bail!("{path}: fixture has neither `steps` nor `expect_error`");
    }
    Ok(())
}

fn validate_golden_step(step: &serde_json::Value, schemas_dir: &Path) -> Result<()> {
    let send = step
        .get("send")
        .ok_or_else(|| anyhow!("step is missing `send`"))?;
    let expect = step
        .get("expect")
        .ok_or_else(|| anyhow!("step is missing `expect`"))?;
    let op = send
        .get("op")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("send is missing `op`"))?;

    // Cross-document bindings: the response echoes the request's id and dli.
    if send.get("id") != expect.get("id") {
        bail!("cross-binding: send.id != expect.id");
    }
    if send.get("dli") != expect.get("dli") {
        bail!("cross-binding: send.dli != expect.dli");
    }

    let empty = serde_json::json!({});
    let params = send.get("params").unwrap_or(&empty);
    let request_schema = load_schema(schemas_dir, &format!("{op}.request"))?;
    validate_json(&request_schema, params).with_context(|| format!("{op} request params"))?;

    if let Some(result) = expect.get("result")
        && !result.is_null()
    {
        let response_schema = load_schema(schemas_dir, &format!("{op}.response"))?;
        validate_json(&response_schema, result).with_context(|| format!("{op} response result"))?;
    }
    Ok(())
}

fn load_schema(schemas_dir: &Path, name: &str) -> Result<serde_json::Value> {
    let path = schemas_dir.join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading schema {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing schema {}", path.display()))
}

fn update_registry_digests(registry_path: &Path, text: &str, fixtures_dir: &Path) -> Result<()> {
    let mut registry: serde_yaml_ng::Value = serde_yaml_ng::from_str(text)?;
    let entries = registry
        .get_mut("fixtures")
        .and_then(serde_yaml_ng::Value::as_sequence_mut)
        .ok_or_else(|| anyhow!("registry.yaml is missing a `fixtures` list"))?;
    for entry in entries.iter_mut() {
        let path = entry
            .get("path")
            .and_then(serde_yaml_ng::Value::as_str)
            .ok_or_else(|| anyhow!("a fixture entry is missing `path`"))?
            .to_owned();
        let bytes = std::fs::read(fixtures_dir.join(&path))
            .with_context(|| format!("reading fixture {path}"))?;
        let digest = hex_digest(&bytes);
        if let Some(map) = entry.as_mapping_mut() {
            map.insert(
                serde_yaml_ng::Value::String("sha256".to_owned()),
                serde_yaml_ng::Value::String(digest),
            );
        }
    }
    let serialized = serde_yaml_ng::to_string(&registry)?;
    std::fs::write(registry_path, serialized)?;
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    crate::util::sha256_hex(bytes)
}
