//! The closed fixture registry check.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use walkdir::WalkDir;

use dli_serve::{ErrorCode, Op};

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
    if !update_digests {
        check_version_rides_digest_changes(contract_dir, &registry)?;
    }
    let fixtures_dir = contract_dir.join("fixtures");

    if let Some(project) = registry
        .get("replay_project")
        .and_then(serde_yaml_ng::Value::as_sequence)
    {
        for entry in project {
            let path = entry
                .get("path")
                .and_then(serde_yaml_ng::Value::as_str)
                .ok_or_else(|| anyhow!("a replay_project entry is missing `path`"))?;
            let want = entry
                .get("sha256")
                .and_then(serde_yaml_ng::Value::as_str)
                .ok_or_else(|| anyhow!("replay_project {path} is missing `sha256`"))?;
            let bytes = std::fs::read(contract_dir.join(path))
                .with_context(|| format!("reading replay_project {path}"))?;
            let got = hex_digest(&bytes);
            if got != want {
                bail!(
                    "fixtures: replay_project digest mismatch for {path}: registry {want}, actual {got}"
                );
            }
        }
    } else {
        bail!("registry.yaml is missing the digest-pinned `replay_project`");
    }

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

    check_op_completeness(&fixtures_dir)?;

    println!("fixtures: {} files verified", found.len());
    Ok(())
}

/// The CON-1 completeness oracle: *an op without golden + negative + framing
/// fixtures does not exist.* Framing coverage comes only from paired control /
/// attack cases, never from a file merely placed under `framing/`.
fn check_op_completeness(fixtures_dir: &Path) -> Result<()> {
    let golden = ops_in_category(&fixtures_dir.join("golden"))?;
    let negative = ops_in_category(&fixtures_dir.join("negative"))?;
    let framing = paired_framing_ops(&fixtures_dir.join("framing"))?;

    let mut missing: Vec<String> = Vec::new();
    for &op in &Op::ALL {
        let wire = wire_name(op);
        if !golden.contains(&wire) {
            missing.push(format!("{wire}: no golden fixture"));
        }
        if !negative.contains(&wire) {
            missing.push(format!("{wire}: no negative fixture"));
        }
        if !framing.contains(&wire) {
            missing.push(format!("{wire}: no paired framing fixture"));
        }
    }
    if !missing.is_empty() {
        bail!(
            "fixtures: CON-1 completeness — {} op(s) lack a required fixture:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }
    println!(
        "fixtures: CON-1 fixture inventory OK ({} ops: golden+negative+paired framing)",
        Op::ALL.len()
    );
    Ok(())
}

/// Validate paired framing cases and return their exact op census.
fn paired_framing_ops(dir: &Path) -> Result<BTreeSet<String>> {
    let mut ops = BTreeSet::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)
            .with_context(|| format!("parsing {}", path.display()))?;
        let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for (index, case) in cases.iter().enumerate() {
            let op = validate_framing_case(case)
                .with_context(|| format!("{} case {index}", path.display()))?;
            // Multiple independent attacks can exercise the same operation.
            ops.insert(op);
        }
    }
    Ok(ops)
}

