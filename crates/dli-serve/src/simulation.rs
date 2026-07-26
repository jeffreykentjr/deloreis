//! Durable, restart-resumable practice and game simulation jobs.
//!
//! A job is addressed by the SHA-256 of its canonical specification. Checkpoint
//! records form an immutable content-addressed chain; only the verified head is
//! replayed, while every predecessor remains required by digest. Trial work is
//! computed outside the project evidence lock and committed with compare-and-swap
//! semantics, so cancellation and unrelated evidence writers are not blocked by
//! a long cadence boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use dli_game::{DealBattery, DealCheckpoint};
use dli_sim::{Battery, Checkpoint, RunManifest};
use serde::{Deserialize, Serialize};

use crate::contract::SimulateJobStatus;
use crate::evidence::{
    EvidenceError, EvidenceLock, canonical_bytes, digest_bytes, digest_value, evidence_directory,
    io_error, publish_immutable, require_regular_json, validate_content_address,
    validate_evidence_root,
};
use crate::project::Project;

const JOB_VERSION: u32 = 2;

/// The verified wire projection of one durable job.
#[derive(Debug, Clone, PartialEq)]
pub struct JobView {
    /// Content-addressed job handle.
    pub job_id: String,
    /// Durable lifecycle state.
    pub status: SimulateJobStatus,
    /// Trials represented by the latest durable checkpoint.
    pub trials_done: u64,
    /// Trials requested by the job.
    pub trials_total: u64,
    /// Final manifest, present only after completion.
    pub manifest: Option<RunManifest>,
    /// SHA-256 over the final manifest's JCS bytes.
    pub manifest_digest: Option<String>,
}

