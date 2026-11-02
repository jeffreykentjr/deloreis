//! Durable evidence acceptance, idempotency, and corruption tests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use dli_serve::CONTRACT_VERSION;
use dli_serve::contract::AttestationAnswer;
use dli_serve::evidence::{
    EvidenceError, append_attestations, canonical_bytes, current_fingerprint, digest_value,
    missing_attestations,
};
use dli_serve::project::Project;
use dli_serve::serve::serve_project_io;
use dli_world::attest::{Attestation, ContentHash, ModelRef, Policy, PromptRef, Sampling};
use dli_world::compile;
use tempfile::TempDir;

fn write_project() -> (TempDir, Project) {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("deloreis.toml"),
        r#"
version = 1
name = "evidence-test"

[paths]
evidence = ".dli-evidence"
cache = ".dli-cache"

[documents.world]
sources = ["world.dli"]
attestation_questions = "questions.json"
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("world.dli"),
        "scope world\n\nratify d-1\n\nmarker(baseline) decided d-1\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("questions.json"),
        r#"{"questions":[{"kind":"marker-basis","atom":"marker-basis(baseline)","subject":"baseline"}]}"#,
    )
    .unwrap();
    let project = Project::load(temp.path()).unwrap();
    (temp, project)
}

fn attestation(project: &Project, supersedes: Option<String>, id: char) -> Attestation {
    let document = project.document("world").unwrap();
    let world = compile(&document.pack).unwrap();
    let question = document.attestation_questions[0].clone();
    Attestation {
        fingerprint: current_fingerprint(&world.model, &question),
        question,
        answer: AttestationAnswer::Yes.as_str().to_owned(),
        rationale_hash: format!("sha256:{}", id.to_string().repeat(64)),
        model: ModelRef {
            provider: "local-test".to_owned(),
            id: "deterministic-judge".to_owned(),
            as_of: "2026-07-25".to_owned(),
        },
        prompt: PromptRef {
            template: "marker-basis@v1".to_owned(),
            hash: format!("sha256:{}", "b".repeat(64)),
        },
        params: Sampling {
            temperature: "0.0".to_owned(),
            max_tokens: 20,
            samples: 1,
            agreement: "1/1".to_owned(),
        },
        policy: Policy {
            reattest: "on-content-change".to_owned(),
        },
        supersedes,
        sweep: "sweeps/test.json".to_owned(),
    }
}

fn json_files(path: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn write_legacy_batch(project: &Project, mut record: Attestation, answer: &str) {
    answer.clone_into(&mut record.answer);
    let batch = serde_json::json!({
        "version": 1,
        "document": "world",
        "parent": null,
        "records": [record],
    });
    let bytes = canonical_bytes(&batch).unwrap();
    let digest = digest_value(&batch).unwrap();
    let directory = project.evidence_dir().join("attestations/world/batches");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join(format!("{}.json", digest.strip_prefix("sha256:").unwrap())),
        bytes,
    )
    .unwrap();
}

