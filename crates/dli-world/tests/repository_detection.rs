#![allow(clippy::unwrap_used, clippy::expect_used)]
//! VAL-1 — the repository validator's detection corpus.
//!
//! `dli_world::validate_repository` emits eleven codes: V1–V8 and V10–V12.
//! There is deliberately no V9; it closed by absorption, and its native
//! successors are a theorem in `dli-core` and the MIG-1 replay legs, not a
//! mutation of this corpus (`docs/design-notes/native-repository-validator.md`).
//!
//! # Why a corpus and not more unit tests
//!
//! `repository.rs` already carries unit tests that build a tree in a tempdir and
//! assert that *some* finding matches a predicate. Those prove each check can
//! fire. They cannot prove what the validator says about a whole repository,
//! because nothing in them is a committed input and nothing asserts the finding
//! set is *exactly* what was expected — a check that started reporting twice, or
//! reporting against a neighbouring file, would pass every one of them.
//!
//! This suite is the other half, and it is what the consumer's shadow-validate
//! harness accrues against:
//!
//! 1. `baseline/` is a committed canon repository that validates to the **empty**
//!    finding list. A validator that reports nothing is worthless, but so is one
//!    that reports on clean input; the pristine census is the null-oracle guard.
//! 2. Every operator in `operators.yaml` seeds **one** divergence into a copy of
//!    that baseline and names **every** finding that must follow — path, line,
//!    and code. The assertion is set equality, so an unexpected finding fails as
//!    loudly as a missing one.
//! 3. The census proves every emitted code has at least one operator, and that
//!    each operator actually produces the code it claims.
//!
//! Where an operator's expected set has more than one row, the extra rows are
//! documenting a real interlock between checks (the id is a join key; a misfiled
//! question is also an uncitable one), not a sloppy seed. The rationale on each
//! record says which.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use dli_world::{RepositoryFinding, validate_repository};

/// Every code `validate_repository` can emit. V9 is absent by ruling (DL-0057).
const EMITTED_CODES: &[&str] = &[
    "V1", "V2", "V3", "V4", "V5", "V6", "V7", "V8", "V10", "V11", "V12",
];

#[derive(Debug, Deserialize)]
struct Corpus {
    operators: Vec<Operator>,
}

#[derive(Debug, Deserialize)]
struct Operator {
    /// The record's stable name; `operator:` in the corpus.
    #[serde(rename = "operator")]
    id: String,
    code: String,
    #[allow(dead_code)]
    description: String,
    seeded_change: SeededChange,
    expected: Vec<ExpectedFinding>,
    #[allow(dead_code)]
    rationale: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SeededChange {
    Write {
        path: String,
        content: String,
    },
    Replace {
        path: String,
        from: String,
        to: String,
    },
    Append {
        path: String,
        content: String,
    },
}

#[derive(Debug, Deserialize)]
struct ExpectedFinding {
    path: String,
    line: usize,
    code: String,
    /// A fragment of the diagnostic that distinguishes this catch from a
    /// different defect landing on the same `(path, line, code)` triple.
    message_contains: String,
}

impl ExpectedFinding {
    fn key(&self) -> (&str, usize, &str) {
        (&self.path, self.line, &self.code)
    }
}

fn finding_key(finding: &RepositoryFinding) -> (&str, usize, &str) {
    (&finding.path, finding.line, &finding.code)
}

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/validator-detection")
}

fn load_corpus() -> Corpus {
    let path = corpus_root().join("operators.yaml");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    serde_yaml_ng::from_str(&source)
        .unwrap_or_else(|error| panic!("parsing {}: {error}", path.display()))
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for entry in fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

/// Apply one seeded change to a staged copy of the baseline.
///
/// `replace` requires its `from` to occur exactly once: a seed that matched
/// nothing (or matched twice) would quietly test something other than what it
/// claims, and that is the failure mode a detection corpus exists to prevent.
fn apply(root: &Path, operator: &str, change: &SeededChange) {
    match change {
        SeededChange::Write { path, content } => {
            let target = root.join(path);
            fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
            fs::write(target, content).expect("write");
        }
        SeededChange::Replace { path, from, to } => {
            let target = root.join(path);
            let source = fs::read_to_string(&target)
                .unwrap_or_else(|error| panic!("{operator}: reading {path}: {error}"));
            let hits = source.matches(from.as_str()).count();
            assert_eq!(
                hits, 1,
                "{operator}: seed `from` must occur exactly once in {path}, found {hits}"
            );
            fs::write(target, source.replace(from.as_str(), to)).expect("write");
        }
        SeededChange::Append { path, content } => {
            let target = root.join(path);
            let mut source = fs::read_to_string(&target)
                .unwrap_or_else(|error| panic!("{operator}: reading {path}: {error}"));
            source.push_str(content);
            fs::write(target, source).expect("write");
        }
    }
}

#[test]
fn pristine_baseline_validates_clean() {
    let findings = validate_repository(corpus_root().join("baseline"));
    assert!(
        findings.is_empty(),
        "the committed baseline must validate to the empty finding list; got {findings:#?}"
    );
}

/// The codes the validator can actually emit, read out of its own source.
///
/// Without this, the census would only prove the corpus agrees with a constant
/// in the corpus. A new check landing a `V13` — or `V9` returning — has to pass
/// through here.
fn codes_in_source() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/repository.rs");
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
    let mut codes = BTreeSet::new();
    for candidate in source.split('"').skip(1).step_by(2) {
        let Some(digits) = candidate.strip_prefix('V') else {
            continue;
        };
        if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
            codes.insert(candidate.to_owned());
        }
    }
    codes
}

