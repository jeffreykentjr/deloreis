//! Durable simulation job behavior through the public worker dispatch.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// Deterministic poll budget for the async simulation workers. The gate is
/// offline and clock-free (DL-0013), so convergence is bounded by poll COUNT,
/// not wall-clock: under heavy machine load a starved worker simply spaces its
/// polls further apart instead of tripping a real-time deadline and flaking the
/// gate. The budget only runs out when a job genuinely fails to converge.
const MAX_POLLS: u32 = 3000;

/// Spend one unit of the poll budget, panicking (with the last response for
/// context) only when the job has genuinely failed to converge. Replaces a
/// load-sensitive `assert!(Instant::now() < deadline, …)`.
#[track_caller]
fn poll_budget(remaining: &mut u32, context: &Value) {
    *remaining = remaining.checked_sub(1).unwrap_or_else(|| {
        panic!("simulation did not converge within {MAX_POLLS} polls: {context}")
    });
}

fn write_project(root: &Path) -> Arc<Project> {
    fs::write(
        root.join("deloreis.toml"),
        r#"
version = 1
name = "simulation-test"

[paths]
evidence = ".dli-evidence"
cache = ".dli-cache"

[documents.simulation]
sources = ["simulation.dli"]

[games.french-deck]
rules = "french-deck.json"
"#,
    )
    .unwrap();
    fs::write(
        root.join("simulation.dli"),
        "scope simulation\n\npractice accounting proposed {\n  resource balance : linear conserved\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("french-deck.json"),
        include_str!("../../../examples/games/french-deck/rules.json"),
    )
    .unwrap();
    Arc::new(Project::load(root).unwrap())
}

fn stores(project: Arc<Project>) -> (Sessions, Store) {
    let packs = project
        .documents()
        .map(|(id, document)| {
            (
                id.to_owned(),
                document.pack.clone(),
                document.killed.clone(),
            )
        })
        .collect::<Vec<_>>();
    (
        Sessions::from_project(project),
        Store::with_packs(packs).unwrap(),
    )
}

fn drive(request: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let response = respond(&request.to_string(), sessions, store).expect("request has a response");
    serde_json::to_value(response).unwrap()
}

fn canonical_record(value: &Value) -> (Vec<u8>, String) {
    let bytes = serde_json_canonicalizer::to_vec(value).unwrap();
    let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    (bytes, digest)
}

fn start(seed: u64, trials: u64, every: u64) -> Value {
    start_asset("simulation", seed, trials, every)
}

fn start_asset(document: &str, seed: u64, trials: u64, every: u64) -> Value {
    serde_json::json!({
        "dli": "0.5.1",
        "id": "start",
        "op": "dli.simulate.start",
        "params": {
            "document": document,
            "seed": seed,
            "trials": trials,
            "checkpoint_every": every
        }
    })
}

fn status(job_id: &str) -> Value {
    serde_json::json!({
        "dli": "0.5.1",
        "id": "status",
        "op": "dli.simulate.status",
        "params": {"job_id": job_id}
    })
}

fn cancel(job_id: &str) -> Value {
    serde_json::json!({
        "dli": "0.5.1",
        "id": "cancel",
        "op": "dli.simulate.cancel",
        "params": {"job_id": job_id}
    })
}

