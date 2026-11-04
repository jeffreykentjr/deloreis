//! Rebuild the casebook through the MCP authoring tools.
#![cfg(feature = "syntax")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

fn check_or_bless(path: &Path, actual: &[u8]) {
    if std::env::var_os("DLI_BLESS_HOLMES").is_some() {
        std::fs::write(path, actual).unwrap();
    } else {
        assert_eq!(
            actual,
            std::fs::read(path).unwrap(),
            "{} is out of date",
            path.display()
        );
    }
}

#[test]
fn casebook_replays_the_authoring_results() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example = repo.join("examples/holmes");
    let output = Command::new("python3")
        .arg(example.join("build.py"))
        .arg(env!("CARGO_BIN_EXE_deloreis"))
        .output()
        .expect("run the Holmes authoring script");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    check_or_bless(&example.join("opening.json"), &output.stdout);

    for (renderer, argument, chapter) in [
        ("render_opening.py", None, "01-watson-name.md"),
        ("render_more.py", Some("wounds"), "02-wounds.md"),
        ("render_more.py", Some("timeline"), "03-timeline.md"),
        ("render_more.py", Some("return"), "04-return.md"),
        ("render_more.py", Some("branch"), "05-branch.md"),
        ("render_more.py", Some("rule"), "06-rules.md"),
    ] {
        let mut command = Command::new("python3");
        command.arg(example.join(renderer));
        if let Some(argument) = argument {
            command.arg(argument);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&output.stdout)
            .unwrap();
        let rendered = child.wait_with_output().unwrap();
        assert!(rendered.status.success());
        check_or_bless(&repo.join("docs/casebook").join(chapter), &rendered.stdout);
    }

    let mut world_process = Command::new("python3")
        .arg(example.join("render_world.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    world_process
        .stdin
        .take()
        .unwrap()
        .write_all(&output.stdout)
        .unwrap();
    let world_output = world_process.wait_with_output().unwrap();
    assert!(world_output.status.success());
    let temp = tempfile::tempdir().unwrap();
    let corpus = temp.path().join("holmes.dli");
    std::fs::write(&corpus, world_output.stdout).unwrap();
    let fmt = Command::new(env!("CARGO_BIN_EXE_deloreis"))
        .arg("fmt")
        .arg(&corpus)
        .output()
        .unwrap();
    assert!(
        fmt.status.success(),
        "{}",
        String::from_utf8_lossy(&fmt.stderr)
    );
    check_or_bless(
        &example.join("holmes.dli"),
        &std::fs::read(&corpus).unwrap(),
    );
    let check = Command::new(env!("CARGO_BIN_EXE_deloreis"))
        .arg("check")
        .arg(&corpus)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stdout)
    );
    assert!(String::from_utf8_lossy(&check.stdout).contains("clean"));
}

#[test]
fn notes_demo_commits_and_reads_the_sample_claims() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/holmes");
    let output = Command::new("python3")
        .arg(example.join("notes_demo.py"))
        .arg(env!("CARGO_BIN_EXE_deloreis"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    check_or_bless(&example.join("notes-demo.txt"), &output.stdout);
}
