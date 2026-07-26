//! Durable evidence acceptance, idempotency, and corruption tests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use dli_serve::evidence::{
    EvidenceError, append_attestations, canonical_bytes, current_fingerprint, digest_value,
    missing_attestations, submit_payload,
};
use dli_serve::project::Project;
use dli_world::attest::{Attestation, ContentHash, ModelRef, Policy, PromptRef, Sampling};
use dli_world::compile;
use dli_world::payload::{Payload, source_hash};
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

fn payload(source: &str) -> Payload {
    let mut payload = Payload::default();
    "draft.md".clone_into(&mut payload.meta.source.file);
    payload.meta.source.sha256 = source_hash(source);
    payload.meta.source.words = 1;
    "payload/v1".clone_into(&mut payload.meta.extractors.schema);
    payload
}

fn attestation(project: &Project, supersedes: Option<String>, id: char) -> Attestation {
    let document = project.document("world").unwrap();
    let world = compile(&document.pack).unwrap();
    let question = document.attestation_questions[0].clone();
    Attestation {
        fingerprint: current_fingerprint(&world.model, &question),
        question,
        answer: "yes".to_owned(),
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

#[test]
fn payload_publication_is_canonical_content_addressed_and_idempotent() {
    let (_temp, project) = write_project();
    let payload = payload("one word");
    let first = submit_payload(&project, "world", &payload).unwrap();
    let second = submit_payload(&project, "world", &payload).unwrap();
    assert_eq!(first, second);

    let directory = project.evidence_dir().join("payloads/world");
    let files = json_files(&directory);
    assert_eq!(files.len(), 1);
    let bytes = fs::read(&files[0]).unwrap();
    let reparsed: Payload = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reparsed, payload);
    assert_eq!(
        bytes,
        serde_json_canonicalizer::to_vec(&reparsed).unwrap(),
        "persisted evidence is JCS, not merely deterministic serde JSON"
    );
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
    let batches = json_files(&project.evidence_dir().join("attestations/world/batches"));
    assert_eq!(batches.len(), 2);
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
