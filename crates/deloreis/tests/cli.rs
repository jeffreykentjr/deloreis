#![allow(clippy::unwrap_used, clippy::expect_used)]
//! End-to-end coverage for the CLI surface: every assertion drives the real
//! binary against the committed example corpus, so the exit codes and the
//! rendered output are checked as a consumer sees them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The binary under test.
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_deloreis"))
}

/// The committed world packs under `worlds/tpth/packs/`, absolute so a recorded
/// corpus path resolves regardless of the test process's working directory.
fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../worlds/tpth/packs")
}

/// The shared saltmarket corpus.
fn saltmarket() -> PathBuf {
    examples().join("saltmarket.dli")
}

/// A fresh temp subdirectory for a test (per-binary, isolated).
///
/// `CARGO_TARGET_TMPDIR` is a *fixed* path that persists across runs (and
/// greengate reuses a warm target), so any state a test writes here — e.g. the
/// fork CLI's `<workspace>/.dli-cache/forks.yaml`, which `fork new` *appends*
/// to — would otherwise leak into the next run and flake the suite. Removing the
/// subdir first makes every test start from a clean, hermetic directory.
fn tmp(sub: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(sub);
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The captured stdout of a run.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn help_exits_zero() {
    let output = bin().arg("--help").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn check_clean_corpus_exits_zero() {
    let dir = tmp("check-clean");
    let doc = dir.join("clean.dli");
    fs::write(
        &doc,
        "scope demo\n\
         ratify d-0001\n\
         place saltmarket \"Saltmarket\" decided d-0001\n\
         place the-shallows \"The Shallows\" decided d-0001\n\
         adjacent(saltmarket, the-shallows) decided d-0001\n",
    )
    .unwrap();

    let output = bin().arg("check").arg(&doc).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("clean"));
}

/// The un-ignored golden (was gated at SURF-1): the check command's clean
/// summary over a known-consistent corpus is byte-stable.
#[test]
fn golden_check_output() {
    let dir = tmp("golden-check");
    let doc = dir.join("clean.dli");
    fs::write(
        &doc,
        "scope demo\n\
         ratify d-0001\n\
         place saltmarket \"Saltmarket\" decided d-0001\n\
         adjacent(saltmarket, saltmarket) decided d-0001\n",
    )
    .unwrap();

    let output = bin().arg("check").arg(&doc).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "checked 1 document(s): clean\n");
}

#[test]
fn check_bad_file_exits_one_with_ladder() {
    let dir = tmp("check-bad");
    let doc = dir.join("bad.dli");
    // An unterminated label quote: a syntax fault the ladder teaches.
    fs::write(&doc, "scope broken\nplace weir \"Weir\n").unwrap();

    let output = bin().arg("check").arg(&doc).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let out = stdout(&output);
    assert!(
        out.contains("= what:"),
        "expected a rendered ladder block, got:\n{out}"
    );
    assert!(out.contains("finding(s)"));
}

#[test]
fn check_reports_a_malformed_bare_world_rule() {
    let dir = tmp("check-malformed-world-rule");
    let doc = dir.join("bad-rule.dli");
    fs::write(
        &doc,
        "scope demo\nratify d-0001\nrule checker decided d-0001\n  beat $b requires slot >= 10\n",
    )
    .unwrap();

    let output = bin().arg("check").arg(&doc).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let out = stdout(&output);
    assert!(
        out.contains("load.malformed-world-rule"),
        "expected a load/check finding:\n{out}"
    );
    assert!(
        out.contains("1 finding(s)"),
        "finding was duplicated:\n{out}"
    );
}

#[test]
fn report_does_not_duplicate_loader_findings() {
    let dir = tmp("report-malformed-world-rule");
    let doc = dir.join("bad-rule.dli");
    fs::write(
        &doc,
        "scope demo\nratify d-0001\nrule checker decided d-0001\n  beat $b requires slot >= 10\n",
    )
    .unwrap();

    let output = bin()
        .args(["report", "--corpus"])
        .arg(&doc)
        .output()
        .unwrap();
    let out = stdout(&output);
    assert_eq!(out.matches("load.malformed-world-rule").count(), 1, "{out}");
    assert!(
        out.contains("report: 1 finding(s)"),
        "finding was duplicated:\n{out}"
    );
}