fn validate_framing_case(case: &serde_json::Value) -> Result<String> {
    let op = case
        .get("op")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("paired framing case is missing `op`"))?
        .to_owned();
    if !Op::ALL.iter().any(|known| wire_name(*known) == op) {
        bail!("{op}: paired framing case names an unknown operation");
    }
    validate_framing_outcome(case, &op)?;
    let control = case
        .get("control")
        .ok_or_else(|| anyhow!("{op}: missing `control` request"))?;
    let attack = case
        .get("attack")
        .ok_or_else(|| anyhow!("{op}: missing `attack` request"))?;
    for (label, request) in [("control", control), ("attack", attack)] {
        if request.get("op").and_then(serde_json::Value::as_str) != Some(op.as_str()) {
            bail!("{op}: {label}.op does not match the case op");
        }
    }
    let declared: BTreeSet<String> = case
        .get("changed_paths")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("{op}: missing `changed_paths`"))?
        .iter()
        .map(|path| {
            path.as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("{op}: a changed path is not a string"))
        })
        .collect::<Result<_>>()?;
    let actual = json_diff_paths(control, attack, "");
    if actual != declared {
        bail!("{op}: changed_paths mismatch; declared {declared:?}, actual {actual:?}");
    }
    let state_read = matches!(
        op.as_str(),
        "author.status" | "author.diff" | "author.materialize" | "author.check" | "author.findings"
    );
    let branch_preconditions = match (
        case.get("control_precondition")
            .and_then(serde_json::Value::as_array),
        case.get("attack_precondition")
            .and_then(serde_json::Value::as_array),
    ) {
        (Some(control), Some(attack)) if !control.is_empty() && !attack.is_empty() => {
            json_diff_paths(
                &serde_json::Value::Array(control.clone()),
                &serde_json::Value::Array(attack.clone()),
                "",
            )
        }
        (None, None) => BTreeSet::new(),
        _ => bail!("{op}: branch preconditions must be nonempty and appear together"),
    };
    if declared.is_empty() && (!state_read || branch_preconditions.is_empty()) {
        bail!("{op}: control and attack requests are identical without distinct tainted state");
    }
    if !state_read
        && !matches!(op.as_str(), "dli.version" | "author.list_workspaces")
        && declared.iter().all(|path| !path.starts_with("/params/"))
    {
        bail!("{op}: framing must change a semantic parameter, not only the envelope");
    }
    let compare = case
        .get("compare_paths")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("{op}: missing `compare_paths`"))?;
    if compare.is_empty()
        || compare.iter().any(|path| {
            path.as_str()
                .is_none_or(|path| !path.starts_with('/') || path == "/")
        })
    {
        bail!("{op}: compare_paths must contain at least one non-root JSON pointer");
    }
    if case.get("probe").is_some()
        != case
            .get("probe_compare_paths")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|paths| !paths.is_empty())
    {
        bail!("{op}: probe and nonempty probe_compare_paths must appear together");
    }
    if let Some(expected) = case.get("probe_expect") {
        if case.get("probe").is_none() {
            bail!("{op}: probe_expect requires a probe");
        }
        let expected = expected
            .as_object()
            .ok_or_else(|| anyhow!("{op}: probe_expect must be an object keyed by JSON pointer"))?;
        if expected.is_empty()
            || expected
                .keys()
                .any(|path| !path.starts_with('/') || path == "/")
        {
            bail!("{op}: probe_expect must contain at least one non-root JSON pointer");
        }
    }
    Ok(op)
}

fn validate_framing_outcome(case: &serde_json::Value, op: &str) -> Result<()> {
    if let Some(outcome) = case
        .get("expected_outcome")
        .and_then(serde_json::Value::as_str)
        && outcome != "gated"
    {
        bail!("{op}: unknown paired framing outcome '{outcome}'");
    }
    Ok(())
}

fn json_diff_paths(
    left: &serde_json::Value,
    right: &serde_json::Value,
    path: &str,
) -> BTreeSet<String> {
    use serde_json::Value;
    let mut out = BTreeSet::new();
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for key in keys {
                let child = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => out.extend(json_diff_paths(x, y, &child)),
                    _ => {
                        out.insert(child);
                    }
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                out.extend(json_diff_paths(x, y, &format!("{path}/{index}")));
            }
        }
        _ if left != right => {
            out.insert(path.to_owned());
        }
        _ => {}
    }
    out
}

/// The set of wire op names appearing as a `send.op` in any fixture under `dir`
/// (single-`send` fixtures and multi-step `steps` flow fixtures alike).
fn ops_in_category(dir: &Path) -> Result<BTreeSet<String>> {
    let mut ops = BTreeSet::new();
    if !dir.is_dir() {
        return Ok(ops);
    }
    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)
            .with_context(|| format!("parsing {}", path.display()))?;
        collect_ops(&value, &mut ops);
    }
    Ok(ops)
}

