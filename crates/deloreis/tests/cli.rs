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

/// The repository root.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The public-domain Camelot example.
fn camelot() -> PathBuf {
    repo().join("examples/camelot.dli")
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

/// The captured stderr of a run.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Every command path in the executable's real Clap graph.
fn command_paths() -> Vec<Vec<String>> {
    fn walk(command: &clap::Command, prefix: &[String], paths: &mut Vec<Vec<String>>) {
        for child in command.get_subcommands() {
            let mut path = prefix.to_vec();
            path.push(child.get_name().to_owned());
            paths.push(path.clone());
            if child.get_name() != "help" {
                walk(child, &path, paths);
            }
        }
    }

    let command = deloreis::command();
    let mut paths = Vec::new();
    walk(&command, &[], &mut paths);
    paths
}

#[test]
fn help_exits_zero() {
    let output = bin().arg("--help").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn every_generated_command_path_has_live_help() {
    let paths = command_paths();
    assert!(!paths.is_empty());
    assert!(paths.iter().any(|path| path == &["help"]));
    assert!(paths.iter().any(|path| path == &["fork", "help"]));
    assert!(paths.iter().any(|path| path == &["timeline", "help"]));
    assert!(paths.iter().any(|path| path == &["name", "help"]));
    for path in paths {
        let mut command = bin();
        command.args(&path);
        if !path.iter().any(|part| part == "help") {
            command.arg("--help");
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "`deloreis {}` help failed:\n{}",
            path.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout(&output).contains("Usage:"),
            "`deloreis {}` emitted no usage:\n{}",
            path.join(" "),
            stdout(&output)
        );
    }
}

#[test]
fn check_clean_corpus_exits_zero() {
    let dir = tmp("check-clean");
    let doc = dir.join("clean.dli");
    fs::write(
        &doc,
        "scope demo\n\
         ratify d-0001\n\
         place thornfield \"Thornfield\" decided d-0001\n\
         place ashby \"Ashby\" decided d-0001\n\
         adjacent(camelot, the-lake) decided d-0001\n",
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
         place thornfield \"Thornfield\" decided d-0001\n\
         adjacent(thornfield, thornfield) decided d-0001\n",
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
fn report_includes_killed_syntax_forms_as_checked_findings() {
    let dir = tmp("report-killed-form");
    let doc = dir.join("killed.dli");
    fs::write(
        &doc,
        "scope demo\nquestion route\n  option north assumes h-north\n",
    )
    .unwrap();

    let output = bin()
        .args(["report", "--corpus"])
        .arg(&doc)
        .output()
        .unwrap();
    let out = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{out}");
    assert!(out.contains("error  dli::syntax::killed-form —"), "{out}");
    assert!(out.contains("report: 1 finding(s)"), "{out}");

    let checked = bin()
        .args(["report", "--corpus"])
        .arg(&doc)
        .args(["--min-grade", "checked"])
        .output()
        .unwrap();
    let checked_out = stdout(&checked);
    assert_eq!(checked.status.code(), Some(1), "{checked_out}");
    assert!(
        checked_out.contains("report: 1 finding(s) at grade checked or stronger"),
        "{checked_out}"
    );
}

#[test]
fn report_includes_malformed_question_children() {
    let dir = tmp("report-malformed-question-child");
    let doc = dir.join("question.dli");
    fs::write(
        &doc,
        "scope demo\nquestion route {\n  option north implies\n}\n",
    )
    .unwrap();

    let output = bin()
        .args(["report", "--corpus"])
        .arg(&doc)
        .output()
        .unwrap();
    let out = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{out}");
    assert!(out.contains("error  dli::syntax::malformed —"), "{out}");
    assert!(out.contains("report: 1 finding(s)"), "{out}");
}

#[test]
fn fmt_check_is_idempotent_on_the_corpus() {
    let dir = tmp("fmt-idempotent");
    let doc = dir.join("thornfield.dli");
    fs::copy(camelot(), &doc).unwrap();

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
    // `fact wren lives-in` is missing its object — a malformed triple.
    let original = "scope demo\nfact wren lives-in\n";
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
        .arg("adjacent(camelot, the-lake)")
        .arg("--corpus")
        .arg(camelot())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let out = stdout(&output);
    assert!(out.contains("certain"));
    assert!(out.contains("adjacent(camelot, the-lake)"));
}

#[test]
fn query_empty_exits_one() {
    let output = bin()
        .arg("query")
        .arg("adjacent(camelot, nowhere)")
        .arg("--corpus")
        .arg(camelot())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("no rows match"));
}

#[test]
fn query_modifiers_and_generated_name_view_are_live() {
    let explained = bin()
        .arg("query")
        .arg("adjacent(camelot, the-lake)")
        .arg("--corpus")
        .arg(camelot())
        .args(["--layer", "certain", "--why"])
        .output()
        .unwrap();
    assert_eq!(explained.status.code(), Some(0), "{}", stderr(&explained));
    assert!(
        stdout(&explained).contains("why:"),
        "{}",
        stdout(&explained)
    );

    let workspace = tmp("query-fork");
    let created = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .arg("new")
        .arg("--corpus")
        .arg(camelot())
        .args(["--name", "query-view"])
        .output()
        .unwrap();
    assert_eq!(created.status.code(), Some(0), "{}", stderr(&created));
    let forked = bin()
        .arg("query")
        .arg("adjacent(camelot, the-lake)")
        .args(["--fork", "query-view", "--workspace"])
        .arg(&workspace)
        .output()
        .unwrap();
    assert_eq!(forked.status.code(), Some(0), "{}", stderr(&forked));

    let names = bin()
        .args(["query", "name", "--layer", "certain", "--json", "--corpus"])
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    assert_eq!(names.status.code(), Some(0), "{}", stderr(&names));
    let names: serde_json::Value = serde_json::from_slice(&names.stdout).unwrap();
    assert_eq!(names[0]["name"], "Gudmund");
    assert!(names.as_array().is_some_and(|rows| {
        rows.iter().all(|row| row["name"] != "no-such-name")
            && rows.iter().any(|row| row["name"] == "Sigrun")
    }));
}

#[test]
fn query_json_refuses_undefined_combinations() {
    let ordinary = bin()
        .arg("query")
        .arg("adjacent(camelot, the-lake)")
        .arg("--corpus")
        .arg(camelot())
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(ordinary.status.code(), Some(2));
    assert!(stderr(&ordinary).contains("--json is defined only"));

    let wrong_layer = bin()
        .args(["query", "name", "--layer", "possible", "--json", "--corpus"])
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    assert_eq!(wrong_layer.status.code(), Some(2));
    assert!(stderr(&wrong_layer).contains("only for --layer certain"));
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
        .arg(camelot())
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
        .arg(camelot())
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
fn fork_management_commands_round_trip_and_alias_preflight() {
    let workspace = tmp("fork-management");

    let created = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .arg("new")
        .arg("--corpus")
        .arg(camelot())
        .args(["--name", "all-ops"])
        .output()
        .unwrap();
    assert_eq!(created.status.code(), Some(0), "{}", stdout(&created));

    let listed = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .arg("list")
        .output()
        .unwrap();
    assert_eq!(listed.status.code(), Some(0), "{}", stdout(&listed));
    assert!(stdout(&listed).contains("all-ops:"), "{}", stdout(&listed));

    let added = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["add", "all-ops", "keeps(merlin, excalibur)"])
        .output()
        .unwrap();
    assert_eq!(added.status.code(), Some(0), "{}", stdout(&added));
    assert!(
        stdout(&added).contains("added keeps("),
        "{}",
        stdout(&added)
    );

    let supposed = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["suppose", "all-ops", "d-0006", "false"])
        .output()
        .unwrap();
    assert_eq!(supposed.status.code(), Some(0), "{}", stdout(&supposed));

    let suppressed = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["suppress", "all-ops", "d-0006"])
        .output()
        .unwrap();
    assert_eq!(suppressed.status.code(), Some(0), "{}", stdout(&suppressed));

    let restricted = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args([
            "restrict",
            "all-ops",
            "--scope",
            "keeps",
            "--visibility",
            "kept-present",
        ])
        .output()
        .unwrap();
    assert_eq!(restricted.status.code(), Some(0), "{}", stdout(&restricted));

    let nested = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["preflight", "all-ops"])
        .output()
        .unwrap();
    let alias = bin()
        .arg("preflight")
        .arg("-w")
        .arg(&workspace)
        .arg("all-ops")
        .output()
        .unwrap();
    assert_eq!(nested.status.code(), Some(0), "{}", stdout(&nested));
    assert_eq!(alias.status.code(), nested.status.code());
    assert_eq!(stdout(&alias), stdout(&nested));
    assert!(stdout(&nested).contains("pre-flight clean"));

    let promoted = bin()
        .args(["fork", "-w"])
        .arg(&workspace)
        .args(["promote", "all-ops"])
        .output()
        .unwrap();
    assert_eq!(promoted.status.code(), Some(0), "{}", stdout(&promoted));
    assert!(
        stdout(&promoted).contains("rq-draft slug:"),
        "{}",
        stdout(&promoted)
    );
}

