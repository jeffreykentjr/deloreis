//! Durable, project-owned evidence stores.
//!
//! Payloads and attestation batches are immutable JCS documents addressed by
//! their SHA-256 digest beneath the project's validated evidence directory.
//! Writers take one project-wide advisory lock and publish with an atomic rename.
//! Readers verify canonical bytes, filenames, and the single linear attestation
//! batch chain before exposing any record.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use dli_world::attest::{
    Attestation, AttestationStore, ContentHash, Question, QuestionKey, fingerprint_facts,
};
use dli_world::model::WorldModel;
use dli_world::payload::Payload;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::project::Project;

const BATCH_VERSION: u32 = 1;

/// A durable-evidence read or write failed.
#[derive(Debug, Error)]
pub enum EvidenceError {
    /// Storage I/O failed.
    #[error("evidence storage failed at {path}: {source}")]
    Io {
        /// The path being accessed.
        path: String,
        /// The underlying filesystem error.
        source: std::io::Error,
    },
    /// Committed evidence is malformed, non-canonical, or has broken lineage.
    #[error("committed evidence is corrupt: {0}")]
    Corrupt(String),
    /// A submitted record conflicts with the current append-only head.
    #[error("evidence head conflict: {0}")]
    Conflict(String),
    /// A submitted record is not a valid evidence record.
    #[error("invalid evidence: {0}")]
    Invalid(String),
}

/// One immutable append to a document's attestation history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttestationBatch {
    version: u32,
    document: String,
    parent: Option<String>,
    records: Vec<Attestation>,
}

/// A configured question that currently needs an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingAttestation {
    /// The full bounded question the factory must pose.
    pub question: Question,
    /// The fingerprint an accepted answer must carry.
    pub fingerprint: ContentHash,
    /// `missing` when no answer exists, `stale` when the current head moved.
    pub reason: &'static str,
}

/// The result of an atomic attestation append.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationAppend {
    /// Number of records accepted in the batch.
    pub accepted: usize,
    /// The immutable batch digest.
    pub batch_digest: String,
}

#[derive(Debug)]
struct LoadedAttestations {
    store: AttestationStore,
    head: Option<String>,
    last_records: Vec<Attestation>,
    record_ids: BTreeSet<(QuestionKey, String)>,
}

struct EvidenceLock {
    _file: File,
}

impl EvidenceLock {
    fn acquire(project: &Project) -> Result<Self, EvidenceError> {
        let path = project.evidence_dir().join(".write.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| io_error(&path, source))?;
        fs2::FileExt::lock_exclusive(&file).map_err(|source| io_error(&path, source))?;
        Ok(Self { _file: file })
    }
}

/// Canonical JCS bytes for a serializable evidence value.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, EvidenceError> {
    serde_json_canonicalizer::to_vec(value)
        .map_err(|error| EvidenceError::Invalid(format!("value cannot be encoded as JCS: {error}")))
}

/// A `sha256:<hex>` digest over JCS bytes.
pub fn digest_value<T: Serialize>(value: &T) -> Result<String, EvidenceError> {
    Ok(digest_bytes(&canonical_bytes(value)?))
}

/// Persist a checked fat payload idempotently.
pub fn submit_payload(
    project: &Project,
    document: &str,
    payload: &Payload,
) -> Result<String, EvidenceError> {
    require_document(project, document)?;
    let bytes = canonical_bytes(payload)?;
    let digest = digest_bytes(&bytes);
    let hex = digest
        .strip_prefix("sha256:")
        .expect("digest_bytes always prefixes sha256");
    let _lock = EvidenceLock::acquire(project)?;
    let directory = evidence_directory(project, &["payloads", document])?;
    publish_immutable(&directory.join(format!("{hex}.json")), &bytes)?;
    Ok(digest)
}

/// Return configured questions whose durable answer is absent or stale.
pub fn missing_attestations(
    project: &Project,
    document: &str,
    model: &WorldModel,
) -> Result<Vec<MissingAttestation>, EvidenceError> {
    let configured = require_document(project, document)?
        .attestation_questions
        .clone();
    let loaded = load_attestations(project, document)?;
    Ok(configured
        .into_iter()
        .filter_map(|question| {
            let fingerprint = current_fingerprint(model, &question);
            match loaded.store.head(&question.key()) {
                None => Some(MissingAttestation {
                    question,
                    fingerprint,
                    reason: "missing",
                }),
                Some(head) if head.question != question || head.is_stale(&fingerprint) => {
                    Some(MissingAttestation {
                        question,
                        fingerprint,
                        reason: "stale",
                    })
                }
                Some(_) => None,
            }
        })
        .collect())
}