/// The two generic simulation engines reachable through the shared job surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "battery", rename_all = "kebab-case")]
pub enum JobBattery {
    /// A declared-practice resource battery.
    Practice(Battery),
    /// A data-driven card-game deal battery.
    Game(DealBattery),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobSpec {
    version: u32,
    document: String,
    checkpoint_every: u64,
    battery: JobBattery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Created {
    version: u32,
    job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "checkpoint", rename_all = "kebab-case")]
enum JobState {
    Practice(Checkpoint),
    Game(DealCheckpoint),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobCheckpoint {
    version: u32,
    job_id: String,
    previous: Option<String>,
    state: JobState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancellation {
    version: u32,
    job_id: String,
    spec: JobSpec,
    checkpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    version: u32,
    job_id: String,
    spec: JobSpec,
    checkpoint: Option<String>,
    manifest_digest: String,
}

struct LoadedJob {
    spec: JobSpec,
    state: JobState,
    checkpoint_digest: Option<String>,
    live_spec: bool,
    cancelled: bool,
    complete: bool,
    manifest: Option<RunManifest>,
    manifest_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JobRevision {
    checkpoint_digest: Option<String>,
    cancelled: bool,
    complete: bool,
    manifest_digest: Option<String>,
}

/// Create or reopen a content-addressed job.
pub fn start_job(
    project: &Project,
    document: &str,
    battery: JobBattery,
    checkpoint_every: u64,
) -> Result<JobView, EvidenceError> {
    if checkpoint_every == 0 {
        return Err(EvidenceError::Invalid(
            "`checkpoint_every` must be at least 1".to_owned(),
        ));
    }
    let spec = JobSpec {
        version: JOB_VERSION,
        document: document.to_owned(),
        checkpoint_every,
        battery,
    };
    let job_id = digest_value(&spec)?;
    let hex = job_hex(&job_id)?;
    {
        let _lock = EvidenceLock::acquire(project)?;
        let simulations = evidence_directory(project, &["simulations"])?;
        let directory = simulations.join(hex);
        if directory.exists() {
            let created: Option<Created> =
                load_fixed_record(&directory.join("created.json"), "simulation creation")?;
            if created.is_none() {
                let active = load_active_spec(&directory, &job_id)?;
                if active.is_none() {
                    require_empty_job_shell(&directory, &job_id)?;
                    evidence_directory(project, &["simulations", hex, "checkpoints"])?;
                    evidence_directory(project, &["simulations", hex, "manifests"])?;
                    publish_immutable(&directory.join("spec.json"), &canonical_bytes(&spec)?)?;
                } else if active.as_ref() != Some(&spec) {
                    return Err(EvidenceError::Corrupt(format!(
                        "simulation job `{job_id}` was interrupted with another specification"
                    )));
                }
                publish_created(&directory, &job_id)?;
            }
        } else {
            evidence_directory(project, &["simulations", hex])?;
            evidence_directory(project, &["simulations", hex, "checkpoints"])?;
            evidence_directory(project, &["simulations", hex, "manifests"])?;
            publish_immutable(&directory.join("spec.json"), &canonical_bytes(&spec)?)?;
            publish_created(&directory, &job_id)?;
        }
    }
    let loaded = load_verified_job(project, &job_id)?;
    Ok(view(&job_id, &loaded))
}

/// Read and verify a durable job.
pub fn status_job(project: &Project, job_id: &str) -> Result<JobView, EvidenceError> {
    let loaded = load_verified_job(project, job_id)?;
    Ok(view(job_id, &loaded))
}

/// Commit cancellation unless completion is already terminal.
pub fn cancel_job(project: &Project, job_id: &str) -> Result<JobView, EvidenceError> {
    loop {
        let verified = load_verified_job(project, job_id)?;
        if verified.complete || verified.cancelled {
            return Ok(view(job_id, &verified));
        }
        let observed = verified.revision();
        let committed = {
            let _lock = EvidenceLock::acquire(project)?;
            let mut current = load_job(project, job_id)?;
            if current.revision() == observed {
                if current.state.is_complete() {
                    finish_job(project, job_id, &mut current)?;
                } else {
                    let cancellation = Cancellation {
                        version: JOB_VERSION,
                        job_id: job_id.to_owned(),
                        spec: current.spec.clone(),
                        checkpoint: current.checkpoint_digest.clone(),
                    };
                    let directory = existing_job_directory(project, job_id)?;
                    publish_immutable(
                        &directory.join("cancelled.json"),
                        &canonical_bytes(&cancellation)?,
                    )?;
                    remove_live_spec(&directory)?;
                    current.cancelled = true;
                }
                Some(view(job_id, &current))
            } else {
                None
            }
        };
        if let Some(view) = committed {
            return Ok(view);
        }
    }
}

/// Run checkpoint boundaries until the job reaches a terminal state.
pub fn run_to_terminal(project: &Project, job_id: &str) -> Result<JobView, EvidenceError> {
    let mut loaded = load_verified_job(project, job_id)?;
    loop {
        if loaded.cancelled || loaded.complete {
            return Ok(view(job_id, &loaded));
        }
        let observed = loaded.revision();
        if !loaded.state.is_complete() {
            loaded
                .state
                .advance_validated_boundary(&loaded.spec.battery);
        }
        let committed = {
            let _lock = EvidenceLock::acquire(project)?;
            let mut current = load_job(project, job_id)?;
            if current.revision() == observed {
                if !current.state.is_complete() {
                    let digest = publish_checkpoint(
                        project,
                        job_id,
                        current.checkpoint_digest.clone(),
                        &loaded.state,
                    )?;
                    current.state = loaded.state.clone();
                    current.checkpoint_digest = Some(digest);
                }
                if current.state.is_complete() {
                    finish_job(project, job_id, &mut current)?;
                }
                Some(current)
            } else {
                None
            }
        };
        loaded = match committed {
            Some(current) => current,
            None => load_verified_job(project, job_id)?,
        };
    }
}

fn finish_job(
    project: &Project,
    job_id: &str,
    loaded: &mut LoadedJob,
) -> Result<(), EvidenceError> {
    let manifest = loaded.state.finalize(&loaded.spec.battery);
    let directory = existing_job_directory(project, job_id)?;
    let bytes = canonical_bytes(&manifest)?;
    let digest = digest_bytes(&bytes);
    let name = format!("{}.json", digest_hex(&digest));
    publish_immutable(&directory.join("manifests").join(name), &bytes)?;
    let completion = Completion {
        version: JOB_VERSION,
        job_id: job_id.to_owned(),
        spec: loaded.spec.clone(),
        checkpoint: loaded.checkpoint_digest.clone(),
        manifest_digest: digest.clone(),
    };
    publish_immutable(
        &directory.join("complete.json"),
        &canonical_bytes(&completion)?,
    )?;
    remove_live_spec(&directory)?;
    loaded.complete = true;
    loaded.manifest = Some(manifest);
    loaded.manifest_digest = Some(digest);
    Ok(())
}

fn publish_checkpoint(
    project: &Project,
    job_id: &str,
    previous: Option<String>,
    state: &JobState,
) -> Result<String, EvidenceError> {
    let record = JobCheckpoint {
        version: JOB_VERSION,
        job_id: job_id.to_owned(),
        previous,
        state: state.persisted(),
    };
    let bytes = canonical_bytes(&record)?;
    let digest = digest_bytes(&bytes);
    let directory = existing_job_directory(project, job_id)?.join("checkpoints");
    publish_immutable(
        &directory.join(format!("{}.json", digest_hex(&digest))),
        &bytes,
    )?;
    Ok(digest)
}

fn load_job(project: &Project, job_id: &str) -> Result<LoadedJob, EvidenceError> {
    let directory = existing_job_directory(project, job_id)?;
    validate_job_entries(&directory)?;
    let created: Created =
        load_fixed_record(&directory.join("created.json"), "simulation creation")?.ok_or_else(
            || EvidenceError::Corrupt(format!("simulation job `{job_id}` has no creation record")),
        )?;
    if created.version != JOB_VERSION || created.job_id != job_id {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has a mismatched creation record"
        )));
    }
    let cancellation: Option<Cancellation> =
        load_fixed_record(&directory.join("cancelled.json"), "simulation cancellation")?;
    let completion: Option<Completion> =
        load_fixed_record(&directory.join("complete.json"), "simulation completion")?;
    if cancellation.is_some() && completion.is_some() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` is both cancelled and complete"
        )));
    }

    let active_spec = load_active_spec(&directory, job_id)?;
    let live_spec = active_spec.is_some();
    let spec = match (&cancellation, &completion, active_spec) {
        (Some(record), None, _) => {
            validate_terminal_spec(job_id, record.version, &record.job_id, &record.spec)?;
            record.spec.clone()
        }
        (None, Some(record), _) => {
            validate_terminal_spec(job_id, record.version, &record.job_id, &record.spec)?;
            record.spec.clone()
        }
        (None, None, Some(spec)) => spec,
        (None, None, None) => {
            return Err(EvidenceError::Corrupt(format!(
                "simulation job `{job_id}` has no live or terminal specification"
            )));
        }
        (Some(_), Some(_), _) => unreachable!("checked above"),
    };
    if spec.checkpoint_every == 0 {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has a zero checkpoint cadence"
        )));
    }

    let (state, checkpoint_digest) = load_checkpoint_chain(&directory, job_id, &spec)?;
    let terminal_checkpoint = cancellation
        .as_ref()
        .map(|record| &record.checkpoint)
        .or_else(|| completion.as_ref().map(|record| &record.checkpoint));
    if terminal_checkpoint.is_some_and(|expected| expected != &checkpoint_digest) {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` terminal record does not pin its checkpoint head"
        )));
    }

    let (manifest, manifest_digest) = load_manifest(&directory, job_id, &spec, &state)?;
    if let Some(record) = &completion {
        if manifest_digest.as_deref() != Some(record.manifest_digest.as_str()) {
            return Err(EvidenceError::Corrupt(format!(
                "simulation job `{job_id}` completion does not pin its manifest"
            )));
        }
    } else if cancellation.is_some() && manifest.is_some() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` is both cancelled and has a final manifest"
        )));
    }

    Ok(LoadedJob {
        spec,
        state,
        checkpoint_digest,
        live_spec,
        cancelled: cancellation.is_some(),
        complete: completion.is_some(),
        manifest,
        manifest_digest,
    })
}

fn load_verified_job(project: &Project, job_id: &str) -> Result<LoadedJob, EvidenceError> {
    loop {
        let mut loaded = {
            let _lock = EvidenceLock::acquire(project)?;
            load_job(project, job_id)?
        };
        loaded.verify()?;
        if !(loaded.live_spec && (loaded.cancelled || loaded.complete)) {
            return Ok(loaded);
        }

        let observed = loaded.revision();
        let cleaned = {
            let _lock = EvidenceLock::acquire(project)?;
            let current = load_job(project, job_id)?;
            if current.revision() == observed
                && current.live_spec
                && (current.cancelled || current.complete)
            {
                let directory = existing_job_directory(project, job_id)?;
                remove_live_spec(&directory)?;
                true
            } else {
                false
            }
        };
        if cleaned {
            loaded.live_spec = false;
            return Ok(loaded);
        }
    }
}

fn load_active_spec(directory: &Path, job_id: &str) -> Result<Option<JobSpec>, EvidenceError> {
    let path = directory.join("spec.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(&path, source)),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job specification `{}` is not a regular file",
            path.display()
        )));
    }
    let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
    let spec: JobSpec = serde_json::from_slice(&bytes).map_err(|error| {
        EvidenceError::Corrupt(format!("{} is malformed: {error}", path.display()))
    })?;
    if canonical_bytes(&spec)? != bytes
        || spec.version != JOB_VERSION
        || digest_bytes(&bytes) != job_id
    {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has a non-canonical or mismatched specification"
        )));
    }
    Ok(Some(spec))
}

fn validate_terminal_spec(
    job_id: &str,
    version: u32,
    record_job_id: &str,
    spec: &JobSpec,
) -> Result<(), EvidenceError> {
    if version != JOB_VERSION
        || record_job_id != job_id
        || spec.version != JOB_VERSION
        || digest_value(spec)? != job_id
    {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has a mismatched terminal record"
        )));
    }
    Ok(())
}

fn load_fixed_record<T: for<'de> Deserialize<'de> + Serialize>(
    path: &Path,
    kind: &str,
) -> Result<Option<T>, EvidenceError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(path, source)),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(EvidenceError::Corrupt(format!(
            "{kind} `{}` is not a regular file",
            path.display()
        )));
    }
    let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
    let value: T = serde_json::from_slice(&bytes).map_err(|error| {
        EvidenceError::Corrupt(format!("{} is malformed: {error}", path.display()))
    })?;
    if canonical_bytes(&value)? != bytes {
        return Err(EvidenceError::Corrupt(format!(
            "{} is not canonical JSON",
            path.display()
        )));
    }
    Ok(Some(value))
}

fn publish_created(directory: &Path, job_id: &str) -> Result<(), EvidenceError> {
    publish_immutable(
        &directory.join("created.json"),
        &canonical_bytes(&Created {
            version: JOB_VERSION,
            job_id: job_id.to_owned(),
        })?,
    )
}

fn require_empty_job_shell(directory: &Path, job_id: &str) -> Result<(), EvidenceError> {
    validate_job_entries(directory)?;
    for child in ["checkpoints", "manifests"] {
        let path = directory.join(child);
        if path.exists() {
            require_directory(&path, "interrupted simulation")?;
            let mut entries = fs::read_dir(&path).map_err(|source| io_error(&path, source))?;
            if entries.next().is_some() {
                return Err(EvidenceError::Corrupt(format!(
                    "simulation job `{job_id}` lost its specification after publication"
                )));
            }
        }
    }
    if directory.join("cancelled.json").exists() || directory.join("complete.json").exists() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has a terminal record but no specification"
        )));
    }
    Ok(())
}

fn load_checkpoint_chain(
    directory: &Path,
    job_id: &str,
    spec: &JobSpec,
) -> Result<(JobState, Option<String>), EvidenceError> {
    let checkpoint_dir = directory.join("checkpoints");
    require_directory(&checkpoint_dir, "simulation checkpoint")?;
    let mut records: BTreeMap<String, JobCheckpoint> = BTreeMap::new();
    let mut referenced = BTreeSet::new();
    for entry in
        fs::read_dir(&checkpoint_dir).map_err(|source| io_error(&checkpoint_dir, source))?
    {
        let path = entry
            .map_err(|source| io_error(&checkpoint_dir, source))?
            .path();
        require_regular_json(&path, "simulation checkpoint")?;
        let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
        let record: JobCheckpoint = serde_json::from_slice(&bytes).map_err(|error| {
            EvidenceError::Corrupt(format!("{} is malformed: {error}", path.display()))
        })?;
        validate_content_address(&path, &bytes, &record)?;
        validate_checkpoint_record(&path, &record, job_id, spec)?;
        if let Some(previous) = &record.previous {
            referenced.insert(previous.clone());
        }
        let digest = digest_bytes(&bytes);
        if records.insert(digest, record).is_some() {
            return Err(EvidenceError::Corrupt(format!(
                "simulation job `{job_id}` contains a duplicate checkpoint"
            )));
        }
    }
    if records.is_empty() {
        return Ok((
            JobState::initial(&spec.battery, spec.checkpoint_every),
            None,
        ));
    }
    let heads = records
        .keys()
        .filter(|digest| !referenced.contains(*digest))
        .cloned()
        .collect::<Vec<_>>();
    if heads.len() != 1 {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` checkpoint records do not form one chain"
        )));
    }
    let head = heads[0].clone();
    let mut cursor = Some(head.clone());
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(digest) = cursor.take() {
        if !seen.insert(digest.clone()) {
            return Err(EvidenceError::Corrupt(format!(
                "simulation job `{job_id}` checkpoint chain contains a cycle"
            )));
        }
        let record = records.get(&digest).ok_or_else(|| {
            EvidenceError::Corrupt(format!(
                "simulation job `{job_id}` is missing checkpoint `{digest}`"
            ))
        })?;
        chain.push(record.state.trials_done());
        cursor.clone_from(&record.previous);
    }
    if seen.len() != records.len() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has an unlinked checkpoint record"
        )));
    }
    chain.reverse();
    let expected = expected_boundaries(
        spec.checkpoint_every,
        *chain.last().unwrap_or(&0),
        spec.battery.trials(),
    );
    if chain != expected {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` checkpoint boundaries do not match its cadence"
        )));
    }
    let mut state = records
        .get(&head)
        .expect("the selected head exists")
        .state
        .clone();
    state.set_lineage(chain);
    state
        .validate_shape_for(&spec.battery)
        .map_err(EvidenceError::Corrupt)?;
    Ok((state, Some(head)))
}

fn validate_checkpoint_record(
    path: &Path,
    record: &JobCheckpoint,
    job_id: &str,
    spec: &JobSpec,
) -> Result<(), EvidenceError> {
    if record.version != JOB_VERSION
        || record.job_id != job_id
        || !record.state.lineage().is_empty()
        || !record
            .state
            .metadata_matches(&spec.battery, spec.checkpoint_every)
    {
        return Err(EvidenceError::Corrupt(format!(
            "{} does not match its simulation job",
            path.display()
        )));
    }
    record
        .state
        .validate_persisted_shape_for(&spec.battery)
        .map_err(|detail| {
            EvidenceError::Corrupt(format!(
                "{} has an invalid checkpoint shape: {detail}",
                path.display()
            ))
        })
}

fn load_manifest(
    directory: &Path,
    job_id: &str,
    spec: &JobSpec,
    state: &JobState,
) -> Result<(Option<RunManifest>, Option<String>), EvidenceError> {
    let manifest_dir = directory.join("manifests");
    require_directory(&manifest_dir, "simulation manifest")?;
    let mut entries =
        fs::read_dir(&manifest_dir).map_err(|source| io_error(&manifest_dir, source))?;
    let Some(entry) = entries.next() else {
        return Ok((None, None));
    };
    let path = entry
        .map_err(|source| io_error(&manifest_dir, source))?
        .path();
    if entries.next().is_some() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has more than one final manifest"
        )));
    }
    require_regular_json(&path, "simulation manifest")?;
    let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
    let manifest: RunManifest = serde_json::from_slice(&bytes).map_err(|error| {
        EvidenceError::Corrupt(format!("{} is malformed: {error}", path.display()))
    })?;
    validate_content_address(&path, &bytes, &manifest)?;
    if !state.is_complete()
        || manifest.canonical_bytes() != state.finalize(&spec.battery).canonical_bytes()
        || !manifest.grades_are_honest()
    {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` final manifest does not match its checkpoint"
        )));
    }
    Ok((Some(manifest), Some(digest_bytes(&bytes))))
}