#[test]
fn every_timeline_read_returns_its_documented_shape() {
    let chronology = repo().join("examples/norse");
    let episodes = repo().join("crates/dli-world/tests/data/episodes.yaml");

    let show = bin()
        .args(["timeline", "show", "--corpus"])
        .arg(&chronology)
        .args(["--at", "the-binding-of-fenrir"])
        .output()
        .unwrap();
    assert_eq!(show.status.code(), Some(0), "{}", stdout(&show));
    assert!(stdout(&show).contains("layer 0:"), "{}", stdout(&show));
    assert!(
        stdout(&show).contains("at the-binding-of-fenrir:"),
        "{}",
        stdout(&show)
    );

    let order = bin()
        .args(["timeline", "order", "--corpus"])
        .arg(&chronology)
        .output()
        .unwrap();
    assert_eq!(order.status.code(), Some(0), "{}", stdout(&order));
    assert!(
        stdout(&order).contains("chronology acyclic;"),
        "{}",
        stdout(&order)
    );

    let knows = bin()
        .args(["timeline", "knows", "--episodes"])
        .arg(&episodes)
        .args(["--at", "2"])
        .output()
        .unwrap();
    assert_eq!(knows.status.code(), Some(0), "{}", stdout(&knows));
    assert!(
        stdout(&knows).contains("letter-delivered"),
        "{}",
        stdout(&knows)
    );

    let known_empty = bin()
        .args(["timeline", "knows", "--episodes"])
        .arg(&episodes)
        .args(["--at", "1", "--who", "wren"])
        .output()
        .unwrap();
    assert_eq!(
        known_empty.status.code(),
        Some(0),
        "{}",
        stderr(&known_empty)
    );
    assert!(
        stdout(&known_empty).contains("wren knows nothing"),
        "{}",
        stdout(&known_empty)
    );

    let unknown = bin()
        .args(["timeline", "knows", "--episodes"])
        .arg(&episodes)
        .args(["--at", "2", "--who", "no-such-cast"])
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2), "{}", stdout(&unknown));
    assert!(
        stderr(&unknown).contains("unknown cast knower 'no-such-cast'"),
        "{}",
        stderr(&unknown)
    );

    let folded = bin()
        .args(["timeline", "episodes", "--episodes"])
        .arg(&episodes)
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(folded.status.code(), Some(0), "{}", stdout(&folded));
    assert!(
        stdout(&folded).contains("3 episode(s); 1 loop(s) resolved, 0 still open"),
        "{}",
        stdout(&folded)
    );

    let phases = bin()
        .args(["timeline", "phases", "--episodes"])
        .arg(&episodes)
        .arg("e01")
        .output()
        .unwrap();
    assert_eq!(phases.status.code(), Some(0), "{}", stdout(&phases));
    assert!(
        stdout(&phases).contains("episode anchor: season = index 0"),
        "{}",
        stdout(&phases)
    );
}