/// Validate and atomically append one attestation batch.
pub fn append_attestations(
    project: &Project,
    document: &str,
    records: &[Attestation],
    model: &WorldModel,
) -> Result<AttestationAppend, EvidenceError> {
    if records.is_empty() {
        return Err(EvidenceError::Invalid(
            "an attestation sweep must contain at least one record".into(),
        ));
    }
    let configured = require_document(project, document)?
        .attestation_questions
        .iter()
        .map(|question| (question.key(), question))
        .collect::<BTreeMap<_, _>>();
    let _lock = EvidenceLock::acquire(project)?;
    let loaded = load_attestations(project, document)?;
    if loaded.last_records == records {
        return Ok(AttestationAppend {
            accepted: records.len(),
            batch_digest: loaded.head.expect("a last batch always has a head digest"),
        });
    }

    let mut heads = loaded
        .store
        .heads()
        .into_iter()
        .map(|record| (record.question.key(), record.record_id().to_owned()))
        .collect::<BTreeMap<_, _>>();
    let mut record_ids = loaded.record_ids;
    for record in records {
        validate_attestation(record)?;
        let key = record.question.key();
        let expected_question = configured.get(&key).ok_or_else(|| {
            EvidenceError::Invalid(format!(
                "question `{key}` is not declared by document `{document}`"
            ))
        })?;
        if *expected_question != &record.question {
            return Err(EvidenceError::Invalid(format!(
                "question `{key}` does not match the document's configured subject"
            )));
        }
        let expected_parent = heads.get(&key).map(String::as_str);
        if record.supersedes.as_deref() != expected_parent {
            return Err(EvidenceError::Conflict(format!(
                "question `{key}` must supersede {}, not {}",
                expected_parent.unwrap_or("nothing"),
                record.supersedes.as_deref().unwrap_or("nothing")
            )));
        }
        let current = current_fingerprint(model, &record.question);
        if record.fingerprint != current {
            return Err(EvidenceError::Conflict(format!(
                "question `{key}` was answered against {}; the current fingerprint is {current}",
                record.fingerprint
            )));
        }
        if !record_ids.insert((key.clone(), record.record_id().to_owned())) {
            return Err(EvidenceError::Conflict(format!(
                "record id `{}` already exists in question `{key}`",
                record.record_id(),
            )));
        }
        heads.insert(key, record.record_id().to_owned());
    }

    let batch = AttestationBatch {
        version: BATCH_VERSION,
        document: document.to_owned(),
        parent: loaded.head,
        records: records.to_vec(),
    };
    let bytes = canonical_bytes(&batch)?;
    let digest = digest_bytes(&bytes);
    let hex = digest
        .strip_prefix("sha256:")
        .expect("digest_bytes always prefixes sha256");
    let directory = evidence_directory(project, &["attestations", document, "batches"])?;
    publish_immutable(&directory.join(format!("{hex}.json")), &bytes)?;
    Ok(AttestationAppend {
        accepted: records.len(),
        batch_digest: digest,
    })
}

/// Recompute a configured question's current staleness fingerprint.
#[must_use]
pub fn current_fingerprint(model: &WorldModel, question: &Question) -> ContentHash {
    fingerprint_facts(
        model
            .facts_about(&question.subject)
            .into_iter()
            .map(|loaded| &loaded.atom),
    )
}

fn load_attestations(
    project: &Project,
    document: &str,
) -> Result<LoadedAttestations, EvidenceError> {
    require_document(project, document)?;
    let directory = evidence_directory(project, &["attestations", document, "batches"])?;
    let mut batches = BTreeMap::<String, AttestationBatch>::new();
    for entry in fs::read_dir(&directory).map_err(|source| io_error(&directory, source))? {
        let entry = entry.map_err(|source| io_error(&directory, source))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| io_error(&path, source))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(EvidenceError::Corrupt(format!(
                "attestation batch entry `{}` is not a regular file",
                path.display()
            )));
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            return Err(EvidenceError::Corrupt(format!(
                "unexpected file `{}` in the attestation batch store",
                path.display()
            )));
        }
        let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
        let batch: AttestationBatch = serde_json::from_slice(&bytes).map_err(|error| {
            EvidenceError::Corrupt(format!("{} is malformed: {error}", path.display()))
        })?;
        let canonical = canonical_bytes(&batch)
            .map_err(|error| EvidenceError::Corrupt(format!("{}: {error}", path.display())))?;
        if canonical != bytes {
            return Err(EvidenceError::Corrupt(format!(
                "{} is not RFC 8785 canonical JSON",
                path.display()
            )));
        }
        let digest = digest_bytes(&bytes);
        let expected_name = format!(
            "{}.json",
            digest
                .strip_prefix("sha256:")
                .expect("digest_bytes always prefixes sha256")
        );
        if path.file_name().and_then(|name| name.to_str()) != Some(&expected_name) {
            return Err(EvidenceError::Corrupt(format!(
                "{} does not match its content digest {digest}",
                path.display()
            )));
        }
        if batch.version != BATCH_VERSION || batch.document != document {
            return Err(EvidenceError::Corrupt(format!(
                "{} belongs to version {} document `{}`",
                path.display(),
                batch.version,
                batch.document
            )));
        }
        if batch.records.is_empty() {
            return Err(EvidenceError::Corrupt(format!(
                "{} contains an empty append",
                path.display()
            )));
        }
        batches.insert(digest, batch);
    }
    linearize_batches(document, &batches)
}