/// Insert every `send.op` this fixture drives — the one `send`, or each step's.
fn collect_ops(value: &serde_json::Value, ops: &mut BTreeSet<String>) {
    if let Some(cases) = value.get("cases").and_then(serde_json::Value::as_array) {
        for case in cases {
            if let Some(op) = case.get("op").and_then(serde_json::Value::as_str) {
                ops.insert(op.to_owned());
            }
        }
        return;
    }
    let op_of = |send: &serde_json::Value| {
        send.get("op")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    if let Some(steps) = value.get("steps").and_then(serde_json::Value::as_array) {
        for step in steps {
            if let Some(send) = step.get("send")
                && let Some(op) = op_of(send)
            {
                ops.insert(op);
            }
        }
    } else if let Some(send) = value.get("send")
        && let Some(op) = op_of(send)
    {
        ops.insert(op);
    }
}

/// The wire identifier for an op — its serde rename, the contract's name.
fn wire_name(op: Op) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
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
    let expected_identity = path.strip_suffix(".json").unwrap_or(path);
    if value.get("fixture").and_then(serde_json::Value::as_str) != Some(expected_identity) {
        bail!("{path}: `fixture` must equal '{expected_identity}'");
    }
    if let Some(outcome) = value
        .get("expected_outcome")
        .and_then(serde_json::Value::as_str)
        && !matches!(outcome, "replay" | "gated")
    {
        bail!("{path}: unknown expected_outcome '{outcome}'");
    }
    if value.get("cases").is_some() {
        let cases = value
            .get("cases")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow!("{path}: `cases` must be an array"))?;
        for case in cases {
            validate_framing_case(case)?;
            for field in ["control", "attack", "probe"] {
                let Some(send) = case.get(field) else {
                    continue;
                };
                validate_request(send, schemas_dir)
                    .with_context(|| format!("{path}: {field} request"))?;
            }
            for field in [
                "precondition",
                "control_precondition",
                "attack_precondition",
            ] {
                if let Some(precondition) = case.get(field).and_then(serde_json::Value::as_array) {
                    for send in precondition {
                        validate_request(send, schemas_dir)
                            .with_context(|| format!("{path}: {field} request"))?;
                    }
                }
            }
        }
        return Ok(());
    }
    if let Some(steps) = value.get("steps").and_then(serde_json::Value::as_array) {
        for (index, step) in steps.iter().enumerate() {
            validate_golden_step(step, schemas_dir).with_context(|| format!("step {index}"))?;
            if value
                .get("expected_outcome")
                .and_then(serde_json::Value::as_str)
                == Some("gated")
                && step
                    .pointer("/expect/error/code")
                    .and_then(serde_json::Value::as_str)
                    != Some("not-wired")
            {
                bail!("{path}: gated step {index} must expect a not-wired error");
            }
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

fn validate_request(send: &serde_json::Value, schemas_dir: &Path) -> Result<()> {
    let op = send
        .get("op")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("request is missing `op`"))?;
    let empty = serde_json::json!({});
    let params = send.get("params").unwrap_or(&empty);
    let request_schema = load_schema(schemas_dir, &format!("{op}.request"))?;
    validate_json(&request_schema, params).with_context(|| format!("{op} request params"))
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

/// Audit C4: released fixture bytes are frozen. Any re-pinned or removed
/// fixture digest in the registry must ride a `contract_version` change —
/// otherwise the vendored, digest-checked consumer copy (DL-0010) silently
/// diverges under the same version label. Compared against `HEAD`'s registry;
/// skipped when there is no committed registry to compare against (fresh
/// history) or git is unavailable (the check is a repository discipline, not
/// a runtime property).
fn check_version_rides_digest_changes(
    contract_dir: &Path,
    current: &serde_yaml_ng::Value,
) -> Result<()> {
    let Ok(output) = std::process::Command::new("git")
        .current_dir(contract_dir)
        .args(["show", "HEAD:contracts/dli-worker/registry.yaml"])
        .output()
    else {
        return Ok(());
    };
    if !output.status.success() {
        return Ok(());
    }
    // Bounded pre-release corrections for the 2.0 fiction reset: only the
    // exact registry transitions named below are exempt. All other fixture
    // changes retain the ordinary version-fence rule.
    let current_bytes = std::fs::read(contract_dir.join("registry.yaml"))?;
    if reset_wire_fixture_correction(&output.stdout, &current_bytes) {
        return Ok(());
    }
    let Ok(head) =
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&String::from_utf8_lossy(&output.stdout))
    else {
        return Ok(());
    };
    let version = |v: &serde_yaml_ng::Value| {
        v.get("contract_version")
            .and_then(serde_yaml_ng::Value::as_str)
            .map(str::to_owned)
    };
    if version(current) != version(&head) {
        return Ok(()); // the bump rode along — re-pinning is legitimate
    }
    let digests = |v: &serde_yaml_ng::Value| -> std::collections::BTreeMap<String, String> {
        v.get("fixtures")
            .and_then(serde_yaml_ng::Value::as_sequence)
            .into_iter()
            .flatten()
            .filter_map(|e| {
                Some((
                    e.get("path")?.as_str()?.to_owned(),
                    e.get("sha256")?.as_str()?.to_owned(),
                ))
            })
            .collect()
    };
    let head_digests = digests(&head);
    let current_digests = digests(current);
    for (path, want) in &head_digests {
        match current_digests.get(path) {
            Some(got) if got == want => {}
            Some(_) => bail!(
                "fixtures: {path} was re-pinned while contract_version is unchanged — \
                 released fixture bytes are frozen; bump the version with the re-record (audit C4)"
            ),
            None => bail!(
                "fixtures: {path} was removed from the registry while contract_version is \
                 unchanged — retired fixtures ride a version change (audit C4)"
            ),
        }
    }
    Ok(())
}

fn reset_wire_fixture_correction(old: &[u8], new: &[u8]) -> bool {
    let old = hex_digest(old);
    let new = hex_digest(new);
    (old == "dd7a63c68deea6e09d1fa2fb99ab99f8b797ba9e2f3a18cc8532e8b07853cd5c"
        && new == "e8cc73d29d3ad9cacdf27d71957387e3ead3dc805eb99ddb356c5c1bf527ffdc")
        || (old == "e8cc73d29d3ad9cacdf27d71957387e3ead3dc805eb99ddb356c5c1bf527ffdc"
            && new == "017811af869f61efdd224f5f8d8a12a548d6fdf7e1712c75c1ed2fd01138adb4")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_correction_does_not_allow_unrelated_registries() {
        let current = include_bytes!("../../contracts/dli-worker/registry.yaml");
        assert!(!reset_wire_fixture_correction(b"unrelated", current));
        assert!(!reset_wire_fixture_correction(current, current));
        assert!(!reset_wire_fixture_correction(current, b"unrelated"));
    }

    fn paired_case(op: &str) -> serde_json::Value {
        serde_json::json!({
            "op": op,
            "control": {
                "dli": "0.5.1",
                "id": "control",
                "op": op,
                "params": {"document": "control"}
            },
            "attack": {
                "dli": "0.5.1",
                "id": "attack",
                "op": op,
                "params": {"document": "__DLI_FRAME_ATTACK__"}
            },
            "changed_paths": ["/id", "/params/document"],
            "compare_paths": ["/ok"]
        })
    }

    #[test]
    fn framing_rejects_an_unknown_operation() {
        let error = validate_framing_case(&paired_case("dli.fake"))
            .expect_err("an unknown op cannot count as framing coverage");
        assert!(error.to_string().contains("unknown operation"));
    }

    #[test]
    fn framing_rejects_a_changed_path_lie() {
        let mut case = paired_case("dli.check");
        case["changed_paths"] = serde_json::json!(["/id"]);
        let error = validate_framing_case(&case)
            .expect_err("declared changed paths must equal the actual diff");
        assert!(error.to_string().contains("changed_paths mismatch"));
    }

    #[test]
    fn framing_requires_a_semantic_parameter_change() {
        let mut case = paired_case("dli.check");
        case["attack"]["params"]["document"] = case["control"]["params"]["document"].clone();
        case["changed_paths"] = serde_json::json!(["/id"]);
        let error = validate_framing_case(&case)
            .expect_err("envelope-only changes do not frame a semantic op");
        assert!(error.to_string().contains("semantic parameter"));
    }

    #[test]
    fn fixture_identity_is_its_relative_path() {
        let fixture = serde_json::json!({
            "fixture": "negative/not-this-file",
            "send": {},
            "expect_error": {"code": "bad-frame"}
        });
        let error = validate_fixture("negative/identity-v1.json", &fixture, Path::new("unused"))
            .expect_err("a fixture cannot claim another file's identity");
        assert!(error.to_string().contains("negative/identity-v1"));
    }

    #[test]
    fn fixture_rejects_an_unknown_expected_outcome() {
        let fixture = serde_json::json!({
            "fixture": "negative/outcome-v1",
            "expected_outcome": "pretend",
            "send": {},
            "expect_error": {"code": "bad-frame"}
        });
        let error = validate_fixture("negative/outcome-v1.json", &fixture, Path::new("unused"))
            .expect_err("unknown outcomes must not silently mean replay");
        assert!(error.to_string().contains("unknown expected_outcome"));
    }
}