#[test]
fn timeline_findings_and_errors_have_distinct_exit_codes() {
    let unknown = bin()
        .args(["timeline", "show", "--corpus"])
        .arg(repo().join("examples/norse"))
        .args(["--at", "no-such-point"])
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    assert!(
        unknown.stdout.is_empty(),
        "an invalid cursor emitted partial output:\n{}",
        stdout(&unknown)
    );

    let dir = tmp("timeline-open-loop");
    let episodes = dir.join("episodes.yaml");
    let source =
        fs::read_to_string(repo().join("crates/dli-world/tests/data/episodes.yaml")).unwrap();
    fs::write(
        &episodes,
        source.replace("closes: [who-sent-the-letter]", "closes: []"),
    )
    .unwrap();
    let open = bin()
        .args(["timeline", "episodes", "--episodes"])
        .arg(&episodes)
        .arg("--check")
        .output()
        .unwrap();
    assert_eq!(open.status.code(), Some(1), "{}", stderr(&open));
    assert!(stdout(&open).contains("open loop never resolved"));
}

#[test]
fn check_proof_replays_a_declared_practice() {
    let corpus = repo().join("examples/norse");
    let proof = corpus.join("seidr.dli");
    let output = bin()
        .arg("check-proof")
        .arg(&proof)
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    let out = stdout(&output);
    assert!(out.contains("proof the-seeress-answers in seidr"), "{out}");
    assert!(out.contains("verdict: holds"), "{out}");
    assert!(out.contains("grade: checked"), "{out}");
}