#[test]
fn fmt_check_is_idempotent_on_the_corpus() {
    let dir = tmp("fmt-idempotent");
    let doc = dir.join("saltmarket.dli");
    fs::copy(saltmarket(), &doc).unwrap();

    // Format once to canonical, then --check must find it already canonical.
    let formatted = bin().arg("fmt").arg(&doc).output().unwrap();
    assert_eq!(formatted.status.code(), Some(0));

    let checked = bin().arg("fmt").arg("--check").arg(&doc).output().unwrap();
    assert_eq!(checked.status.code(), Some(0));
    assert!(stdout(&checked).contains("all canonical"));
}

/// H3: `fmt` on a malformed file must refuse (exit 2) through the safe
/// `format_source` entry, not silently drop the bad line via a lossy rewrite.
#[test]
fn fmt_refuses_malformed_and_does_not_rewrite() {
    let dir = tmp("fmt-malformed");
    let doc = dir.join("bad.dli");
    // `fact mara lives-in` is missing its object — a malformed triple.
    let original = "scope demo\nfact mara lives-in\n";
    fs::write(&doc, original).unwrap();

    let output = bin().arg("fmt").arg(&doc).output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "malformed fmt must refuse:\n{}",
        stdout(&output)
    );
    // The file is left byte-for-byte untouched — no lossy rewrite.
    let after = fs::read_to_string(&doc).unwrap();
    assert_eq!(after, original, "fmt must not rewrite a malformed file");
}

#[test]
fn query_found_exits_zero() {
    let output = bin()
        .arg("query")
        .arg("adjacent(saltmarket, the-shallows)")
        .arg("--corpus")
        .arg(saltmarket())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let out = stdout(&output);
    assert!(out.contains("certain"));
    assert!(out.contains("adjacent(saltmarket, the-shallows)"));
}

#[test]
fn query_empty_exits_one() {
    let output = bin()
        .arg("query")
        .arg("adjacent(saltmarket, nowhere)")
        .arg("--corpus")
        .arg(saltmarket())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("no rows match"));
}

#[test]
fn fork_new_suppose_diff_flow() {
    let workspace = tmp("fork-flow");

    // new: establish the base over the corpus.
    let created = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .arg("new")
        .arg("--corpus")
        .arg(saltmarket())
        .args(["--name", "drop-people"])
        .output()
        .unwrap();
    assert_eq!(
        created.status.code(),
        Some(0),
        "fork new: {}",
        stdout(&created)
    );

    // suppose: rule the decision that grounds the people-facts false.
    let supposed = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["suppose", "drop-people", "d-0006", "false"])
        .output()
        .unwrap();
    assert_eq!(
        supposed.status.code(),
        Some(0),
        "fork suppose: {}",
        stdout(&supposed)
    );

    // diff: the supposition kills the facts d-0006 grounds, and the fork
    // contradicts a certain ruling — a conflict, so exit 1.
    let diff = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["diff", "drop-people"])
        .output()
        .unwrap();
    assert_eq!(diff.status.code(), Some(1), "fork diff: {}", stdout(&diff));
    let out = stdout(&diff);
    assert!(
        out.contains("newly dead"),
        "expected newly-dead atoms, got:\n{out}"
    );
    assert!(out.contains("conflict"), "expected a conflict, got:\n{out}");
}