#[test]
fn job_resumes_after_restart_and_reopens_idempotently() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(Arc::clone(&project));

    let started = drive(&start(41, 5, 2), &mut sessions, &mut store);
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(started["result"]["status"], "pending", "{started}");
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();
    assert!(job_id.starts_with("sha256:"));

    drop(sessions);
    drop(store);
    let (mut sessions, mut store) = stores(Arc::clone(&project));
    let resumed = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(resumed["result"]["status"], "pending", "{resumed}");

    sessions.release_simulation_workers();
    let mut budget = MAX_POLLS;
    let completed = loop {
        let response = drive(&status(&job_id), &mut sessions, &mut store);
        if response["result"]["status"] == "complete" {
            break response;
        }
        poll_budget(&mut budget, &response);
        sessions.release_simulation_workers();
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(completed["result"]["trials_done"], 5, "{completed}");
    assert_eq!(completed["result"]["trials_total"], 5, "{completed}");
    assert_eq!(
        completed["result"]["manifest"]["checkpoint_lineage"],
        serde_json::json!([2, 4, 5]),
        "{completed}"
    );
    assert!(
        completed["result"]["manifest_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:")),
        "{completed}"
    );
    let hex = job_id.strip_prefix("sha256:").unwrap();
    let job_dir = project.evidence_dir().join("simulations").join(hex);
    assert!(!job_dir.join("spec.json").exists());
    assert!(job_dir.join("complete.json").is_file());
    let checkpoints = fs::read_dir(job_dir.join("checkpoints"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(checkpoints.len(), 3);
    assert!(
        checkpoints
            .iter()
            .all(|record| { record["state"]["checkpoint"]["lineage"] == serde_json::json!([]) })
    );
    assert_eq!(
        checkpoints
            .iter()
            .filter(|record| record["previous"].is_null())
            .count(),
        1
    );

    drop(sessions);
    drop(store);
    let (mut sessions, mut store) = stores(project);
    let reopened = drive(&start(41, 5, 2), &mut sessions, &mut store);
    assert_eq!(reopened["result"]["job_id"], job_id, "{reopened}");
    assert_eq!(reopened["result"]["status"], "complete", "{reopened}");
}

#[test]
fn concurrent_runners_converge_on_one_checkpoint_chain() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut left_sessions, mut left_store) = stores(Arc::clone(&project));
    let started = drive(&start(47, 17, 1), &mut left_sessions, &mut left_store);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();

    let (mut right_sessions, mut right_store) = stores(Arc::clone(&project));
    let queued = drive(&status(&job_id), &mut right_sessions, &mut right_store);
    assert_eq!(queued["result"]["status"], "pending", "{queued}");
    left_sessions.release_simulation_workers();
    right_sessions.release_simulation_workers();

    let mut budget = MAX_POLLS;
    let response = loop {
        let response = drive(&status(&job_id), &mut left_sessions, &mut left_store);
        if response["result"]["status"] == "complete" {
            break response;
        }
        poll_budget(&mut budget, &response);
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(response["result"]["trials_done"], 17, "{response}");
    let directory = project
        .evidence_dir()
        .join("simulations")
        .join(job_id.strip_prefix("sha256:").unwrap())
        .join("checkpoints");
    assert_eq!(fs::read_dir(directory).unwrap().count(), 17);
}

#[test]
fn zero_trial_job_completes_without_a_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(Arc::clone(&project));
    let started = drive(&start(48, 0, 1), &mut sessions, &mut store);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();
    sessions.release_simulation_workers();
    let mut budget = MAX_POLLS;
    let response = loop {
        let response = drive(&status(&job_id), &mut sessions, &mut store);
        if response["result"]["status"] == "complete" {
            break response;
        }
        poll_budget(&mut budget, &response);
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(response["result"]["trials_done"], 0, "{response}");
    assert_eq!(
        response["result"]["manifest"]["checkpoint_lineage"],
        serde_json::json!([])
    );
}

