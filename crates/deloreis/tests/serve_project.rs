#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Standalone binary smoke tests for explicit project/demo worker selection.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_deloreis"))
}

fn norse() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/norse")
}

fn run_with_input(args: &[&str], input: &str) -> std::process::Output {
    let mut child = bin()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn lines(output: &std::process::Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn worker_transports_require_an_explicit_project_or_demo() {
    for command in ["serve", "mcp"] {
        let output = bin().arg(command).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{command}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--project <PATH>"), "{command}: {stderr}");
        assert!(stderr.contains("--demo"), "{command}: {stderr}");
    }
}

#[test]
fn stdio_project_mode_serves_norse_and_never_camelot() {
    let project = norse().display().to_string();
    let output = run_with_input(
        &["serve", "--project", &project],
        concat!(
            "{\"dli\":\"2.0.0\",\"id\":\"1\",\"op\":\"dli.query\",",
            "\"params\":{\"document\":\"norse\",\"goal_atom\":{\"pred\":\"adjacent\",\"args\":[{\"Sym\":\"asgard\"},{\"Sym\":\"midgard\"}]}}}\n",
            "{\"dli\":\"2.0.0\",\"id\":\"2\",\"op\":\"dli.check\",",
            "\"params\":{\"document\":\"camelot\"}}\n"
        ),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = lines(&output);
    assert_eq!(responses[0]["ok"], true);
    assert_eq!(responses[1]["error"]["code"], "unknown-doc");
}

#[test]
fn explicit_demo_mode_keeps_the_neutral_fixture_available() {
    let output = run_with_input(
        &["serve", "--demo"],
        "{\"dli\":\"2.0.0\",\"id\":\"1\",\"op\":\"dli.check\",\"params\":{\"document\":\"camelot\"}}\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = lines(&output);
    assert_eq!(responses[0]["ok"], true);
}

#[test]
fn mcp_project_mode_uses_the_same_project_resolver() {
    let project = norse().display().to_string();
    let output = run_with_input(
        &["mcp", "--project", &project],
        concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":",
            "{\"name\":\"dli.check\",\"arguments\":{\"document\":\"norse\"}}}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":",
            "{\"name\":\"dli.check\",\"arguments\":{\"document\":\"camelot\"}}}\n"
        ),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = lines(&output);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "deloreis");
    assert_eq!(responses[1]["result"]["isError"], false);
    assert_eq!(responses[2]["result"]["isError"], true);
    let inner: Value = serde_json::from_str(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(inner["error"]["code"], "unknown-doc");
}

#[test]
fn mcp_tool_profile_is_selected_on_the_command_line() {
    let request = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n";
    let core = run_with_input(&["mcp", "--demo"], request);
    let all = run_with_input(&["mcp", "--demo", "--tools", "all"], request);
    assert!(core.status.success());
    assert!(all.status.success());
    assert!(core.stdout.len() < 60_000);
    let core_lines = lines(&core);
    let all_lines = lines(&all);
    let core_tools = core_lines[0]["result"]["tools"].as_array().unwrap();
    let all_tools = all_lines[0]["result"]["tools"].as_array().unwrap();
    assert!(core_tools.len() < all_tools.len());
    assert!(
        all_tools
            .iter()
            .any(|tool| tool["name"] == "dli.export.run")
    );
    assert!(
        !core_tools
            .iter()
            .any(|tool| tool["name"] == "dli.export.run")
    );
}