fn view(job_id: &str, loaded: &LoadedJob) -> JobView {
    let status = if loaded.complete {
        SimulateJobStatus::Complete
    } else if loaded.cancelled {
        SimulateJobStatus::Cancelled
    } else if loaded.state.trials_done() == 0 {
        SimulateJobStatus::Pending
    } else {
        SimulateJobStatus::Running
    };
    JobView {
        job_id: job_id.to_owned(),
        status,
        trials_done: loaded.state.trials_done(),
        trials_total: loaded.spec.battery.trials(),
        manifest: loaded.complete.then(|| loaded.manifest.clone()).flatten(),
        manifest_digest: loaded
            .complete
            .then(|| loaded.manifest_digest.clone())
            .flatten(),
    }
}

impl JobBattery {
    fn trials(&self) -> u64 {
        match self {
            Self::Practice(battery) => battery.trials,
            Self::Game(battery) => battery.trials,
        }
    }
}

impl JobState {
    fn initial(battery: &JobBattery, every: u64) -> Self {
        match battery {
            JobBattery::Practice(battery) => {
                Self::Practice(Checkpoint::initial(battery, Some(every)))
            }
            JobBattery::Game(battery) => Self::Game(DealCheckpoint::initial(battery, every)),
        }
    }

    fn persisted(&self) -> Self {
        let mut persisted = self.clone();
        persisted.set_lineage(Vec::new());
        persisted
    }