#[test]
fn deleting_a_committed_completion_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(Arc::clone(&project));
    let started = drive(&start(46, 1, 1), &mut sessions, &mut store);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();
    sessions.release_simulation_workers();
    let mut budget = MAX_POLLS;
    loop {
        let response = drive(&status(&job_id), &mut sessions, &mut store);
        if response["result"]["status"] == "complete" {
            break;
        }
        poll_budget(&mut budget, &response);
        sessions.release_simulation_workers();
        thread::sleep(Duration::from_millis(10));
    }
    let directory = project
        .evidence_dir()
        .join("simulations")
        .join(job_id.strip_prefix("sha256:").unwrap());
    let completion: Value =
        serde_json::from_slice(&fs::read(directory.join("complete.json")).unwrap()).unwrap();
    fs::write(
        directory.join("spec.json"),
        serde_json_canonicalizer::to_vec(&completion["spec"]).unwrap(),
    )
    .unwrap();
    let healed = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(healed["result"]["status"], "complete", "{healed}");
    assert!(!directory.join("spec.json").exists());

    fs::remove_file(directory.join("complete.json")).unwrap();
    let response = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(response["error"]["code"], "evidence-corrupt", "{response}");
}

#[test]
fn cancellation_is_terminal_across_restart() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(Arc::clone(&project));
    let started = drive(&start(42, 10, 2), &mut sessions, &mut store);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();

    let cancelled = drive(&cancel(&job_id), &mut sessions, &mut store);
    assert_eq!(cancelled["result"]["status"], "cancelled", "{cancelled}");

    drop(sessions);
    drop(store);
    let (mut sessions, mut store) = stores(project);
    let status = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(status["result"]["status"], "cancelled", "{status}");
    assert_eq!(status["result"]["trials_done"], 0, "{status}");
}

#[test]
fn deleting_a_committed_cancellation_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(Arc::clone(&project));
    let started = drive(&start(44, 10, 2), &mut sessions, &mut store);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();
    let cancelled = drive(&cancel(&job_id), &mut sessions, &mut store);
    assert_eq!(cancelled["result"]["status"], "cancelled", "{cancelled}");

    let directory = project
        .evidence_dir()
        .join("simulations")
        .join(job_id.strip_prefix("sha256:").unwrap());
    assert!(!directory.join("spec.json").exists());
    let cancellation: Value =
        serde_json::from_slice(&fs::read(directory.join("cancelled.json")).unwrap()).unwrap();
    fs::write(
        directory.join("spec.json"),
        serde_json_canonicalizer::to_vec(&cancellation["spec"]).unwrap(),
    )
    .unwrap();
    let healed = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(healed["result"]["status"], "cancelled", "{healed}");
    assert!(!directory.join("spec.json").exists());

    fs::remove_file(directory.join("cancelled.json")).unwrap();

    let response = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(response["error"]["code"], "evidence-corrupt", "{response}");
}

#[test]
fn manifest_declared_game_runs_through_the_durable_job_surface() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(project);
    let started = drive(
        &start_asset("french-deck", 45, 5, 2),
        &mut sessions,
        &mut store,
    );
    assert_eq!(started["ok"], true, "{started}");
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();
    sessions.release_simulation_workers();

    let mut budget = MAX_POLLS;
    loop {
        let response = drive(&status(&job_id), &mut sessions, &mut store);
        if response["result"]["status"] == "complete" {
            assert_eq!(response["result"]["trials_done"], 5, "{response}");
            assert_eq!(
                response["result"]["manifest"]["checkpoint_lineage"],
                serde_json::json!([2, 4, 5]),
                "{response}"
            );
            assert_eq!(
                response["result"]["manifest"]["metrics"][0]["grade"], "tested",
                "{response}"
            );
            break;
        }
        poll_budget(&mut budget, &response);
        sessions.release_simulation_workers();
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn committed_checkpoint_corruption_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(Arc::clone(&project));
    let started = drive(&start(43, 2, 1), &mut sessions, &mut store);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();
    sessions.release_simulation_workers();

    let mut budget = MAX_POLLS;
    loop {
        let response = drive(&status(&job_id), &mut sessions, &mut store);
        if response["result"]["status"] == "complete" {
            break;
        }
        poll_budget(&mut budget, &response);
        sessions.release_simulation_workers();
        thread::sleep(Duration::from_millis(10));
    }

    let hex = job_id.strip_prefix("sha256:").unwrap();
    let checkpoint_dir = project
        .evidence_dir()
        .join("simulations")
        .join(hex)
        .join("checkpoints");
    let (checkpoint, mut value) = fs::read_dir(checkpoint_dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            (path, value)
        })
        .max_by_key(|(_, value)| {
            value["state"]["checkpoint"]["trials_done"]
                .as_u64()
                .unwrap()
        })
        .unwrap();
    value["state"]["checkpoint"]["metrics"][0]["sum"] = serde_json::json!(
        value["state"]["checkpoint"]["metrics"][0]["sum"]
            .as_i64()
            .unwrap()
            + 1
    );
    let forged = serde_json_canonicalizer::to_vec(&value).unwrap();
    let forged_name = format!("{:x}.json", Sha256::digest(&forged));
    fs::remove_file(checkpoint).unwrap();
    fs::write(
        project
            .evidence_dir()
            .join("simulations")
            .join(hex)
            .join("checkpoints")
            .join(forged_name),
        forged,
    )
    .unwrap();

    let response = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(response["error"]["code"], "evidence-corrupt", "{response}");
}