#[test]
fn check_proof_requires_selection_when_a_file_declares_multiple_proofs() {
    let dir = tmp("proof-selection");
    let proof_path = dir.join("proofs.dli");
    let original = fs::read_to_string(repo().join("examples/norse/seidr.dli")).unwrap();
    fs::write(
        &proof_path,
        format!(
            "{original}\n\
             proof second-reading speaker the-seeress in seidr {{\n\
             \x20 assume p1 omen\n\
             \x20 close omen expect blocked\n\
             }}\n"
        ),
    )
    .unwrap();
    let corpus = repo().join("examples/norse");

    let ambiguous = bin()
        .arg("check-proof")
        .arg(&proof_path)
        .arg("--corpus")
        .arg(&corpus)
        .output()
        .unwrap();
    assert_eq!(ambiguous.status.code(), Some(2));
    assert!(stderr(&ambiguous).contains("declares multiple proofs"));

    let first = bin()
        .arg("check-proof")
        .arg(&proof_path)
        .arg("--corpus")
        .arg(&corpus)
        .args(["--proof", "the-seeress-answers"])
        .output()
        .unwrap();
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    assert!(stdout(&first).contains("proof the-seeress-answers"));

    let second = bin()
        .arg("check-proof")
        .arg(&proof_path)
        .arg("--corpus")
        .arg(&corpus)
        .args(["--proof", "second-reading"])
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(1), "{}", stderr(&second));
    assert!(stdout(&second).contains("proof second-reading"));

    let missing = bin()
        .arg("check-proof")
        .arg(&proof_path)
        .arg("--corpus")
        .arg(&corpus)
        .args(["--proof", "missing"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("no proof named 'missing'"));
}

#[test]
fn check_proof_findings_and_errors_have_distinct_exit_codes() {
    let dir = tmp("proof-verdicts");
    let bad_expectation = dir.join("bad-expectation.dli");
    let proof = fs::read_to_string(repo().join("examples/norse/seidr.dli")).unwrap();
    fs::write(
        &bad_expectation,
        proof.replace("expect holds", "expect fails"),
    )
    .unwrap();
    let findings = bin()
        .arg("check-proof")
        .arg(&bad_expectation)
        .arg("--corpus")
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    assert_eq!(findings.status.code(), Some(1), "{}", stderr(&findings));
    assert!(stdout(&findings).contains("proof.unknown-expectation"));

    let missing = bin()
        .arg("check-proof")
        .arg(dir.join("missing.dli"))
        .arg("--corpus")
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
}

#[test]
fn check_proof_distinguishes_invalid_shape_from_unmet_expectation() {
    let dir = tmp("proof-shape-message");
    let proof_path = dir.join("unknown-role.dli");
    let proof = fs::read_to_string(repo().join("examples/norse/seidr.dli")).unwrap();
    fs::write(
        &proof_path,
        proof.replace(
            "proof the-seeress-answers speaker the-seeress in seidr",
            "proof the-seeress-answers reader the-seeress in seidr",
        ),
    )
    .unwrap();

    let output = bin()
        .arg("check-proof")
        .arg(&proof_path)
        .arg("--corpus")
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    let out = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{out}");
    assert!(out.contains("proof.unknown-role"), "{out}");
    assert!(
        out.contains("the declared expectation was met, but the proof did not check"),
        "{out}"
    );
    assert!(!out.contains("expectation was not met"), "{out}");
}

#[test]
fn malformed_proof_child_blocks_check_replay_and_formatting() {
    let dir = tmp("proof-malformed-child");
    let proof_path = dir.join("malformed.dli");
    let proof = fs::read_to_string(repo().join("examples/norse/seidr.dli")).unwrap();
    fs::write(
        &proof_path,
        proof.replace(
            "  spend p3\n",
            "  spend p3\n  wholly-invalid proof syntax\n",
        ),
    )
    .unwrap();

    let checked_proof = bin()
        .arg("check-proof")
        .arg(&proof_path)
        .arg("--corpus")
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    assert_eq!(checked_proof.status.code(), Some(1));
    assert!(stdout(&checked_proof).contains("syntax finding"));
    assert!(stdout(&checked_proof).contains("proof line cannot begin"));

    let checked_document = bin().arg("check").arg(&proof_path).output().unwrap();
    assert_eq!(checked_document.status.code(), Some(1));
    assert!(stdout(&checked_document).contains("dli::syntax::malformed"));

    let formatted = bin()
        .args(["fmt", "--check"])
        .arg(&proof_path)
        .output()
        .unwrap();
    assert_eq!(formatted.status.code(), Some(2));
    assert!(stdout(&formatted).contains("refusing to format"));
    assert!(
        fs::read_to_string(&proof_path)
            .unwrap()
            .contains("wholly-invalid proof syntax"),
        "formatter changed malformed input"
    );
}