    fn lineage(&self) -> &[u64] {
        match self {
            Self::Practice(checkpoint) => &checkpoint.lineage,
            Self::Game(checkpoint) => &checkpoint.lineage,
        }
    }

    fn set_lineage(&mut self, lineage: Vec<u64>) {
        match self {
            Self::Practice(checkpoint) => checkpoint.lineage = lineage,
            Self::Game(checkpoint) => checkpoint.lineage = lineage,
        }
    }

    fn trials_done(&self) -> u64 {
        match self {
            Self::Practice(checkpoint) => checkpoint.trials_done,
            Self::Game(checkpoint) => checkpoint.trials_done,
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            Self::Practice(checkpoint) => checkpoint.is_complete(),
            Self::Game(checkpoint) => checkpoint.is_complete(),
        }
    }

    fn metadata_matches(&self, battery: &JobBattery, every: u64) -> bool {
        match (self, battery) {
            (Self::Practice(checkpoint), JobBattery::Practice(battery)) => {
                checkpoint.matches(battery)
                    && checkpoint.seed == battery.seed
                    && checkpoint.trials_total == battery.trials
                    && checkpoint.trials_done <= battery.trials
                    && checkpoint.every == Some(every)
            }
            (Self::Game(checkpoint), JobBattery::Game(battery)) => {
                checkpoint.battery_digest == battery.digest()
                    && checkpoint.seed == battery.seed
                    && checkpoint.trials_total == battery.trials
                    && checkpoint.trials_done <= battery.trials
                    && checkpoint.every == every
            }
            _ => false,
        }
    }