#[test]
fn fork_suppose_rejects_unknown_target() {
    let workspace = tmp("fork-unknown");
    bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .arg("new")
        .arg("--corpus")
        .arg(saltmarket())
        .args(["--name", "f"])
        .output()
        .unwrap();

    let output = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["suppose", "f", "no-such-variable", "true"])
        .output()
        .unwrap();
    // An unknown supposition target is an operational error.
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn simulate_practice_runs_an_honest_battery() {
    // simulate is wired (SIM-1): a bounded seeded battery over a declared practice,
    // graded with honest metrics (tested) and invariants (checked). It no longer
    // reports a typed not-yet.
    let corpus = examples().join("true-naming.dli");
    let output = bin()
        .args(["simulate", "true-naming", "--games", "64", "--seed", "1"])
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .unwrap();
    let code = output.status.code();
    // A verdict code: clean (0) if every law held, or findings (1) if a random
    // casting broke one — never the old usage code (2) for an unwired engine.
    assert!(
        code == Some(0) || code == Some(1),
        "simulate exits a verdict code, got {code:?}:\n{}",
        stdout(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("grades are honest"), "honest manifest:\n{out}");
    assert!(
        !out.contains("not wired yet"),
        "simulate is wired now:\n{out}"
    );
}

/// H7: a downstream consumer closing the pipe early (`head`) must not panic the
/// process — the standard SIGPIPE-abort a chatty CLI would otherwise hit.
#[test]
fn broken_pipe_does_not_panic() {
    let pipeline = format!(
        "{bin:?} timeline order -c {corpus:?} | head -1",
        bin = env!("CARGO_BIN_EXE_deloreis"),
        corpus = saltmarket(),
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(&pipeline)
        .output()
        .unwrap();
    let combined = format!(
        "{}{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains("panicked"),
        "EPIPE panicked:\n{combined}"
    );
    assert!(
        !combined.contains("Broken pipe"),
        "EPIPE surfaced:\n{combined}"
    );
}

/// M7: a dangling `--as-of` era is an unknown-reference error (exit 2), not a
/// silent no-op that returns unfiltered rows.
#[test]
fn query_unknown_as_of_exits_two() {
    let output = bin()
        .arg("query")
        .arg("adjacent(saltmarket, the-shallows)")
        .arg("--corpus")
        .arg(saltmarket())
        .arg("--as-of")
        .arg("no-such-era")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

/// M7 regression: a real chronology point is still accepted.
#[test]
fn query_known_as_of_is_accepted() {
    let output = bin()
        .arg("query")
        .arg("adjacent(saltmarket, the-shallows)")
        .arg("--corpus")
        .arg(saltmarket())
        .arg("--as-of")
        .arg("the-drought-years")
        .output()
        .unwrap();
    // A real point resolves; the atom carries no time anchor, so it survives.
    assert_eq!(output.status.code(), Some(0));
}

/// M8: `name check --dialect` must not claim register-fit that the kernel vet
/// does not run over a bare candidate.
#[test]
fn name_check_does_not_overclaim_register_fit() {
    let output = bin()
        .arg("name")
        .arg("check")
        .arg("Zzgrbl")
        .arg("--corpus")
        .arg(examples().join("cymric-fringe.dli"))
        .arg("--dialect")
        .arg("cymric-fringe")
        .output()
        .unwrap();
    let out = stdout(&output);
    assert!(
        !out.contains("clean in dialect"),
        "must not over-claim fit:\n{out}"
    );
    assert!(
        out.contains("register-fit"),
        "must disclose what was skipped:\n{out}"
    );
    assert!(
        out.contains("NOT run"),
        "must disclose what was skipped:\n{out}"
    );
}

/// F8-2: `check-draft --killed` flags a draft that revives a killed token.
#[test]
fn check_draft_killed_token_exits_one() {
    let dir = tmp("check-draft-killed");

    // A minimal corpus the draft checks against.
    let corpus = dir.join("world.dli");
    fs::write(
        &corpus,
        "scope demo\n\
         ratify d-0001\n\
         place saltmarket \"Saltmarket\" decided d-0001\n",
    )
    .unwrap();

    // A killed-token registry naming `irondale` as struck.
    let registry = dir.join("killed_tokens.yaml");
    fs::write(
        &registry,
        "killed:\n  - {token: irondale, match: word, decision: D-0101, scope: region name}\n",
    )
    .unwrap();

    // A draft payload introducing the killed name.
    let payload = dir.join("draft.yaml");
    fs::write(
        &payload,
        "meta:\n  title: test-draft\n  scope: demo\n  version: 1\n\
         claims: []\n\
         deltas: []\n\
         scenes: []\n\
         beats: []\n\
         promises: []\n\
         proofs: []\n\
         names:\n  - Irondale\n\
         lexicon: []\n",
    )
    .unwrap();

    // Without --killed: the draft exits clean (the bug this fixes).
    let output_no_killed = bin()
        .arg("check-draft")
        .arg(&payload)
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .unwrap();
    assert_eq!(
        output_no_killed.status.code(),
        Some(0),
        "without --killed should be clean:\n{}",
        stdout(&output_no_killed)
    );

    // With --killed: the draft exits 1 because `Irondale` is a killed token.
    let output_killed = bin()
        .arg("check-draft")
        .arg(&payload)
        .arg("--corpus")
        .arg(&corpus)
        .arg("--killed")
        .arg(&registry)
        .output()
        .unwrap();
    assert_eq!(
        output_killed.status.code(),
        Some(1),
        "with --killed should flag the revived token:\n{}",
        stdout(&output_killed)
    );
    let out = stdout(&output_killed);
    assert!(
        out.contains("killed token"),
        "expected killed-token finding:\n{out}"
    );
}