#[test]
fn malformed_instantiate_tails_block_check_replay_and_formatting() {
    let original = fs::read_to_string(repo().join("examples/norse/seidr.dli")).unwrap();
    let valid = "working = prophecy, seer = the-seeress";
    for (name, malformed) in [
        ("trailing-comma", "working = prophecy, seer = the-seeress,"),
        (
            "dangling-binding",
            "working = prophecy, seer = the-seeress, dangling",
        ),
    ] {
        let dir = tmp(&format!("proof-{name}"));
        let proof_path = dir.join("malformed.dli");
        fs::write(&proof_path, original.replace(valid, malformed)).unwrap();

        let checked = bin()
            .arg("check-proof")
            .arg(&proof_path)
            .arg("--corpus")
            .arg(repo().join("examples/norse"))
            .output()
            .unwrap();
        assert_eq!(
            checked.status.code(),
            Some(1),
            "{name}: {}",
            stdout(&checked)
        );
        assert!(
            stdout(&checked).contains("syntax finding"),
            "{name}: {}",
            stdout(&checked)
        );

        let formatted = bin()
            .args(["fmt", "--check"])
            .arg(&proof_path)
            .output()
            .unwrap();
        assert_eq!(
            formatted.status.code(),
            Some(2),
            "{name}: {}",
            stdout(&formatted)
        );
        assert!(
            stdout(&formatted).contains("refusing to format"),
            "{name}: {}",
            stdout(&formatted)
        );
    }
}

#[test]
fn proof_atoms_and_quoted_bindings_share_the_parser_grammar() {
    let original = fs::read_to_string(repo().join("examples/norse/seidr.dli")).unwrap();
    let dir = tmp("proof-grammar-consistency");

    let quoted = dir.join("quoted.dli");
    fs::write(
        &quoted,
        original.replace("seer = the-seeress", "seer = \"the, seeress\""),
    )
    .unwrap();
    let checked = bin()
        .arg("check-proof")
        .arg(&quoted)
        .arg("--corpus")
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    assert_eq!(checked.status.code(), Some(0), "{}", stdout(&checked));
    assert!(stdout(&checked).contains("verdict: holds"));

    let unterminated = dir.join("unterminated.dli");
    fs::write(
        &unterminated,
        original.replace(
            "conclusion foretold(the-seeress, doom)",
            "conclusion foretold(the-seeress, doom",
        ),
    )
    .unwrap();
    let checked_proof = bin()
        .arg("check-proof")
        .arg(&unterminated)
        .arg("--corpus")
        .arg(repo().join("examples/norse"))
        .output()
        .unwrap();
    assert_eq!(checked_proof.status.code(), Some(1));
    assert!(stdout(&checked_proof).contains("syntax finding"));

    let checked_document = bin().arg("check").arg(&unterminated).output().unwrap();
    assert_eq!(checked_document.status.code(), Some(1));
    assert!(stdout(&checked_document).contains("dli::syntax::malformed"));

    let formatted = bin()
        .args(["fmt", "--check"])
        .arg(&unterminated)
        .output()
        .unwrap();
    assert_eq!(formatted.status.code(), Some(2));
    assert!(stdout(&formatted).contains("refusing to format"));
}

#[test]
fn export_emits_complete_world_artifacts_and_manifests() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = repo.join("examples/norse");
    let profiles = corpus.join("export-profiles.json");
    for (target, profile_name, artifact_field) in [
        ("articy", "video/nine-realms@1", "entities"),
        ("csv", "tabular/nine-realms@1", "content"),
        ("static-wiki", "wiki/nine-realms@1", "pages"),
    ] {
        let output = bin()
            .args(["export", "--profile", target, "--corpus"])
            .arg(&corpus)
            .arg("--profiles")
            .arg(&profiles)
            .args(["--profile-name", profile_name, "--commit", "abc123"])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{target}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let rendered: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            !rendered["artifact"][artifact_field].is_null(),
            "{target}: {rendered}"
        );
        assert_eq!(rendered["manifest"]["commit"], "abc123");
        assert_eq!(rendered["manifest"]["layer"], "certain");
    }

    let output = bin()
        .args(["export", "--profile", "csv", "--corpus"])
        .arg(&corpus)
        .arg("--profiles")
        .arg(&profiles)
        .args(["--profile-name", "tabular/nine-realms@1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let rendered: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        rendered["manifest"]["commit"]
            .as_str()
            .is_some_and(|commit| commit.starts_with("content-sha256:")),
        "{rendered}"
    );
}

#[test]
fn context_pack_export_emits_complete_artifact() {
    let corpus = repo().join("examples/norse");
    let context = bin()
        .args(["export", "--profile", "context-pack", "--corpus"])
        .arg(&corpus)
        .args(["--commit", "cli-smoke"])
        .output()
        .unwrap();
    assert_eq!(context.status.code(), Some(0), "{}", stdout(&context));
    let context: serde_json::Value = serde_json::from_slice(&context.stdout).unwrap();
    assert!(
        context["artifact"]["text"]
            .as_str()
            .is_some_and(|text| text.contains("canon: nine-realms")),
        "{context}"
    );
    assert_eq!(context["manifest"]["commit"], "cli-smoke");
}

