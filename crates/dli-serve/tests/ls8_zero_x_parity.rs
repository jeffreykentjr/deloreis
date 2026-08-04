//! The `0.x` byte-identity oracle: LS-8 must not have moved any wire below
//! contract 1.0, **including the refusals**.
//!
//! # Why this file exists
//!
//! The rest of the suite asserts what the server *should* do. This one asserts
//! what it *did* do — every response in `data/zero_x_parity.json` was captured
//! by running the corpus against the pre-LS-8 build at `e477306` and is replayed
//! against the current one. That difference matters, because LS-8's risk is not
//! in the behaviour anyone thought to specify; it is in the behaviour nobody
//! did.
//!
//! The whole existing gate — 132 contract fixtures replaying cold, a full
//! `nextest` in two lanes — passed while three `0.x` breaks were live:
//!
//! 1. `Claim` is carried by **two** ops. Relaxing `atom` to make contract 1.0
//!    usable un-required it on `dli.check-draft` as well, which had no registry
//!    entry to re-impose it — a claim missing `atom` went from refused to
//!    *accepted and checked*.
//! 2. `atom` is declared **first** in `Claim`, before the still-required
//!    `quote`. Re-imposing the requirement after the decode meant serde failed
//!    on `quote` first, so a claim missing both reported the wrong field.
//! 3. A missing retired field lost to a *post-decode* unknown-field report,
//!    where serde had answered first.
//!
//! None was reachable from a fixture, because a fixture records a verdict
//! someone chose to record. A captured corpus records the verdicts nobody
//! thought about, which is exactly where these lived.
//!
//! # Reading a failure
//!
//! A failure here means a `0.x` caller sees something it did not see before.
//! That is a contract break by default — not a test to update. If the change is
//! genuinely intended, it belongs in the corpus as a `deviates_to` entry with a
//! `why`, reviewed as the wire change it is.
//!
//! The five recorded deviations are all pre-dispatch framing refusals, whose
//! echoed version could not stay identical either way: they used to carry the
//! *build's* `CONTRACT_VERSION`, which this build changed by definition.
#![cfg(feature = "syntax")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use serde_json::{Value, json};

fn corpus() -> Value {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/zero_x_parity.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("read the parity corpus")).unwrap()
}

fn fixture_project() -> (tempfile::TempDir, Sessions, Store) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/dli-worker/fixture-project");
    let temp = tempfile::tempdir().unwrap();
    copy_tree(&source, temp.path());
    let project = Arc::new(Project::load(temp.path()).unwrap());
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let store = Store::with_packs(packs).unwrap();
    let sessions = Sessions::from_project(Arc::clone(&project));
    (temp, sessions, store)
}

fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let to = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir(&to).unwrap();
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// Drive one frame from a fresh project, after the fixed preconditions the
/// corpus was captured with (a workspace and a fork exist, so an op that needs
/// one reaches its real handler rather than an unknown-handle refusal).
fn drive(frame: &Value) -> Value {
    let (_temp, mut sessions, mut store) = fixture_project();
    for pre in [
        json!({ "dli": "0.5.1", "id": "p1", "op": "author.open",
                "params": { "scope": "saltmarket" } }),
        json!({ "dli": "0.5.1", "id": "p2", "op": "dli.fork.new",
                "params": { "document": "saltmarket" } }),
    ] {
        let _ = respond(
            &serde_json::to_string(&pre).unwrap(),
            &mut sessions,
            &mut store,
        );
    }
    let line = serde_json::to_string(frame).unwrap();
    respond(&line, &mut sessions, &mut store).map_or(Value::Null, |response| {
        serde_json::to_value(&response).unwrap()
    })
}

/// Every captured `0.x` response is reproduced exactly, and every recorded
/// deviation deviates exactly as recorded.
#[test]
fn every_captured_zero_x_response_is_reproduced() {
    let corpus = corpus();
    let cases = corpus["cases"]
        .as_array()
        .expect("the corpus is a case list");
    assert!(
        cases.len() > 250,
        "the corpus did not load: {} cases",
        cases.len()
    );
    let baseline = corpus["baseline_commit"].as_str().unwrap();

    let mut deviations = 0;
    for case in cases {
        let label = case["label"].as_str().unwrap();
        let got = drive(&case["frame"]);
        if let Some(expected) = case.get("deviates_to") {
            deviations += 1;
            assert_eq!(
                &got,
                expected,
                "{label}: this is a RECORDED deviation from {baseline}, and it must deviate \
                 exactly as recorded.\nwhy: {}\ncaptured at {baseline}: {}",
                case["why"].as_str().unwrap_or(""),
                case["expect"]
            );
            continue;
        }
        assert_eq!(
            got, case["expect"],
            "{label}: a 0.x wire moved. This response was captured from {baseline} and must \
             be byte-identical. If the change is intended, record it in the corpus as a \
             `deviates_to` entry with a `why` and review it as the contract change it is."
        );
    }
    assert_eq!(
        deviations,
        cases
            .iter()
            .filter(|c| c.get("deviates_to").is_some())
            .count(),
        "every recorded deviation must have been exercised"
    );
}

/// The corpus covers what LS-8 actually touched. A corpus that quietly stopped
/// exercising the retired fields would pass while proving nothing, so the shape
/// of the coverage is asserted, not assumed.
#[test]
fn the_corpus_covers_every_retired_field_and_every_pre_1_0_version() {
    let corpus = corpus();
    let cases = corpus["cases"].as_array().unwrap();
    let labels: Vec<&str> = cases
        .iter()
        .map(|case| case["label"].as_str().unwrap())
        .collect();

    // Every wire below the retirement is exercised.
    for version in dli_serve::ACCEPTED_CONTRACT_VERSIONS
        .iter()
        .filter(|v| !v.starts_with("1."))
    {
        assert!(
            labels
                .iter()
                .any(|label| label.ends_with(&format!("@{version}"))),
            "the corpus does not exercise {version}"
        );
    }

    // Every op carrying a retired field, and — the miss that started this —
    // BOTH ops that carry the shared `Claim`.
    for probe in [
        "query/document-only@0.6.4",
        "author-query/workspace-only@0.6.4",
        "fork-add/fork-only@0.6.4",
        "submit/claim-missing-atom@0.6.4",
        "submit/claim-missing-atom-and-quote@0.6.4",
        "draft/claim-missing-atom@0.6.4",
        "draft/claim-missing-atom-and-quote@0.6.4",
        "query/unknown-key-inside-typed@0.6.4",
    ] {
        assert!(
            labels.contains(&probe),
            "the corpus no longer exercises {probe} — the case that caught a real break"
        );
    }
}
