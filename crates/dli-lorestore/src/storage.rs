//! The abstract storage interface and the filesystem backend.
//!
//! `Storage` is the seam between the `Lorestore` facade and the on-disk
//! layout. The filesystem implementation (`FilesystemStorage`) is the only
//! production backend (roadmap §LS-2); a fault-injection mock that implements
//! the same trait is used by the crash-injection oracle.
//!
//! The layout is git-style loose objects:
//!
//! ```text
//!   <root>/
//!     objects/
//!       ab/
//!         cdef0123456789...json   ← one file per object, named by its content hash
//!     refs/
//!       main                        ← a Ref JSON doc
//!     .lock                         ← advisory file lock
//! ```

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use fs2::FileExt as _;
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::canonical::{canonical_bytes, content_hash};
use crate::identity::ContentHash;
use crate::store::{Commit, Ref};

/// One storage error.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The requested object or ref does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// An on-disk object's bytes do not hash to its filename — corruption.
    #[error("corrupt {what} at {path}: expected {expected}, got {actual}")]
    Corrupt {
        /// What kind of record is corrupt (object, ref, commit).
        what: &'static str,
        /// The path being accessed.
        path: String,
        /// The hash encoded in the filename.
        expected: String,
        /// The hash computed from the file's bytes.
        actual: String,
    },
    /// The CAS ref update lost the race — another writer moved the ref first.
    #[error("ref `{name}` CAS conflict: expected {expected:?}, found {found:?}")]
    CasConflict {
        /// The ref that was being updated.
        name: String,
        /// The commit hash the caller expected to find.
        expected: Option<String>,
        /// The commit hash actually found in the ref.
        found: Option<String>,
    },
    /// Filesystem I/O failed.
    #[error("I/O on {path}: {source}")]
    Io {
        /// The path being accessed.
        path: String,
        /// The underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// JCS canonicalization failed.
    #[error("canonical encoding: {0}")]
    Canonical(#[from] crate::canonical::CanonicalError),
    /// Lock acquisition failed.
    #[error("could not acquire store lock: {0}")]
    Lock(String),
}

/// The abstract storage interface.
///
/// This trait is the seam that lets the crash-injection oracle use a mock
/// backend while the production path uses `FilesystemStorage`.
pub trait Storage: Send + Sync + std::fmt::Debug {
    /// Read an object's raw bytes by content hash.
    ///
    /// Returns `Ok(None)` if the object does not exist (a cache miss, not an
    /// error). If the bytes exist but do not hash to the expected address,
    /// returns `Err(Corrupt)`.
    fn read_object(&self, hash: &ContentHash) -> Result<Option<Vec<u8>>, StoreError>;

    /// Write raw bytes as an object, returning the content hash.
    ///
    /// If an object with the same hash already exists, the write is a no-op
    /// (idempotent CAS — git does the same).
    fn write_object(&self, bytes: &[u8]) -> Result<ContentHash, StoreError>;

    /// Atomically update a ref if its current value matches `expected`.
    ///
    /// `expected: None` means "create the ref only if it does not exist."
    /// Returns `true` if the CAS succeeded, `false` if it lost the race.
    fn cas_ref(&self, name: &str, expected: Option<&str>, new: &Ref) -> Result<bool, StoreError>;

    /// Load a ref by name. `Ok(None)` if the ref does not exist.
    fn load_ref(&self, name: &str) -> Result<Option<Ref>, StoreError>;

    /// Acquire the store-wide advisory lock. The lock is released when the
    /// returned `File` is dropped.
    fn acquire_lock(&self) -> Result<File, StoreError>;

    /// Walk the commit DAG from `tip` back toward the root, returning each
    /// `Commit` in order (tip first). Stops early if `base` is reached.
    fn walk_dag(
        &self,
        tip: &ContentHash,
        base: Option<&ContentHash>,
    ) -> Result<Vec<Commit>, StoreError>;

    /// Clean up orphaned `.tmp` files left by crashed writers.
    fn cleanup_orphans(&self) -> Result<usize, StoreError>;
}

/// Serialize a value to JCS canonical bytes.
pub fn jcs_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    canonical_bytes(value).map_err(StoreError::from)
}

