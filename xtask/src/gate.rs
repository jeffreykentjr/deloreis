//! The canonical gate: the ordered pipeline, its execution, and its receipt.

use std::path::Path;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::jsonval::validate_json;

/// The exact command CI's `gate` job runs (and its only cargo command).
pub const GATE_CI_INVOCATION: &str =
    "cargo run -p xtask --locked -- gate --ci --receipt out/greengate/ci";

/// Cargo commands that may appear in CI jobs but are not gate steps.
pub const AUX_CARGO_COMMANDS: &[&str] = &[
    "cargo deny check advisories bans licenses sources",
    // The book build is non-gating (external `mdbook`) but runs in the
    // docs-oracle CI job; keep it off-gate yet parity-allowed.
    "cargo run -p xtask --locked -- book",
];

/// How a gate step is executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Assert `rustc -V` is the pinned toolchain.
    ToolchainAssert,
    /// Assert the git hooks path (skipped under CI).
    HooksAssert,
    /// Spawn the argv as a subprocess and use its exit code.
    Command,
}

/// One gate step: a label, the argv recorded in the receipt, and how it runs.
#[derive(Debug, Clone)]
pub struct GateStep {
    /// A short human label.
    pub label: &'static str,
    /// The argv, as recorded and (for `Command`) spawned.
    pub argv: Vec<String>,
    /// How the step is executed.
    pub kind: StepKind,
    /// Extra environment applied to a `Command` step (e.g. `RUSTDOCFLAGS`).
    /// Not part of the argv, the receipt command line, or the parity check.
    pub env: Vec<(&'static str, &'static str)>,
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

/// A `Command` step with no extra environment (the common case).
fn command_step(label: &'static str, parts: &[&str]) -> GateStep {
    GateStep {
        label,
        argv: argv(parts),
        kind: StepKind::Command,
        env: Vec::new(),
    }
}

/// The canonical, ordered gate list. `ci` selects the nextest CI profile.
#[must_use]
pub fn gate_steps(ci: bool) -> Vec<GateStep> {
    let mut nextest = vec!["cargo", "nextest", "run", "--workspace", "--locked"];
    if ci {
        nextest.push("--profile");
        nextest.push("ci");
    }
    vec![
        GateStep {
            label: "preflight: toolchain 1.96.0",
            argv: argv(&["rustc", "-V"]),
            kind: StepKind::ToolchainAssert,
            env: Vec::new(),
        },
        GateStep {
            label: "preflight: hooks path",
            argv: argv(&["git", "config", "--get", "core.hooksPath"]),
            kind: StepKind::HooksAssert,
            env: Vec::new(),
        },
        command_step("rustfmt", &["cargo", "fmt", "--all", "--check"]),
        command_step(
            "clippy",
            &[
                "cargo",
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        command_step("nextest", &nextest),
        // Doc build: rustdoc warnings (broken intra-doc links, missing docs when
        // the workspace lint is on) are errors. nextest skips doctests, so the
        // doctest run below closes that hole.
        GateStep {
            label: "doc build",
            argv: argv(&["cargo", "doc", "--workspace", "--no-deps", "--locked"]),
            kind: StepKind::Command,
            env: vec![("RUSTDOCFLAGS", "-D warnings")],
        },
        command_step(
            "doctests",
            &["cargo", "test", "--doc", "--workspace", "--locked"],
        ),
        command_step("build", &["cargo", "build", "--workspace", "--locked"]),
        command_step(
            "schema --check",
            &[
                "cargo", "run", "-p", "xtask", "--locked", "--", "schema", "--check",
            ],
        ),
        command_step(
            "docgen --check",
            &[
                "cargo", "run", "-p", "xtask", "--locked", "--", "docgen", "--check",
            ],
        ),
        command_step(
            "fixtures",
            &["cargo", "run", "-p", "xtask", "--locked", "--", "fixtures"],
        ),
        command_step(
            "docs-oracle",
            &[
                "cargo",
                "run",
                "-p",
                "xtask",
                "--locked",
                "--",
                "docs-oracle",
            ],
        ),
        command_step(
            "loc",
            &["cargo", "run", "-p", "xtask", "--locked", "--", "loc"],
        ),
        command_step(
            "cargo-deny",
            &["cargo", "deny", "check", "bans", "licenses", "sources"],
        ),
    ]
}

/// The cargo commands in the canonical gate list, joined for parity checks.
#[must_use]
pub fn canonical_cargo_commands(ci: bool) -> Vec<String> {
    gate_steps(ci)
        .into_iter()
        .filter(|step| step.argv.first().map(String::as_str) == Some("cargo"))
        .map(|step| step.argv.join(" "))
        .collect()
}

/// Print the gate list.
pub fn list_gate(ci: bool, json: bool) -> Result<()> {
    let steps = gate_steps(ci);
    if json {
        let items: Vec<serde_json::Value> = steps
            .iter()
            .map(|step| serde_json::json!({ "label": step.label, "argv": step.argv }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for step in &steps {
            println!("{}\t{}", step.label, step.argv.join(" "));
        }
    }
    Ok(())
}

struct CommandRecord {
    argv: Vec<String>,
    exit_code: i32,
    duration_s: f64,
}

/// Run the full gate. Writes a receipt to `receipt_dir` when given.
pub fn run_gate(root: &Path, ci: bool, receipt_dir: Option<&Path>) -> Result<()> {
    let steps = gate_steps(ci);
    let started_at = unix_seconds();
    let mut records: Vec<CommandRecord> = Vec::new();
    let mut result = "green";

    for step in &steps {
        println!("gate: >> {}", step.label);
        let start = Instant::now();
        let exit_code = execute_step(step)?;
        let duration_s = start.elapsed().as_secs_f64();
        records.push(CommandRecord {
            argv: step.argv.clone(),
            exit_code,
            duration_s,
        });
        if exit_code != 0 {
            result = "red";
            eprintln!("gate: FAILED at step: {} (exit {exit_code})", step.label);
            break;
        }
    }

    let finished_at = unix_seconds();
    if let Some(dir) = receipt_dir {
        write_receipt(root, dir, &records, result, &started_at, &finished_at)?;
    }

    if result == "red" {
        bail!("gate: the tree is RED — do not push");
    }
    println!("gate: PASSED — {} steps green", records.len());
    Ok(())
}

fn execute_step(step: &GateStep) -> Result<i32> {
    match step.kind {
        StepKind::ToolchainAssert => {
            let output = capture(&step.argv)?;
            if output.contains("1.96.0") {
                Ok(0)
            } else {
                eprintln!("gate: rustc is not 1.96.0: {}", output.trim());
                Ok(1)
            }
        }
        StepKind::HooksAssert => {
            if std::env::var_os("CI").is_some() {
                println!("gate: (skipping hooks assert under CI)");
                return Ok(0);
            }
            let value = capture(&step.argv).unwrap_or_default();
            if value.trim() == ".githooks" {
                Ok(0)
            } else {
                eprintln!(
                    "gate: core.hooksPath is '{}', expected .githooks; run scripts/install-hooks.sh",
                    value.trim()
                );
                Ok(1)
            }
        }
        StepKind::Command => {
            let mut command = Command::new(&step.argv[0]);
            command.args(&step.argv[1..]);
            for (key, value) in &step.env {
                command.env(key, value);
            }
            let status = command
                .status()
                .with_context(|| format!("spawning {}", step.argv.join(" ")))?;
            Ok(status.code().unwrap_or(1))
        }
    }
}

fn write_receipt(
    root: &Path,
    dir: &Path,
    records: &[CommandRecord],
    result: &str,
    started_at: &str,
    finished_at: &str,
) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let commands: Vec<serde_json::Value> = records
        .iter()
        .map(|record| {
            serde_json::json!({
                "argv": record.argv,
                "exit_code": record.exit_code,
                "duration_s": record.duration_s,
            })
        })
        .collect();
    let receipt = serde_json::json!({
        "schema": "greengate-receipt/v1",
        "sha": git_head_sha(),
        "toolchain": {
            "channel": toolchain_channel(root),
            "rustc_version": rustc_version(),
        },
        "started_at": started_at,
        "finished_at": finished_at,
        "commands": commands,
        "result": result,
        "host": host_name(),
        "gate_list_digest": gate_list_digest(),
    });

    let schema_path = root.join("schemas/greengate-receipt.schema.json");
    let schema_text = std::fs::read_to_string(&schema_path)
        .with_context(|| format!("reading {}", schema_path.display()))?;
    let schema: serde_json::Value =
        serde_json::from_str(&schema_text).context("parsing receipt schema")?;
    validate_json(&schema, &receipt).context("receipt failed its own schema")?;

    let path = dir.join("receipt.json");
    let body = serde_json::to_string_pretty(&receipt)? + "\n";
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    println!("gate: receipt {}", path.display());
    Ok(())
}

fn gate_list_digest() -> String {
    let mut hasher = Sha256::new();
    for step in gate_steps(true) {
        hasher.update(step.argv.join(" "));
        hasher.update(b"\n");
    }
    crate::util::hex(&hasher.finalize())
}

fn capture(command: &[String]) -> Result<String> {
    let output = Command::new(&command[0])
        .args(&command[1..])
        .output()
        .with_context(|| format!("running {}", command.join(" ")))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_head_sha() -> String {
    capture(&argv(&["git", "rev-parse", "HEAD"]))
        .map(|value| value.trim().to_owned())
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "uncommitted".to_owned())
}

fn rustc_version() -> String {
    capture(&argv(&["rustc", "-V"]))
        .map_or_else(|_| "unknown".to_owned(), |value| value.trim().to_owned())
}

fn host_name() -> String {
    if let Ok(host) = capture(&argv(&["hostname"])) {
        let host = host.trim();
        if !host.is_empty() {
            return host.to_owned();
        }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_owned())
}

fn toolchain_channel(root: &Path) -> String {
    let path = root.join("rust-toolchain.toml");
    let Ok(text) = std::fs::read_to_string(path) else {
        return "1.96.0".to_owned();
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("channel")
            && let Some(quoted) = rest.split('"').nth(1)
        {
            return quoted.to_owned();
        }
    }
    "1.96.0".to_owned()
}

fn unix_seconds() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or_else(|_| "0".to_owned(), |elapsed| elapsed.as_secs().to_string())
}