#[test]
fn the_declared_code_set_is_the_one_the_validator_emits() {
    let declared: BTreeSet<String> = EMITTED_CODES
        .iter()
        .map(|code| (*code).to_owned())
        .collect();
    assert_eq!(
        codes_in_source(),
        declared,
        "the codes appearing as string literals in repository.rs must match EMITTED_CODES; a new \
         check needs an operator, and V9 must stay absent (DL-0057)"
    );
}

#[test]
fn every_emitted_code_has_an_operator() {
    let corpus = load_corpus();
    let covered: BTreeSet<&str> = corpus
        .operators
        .iter()
        .map(|operator| operator.code.as_str())
        .collect();
    let expected: BTreeSet<&str> = EMITTED_CODES.iter().copied().collect();
    assert_eq!(
        covered, expected,
        "every emitted code needs at least one operator and no operator may claim a code the \
         validator does not emit"
    );
    assert!(
        !covered.contains("V9"),
        "V9 closed by absorption; its coverage is the derivation theorem plus mig1_replay, never \
         a corpus mutation"
    );
}

#[test]
fn operator_names_are_unique() {
    let corpus = load_corpus();
    let mut seen = BTreeSet::new();
    for operator in &corpus.operators {
        assert!(
            seen.insert(operator.id.clone()),
            "duplicate operator id `{}`",
            operator.id
        );
    }
}

/// The corpus README's coverage table must name every operator and no others.
///
/// It is written by hand beside data that already carries the truth, which is
/// exactly how it drifts: the governance docs shipped a count of 25 against a
/// corpus of 26 because a prose number restated a fact nobody was checking. The
/// table is the load-bearing half — a reader uses it to find an operator — so
/// it is the half that gets an assertion.
#[test]
fn the_readme_coverage_table_names_every_operator() {
    let path = corpus_root().join("README.md");
    let readme = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));

    let mut documented = BTreeSet::new();
    for line in readme.lines().filter(|line| line.starts_with("| V")) {
        for name in line.split('`').skip(1).step_by(2) {
            documented.insert(name.to_owned());
        }
    }

    let declared: BTreeSet<String> = load_corpus()
        .operators
        .into_iter()
        .map(|operator| operator.id)
        .collect();
    assert_eq!(
        documented, declared,
        "the README coverage table and operators.yaml must name the same operators"
    );
}

#[test]
fn every_operator_produces_the_code_it_claims() {
    let corpus = load_corpus();
    for operator in &corpus.operators {
        assert!(
            operator
                .expected
                .iter()
                .any(|finding| finding.code == operator.code),
            "operator `{}` claims {} but expects none",
            operator.id,
            operator.code
        );
        assert!(
            !operator.expected.is_empty(),
            "operator `{}` expects no findings; a seed that changes nothing is not an operator",
            operator.id
        );
    }
}

// ===========================================================================
// The finding-shape compatibility promise (design note §3). These are the
// properties the consuming project's shadow-validate harness is told it may
// rely on, so they are asserted rather than merely written down.
// ===========================================================================

#[test]
fn a_missing_root_is_an_empty_list_and_not_a_panic() {
    let staging = tempfile::tempdir().expect("tempdir");
    let findings = validate_repository(staging.path().join("no-such-canon-root"));
    assert!(
        findings.is_empty(),
        "a missing root yields an empty list; deciding that absence is an error belongs to the \
         CLI, not the library. got {findings:#?}"
    );
}

#[test]
fn an_empty_root_is_an_empty_list() {
    let staging = tempfile::tempdir().expect("tempdir");
    assert!(validate_repository(staging.path()).is_empty());
}

