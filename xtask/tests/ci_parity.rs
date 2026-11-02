#![allow(clippy::unwrap_used, clippy::expect_used)]
//! CI must run exactly the canonical gate: no drift between ci.yml and gate.rs.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(manifest).parent().unwrap().to_path_buf()
}

fn cargo_lines(run: &str) -> Vec<String> {
    run.lines()
        .map(str::trim)
        .filter(|line| line.contains("cargo "))
        .map(str::to_owned)
        .collect()
}

#[test]
fn ci_workflow_matches_the_canonical_gate() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let ci: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap();
    let jobs = ci.get("jobs").and_then(|v| v.as_mapping()).unwrap();

    let allow: BTreeSet<String> = xtask::gate::canonical_cargo_commands(true)
        .into_iter()
        .chain(
            xtask::gate::AUX_CARGO_COMMANDS
                .iter()
                .map(|command| (*command).to_owned()),
        )
        .collect();

    let mut saw_gate_job = false;
    for (name, job) in jobs {
        let job_name = name.as_str().unwrap();
        let mut cargo_cmds: Vec<String> = Vec::new();
        if let Some(steps) = job.get("steps").and_then(|v| v.as_sequence()) {
            for step in steps {
                if let Some(run) = step.get("run").and_then(|v| v.as_str()) {
                    cargo_cmds.extend(cargo_lines(run));
                }
            }
        }
        if job_name == "gate" {
            saw_gate_job = true;
            assert_eq!(
                cargo_cmds,
                vec![xtask::gate::GATE_CI_INVOCATION.to_owned()],
                "the gate job must run exactly the canonical gate invocation"
            );
        } else {
            for cmd in cargo_cmds {
                assert!(
                    allow.contains(&cmd),
                    "job {job_name} runs an off-list cargo command: {cmd}"
                );
            }
        }
    }
    assert!(saw_gate_job, "ci.yml has no `gate` job");
}

#[test]
fn canonical_gate_carries_the_doc_hygiene_steps() {
    let commands = xtask::gate::canonical_cargo_commands(true);
    for expected in [
        "cargo doc --workspace --no-deps --locked --features syntax",
        "cargo test --doc --workspace --locked --features syntax",
        "cargo run -p xtask --locked --features syntax -- docgen --check",
    ] {
        assert!(
            commands.iter().any(|command| command == expected),
            "the canonical gate is missing the `{expected}` step"
        );
    }
    // The book build is deliberately off-gate (external mdbook) yet allowed in
    // CI's docs-oracle job.
    assert!(
        xtask::gate::AUX_CARGO_COMMANDS.contains(&"cargo run -p xtask --locked -- book"),
        "the book build must be an allowed aux command for CI parity"
    );
}

/// LS-7 made the build two builds, so the gate is two lanes. Losing either one
/// silently would make the gate's verdict a half-truth: without the default
/// lane, nothing stops `dli-syntax` creeping back into the shipped artifacts;
/// without the syntax lane, nothing proves the legacy surface still replays.
#[test]
fn canonical_gate_runs_both_build_lanes() {
    let commands = xtask::gate::canonical_cargo_commands(true);

    // The default lane: no features anywhere, and the 1.0-native acceptance run.
    for expected in [
        "cargo clippy -p dli-serve -p deloreis --all-targets --locked -- -D warnings",
        "cargo build -p dli-serve -p deloreis --locked",
    ] {
        assert!(
            commands.iter().any(|command| command == expected),
            "the canonical gate is missing the default-lane `{expected}` step"
        );
    }
    // Asserted component-wise, not as one frozen argv: the lane is allowed to
    // grow more default-lane tests (DL-0055, DL-0056 added two), and a guard that
    // pins the flag order would read that strengthening as a regression.
    assert!(
        commands.iter().any(|command| {
            command.starts_with("cargo nextest run -p dli-serve -p dli-lorestore")
                && command.contains("--lib")
                && command.contains("--test default_build")
                && command.contains("--no-default-features")
                && command.contains("--locked")
                && !command.contains("--features")
        }),
        "the canonical gate must exercise the parser-free acceptance build"
    );

    // The syntax lane: the whole workspace, parser linked.
    assert!(
        commands.iter().any(|command| command
            .starts_with("cargo nextest run --workspace --locked")
            && command.contains("--features syntax")),
        "the canonical gate must run the full test suite with `--features syntax`"
    );

    // And the oracle that keeps the two honest.
    assert!(
        commands
            .iter()
            .any(|command| command.ends_with("-- syntax-free")),
        "the canonical gate must run the no-parse oracle"
    );
}