    fn validate_for(&self, battery: &JobBattery) -> Result<(), String> {
        match (self, battery) {
            (Self::Practice(checkpoint), JobBattery::Practice(battery)) => {
                checkpoint.validate_for(battery)
            }
            (Self::Game(checkpoint), JobBattery::Game(battery)) => checkpoint.validate_for(battery),
            _ => Err("simulation checkpoint kind does not match its battery".to_owned()),
        }
    }

    fn validate_shape_for(&self, battery: &JobBattery) -> Result<(), String> {
        match (self, battery) {
            (Self::Practice(checkpoint), JobBattery::Practice(battery)) => {
                checkpoint.validate_shape_for(battery)
            }
            (Self::Game(checkpoint), JobBattery::Game(battery)) => {
                checkpoint.validate_shape_for(battery)
            }
            _ => Err("simulation checkpoint kind does not match its battery".to_owned()),
        }
    }

    fn validate_persisted_shape_for(&self, battery: &JobBattery) -> Result<(), String> {
        match (self, battery) {
            (Self::Practice(checkpoint), JobBattery::Practice(battery)) => {
                checkpoint.validate_persisted_shape_for(battery)
            }
            (Self::Game(checkpoint), JobBattery::Game(battery)) => {
                checkpoint.validate_persisted_shape_for(battery)
            }
            _ => Err("simulation checkpoint kind does not match its battery".to_owned()),
        }
    }