fn check_response(project: Project) -> serde_json::Value {
    let request = serde_json::json!({
        "dli": CONTRACT_VERSION,
        "id": "check",
        "op": "dli.check",
        "params": {"document": "world"}
    });
    let mut output = Vec::new();
    serve_project_io(
        &mut Cursor::new(format!("{request}\n")),
        &mut output,
        project,
    )
    .unwrap();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn concurrent_first_reads_share_directory_creation() {
    let (_temp, project) = write_project();
    let world = compile(&project.document("world").unwrap().pack)
        .unwrap()
        .model;
    let project = Arc::new(project);
    let world = Arc::new(world);
    let barrier = Arc::new(Barrier::new(8));
    let workers = (0..8)
        .map(|_| {
            let project = Arc::clone(&project);
            let world = Arc::clone(&world);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                missing_attestations(&project, "world", &world)
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        assert_eq!(worker.join().unwrap().unwrap().len(), 1);
    }
}

#[test]
fn attestation_history_is_linear_current_and_retry_safe() {
    let (_temp, project) = write_project();
    let world = compile(&project.document("world").unwrap().pack).unwrap();
    let missing = missing_attestations(&project, "world", &world.model).unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].reason, "missing");

    let first_record = attestation(&project, None, 'a');
    let first = append_attestations(
        &project,
        "world",
        std::slice::from_ref(&first_record),
        &world.model,
    )
    .unwrap();
    let retry = append_attestations(
        &project,
        "world",
        std::slice::from_ref(&first_record),
        &world.model,
    )
    .unwrap();
    assert_eq!(first, retry);
    let first_hex = first.batch_digest.trim_start_matches("sha256:");
    let temp = project
        .evidence_dir()
        .join("attestations/world")
        .join(format!(".batches.{first_hex}.json.tmp"));
    fs::write(&temp, b"interrupted retry").unwrap();
    let recovered = append_attestations(
        &project,
        "world",
        std::slice::from_ref(&first_record),
        &world.model,
    )
    .unwrap();
    assert_eq!(first, recovered);
    assert!(
        !temp.exists(),
        "an attestation retry completes stale-temp recovery"
    );
    assert!(
        missing_attestations(&project, "world", &world.model)
            .unwrap()
            .is_empty()
    );

    let second_record = attestation(&project, Some(first_record.rationale_hash.clone()), 'c');
    let second = append_attestations(
        &project,
        "world",
        std::slice::from_ref(&second_record),
        &world.model,
    )
    .unwrap();
    assert_ne!(first.batch_digest, second.batch_digest);
    let delayed_retry = append_attestations(
        &project,
        "world",
        std::slice::from_ref(&first_record),
        &world.model,
    )
    .unwrap();
    assert_eq!(
        delayed_retry, first,
        "a delayed retry must replay the originally committed batch"
    );
    let batches = json_files(&project.evidence_dir().join("attestations/world/batches"));
    assert_eq!(batches.len(), 2);
}

#[test]
fn document_check_consumes_current_missing_and_negative_attestations() {
    let (temp, project) = write_project();
    let missing = check_response(Project::load(temp.path()).unwrap());
    assert_eq!(missing["result"]["ok"], false, "{missing}");
    assert_eq!(missing["result"]["grade"], "checked", "{missing}");
    assert_eq!(missing["result"]["findings"][0]["domain"], "attestation");
    assert_eq!(missing["result"]["findings"][0]["severity"], "error");
    assert_eq!(
        missing["result"]["findings"][0]["outcome"], "missing",
        "{missing}"
    );

    let world = compile(&project.document("world").unwrap().pack).unwrap();
    let first = attestation(&project, None, 'a');
    append_attestations(
        &project,
        "world",
        std::slice::from_ref(&first),
        &world.model,
    )
    .unwrap();
    let passed = check_response(Project::load(temp.path()).unwrap());
    assert_eq!(passed["result"]["ok"], true, "{passed}");
    assert_eq!(passed["result"]["grade"], "attested", "{passed}");
    assert_eq!(passed["result"]["findings"][0]["domain"], "attestation");
    assert!(passed["result"]["findings"][0].get("severity").is_none());
    assert_eq!(
        passed["result"]["findings"][0]["outcome"], "pass",
        "{passed}"
    );
    assert_eq!(
        passed["result"]["findings"][0]["record_id"], first.rationale_hash,
        "{passed}"
    );
    assert_eq!(
        passed["result"]["findings"][0]["stored_fingerprint"], first.fingerprint.0,
        "{passed}"
    );
    assert_eq!(
        passed["result"]["findings"][0]["current_fingerprint"], first.fingerprint.0,
        "{passed}"
    );

    fs::write(
        temp.path().join("world.dli"),
        "scope world\n\nratify d-1\n\nmarker(baseline) decided d-1\n\
         marker(changed, baseline) decided d-1\n",
    )
    .unwrap();
    let changed_project = Project::load(temp.path()).unwrap();
    let stale = check_response(changed_project.clone());
    assert_eq!(stale["result"]["ok"], false, "{stale}");
    assert_eq!(stale["result"]["grade"], "checked", "{stale}");
    assert_eq!(stale["result"]["findings"][0]["outcome"], "stale");
    assert_eq!(stale["result"]["findings"][0]["severity"], "error");
    assert_eq!(
        stale["result"]["findings"][0]["record_id"], first.rationale_hash,
        "{stale}"
    );
    assert_ne!(
        stale["result"]["findings"][0]["stored_fingerprint"],
        stale["result"]["findings"][0]["current_fingerprint"],
        "{stale}"
    );

    let changed_world = compile(&changed_project.document("world").unwrap().pack).unwrap();
    let mut second = attestation(&changed_project, Some(first.rationale_hash.clone()), 'c');
    second.answer = AttestationAnswer::No.as_str().to_owned();
    append_attestations(
        &changed_project,
        "world",
        std::slice::from_ref(&second),
        &changed_world.model,
    )
    .unwrap();
    let failed = check_response(Project::load(temp.path()).unwrap());
    assert_eq!(failed["result"]["ok"], false, "{failed}");
    assert_eq!(failed["result"]["grade"], "attested", "{failed}");
    assert_eq!(failed["result"]["findings"][0]["severity"], "error");
    assert_eq!(
        failed["result"]["findings"][0]["outcome"], "fail",
        "{failed}"
    );
    assert_eq!(
        failed["result"]["findings"][0]["record_id"], second.rationale_hash,
        "{failed}"
    );
    assert_eq!(
        failed["result"]["findings"][0]["supersedes"], first.rationale_hash,
        "{failed}"
    );
    assert_eq!(
        failed["result"]["findings"][0]["previous_answer"], "yes",
        "{failed}"
    );

    let directory = changed_project
        .evidence_dir()
        .join("attestations/world/batches");
    let path = json_files(&directory).remove(0);
    fs::write(path, b"{}").unwrap();
    let corrupt = check_response(Project::load(temp.path()).unwrap());
    assert_eq!(corrupt["error"]["code"], "evidence-corrupt", "{corrupt}");
}