#[test]
fn malformed_predecessor_checkpoint_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(Arc::clone(&project));
    let started = drive(&start(49, 3, 1), &mut sessions, &mut store);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_owned();
    sessions.release_simulation_workers();
    let mut budget = MAX_POLLS;
    loop {
        let response = drive(&status(&job_id), &mut sessions, &mut store);
        if response["result"]["status"] == "complete" {
            break;
        }
        poll_budget(&mut budget, &response);
        thread::sleep(Duration::from_millis(10));
    }

    let job_dir = project
        .evidence_dir()
        .join("simulations")
        .join(job_id.strip_prefix("sha256:").unwrap());
    let checkpoint_dir = job_dir.join("checkpoints");
    let mut records = fs::read_dir(&checkpoint_dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            (path, value)
        })
        .collect::<Vec<_>>();
    records.sort_by_key(|(_, value)| {
        value["state"]["checkpoint"]["trials_done"]
            .as_u64()
            .unwrap()
    });
    records[0].1["state"]["checkpoint"]["metrics"][0]["trials"] = serde_json::json!(2);

    let mut rewritten = Vec::new();
    let mut previous: Option<String> = None;
    for (_, mut value) in records.iter().cloned() {
        value["previous"] = previous
            .as_ref()
            .map_or(Value::Null, |digest| Value::String(digest.clone()));
        let (bytes, digest) = canonical_record(&value);
        rewritten.push((digest.clone(), bytes));
        previous = Some(digest);
    }
    for (path, _) in records {
        fs::remove_file(path).unwrap();
    }
    for (digest, bytes) in &rewritten {
        fs::write(
            checkpoint_dir.join(format!("{}.json", digest.strip_prefix("sha256:").unwrap())),
            bytes,
        )
        .unwrap();
    }
    let mut completion: Value =
        serde_json::from_slice(&fs::read(job_dir.join("complete.json")).unwrap()).unwrap();
    completion["checkpoint"] = Value::String(previous.unwrap());
    fs::write(
        job_dir.join("complete.json"),
        serde_json_canonicalizer::to_vec(&completion).unwrap(),
    )
    .unwrap();

    let response = drive(&status(&job_id), &mut sessions, &mut store);
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(response["error"]["code"], "evidence-corrupt", "{response}");
}

#[test]
fn invalid_or_unknown_job_ids_are_typed_unknown_job_errors() {
    let temp = tempfile::tempdir().unwrap();
    let project = write_project(temp.path());
    let (mut sessions, mut store) = stores(project);

    for job_id in ["job-1", &format!("sha256:{}", "0".repeat(64))] {
        let response = drive(&status(job_id), &mut sessions, &mut store);
        assert_eq!(response["ok"], false, "{response}");
        assert_eq!(response["error"]["code"], "unknown-job", "{response}");
    }
}