    fn advance_validated_boundary(&mut self, battery: &JobBattery) -> bool {
        match (self, battery) {
            (Self::Practice(checkpoint), JobBattery::Practice(battery)) => {
                checkpoint.advance_validated_boundary(battery)
            }
            (Self::Game(checkpoint), JobBattery::Game(battery)) => {
                checkpoint.advance_validated_boundary(battery)
            }
            _ => unreachable!("validated simulation state matches its battery"),
        }
    }

    fn finalize(&self, battery: &JobBattery) -> RunManifest {
        match (self, battery) {
            (Self::Practice(checkpoint), JobBattery::Practice(battery)) => {
                checkpoint.finalize(battery)
            }
            (Self::Game(checkpoint), JobBattery::Game(battery)) => checkpoint.finalize(battery),
            _ => unreachable!("validated simulation state matches its battery"),
        }
    }
}

impl LoadedJob {
    fn revision(&self) -> JobRevision {
        JobRevision {
            checkpoint_digest: self.checkpoint_digest.clone(),
            cancelled: self.cancelled,
            complete: self.complete,
            manifest_digest: self.manifest_digest.clone(),
        }
    }

    fn verify(&self) -> Result<(), EvidenceError> {
        self.state
            .validate_for(&self.spec.battery)
            .map_err(EvidenceError::Corrupt)
    }
}