#[test]
fn legacy_batch_answers_replay_without_reinterpreting_version_one_storage() {
    let (yes_temp, yes_project) = write_project();
    write_legacy_batch(&yes_project, attestation(&yes_project, None, 'a'), "yes");
    let current = check_response(Project::load(yes_temp.path()).unwrap());
    assert_eq!(current["result"]["ok"], true, "{current}");
    assert_eq!(
        current["result"]["findings"][0]["outcome"], "pass",
        "{current}"
    );

    let (legacy_temp, legacy_project) = write_project();
    write_legacy_batch(
        &legacy_project,
        attestation(&legacy_project, None, 'b'),
        "perhaps",
    );
    let stale = check_response(Project::load(legacy_temp.path()).unwrap());
    assert_eq!(stale["result"]["ok"], false, "{stale}");
    assert_eq!(
        stale["result"]["findings"][0]["outcome"], "stale",
        "{stale}"
    );
    assert!(
        stale["result"]["findings"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("outside the current yes/no contract")),
        "{stale}"
    );
    assert!(
        stale["error"].is_null(),
        "legacy evidence is stale, not corrupt: {stale}"
    );
}

#[test]
fn rationale_hash_identity_is_scoped_to_one_question_chain() {
    let (temp, _original_project) = write_project();
    fs::write(
        temp.path().join("questions.json"),
        r#"{"questions":[
            {"kind":"marker-basis","atom":"marker-basis(baseline)","subject":"baseline"},
            {"kind":"marker-tone","atom":"marker-tone(baseline)","subject":"baseline"}
        ]}"#,
    )
    .unwrap();
    let project = Project::load(temp.path()).unwrap();
    let world = compile(&project.document("world").unwrap().pack).unwrap();

    let first = attestation(&project, None, 'a');
    let mut second = first.clone();
    second.question = project.document("world").unwrap().attestation_questions[1].clone();
    second.fingerprint = current_fingerprint(&world.model, &second.question);
    append_attestations(&project, "world", &[first, second], &world.model).unwrap();

    assert!(
        missing_attestations(&project, "world", &world.model)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn changed_subject_anchor_is_stale_even_when_empty_fingerprints_collide() {
    let (temp, _original_project) = write_project();
    fs::write(
        temp.path().join("questions.json"),
        r#"{"questions":[{"kind":"marker-basis","atom":"marker-basis(baseline)","subject":"missing-a"}]}"#,
    )
    .unwrap();
    let first_project = Project::load(temp.path()).unwrap();
    let world = compile(&first_project.document("world").unwrap().pack).unwrap();
    let first = attestation(&first_project, None, 'a');
    append_attestations(
        &first_project,
        "world",
        std::slice::from_ref(&first),
        &world.model,
    )
    .unwrap();

    fs::write(
        temp.path().join("questions.json"),
        r#"{"questions":[{"kind":"marker-basis","atom":"marker-basis(baseline)","subject":"missing-b"}]}"#,
    )
    .unwrap();
    let changed_project = Project::load(temp.path()).unwrap();
    let changed_world = compile(&changed_project.document("world").unwrap().pack).unwrap();
    assert_eq!(
        first.fingerprint,
        current_fingerprint(
            &changed_world.model,
            &changed_project
                .document("world")
                .unwrap()
                .attestation_questions[0]
        ),
        "the regression requires equal content fingerprints"
    );
    let missing = missing_attestations(&changed_project, "world", &changed_world.model).unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].reason, "stale");
}