fn linearize_batches(
    document: &str,
    batches: &BTreeMap<String, AttestationBatch>,
) -> Result<LoadedAttestations, EvidenceError> {
    if batches.is_empty() {
        return Ok(LoadedAttestations {
            store: AttestationStore::new(),
            head: None,
            last_records: Vec::new(),
            record_ids: BTreeSet::new(),
        });
    }
    let roots = batches
        .iter()
        .filter(|(_, batch)| batch.parent.is_none())
        .map(|(digest, _)| digest.clone())
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(EvidenceError::Corrupt(format!(
            "document `{document}` has {} attestation roots; exactly one is required",
            roots.len()
        )));
    }
    let mut children = BTreeMap::<String, String>::new();
    for (digest, batch) in batches {
        if let Some(parent) = &batch.parent {
            if !batches.contains_key(parent) {
                return Err(EvidenceError::Corrupt(format!(
                    "attestation batch {digest} names missing parent {parent}"
                )));
            }
            if let Some(first) = children.insert(parent.clone(), digest.clone()) {
                return Err(EvidenceError::Corrupt(format!(
                    "attestation history branches after {parent}: {first} and {digest}"
                )));
            }
        }
    }

    let mut store = AttestationStore::new();
    let mut current_heads = BTreeMap::<QuestionKey, String>::new();
    let mut record_ids = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut current = roots[0].clone();
    let last_records = loop {
        if !visited.insert(current.clone()) {
            return Err(EvidenceError::Corrupt(format!(
                "attestation history cycles at {current}"
            )));
        }
        let batch = batches
            .get(&current)
            .expect("a traversed digest came from the batch map");
        for record in &batch.records {
            validate_attestation(record).map_err(|error| {
                EvidenceError::Corrupt(format!("batch {current} contains {error}"))
            })?;
            let key = record.question.key();
            let expected = current_heads.get(&key).map(String::as_str);
            if record.supersedes.as_deref() != expected {
                return Err(EvidenceError::Corrupt(format!(
                    "batch {current} breaks question `{key}` supersession"
                )));
            }
            if !record_ids.insert((key.clone(), record.record_id().to_owned())) {
                return Err(EvidenceError::Corrupt(format!(
                    "record id `{}` occurs more than once in question `{key}`",
                    record.record_id(),
                )));
            }
            current_heads.insert(key, record.record_id().to_owned());
            store.append(record.clone());
        }
        let Some(next) = children.get(&current) else {
            break batch.records.clone();
        };
        current.clone_from(next);
    };
    if visited.len() != batches.len() {
        return Err(EvidenceError::Corrupt(
            "attestation history contains a disconnected batch".into(),
        ));
    }
    Ok(LoadedAttestations {
        store,
        head: Some(current),
        last_records,
        record_ids,
    })
}

