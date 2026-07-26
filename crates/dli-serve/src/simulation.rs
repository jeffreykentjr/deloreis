//! Durable, restart-resumable simulation jobs.
//!
//! A job is addressed by the SHA-256 of its canonical specification. Its
//! checkpoints and final manifest are immutable content-addressed records under
//! the caller project's evidence root. There is no mutable head file: the
//! greatest verified checkpoint is the head, so a crash can leave either the old
//! complete state or one additional complete record, never a torn overwrite.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use dli_sim::{Battery, Checkpoint, RunManifest};
use serde::{Deserialize, Serialize};

use crate::contract::SimulateJobStatus;
use crate::evidence::{
    EvidenceError, EvidenceLock, canonical_bytes, digest_bytes, digest_value, evidence_directory,
    io_error, publish_immutable, require_regular_json, validate_content_address,
    validate_evidence_root,
};
use crate::project::Project;

const JOB_VERSION: u32 = 1;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobSpec {
    version: u32,
    document: String,
    checkpoint_every: u64,
    battery: Battery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobCheckpoint {
    version: u32,
    job_id: String,
    checkpoint: Checkpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancellation {
    version: u32,
    job_id: String,
    battery_digest: String,
}

struct LoadedJob {
    spec: JobSpec,
    checkpoint: Checkpoint,
    cancelled: bool,
    manifest: Option<RunManifest>,
    manifest_digest: Option<String>,
}

/// Create or reopen a content-addressed job.
pub fn start_job(
    project: &Project,
    document: &str,
    battery: Battery,
    checkpoint_every: u64,
) -> Result<JobView, EvidenceError> {
    if checkpoint_every == 0 {
        return Err(EvidenceError::Invalid(
            "`checkpoint_every` must be at least 1".to_owned(),
        ));
    }
    if project.document(document).is_none() {
        return Err(EvidenceError::Invalid(format!(
            "project has no document named `{document}`"
        )));
    }
    let spec = JobSpec {
        version: JOB_VERSION,
        document: document.to_owned(),
        checkpoint_every,
        battery,
    };
    let job_id = digest_value(&spec)?;
    let hex = job_hex(&job_id)?;
    let _lock = EvidenceLock::acquire(project)?;
    let directory = evidence_directory(project, &["simulations", hex])?;
    evidence_directory(project, &["simulations", hex, "checkpoints"])?;
    evidence_directory(project, &["simulations", hex, "manifests"])?;
    publish_immutable(&directory.join("spec.json"), &canonical_bytes(&spec)?)?;
    let loaded = load_job(project, &job_id)?;
    view(&job_id, &loaded)
}

/// Read and verify a durable job.
pub fn status_job(project: &Project, job_id: &str) -> Result<JobView, EvidenceError> {
    let _lock = EvidenceLock::acquire(project)?;
    let loaded = load_job(project, job_id)?;
    view(job_id, &loaded)
}

/// Commit cancellation unless the final manifest already exists.
pub fn cancel_job(project: &Project, job_id: &str) -> Result<JobView, EvidenceError> {
    let _lock = EvidenceLock::acquire(project)?;
    let mut loaded = load_job(project, job_id)?;
    if loaded.manifest.is_none() && !loaded.cancelled {
        let cancellation = Cancellation {
            version: JOB_VERSION,
            job_id: job_id.to_owned(),
            battery_digest: loaded.spec.battery.digest(),
        };
        let directory = existing_job_directory(project, job_id)?;
        publish_immutable(
            &directory.join("cancelled.json"),
            &canonical_bytes(&cancellation)?,
        )?;
        loaded.cancelled = true;
    }
    view(job_id, &loaded)
}

/// Advance one checkpoint boundary, or finalize an already-complete checkpoint.
///
/// The project-wide advisory lock is held while a boundary is computed. That
/// makes cancellation a boundary operation and prevents two worker processes
/// from publishing divergent heads. A crash releases the OS lock; the last
/// immutable checkpoint remains the resume point.
pub fn advance_job(project: &Project, job_id: &str) -> Result<JobView, EvidenceError> {
    let _lock = EvidenceLock::acquire(project)?;
    let mut loaded = load_job(project, job_id)?;
    if loaded.cancelled || loaded.manifest.is_some() {
        return view(job_id, &loaded);
    }

    if !loaded.checkpoint.is_complete() {
        loaded
            .checkpoint
            .advance_boundary(&loaded.spec.battery)
            .map_err(EvidenceError::Corrupt)?;
        publish_checkpoint(project, job_id, &loaded.checkpoint)?;
    }
    if loaded.checkpoint.is_complete() {
        let manifest = loaded.checkpoint.finalize(&loaded.spec.battery);
        let directory = existing_job_directory(project, job_id)?.join("manifests");
        let bytes = canonical_bytes(&manifest)?;
        let digest = digest_bytes(&bytes);
        let name = format!(
            "{}.json",
            digest
                .strip_prefix("sha256:")
                .expect("SHA-256 digests carry their prefix")
        );
        publish_immutable(&directory.join(name), &bytes)?;
        loaded.manifest = Some(manifest);
        loaded.manifest_digest = Some(digest);
    }
    view(job_id, &loaded)
}

/// Run checkpoint boundaries until the job reaches a terminal state.
pub fn run_to_terminal(project: &Project, job_id: &str) -> Result<JobView, EvidenceError> {
    loop {
        let current = advance_job(project, job_id)?;
        if matches!(
            current.status,
            SimulateJobStatus::Complete | SimulateJobStatus::Cancelled
        ) {
            return Ok(current);
        }
    }
}

fn publish_checkpoint(
    project: &Project,
    job_id: &str,
    checkpoint: &Checkpoint,
) -> Result<(), EvidenceError> {
    let record = JobCheckpoint {
        version: JOB_VERSION,
        job_id: job_id.to_owned(),
        checkpoint: checkpoint.clone(),
    };
    let bytes = canonical_bytes(&record)?;
    let digest = digest_bytes(&bytes);
    let name = format!(
        "{}.json",
        digest
            .strip_prefix("sha256:")
            .expect("SHA-256 digests carry their prefix")
    );
    let directory = existing_job_directory(project, job_id)?.join("checkpoints");
    publish_immutable(&directory.join(name), &bytes)
}

fn load_job(project: &Project, job_id: &str) -> Result<LoadedJob, EvidenceError> {
    let directory = existing_job_directory(project, job_id)?;
    validate_job_entries(&directory)?;

    let spec_path = directory.join("spec.json");
    require_regular_json(&spec_path, "simulation job specification")
        .map_err(|error| missing_spec(error, job_id))?;
    let spec_bytes = fs::read(&spec_path).map_err(|source| io_error(&spec_path, source))?;
    let spec: JobSpec = serde_json::from_slice(&spec_bytes).map_err(|error| {
        EvidenceError::Corrupt(format!("{} is malformed: {error}", spec_path.display()))
    })?;
    if canonical_bytes(&spec)? != spec_bytes
        || spec.version != JOB_VERSION
        || digest_bytes(&spec_bytes) != job_id
    {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has a non-canonical or mismatched specification"
        )));
    }
    if spec.checkpoint_every == 0 {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` has a zero checkpoint cadence"
        )));
    }

    let checkpoints = load_checkpoints(&directory, job_id, &spec)?;
    let checkpoint = checkpoints.last_key_value().map_or_else(
        || Checkpoint::initial(&spec.battery, Some(spec.checkpoint_every)),
        |(_, checkpoint)| checkpoint.clone(),
    );
    let expected_boundaries = checkpoint.lineage.clone();
    let found_boundaries = checkpoints.keys().copied().collect::<Vec<_>>();
    if expected_boundaries != found_boundaries {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` is missing a checkpoint from its lineage"
        )));
    }

    let (manifest, manifest_digest) = load_manifest(&directory, job_id, &spec, &checkpoint)?;
    let cancelled = load_cancellation(&directory, job_id, &spec)?;
    if cancelled && manifest.is_some() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` is both cancelled and complete"
        )));
    }
    Ok(LoadedJob {
        spec,
        checkpoint,
        cancelled,
        manifest,
        manifest_digest,
    })
}

fn load_checkpoints(
    directory: &Path,
    job_id: &str,
    spec: &JobSpec,
) -> Result<BTreeMap<u64, Checkpoint>, EvidenceError> {
    let checkpoint_dir = directory.join("checkpoints");
    require_directory(&checkpoint_dir, "simulation checkpoint")?;
    let mut checkpoints = BTreeMap::new();
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
        if record.version != JOB_VERSION || record.job_id != job_id {
            return Err(EvidenceError::Corrupt(format!(
                "{} belongs to another simulation job version or id",
                path.display()
            )));
        }
        record
            .checkpoint
            .validate_for(&spec.battery)
            .map_err(|error| EvidenceError::Corrupt(format!("{}: {error}", path.display())))?;
        if record.checkpoint.every != Some(spec.checkpoint_every) {
            return Err(EvidenceError::Corrupt(format!(
                "{} carries the wrong checkpoint cadence",
                path.display()
            )));
        }
        if checkpoints
            .insert(record.checkpoint.trials_done, record.checkpoint)
            .is_some()
        {
            return Err(EvidenceError::Corrupt(format!(
                "simulation job `{job_id}` has two checkpoints at the same boundary"
            )));
        }
    }
    Ok(checkpoints)
}

fn load_manifest(
    directory: &Path,
    job_id: &str,
    spec: &JobSpec,
    checkpoint: &Checkpoint,
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
    if !checkpoint.is_complete()
        || manifest.canonical_bytes() != checkpoint.finalize(&spec.battery).canonical_bytes()
        || !manifest.grades_are_honest()
    {
        return Err(EvidenceError::Corrupt(format!(
            "simulation job `{job_id}` final manifest does not match its checkpoint"
        )));
    }
    Ok((Some(manifest), Some(digest_bytes(&bytes))))
}

fn load_cancellation(
    directory: &Path,
    job_id: &str,
    spec: &JobSpec,
) -> Result<bool, EvidenceError> {
    let path = directory.join("cancelled.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(source) => return Err(io_error(&path, source)),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(EvidenceError::Corrupt(format!(
            "simulation cancellation `{}` is not a regular file",
            path.display()
        )));
    }
    let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
    let cancellation: Cancellation = serde_json::from_slice(&bytes).map_err(|error| {
        EvidenceError::Corrupt(format!("{} is malformed: {error}", path.display()))
    })?;
    if canonical_bytes(&cancellation)? != bytes
        || cancellation.version != JOB_VERSION
        || cancellation.job_id != job_id
        || cancellation.battery_digest != spec.battery.digest()
    {
        return Err(EvidenceError::Corrupt(format!(
            "simulation cancellation `{}` does not match its job",
            path.display()
        )));
    }
    Ok(true)
}

fn view(job_id: &str, loaded: &LoadedJob) -> Result<JobView, EvidenceError> {
    loaded
        .checkpoint
        .validate_for(&loaded.spec.battery)
        .map_err(EvidenceError::Corrupt)?;
    let status = if loaded.manifest.is_some() {
        SimulateJobStatus::Complete
    } else if loaded.cancelled {
        SimulateJobStatus::Cancelled
    } else if loaded.checkpoint.trials_done == 0 {
        SimulateJobStatus::Pending
    } else {
        SimulateJobStatus::Running
    };
    Ok(JobView {
        job_id: job_id.to_owned(),
        status,
        trials_done: loaded.checkpoint.trials_done,
        trials_total: loaded.spec.battery.trials,
        manifest: loaded.manifest.clone(),
        manifest_digest: loaded.manifest_digest.clone(),
    })
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
            "spec.json" | "cancelled.json" | "checkpoints" | "manifests"
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

fn missing_spec(error: EvidenceError, job_id: &str) -> EvidenceError {
    match error {
        EvidenceError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            EvidenceError::Corrupt(format!("simulation job `{job_id}` has no specification"))
        }
        other => other,
    }
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