#[test]
fn changed_subject_standing_is_stale_even_when_atom_text_survives() {
    let (temp, first_project) = write_project();
    let first_world = compile(&first_project.document("world").unwrap().pack).unwrap();
    let first = attestation(&first_project, None, 'a');
    append_attestations(
        &first_project,
        "world",
        std::slice::from_ref(&first),
        &first_world.model,
    )
    .unwrap();

    fs::write(
        temp.path().join("world.dli"),
        "scope world\n\nmarker(baseline) decided d-1\n",
    )
    .unwrap();
    let changed_project = Project::load(temp.path()).unwrap();
    let changed_world = compile(&changed_project.document("world").unwrap().pack).unwrap();
    assert_ne!(
        first.fingerprint,
        current_fingerprint(
            &changed_world.model,
            &changed_project
                .document("world")
                .unwrap()
                .attestation_questions[0]
        ),
        "certain-to-possible standing changes must alter the fingerprint"
    );
    let missing = missing_attestations(&changed_project, "world", &changed_world.model).unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].reason, "stale");
}

#[test]
fn changed_derived_subject_fact_alters_the_fingerprint() {
    let (temp, _project) = write_project();
    fs::write(
        temp.path().join("world.dli"),
        "scope world\n\nratify d-1\n\nseated(baseline) decided d-1\n\
         rule derive-ward decided d-1\n  ward-holds($ground) :- seated($ground)\n",
    )
    .unwrap();
    let first_project = Project::load(temp.path()).unwrap();
    let first_world = compile(&first_project.document("world").unwrap().pack).unwrap();
    assert!(
        first_world
            .model
            .world
            .derived
            .iter()
            .any(|derived| derived.atom.pred == "ward-holds"),
        "{:?}",
        first_world.model.findings()
    );
    let question = &first_project
        .document("world")
        .unwrap()
        .attestation_questions[0];
    let first = current_fingerprint(&first_world.model, question);

    fs::write(
        temp.path().join("world.dli"),
        "scope world\n\nratify d-1\n\nseated(baseline) decided d-1\n",
    )
    .unwrap();
    let changed_project = Project::load(temp.path()).unwrap();
    let changed_world = compile(&changed_project.document("world").unwrap().pack).unwrap();
    let changed = current_fingerprint(
        &changed_world.model,
        &changed_project
            .document("world")
            .unwrap()
            .attestation_questions[0],
    );
    assert_ne!(
        first, changed,
        "derived subject facts participate in staleness"
    );
}