fn expected_boundaries(every: u64, done: u64, total: u64) -> Vec<u64> {
    let mut expected = Vec::new();
    let mut boundary = every;
    while boundary <= done {
        expected.push(boundary);
        let next = boundary.saturating_add(every);
        if next == boundary {
            break;
        }
        boundary = next;
    }
    if done == total && done > 0 && expected.last() != Some(&done) {
        expected.push(done);
    }
    expected
}

fn remove_live_spec(directory: &Path) -> Result<(), EvidenceError> {
    let path = directory.join("spec.json");
    match fs::remove_file(&path) {
        Ok(()) => File::open(directory)
            .and_then(|dir| dir.sync_all())
            .map_err(|source| io_error(directory, source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(&path, source)),
    }
}

fn existing_job_directory(project: &Project, job_id: &str) -> Result<PathBuf, EvidenceError> {
    validate_evidence_root(project)?;
    let hex = job_hex(job_id)?;
    let path = project.evidence_dir().join("simulations").join(hex);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(EvidenceError::NotFound(format!(
                "no simulation job named `{job_id}`"
            )));
        }
        Err(source) => return Err(io_error(&path, source)),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job path `{}` is not a regular directory",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(&path).map_err(|source| io_error(&path, source))?;
    if canonical != path || !canonical.starts_with(project.evidence_dir()) {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job path `{}` escaped the project store",
            path.display()
        )));
    }
    Ok(path)
}

fn validate_job_entries(directory: &Path) -> Result<(), EvidenceError> {
    for entry in fs::read_dir(directory).map_err(|source| io_error(directory, source))? {
        let entry = entry.map_err(|source| io_error(directory, source))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(
            name.as_ref(),
            "spec.json"
                | "created.json"
                | "cancelled.json"
                | "complete.json"
                | "checkpoints"
                | "manifests"
        ) || (name.starts_with('.') && name.ends_with(".tmp"))
        {
            continue;
        }
        return Err(EvidenceError::Corrupt(format!(
            "unexpected entry `{}` in simulation job store",
            entry.path().display()
        )));
    }
    Ok(())
}

fn require_directory(path: &Path, kind: &str) -> Result<(), EvidenceError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(EvidenceError::Corrupt(format!(
            "{kind} path `{}` is not a regular directory",
            path.display()
        )));
    }
    Ok(())
}

fn digest_hex(digest: &str) -> &str {
    digest
        .strip_prefix("sha256:")
        .expect("SHA-256 digests carry their prefix")
}

fn job_hex(job_id: &str) -> Result<&str, EvidenceError> {
    let Some(hex) = job_id.strip_prefix("sha256:") else {
        return Err(EvidenceError::NotFound(format!(
            "no simulation job named `{job_id}`"
        )));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(EvidenceError::NotFound(format!(
            "no simulation job named `{job_id}`"
        )));
    }
    Ok(hex)
}