#[test]
fn findings_are_sorted_and_shaped_as_promised() {
    let corpus = load_corpus();
    let baseline = corpus_root().join("baseline");
    let staging = tempfile::tempdir().expect("tempdir");
    let root = staging.path().join("canon");
    copy_tree(&baseline, &root);

    // Many operators at once, so the finding list is long enough for the
    // ordering guarantee to mean something. `replace` seeds are skipped: their
    // `from` text stops matching uniquely once a neighbouring seed has landed,
    // and the write and append seeds alone fill the list.
    for operator in &corpus.operators {
        if matches!(operator.seeded_change, SeededChange::Replace { .. }) {
            continue;
        }
        apply(&root, &operator.id, &operator.seeded_change);
    }

    let findings = validate_repository(&root);
    assert!(
        findings.len() > 5,
        "expected a substantial finding list, got {findings:#?}"
    );

    let keys: Vec<(&str, usize, &str, &str)> = findings
        .iter()
        .map(|finding| {
            (
                finding.path.as_str(),
                finding.line,
                finding.code.as_str(),
                finding.message.as_str(),
            )
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(
        keys, sorted,
        "findings must be sorted by (path, line, code, message)"
    );

    let declared: BTreeSet<&str> = EMITTED_CODES.iter().copied().collect();
    for finding in &findings {
        assert_eq!(finding.severity, "error", "severity is always `error`");
        assert!(
            declared.contains(finding.code.as_str()),
            "unexpected code {}",
            finding.code
        );
        assert!(finding.line >= 1, "lines are one-based");
        assert!(
            !finding.path.starts_with('/') && !finding.path.starts_with("./"),
            "paths are relative to the canon root: {}",
            finding.path
        );
        assert!(
            !finding.path.contains('\\'),
            "paths use POSIX separators: {}",
            finding.path
        );
        assert!(
            !finding.message.is_empty(),
            "every finding carries a message"
        );
    }

    // Determinism: the same bytes twice produce the same list.
    assert_eq!(findings, validate_repository(&root));
}

#[test]
fn the_finding_object_serializes_with_the_promised_field_names() {
    let staging = tempfile::tempdir().expect("tempdir");
    let root = staging.path().join("canon");
    fs::create_dir_all(&root).expect("mkdir");
    fs::write(root.join("nameless.md"), "no frontmatter here\n").expect("write");

    let findings = validate_repository(&root);
    let json = serde_json::to_value(&findings).expect("json");
    assert_eq!(
        json,
        serde_json::json!([{
            "path": "nameless.md",
            "line": 1,
            "code": "V1",
            "message": "missing frontmatter block",
            "severity": "error",
        }]),
        "the wire object is {{path, line, code, message, severity}} with those exact names"
    );
}

#[test]
fn every_operator_detects_exactly_its_expected_findings() {
    let corpus = load_corpus();
    let baseline = corpus_root().join("baseline");
    let mut divergences = Vec::new();
    for operator in &corpus.operators {
        let staging = tempfile::tempdir().expect("tempdir");
        let root = staging.path().join("canon");
        copy_tree(&baseline, &root);
        apply(&root, &operator.id, &operator.seeded_change);

        let findings = validate_repository(&root);
        let mut expected: Vec<&ExpectedFinding> = operator.expected.iter().collect();
        expected.sort_by_key(|finding| finding.key());

        // `validate_repository` already sorts, so the two lists line up row for
        // row once the expectations are in the same order.
        let observed: Vec<(&str, usize, &str)> = findings.iter().map(finding_key).collect();
        let wanted: Vec<(&str, usize, &str)> =
            expected.iter().map(|finding| finding.key()).collect();
        let mismatch = if observed == wanted {
            expected
                .iter()
                .zip(&findings)
                .find(|(want, got)| !got.message.contains(&want.message_contains))
                .map(|(want, got)| {
                    format!(
                        "  right rows, wrong catch: expected a message containing {:?}, got {:?}",
                        want.message_contains, got.message
                    )
                })
        } else {
            Some(format!("  expected {wanted:?}\n  observed {observed:?}"))
        };
        if let Some(detail) = mismatch {
            divergences.push(format!(
                "operator `{}` ({}):\n{detail}\n  full findings: {:#?}",
                operator.id,
                operator.code,
                findings
                    .iter()
                    .map(|finding| format!(
                        "{}:{} {} — {}",
                        finding.path, finding.line, finding.code, finding.message
                    ))
                    .collect::<Vec<_>>()
            ));
        }
    }
    assert!(
        divergences.is_empty(),
        "{} operator(s) did not produce their recorded finding set:\n\n{}",
        divergences.len(),
        divergences.join("\n\n")
    );
}