fn validate_attestation(record: &Attestation) -> Result<(), EvidenceError> {
    let q = &record.question;
    for (field, value) in [
        ("question.kind", q.kind.as_str()),
        ("question.atom", q.atom.as_str()),
        ("question.subject", q.subject.as_str()),
        ("answer", record.answer.as_str()),
        ("model.provider", record.model.provider.as_str()),
        ("model.id", record.model.id.as_str()),
        ("model.as_of", record.model.as_of.as_str()),
        ("prompt.template", record.prompt.template.as_str()),
        ("params.temperature", record.params.temperature.as_str()),
        ("params.agreement", record.params.agreement.as_str()),
        ("policy.reattest", record.policy.reattest.as_str()),
        ("sweep", record.sweep.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(EvidenceError::Invalid(format!(
                "`{field}` must not be empty"
            )));
        }
    }
    require_hash("rationale_hash", &record.rationale_hash, "sha256:", 64)?;
    require_hash("prompt.hash", &record.prompt.hash, "sha256:", 64)?;
    require_hash("fingerprint", &record.fingerprint.0, "fnv1a64:", 16)?;
    if let Some(parent) = &record.supersedes {
        require_hash("supersedes", parent, "sha256:", 64)?;
    }
    if record.params.max_tokens == 0 || record.params.samples == 0 {
        return Err(EvidenceError::Invalid(
            "`max_tokens` and `samples` must be positive".into(),
        ));
    }
    let (agree, total) = record
        .params
        .agreement
        .split_once('/')
        .ok_or_else(|| EvidenceError::Invalid("`agreement` must be `n/n`".into()))?;
    let agree = agree
        .parse::<u32>()
        .map_err(|_| EvidenceError::Invalid("`agreement` must be `n/n`".into()))?;
    let total = total
        .parse::<u32>()
        .map_err(|_| EvidenceError::Invalid("`agreement` must be `n/n`".into()))?;
    if total != record.params.samples || agree > total {
        return Err(EvidenceError::Invalid(
            "`agreement` denominator must equal `samples` and its numerator may not exceed it"
                .into(),
        ));
    }
    validate_date(&record.model.as_of)?;
    Ok(())
}

fn validate_date(value: &str) -> Result<(), EvidenceError> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(EvidenceError::Invalid(
            "`model.as_of` must use YYYY-MM-DD".into(),
        ));
    }
    let month = value[5..7].parse::<u32>().unwrap_or(0);
    let day = value[8..10].parse::<u32>().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(EvidenceError::Invalid(
            "`model.as_of` contains an invalid month or day".into(),
        ));
    }
    Ok(())
}

fn require_hash(
    field: &str,
    value: &str,
    prefix: &str,
    digits: usize,
) -> Result<(), EvidenceError> {
    let Some(hex) = value.strip_prefix(prefix) else {
        return Err(EvidenceError::Invalid(format!(
            "`{field}` must be a {prefix} digest"
        )));
    };
    if hex.len() != digits
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EvidenceError::Invalid(format!(
            "`{field}` must carry exactly {digits} lowercase hexadecimal digits"
        )));
    }
    Ok(())
}

fn require_document<'a>(
    project: &'a Project,
    document: &str,
) -> Result<&'a crate::project::ProjectDocument, EvidenceError> {
    project.document(document).ok_or_else(|| {
        EvidenceError::Invalid(format!("project has no document named `{document}`"))
    })
}

fn evidence_directory(project: &Project, components: &[&str]) -> Result<PathBuf, EvidenceError> {
    let mut path = project.evidence_dir().to_path_buf();
    for component in components {
        path.push(component);
    }
    fs::create_dir_all(&path).map_err(|source| io_error(&path, source))?;
    let canonical = fs::canonicalize(&path).map_err(|source| io_error(&path, source))?;
    if canonical != path || !canonical.starts_with(project.evidence_dir()) {
        return Err(EvidenceError::Corrupt(format!(
            "evidence directory `{}` traverses a symlink or escaped the project store",
            path.display()
        )));
    }
    Ok(path)
}

fn publish_immutable(path: &Path, bytes: &[u8]) -> Result<(), EvidenceError> {
    if path.exists() {
        let existing = fs::read(path).map_err(|source| io_error(path, source))?;
        return if existing == bytes {
            Ok(())
        } else {
            Err(EvidenceError::Corrupt(format!(
                "content-addressed file `{}` has different bytes",
                path.display()
            )))
        };
    }
    let directory = path
        .parent()
        .expect("an evidence file always has a parent directory");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("evidence filenames are ASCII");
    let temp = directory.join(format!(".{name}.tmp"));
    if temp.exists() {
        let metadata = fs::symlink_metadata(&temp).map_err(|source| io_error(&temp, source))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(EvidenceError::Corrupt(format!(
                "stale publication path `{}` is not a regular file",
                temp.display()
            )));
        }
        fs::remove_file(&temp).map_err(|source| io_error(&temp, source))?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|source| io_error(&temp, source))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| io_error(&temp, source))?;
    fs::rename(&temp, path).map_err(|source| io_error(path, source))?;
    File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|source| io_error(directory, source))?;
    Ok(())
}

fn digest_bytes(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(71);
    out.push_str("sha256:");
    for byte in hash {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn io_error(path: &Path, source: std::io::Error) -> EvidenceError {
    EvidenceError::Io {
        path: path.display().to_string(),
        source,
    }
}