#[test]
fn export_rejects_a_named_profile_for_another_target() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = repo.join("examples/norse");
    let output = bin()
        .args(["export", "--profile", "csv", "--corpus"])
        .arg(&corpus)
        .arg("--profiles")
        .arg(corpus.join("export-profiles.json"))
        .args(["--profile-name", "wiki/nine-realms@1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("requested Csv"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// H7: a downstream consumer closing the pipe early (`head`) must not panic the
/// process — the standard SIGPIPE-abort a chatty CLI would otherwise hit.
#[test]
fn broken_pipe_does_not_panic() {
    let pipeline = format!(
        "{bin:?} timeline order -c {corpus:?} | head -1",
        bin = env!("CARGO_BIN_EXE_deloreis"),
        corpus = camelot(),
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
        .arg("adjacent(camelot, the-lake)")
        .arg("--corpus")
        .arg(camelot())
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
        .arg("adjacent(camelot, the-lake)")
        .arg("--corpus")
        .arg(camelot())
        .arg("--as-of")
        .arg("arthurs-reign")
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
        .arg(camelot())
        .arg("--dialect")
        .arg("old-speech")
        .output()
        .unwrap();
    let out = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "{out}");
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

#[test]
fn name_check_clean_candidate_exits_zero() {
    let output = bin()
        .arg("name")
        .arg("check")
        .arg("Lena")
        .arg("--corpus")
        .arg(camelot())
        .arg("--dialect")
        .arg("old-speech")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
}

#[test]
fn init_import_and_mcp_serve_the_imported_world() {
    use std::io::Write;
    use std::process::Stdio;
    let root = tmp("init-import-project");
    let output = bin().arg("init").arg(&root).output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let output = bin()
        .arg("import")
        .arg(camelot())
        .arg("--project")
        .arg(&root)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let mut child = bin()
        .arg("mcp")
        .arg("--project")
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"author.open\",\"arguments\":{\"scope\":\"world\"}}}\n").unwrap();
    let response = child.wait_with_output().unwrap();
    assert!(response.status.success(), "{}", stderr(&response));
    let frame: serde_json::Value = serde_json::from_slice(&response.stdout).unwrap();
    let body: serde_json::Value =
        serde_json::from_str(frame["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(body["ok"], true, "{body}");
    assert!(body["result"]["workspace"].is_string());
    let repeated = bin()
        .arg("import")
        .arg(camelot())
        .arg("--project")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        !repeated.status.success(),
        "a second import must not replace the world"
    );
}

#[test]
fn init_refuses_to_overwrite_a_manifest() {
    let root = tmp("init-existing-project");
    fs::write(root.join("deloreis.toml"), "existing content\n").unwrap();
    assert!(
        !bin()
            .arg("init")
            .arg(&root)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(root.join("deloreis.toml")).unwrap(),
        "existing content\n"
    );
}

#[test]
fn import_ref_fence_refuses_a_second_writer() {
    let dir = tmp("import-ref-fence");
    let store = dli_lorestore::Lorestore::open(dir).unwrap();
    let before = store
        .branch_ref(dli_lorestore::MAIN_REF)
        .unwrap()
        .unwrap()
        .digest()
        .unwrap();
    let source = fs::read_to_string(camelot()).unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(&source);
    assert!(diagnostics.is_empty());
    let pack = dli_world::Pack {
        name: "camelot".to_owned(),
        documents: vec![document],
    };
    let committed = store
        .import_pack_at_ref(&pack, Some(before.as_str()))
        .unwrap();
    let error = store
        .import_pack_at_ref(&pack, Some(before.as_str()))
        .unwrap_err();
    assert!(
        matches!(error, dli_lorestore::StoreError::RefSnapshotConflict { .. }),
        "{error}"
    );
    assert_eq!(store.root_hash().unwrap().unwrap(), committed);
}

#[test]
fn simultaneous_imports_accept_exactly_one_world() {
    let dir = tmp("import-concurrent");
    let init = bin().arg("init").arg(&dir).output().unwrap();
    assert!(init.status.success(), "{}", stderr(&init));
    let children: Vec<_> = (0..8)
        .map(|_| {
            bin()
                .arg("import")
                .arg(camelot())
                .arg("--project")
                .arg(&dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let outputs: Vec<_> = children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();
    assert_eq!(
        outputs.iter().filter(|o| o.status.success()).count(),
        1,
        "{outputs:?}"
    );
}

#[test]
fn import_rejects_invalid_manifest_before_changing_store() {
    let dir = tmp("import-invalid-manifest");
    assert!(
        bin()
            .arg("init")
            .arg(&dir)
            .output()
            .unwrap()
            .status
            .success()
    );
    let manifest = dir.join("deloreis.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, text.replace("version = 1", "version = 999")).unwrap();
    let output = bin()
        .arg("import")
        .arg(camelot())
        .arg("--project")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported"),
        "{}",
        stderr(&output)
    );
    let store = dli_lorestore::Lorestore::open(dir.join(".deloreis/store")).unwrap();
    assert!(store.head_pack_name().unwrap().is_none());
}

#[test]
fn import_refuses_a_project_pinned_to_genesis() {
    let dir = tmp("import-pinned-project");
    assert!(
        bin()
            .arg("init")
            .arg(&dir)
            .output()
            .unwrap()
            .status
            .success()
    );
    let store = dli_lorestore::Lorestore::open(dir.join(".deloreis/store")).unwrap();
    let genesis = store.root_hash().unwrap().unwrap();
    let manifest = dir.join("deloreis.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(
        &manifest,
        format!("{text}head = \"{}\"\n", genesis.as_str()),
    )
    .unwrap();
    let output = bin()
        .arg("import")
        .arg(camelot())
        .arg("--project")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("pinned head"),
        "{}",
        stderr(&output)
    );
    assert_eq!(store.root_hash().unwrap().unwrap(), genesis);
}

#[cfg(unix)]
#[test]
fn import_refuses_a_manifest_linked_to_a_different_project() {
    let first = tmp("import-linked-first");
    let second = tmp("import-linked-second");
    for dir in [&first, &second] {
        assert!(
            bin()
                .arg("init")
                .arg(dir)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    fs::remove_file(first.join("deloreis.toml")).unwrap();
    std::os::unix::fs::symlink(second.join("deloreis.toml"), first.join("deloreis.toml")).unwrap();
    let output = bin()
        .arg("import")
        .arg(camelot())
        .arg("--project")
        .arg(&first)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("manifest must resolve"),
        "{}",
        stderr(&output)
    );
    let store = dli_lorestore::Lorestore::open(first.join(".deloreis/store")).unwrap();
    assert!(store.head_pack_name().unwrap().is_none());
}

#[test]
fn check_suggests_a_declared_entity_for_a_misspelled_subject() {
    let dir = tmp("entity-typo");
    let doc = dir.join("story.dli");
    fs::write(
        &doc,
        "scope story\nperson wren \"Wren\" proposed author\nwrne.home = ashby proposed author\n",
    )
    .unwrap();
    let output = bin().arg("check").arg(&doc).output().unwrap();
    let out = stdout(&output);
    assert!(out.contains("entity.undeclared"), "{out}");
    assert!(out.contains("did you mean `wren`"), "{out}");
    assert!(out.contains(":3:1"), "{out}");
}

#[test]
fn typo_hints_cover_targets_and_stay_in_scope() {
    let dir = tmp("entity-typo-scopes");
    fs::write(dir.join("declarations.dli"), "scope story\nperson wren \"Wren\" proposed author\nplace ashby \"Ashby\" proposed author\n").unwrap();
    fs::write(
        dir.join("claims.dli"),
        "scope story\nwren.home = ashby proposed author\nwren.home = ashbee proposed author\nfriend(wren, wren) proposed author\nfriend(wren, wrne) proposed author\n",
    )
    .unwrap();
    fs::write(
        dir.join("other.dli"),
        "scope elsewhere\nperson wreck \"Wreck\" proposed author\n",
    )
    .unwrap();
    let output = bin().arg("check").arg(&dir).output().unwrap();
    let out = stdout(&output);
    assert!(
        out.contains("`ashbee` is not declared; did you mean `ashby`"),
        "{out}"
    );
    assert!(
        out.contains("`wrne` is not declared; did you mean `wren`"),
        "{out}"
    );
    assert!(!out.contains("did you mean `wreck`"), "{out}");
}

#[test]
fn typo_hints_cover_indented_slots_and_unicode() {
    let dir = tmp("entity-typo-indented");
    let doc = dir.join("story.dli");
    fs::write(&doc, "scope story\nperson élise \"Élise\" proposed author\n  élisa.home = ashby proposed author\n").unwrap();
    let output = bin().arg("check").arg(&doc).output().unwrap();
    let out = stdout(&output);
    assert!(
        out.contains("`élisa` is not declared; did you mean `élise`"),
        "{out}"
    );
    assert!(out.contains(":3:3"), "{out}");
}

#[test]
fn typo_hints_handle_long_identifiers() {
    let dir = tmp("entity-typo-long");
    let doc = dir.join("story.dli");
    let name = format!("person{}", "a".repeat(150));
    let typo = format!("{name}b");
    fs::write(&doc, format!("scope story\nperson {name} \"Long\" proposed author\n{typo}.home = place proposed author\n")).unwrap();
    let output = bin().arg("check").arg(&doc).output().unwrap();
    assert!(
        stdout(&output).contains(&format!("did you mean `{name}`")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn typo_hints_do_not_treat_untyped_values_as_entities() {
    let dir = tmp("entity-typo-concept");
    let doc = dir.join("story.dli");
    fs::write(&doc, "scope story\nperson ned \"Ned\" proposed author\nperson wren \"Wren\" proposed author\nwren.hair = red proposed author\n").unwrap();
    let output = bin().arg("check").arg(&doc).output().unwrap();
    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn timeline_orders_certain_events_in_ordered_eras() {
    let output = bin()
        .args(["timeline", "show", "--corpus"])
        .arg(camelot())
        .args(["--at", "the-sword-drawn"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    let text = stdout(&output);
    let cursor = text
        .lines()
        .find(|line| line.starts_with("at the-sword-drawn:"))
        .unwrap();
    assert!(
        cursor.contains("before [the-round-table-founded, the-grail-quest"),
        "{cursor}"
    );
}

#[test]
fn as_of_queries_project_fluents_and_recompute_rules() {
    let dir = tmp("temporal-query-projection");
    let corpus = dir.join("world.dli");
    fs::write(&corpus, "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\ninertial home ratified d-0001\nstate wren.home = old since first ratified d-0001\nstate wren.home = new since second ratified d-0001\nrule copy-home ratified d-0001\n  resides($who, $where) :- home($who, $where)\n").unwrap();
    for (point, expected, absent) in [("first", "old", "new"), ("second", "new", "old")] {
        for pred in ["home", "resides"] {
            let output = bin()
                .arg("query")
                .arg(format!("{pred}(wren, $where)"))
                .arg("--corpus")
                .arg(&corpus)
                .args(["--as-of", point, "--layer", "certain"])
                .output()
                .unwrap();
            let text = stdout(&output);
            assert_eq!(output.status.code(), Some(0), "{text} {}", stderr(&output));
            assert!(
                text.contains(&format!("{pred}(wren, {expected})")),
                "{text}"
            );
            assert!(!text.contains(&format!("{pred}(wren, {absent})")), "{text}");
        }
    }
}

#[test]
fn historical_exports_project_state_and_bind_cutoff() {
    let dir = tmp("temporal-export-projection");
    let corpus = dir.join("world.dli");
    fs::write(&corpus, "scope storybook\nratify d-0001\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\ninertial home ratified d-0001\nstate wren.home = old since first ratified d-0001\nstate wren.home = new since second ratified d-0001\n").unwrap();
    let mut hashes = Vec::new();
    for (point, expected, absent) in [
        ("first", "home(wren, old)", "home(wren, new)"),
        ("second", "home(wren, new)", "home(wren, old)"),
    ] {
        let output = bin()
            .args(["export", "--profile", "context-pack", "--corpus"])
            .arg(&corpus)
            .args(["--as-of", point])
            .output()
            .unwrap();
        let text = stdout(&output);
        assert_eq!(output.status.code(), Some(0), "{text} {}", stderr(&output));
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let rendered = json["artifact"]["text"].as_str().unwrap();
        assert!(rendered.contains(expected), "{rendered}");
        assert!(!rendered.contains(absent), "{rendered}");
        assert_eq!(json["manifest"]["as_of"], point);
        hashes.push(json["manifest"]["profile_hash"].clone());
    }
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn historical_export_keeps_independent_timeless_claim() {
    let dir = tmp("temporal-export-timeless");
    let corpus = dir.join("world.dli");
    fs::write(&corpus, "scope storybook\nratify d-0001\nratify d-0002\nevent first ratified d-0001\nevent second ratified d-0001\nbefore first -> second ratified d-0001\ninertial home ratified d-0001\nstate wren.home = old since first ratified d-0001\nstate wren.home = new since second ratified d-0001\nfact wren home old ratified d-0002\n").unwrap();
    let output = bin()
        .args(["export", "--profile", "context-pack", "--corpus"])
        .arg(&corpus)
        .args(["--as-of", "second"])
        .output()
        .unwrap();
    let text = stdout(&output);
    assert_eq!(output.status.code(), Some(0), "{text} {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let rendered = json["artifact"]["text"].as_str().unwrap();
    assert!(rendered.contains("home(wren, old)"), "{rendered}");
    assert!(rendered.contains("home(wren, new)"), "{rendered}");
    assert_eq!(rendered.matches("home(wren, old)").count(), 1, "{rendered}");
}