#[test]
fn rejected_attestation_writes_nothing() {
    let (_temp, project) = write_project();
    let world = compile(&project.document("world").unwrap().pack).unwrap();
    let first_record = attestation(&project, None, 'a');
    append_attestations(
        &project,
        "world",
        std::slice::from_ref(&first_record),
        &world.model,
    )
    .unwrap();
    let directory = project.evidence_dir().join("attestations/world/batches");
    let before = json_files(&directory);

    let wrong_parent = attestation(&project, None, 'c');
    assert!(matches!(
        append_attestations(
            &project,
            "world",
            std::slice::from_ref(&wrong_parent),
            &world.model
        ),
        Err(EvidenceError::Conflict(_))
    ));

    let mut unbounded = attestation(&project, Some(first_record.rationale_hash.clone()), 'e');
    unbounded.answer = "perhaps".to_owned();
    assert!(matches!(
        append_attestations(
            &project,
            "world",
            std::slice::from_ref(&unbounded),
            &world.model
        ),
        Err(EvidenceError::Invalid(_))
    ));

    let mut stale = attestation(&project, Some(first_record.rationale_hash.clone()), 'd');
    stale.fingerprint = ContentHash("fnv1a64:0000000000000000".to_owned());
    assert!(matches!(
        append_attestations(
            &project,
            "world",
            std::slice::from_ref(&stale),
            &world.model
        ),
        Err(EvidenceError::Conflict(_))
    ));
    assert_eq!(before, json_files(&directory));
}

#[test]
fn corrupt_committed_batch_fails_closed() {
    let (_temp, project) = write_project();
    let world = compile(&project.document("world").unwrap().pack).unwrap();
    let record = attestation(&project, None, 'a');
    append_attestations(
        &project,
        "world",
        std::slice::from_ref(&record),
        &world.model,
    )
    .unwrap();
    let directory = project.evidence_dir().join("attestations/world/batches");
    let path = json_files(&directory).remove(0);
    fs::write(&path, b"{}").unwrap();
    assert!(matches!(
        missing_attestations(&project, "world", &world.model),
        Err(EvidenceError::Corrupt(_))
    ));
}

#[cfg(unix)]
#[test]
fn post_load_evidence_root_symlink_is_rejected_without_target_writes() {
    use std::os::unix::fs::symlink;

    let (temp, project) = write_project();
    let outside = tempfile::tempdir().unwrap();
    let original = temp.path().join("original-evidence");
    fs::rename(project.evidence_dir(), &original).unwrap();
    symlink(outside.path(), project.evidence_dir()).unwrap();

    let model = compile(&project.document("world").unwrap().pack)
        .unwrap()
        .model;
    assert!(matches!(
        missing_attestations(&project, "world", &model),
        Err(EvidenceError::Corrupt(_))
    ));
    assert_eq!(
        fs::read_dir(outside.path()).unwrap().count(),
        0,
        "root substitution must be detected before creating a lock or directory"
    );
}

#[test]
fn calendar_invalid_attestation_date_is_rejected() {
    let (_temp, project) = write_project();
    let world = compile(&project.document("world").unwrap().pack).unwrap();
    let mut record = attestation(&project, None, 'a');
    record.model.as_of = "2026-02-31".to_owned();
    assert!(matches!(
        append_attestations(&project, "world", &[record], &world.model),
        Err(EvidenceError::Invalid(_))
    ));
}

#[test]
fn branched_batch_history_fails_closed() {
    let (_temp, project) = write_project();
    let world = compile(&project.document("world").unwrap().pack).unwrap();
    let record = attestation(&project, None, 'a');
    append_attestations(
        &project,
        "world",
        std::slice::from_ref(&record),
        &world.model,
    )
    .unwrap();
    let directory = project.evidence_dir().join("attestations/world/batches");
    let original = json_files(&directory).remove(0);
    let mut second_root: serde_json::Value =
        serde_json::from_slice(&fs::read(original).unwrap()).unwrap();
    second_root["records"][0]["rationale_hash"] =
        serde_json::Value::String(format!("sha256:{}", "c".repeat(64)));
    let digest = digest_value(&second_root).unwrap();
    let name = format!("{}.json", digest.trim_start_matches("sha256:"));
    fs::write(directory.join(name), canonical_bytes(&second_root).unwrap()).unwrap();

    assert!(matches!(
        missing_attestations(&project, "world", &world.model),
        Err(EvidenceError::Corrupt(_))
    ));
}