#[test]
fn gate_job_has_no_conditional_or_continue_on_error_steps() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let ci: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap();
    let jobs = ci.get("jobs").and_then(|v| v.as_mapping()).unwrap();

    let gate_job = jobs
        .iter()
        .find(|(k, _)| k.as_str() == Some("gate"))
        .map(|(_, v)| v)
        .expect("ci.yml must have a `gate` job");

    if let Some(steps) = gate_job.get("steps").and_then(|v| v.as_sequence()) {
        for (i, step) in steps.iter().enumerate() {
            let step_name = step
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("<unnamed>");
            assert!(
                step.get("continue-on-error").is_none(),
                "gate job step {i} ({step_name}) has `continue-on-error` — \
                 the gate must fail-fast, never swallow failures"
            );
            assert!(
                step.get("if").is_none(),
                "gate job step {i} ({step_name}) has an `if:` condition — \
                 the gate must run unconditionally"
            );
        }
    }
}

#[test]
fn gate_job_steps_are_run_commands_only() {
    /// Infrastructure-only setup actions that are tolerated before the gate run.
    const ALLOWED_ACTIONS: &[&str] = &[
        "actions/checkout",
        "Swatinem/rust-cache",
        "taiki-e/install-action",
    ];

    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let ci: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap();
    let jobs = ci.get("jobs").and_then(|v| v.as_mapping()).unwrap();

    let gate_job = jobs
        .iter()
        .find(|(k, _)| k.as_str() == Some("gate"))
        .map(|(_, v)| v)
        .expect("ci.yml must have a `gate` job");

    if let Some(steps) = gate_job.get("steps").and_then(|v| v.as_sequence()) {
        for (i, step) in steps.iter().enumerate() {
            if let Some(uses) = step.get("uses").and_then(|v| v.as_str()) {
                let step_name = step
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<unnamed>");
                let is_infra = ALLOWED_ACTIONS
                    .iter()
                    .any(|prefix| uses.starts_with(prefix));
                assert!(
                    is_infra,
                    "gate job step {i} ({step_name}) uses a non-infrastructure action \
                     `{uses}` — gate logic must be `run:` commands only (DL-0013/DL-0015). \
                     Only setup actions are tolerated: {ALLOWED_ACTIONS:?}"
                );
            }
        }
    }
}

#[test]
fn gate_job_has_no_job_level_if_condition() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let ci: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap();
    let jobs = ci.get("jobs").and_then(|v| v.as_mapping()).unwrap();

    let gate_job = jobs
        .iter()
        .find(|(k, _)| k.as_str() == Some("gate"))
        .map(|(_, v)| v)
        .expect("ci.yml must have a `gate` job");

    assert!(
        gate_job.get("if").is_none(),
        "the gate job has a job-level `if:` condition — \
         the gate must run unconditionally on every trigger"
    );
    assert!(
        gate_job.get("continue-on-error").is_none(),
        "the gate job has job-level `continue-on-error` — \
         the canonical verdict must fail the workflow"
    );
}

#[test]
fn ci_on_trigger_matches_expected_pattern() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let ci: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap();

    let on = ci.get("on").expect("ci.yml must have an `on:` trigger");
    for trigger_name in ["push", "pull_request"] {
        let trigger = on
            .get(trigger_name)
            .unwrap_or_else(|| panic!("ci.yml must trigger on `{trigger_name}`"));
        let branches = trigger
            .get("branches")
            .and_then(|v| v.as_sequence())
            .unwrap_or_else(|| panic!("{trigger_name} trigger must have a `branches` list"));
        assert_eq!(
            branches
                .iter()
                .filter_map(|b| b.as_str())
                .collect::<Vec<_>>(),
            vec!["main"],
            "{trigger_name} must run exactly for the main branch"
        );
        for bypass in ["paths", "paths-ignore", "branches-ignore"] {
            assert!(
                trigger.get(bypass).is_none(),
                "{trigger_name} carries `{bypass}` and can bypass the canonical gate"
            );
        }
    }
}
