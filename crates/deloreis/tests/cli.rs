#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the CLI surface.

use std::process::Command;

#[test]
fn help_exits_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_deloreis"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn check_exits_two_as_unimplemented() {
    let output = Command::new(env!("CARGO_BIN_EXE_deloreis"))
        .arg("check")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
#[ignore = "enabled at SURF-1"]
fn golden_check_output() {
    // Once check is real, snapshot its output over the saltmarket example.
    todo!("SURF-1: golden CLI check output")
}