/// Hash a serializable value to a `ContentHash` via JCS + SHA-256.
pub fn hash_object<T: Serialize>(value: &T) -> Result<ContentHash, StoreError> {
    content_hash(value).map_err(StoreError::from)
}

/// Read and deserialize a typed object from the store.
pub fn read_typed<T: DeserializeOwned + Serialize>(
    storage: &dyn Storage,
    hash: &ContentHash,
) -> Result<Option<T>, StoreError> {
    match storage.read_object(hash)? {
        Some(bytes) => {
            let value: T = serde_json::from_slice(&bytes).map_err(|e| StoreError::Corrupt {
                what: "object",
                path: hash.0.clone(),
                expected: "valid typed JSON".to_owned(),
                actual: e.to_string(),
            })?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

/// Serialize, hash, and persist a typed object; return its content hash.
pub fn write_typed<T: Serialize>(
    storage: &dyn Storage,
    obj: &T,
) -> Result<ContentHash, StoreError> {
    let bytes = jcs_bytes(obj)?;
    storage.write_object(&bytes)
}

// ── Filesystem backend ────────────────────────────────────────────────

/// Filesystem-backed storage: loose objects under `objects/`, refs under
/// `refs/`, advisory lock at `.lock`.
///
/// This is the only production backend (roadmap §LS-2: "exactly one backend:
/// the filesystem").
#[derive(Debug, Clone)]
pub struct FilesystemStorage {
    root: PathBuf,
}

impl FilesystemStorage {
    /// Open (or create) a store at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let storage = Self { root: root.clone() };
        std::fs::create_dir_all(root.join("objects")).expect("create objects dir");
        std::fs::create_dir_all(root.join("refs")).expect("create refs dir");
        storage
    }

    /// The store root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Compute the on-disk path for an object hash.
    /// `sha256:abc...` → `objects/ab/cdef....json`
    fn object_path(&self, hash: &ContentHash) -> PathBuf {
        let hex = hash.as_str().strip_prefix("sha256:").unwrap_or(&hash.0);
        let (prefix, rest) = hex.split_at(2.min(hex.len()));
        self.root
            .join("objects")
            .join(prefix)
            .join(format!("{rest}.json"))
    }

    /// The on-disk path for a ref named `name`.
    fn ref_path(&self, name: &str) -> PathBuf {
        // Names can contain '/' (e.g. "fork/option-x-true") for nested refs.
        self.root.join("refs").join(name)
    }

    /// The lock file path.
    fn lock_path(&self) -> PathBuf {
        self.root.join(".lock")
    }

    /// Make a directory durable.
    fn sync_dir(dir: &Path) -> Result<(), StoreError> {
        let file = File::open(dir).map_err(|e| StoreError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        file.sync_all().map_err(|e| StoreError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    /// Publish bytes to a path atomically: write to `.tmp`, fsync, rename,
    /// fsync parent dir.
    fn publish_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
        let parent = path.parent().expect("path has a parent directory");
        std::fs::create_dir_all(parent).map_err(|e| StoreError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| StoreError::Io {
                path: path.display().to_string(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "non-UTF-8 filename"),
            })?;

        let dir_name = parent
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("objects");

        let temp_name = format!(".{dir_name}.{filename}.tmp");
        let temp_path = parent.join(&temp_name);

        // Remove a stale temp file from a crashed previous write.
        let _ = fs::remove_file(&temp_path);

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&temp_path)
            .map_err(|e| StoreError::Io {
                path: temp_path.display().to_string(),
                source: e,
            })?;

        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| StoreError::Io {
                path: temp_path.display().to_string(),
                source: e,
            })?;

        // Atomic rename: either the old or the new file is visible, never torn.
        fs::rename(&temp_path, path).map_err(|e| StoreError::Io {
            path: path.display().to_string(),
            source: e,
        })?;

        // Make the directory entry durable.
        Self::sync_dir(parent)?;
        if let Some(grandparent) = parent.parent() {
            Self::sync_dir(grandparent)?;
        }
        Ok(())
    }
}

impl Storage for FilesystemStorage {
    fn read_object(&self, hash: &ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.object_path(hash);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|e| StoreError::Io {
            path: path.display().to_string(),
            source: e,
        })?;

        // Verify content address: re-hash the bytes and compare to the hash.
        let actual = crate::canonical::digest_bytes(&bytes);
        if actual != hash.0 {
            return Err(StoreError::Corrupt {
                what: "object",
                path: path.display().to_string(),
                expected: hash.0.clone(),
                actual,
            });
        }
        Ok(Some(bytes))
    }

    fn write_object(&self, bytes: &[u8]) -> Result<ContentHash, StoreError> {
        let digest = crate::canonical::digest_bytes(bytes);
        let hash = ContentHash(digest);
        let path = self.object_path(&hash);

        // Idempotent: if the object already exists, skip the write.
        if path.exists() {
            return Ok(hash);
        }

        Self::publish_atomic(&path, bytes)?;
        Ok(hash)
    }

    fn cas_ref(&self, name: &str, expected: Option<&str>, new: &Ref) -> Result<bool, StoreError> {
        let path = self.ref_path(name);

        // Read the current commit hash (if any).
        let current: Option<String> = if path.exists() {
            let bytes = fs::read(&path).map_err(|e| StoreError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            let current_ref: Ref =
                serde_json::from_slice(&bytes).map_err(|e| StoreError::Corrupt {
                    what: "ref",
                    path: path.display().to_string(),
                    expected: "valid ref JSON".to_owned(),
                    actual: e.to_string(),
                })?;
            Some(current_ref.commit.0.clone())
        } else {
            None
        };

        // Compare-and-swap check.
        let expected_str = expected.unwrap_or("");
        let current_str = current.as_deref().unwrap_or("");
        if expected_str != current_str {
            return Ok(false); // lost the race
        }

        // Write the new ref atomically.
        let ref_bytes = jcs_bytes(new)?;
        Self::publish_atomic(&path, &ref_bytes)?;
        Ok(true)
    }

    fn load_ref(&self, name: &str) -> Result<Option<Ref>, StoreError> {
        let path = self.ref_path(name);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|e| StoreError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let value: Ref = serde_json::from_slice(&bytes).map_err(|e| StoreError::Corrupt {
            what: "ref",
            path: path.display().to_string(),
            expected: "valid ref JSON".to_owned(),
            actual: e.to_string(),
        })?;
        Ok(Some(value))
    }

    fn acquire_lock(&self) -> Result<File, StoreError> {
        let path = self.lock_path();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|e| StoreError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
        file.lock_exclusive()
            .map_err(|e| StoreError::Lock(e.to_string()))?;
        Ok(file)
    }

    fn walk_dag(
        &self,
        tip: &ContentHash,
        base: Option<&ContentHash>,
    ) -> Result<Vec<Commit>, StoreError> {
        let mut chain = Vec::new();
        let mut current: Option<ContentHash> = Some(tip.clone());

        while let Some(hash) = &current {
            let Some(bytes) = self.read_object(hash)? else {
                return Err(StoreError::NotFound(format!(
                    "commit object not found: {hash}"
                )));
            };
            let commit: Commit =
                serde_json::from_slice(&bytes).map_err(|e| StoreError::Corrupt {
                    what: "commit",
                    path: format!("object {hash}"),
                    expected: "valid commit JSON".to_owned(),
                    actual: e.to_string(),
                })?;

            chain.push(commit.clone());
            let is_base = base.is_some_and(|b| hash == b);
            current.clone_from(&commit.parent);
            if is_base {
                break;
            }
        }

        Ok(chain)
    }

    fn cleanup_orphans(&self) -> Result<usize, StoreError> {
        let mut count = 0;
        let objects_dir = self.root.join("objects");
        if !objects_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&objects_dir).map_err(|e| StoreError::Io {
            path: objects_dir.display().to_string(),
            source: e,
        })? {
            let entry = entry.map_err(|e| StoreError::Io {
                path: objects_dir.display().to_string(),
                source: e,
            })?;
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            for file_entry in fs::read_dir(&dir).map_err(|e| StoreError::Io {
                path: dir.display().to_string(),
                source: e,
            })? {
                let file_entry = file_entry.map_err(|e| StoreError::Io {
                    path: dir.display().to_string(),
                    source: e,
                })?;
                let path = file_entry.path();
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
                {
                    fs::remove_file(&path).map_err(|e| StoreError::Io {
                        path: path.display().to_string(),
                        source: e,
                    })?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }
}
