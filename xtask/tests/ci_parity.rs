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
        "cargo doc --workspace --no-deps --locked",
        "cargo test --doc --workspace --locked",
        "cargo run -p xtask --locked -- docgen --check",
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
