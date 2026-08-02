//! LS-2 oracle: crash safety.
//!
//! The store must never be left in a torn state when the process is killed
//! mid-commit. The crash model is:
//!
//! 1. `FilesystemStorage` writes objects via `publish_atomic` — a temp file,
//!    fsync, rename, fsync parent dir. Either the old or the new filename is
//!    visible, never a half-written file.
//! 2. The ref update uses `publish_atomic` + CAS — either the old or the new
//!    ref value is visible.
//!
//! The test injects crashes at fault points (after object write, before/after
//! ref update) using a `FaultStorage` wrapper that panics at configurable
//! injection points. On "recovery" (re-opening with a new store, faults
//! disabled), the store must satisfy:
//!
//! - The `main` ref points to a valid commit (or genesis if the crash
//!   happened before the ref was updated).
//! - Every object named by reachable commits exists.
//! - The crash did not corrupt any pre-existing data.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_lorestore::{ContentHash, Lorestore, Storage, StoreError};

/// A fault injection counter shared between the test and the `FaultStorage`
/// wrapper. When the counter reaches the configured threshold, the fault
/// triggers a panic (simulating a process crash).
#[derive(Clone)]
struct FaultInjector {
    threshold: Arc<AtomicUsize>,
    counter: Arc<AtomicUsize>,
}

impl FaultInjector {
    fn new(threshold: usize) -> Self {
        Self {
            threshold: Arc::new(AtomicUsize::new(threshold)),
            counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Check the fault counter. If the threshold is reached, panic.
    ///
    /// Threshold semantics (1-indexed): threshold=1 crashes on the first
    /// `check` call, threshold=2 on the second, etc. Threshold=0 disables
    /// faults.
    fn check(&self, where_: &str) {
        let threshold = self.threshold.load(Ordering::SeqCst);
        if threshold == 0 {
            return; // disabled
        }
        let count = self.counter.fetch_add(1, Ordering::SeqCst);
        assert!(count + 1 < threshold, "FAULT INJECTED at {where_}");
    }

    /// Reset the threshold to 0 (disable faults).
    fn disable(&self) {
        self.threshold.store(0, Ordering::SeqCst);
    }
}

/// A `Storage` wrapper that injects faults (panics) at configurable points.
struct FaultStorage {
    inner: dli_lorestore::FilesystemStorage,
    injector: FaultInjector,
}

impl FaultStorage {
    fn new(inner: dli_lorestore::FilesystemStorage, injector: FaultInjector) -> Self {
        Self { inner, injector }
    }
}

impl std::fmt::Debug for FaultStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaultStorage").finish()
    }
}

impl Storage for FaultStorage {
    fn read_object(&self, hash: &ContentHash) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.read_object(hash)
    }

    fn write_object(&self, bytes: &[u8]) -> Result<ContentHash, StoreError> {
        self.injector.check("write_object");
        self.inner.write_object(bytes)
    }

    fn cas_ref(
        &self,
        name: &str,
        expected: Option<&str>,
        new: &dli_lorestore::Ref,
    ) -> Result<bool, StoreError> {
        self.injector.check("cas_ref");
        self.inner.cas_ref(name, expected, new)
    }

    fn load_ref(&self, name: &str) -> Result<Option<dli_lorestore::Ref>, StoreError> {
        self.inner.load_ref(name)
    }

    fn acquire_lock(&self) -> Result<std::fs::File, StoreError> {
        self.inner.acquire_lock()
    }

    fn walk_dag(
        &self,
        tip: &ContentHash,
        base: Option<&ContentHash>,
    ) -> Result<Vec<dli_lorestore::Commit>, StoreError> {
        self.inner.walk_dag(tip, base)
    }

    fn cleanup_orphans(&self) -> Result<usize, StoreError> {
        self.inner.cleanup_orphans()
    }
}

/// Helper: create a `Lorestore` backed by a `FaultStorage`. This is a thin
/// wrapper around the same construction logic as `Lorestore::open`, but lets
/// us inject the fault counter.
fn open_with_faults(dir: &std::path::Path, threshold: usize) -> (Lorestore, FaultInjector) {
    // First, initialise the genesis commit without faults.
    let _ = Lorestore::open(dir.to_path_buf()).expect("bootstrap genesis");

    // Then, re-open with fault injection wrapped around the storage.
    // The genesis commit and ref are already on disk.
    let injector = FaultInjector::new(threshold);
    let inner = dli_lorestore::FilesystemStorage::new(dir.to_path_buf());
    let faulted = FaultStorage::new(inner, injector.clone());
    (Lorestore::from_storage(Box::new(faulted)), injector)
}

/// Verify that the store at `dir` is consistent after a crash:
/// - The `main` ref exists and points to a readable commit.
/// - All objects named by the commit chain exist.
fn verify_store_consistency(dir: &std::path::Path) {
    let store = Lorestore::open(dir.to_path_buf()).expect("reopen store after crash");
    let head = store
        .root_hash()
        .expect("root hash")
        .expect("has a root ref after crash");

    // The head commit must be readable.
    let head_commit: Option<dli_lorestore::Commit> =
        store.read_object(&head).expect("read head commit");
    assert!(
        head_commit.is_some(),
        "head commit must be readable after crash recovery"
    );
    let _commit = head_commit.unwrap();

    // Walk the full DAG and verify every named object exists.
    let chain = store.walk_dag(&head, None).expect("walk dag");
    for c in &chain {
        for fact in &c.facts {
            let obj: Option<serde_json::Value> = store.read_object(fact).expect("read fact");
            assert!(obj.is_some(), "fact object {fact} must exist in store");
        }
        for lineage in &c.lineage {
            let obj: Option<serde_json::Value> = store.read_object(lineage).expect("read lineage");
            assert!(obj.is_some(), "lineage edge {lineage} must exist in store");
        }
        for evidence in &c.evidence {
            let obj: Option<serde_json::Value> =
                store.read_object(evidence).expect("read evidence");
            assert!(
                obj.is_some(),
                "evidence envelope {evidence} must exist in store"
            );
        }
    }
}

#[test]
fn store_not_torn_when_crash_before_ref_update() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Open with fault injection: crash on the cas_ref call (3rd fault point:
    // 1=write fact, 2=write commit object, 3=cas_ref ref).
    let (store, injector) = open_with_faults(dir.path(), 3);

    // Write a fact object (this should succeed — fault is on cas_ref, the 3rd
    // fault point: 1=write fact, 2=write commit object, 3=cas_ref).
    let fo = common::test_fact(
        "test/place/camelot",
        "place",
        &["camelot"],
        PosBool::truth(),
        Grade::Asserted,
    );
    let fact_hash = store.write_object(&fo).expect("write fact object");

    // Attempt a commit — this should panic at cas_ref (fault point 3).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.commit(
            vec!["atom-0".to_owned()],
            vec![fact_hash],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
    }));
    assert!(result.is_err(), "commit must panic at fault point");
    injector.disable();

    // After crash: the ref was NOT updated (CAS never completed).
    // The genesis commit must still be the head.
    // But the fact object was written to disk (it's just orphaned).
    verify_store_consistency(dir.path());

    // After crash before ref update, head must be the genesis commit.
    let store2 = Lorestore::open(dir.path().to_path_buf()).expect("reopen store");
    let head = store2.root_hash().expect("root hash").expect("has root");
    let genesis = dli_lorestore::Commit::root(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        String::new(),
    );
    let genesis_digest = genesis.content_hash().expect("genesis digest");
    assert_eq!(
        head.to_string(),
        genesis_digest.to_string(),
        "after crash before ref update, head must be genesis"
    );
}

#[test]
fn store_not_torn_when_crash_during_object_write() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Open with fault injection: crash on the FIRST write_object call.
    let (store, injector) = open_with_faults(dir.path(), 1);

    // Attempt to write a fact object — this should panic immediately.
    let fo = common::test_fact(
        "test/place/gwern",
        "place",
        &["gwern"],
        PosBool::truth(),
        Grade::Asserted,
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| store.write_object(&fo)));
    assert!(result.is_err(), "write_object must panic at fault point");
    injector.disable();

    // The store must still be consistent — the head ref is still genesis.
    verify_store_consistency(dir.path());

    // No orphaned objects should have been created by the failed write.
    let inner = dli_lorestore::FilesystemStorage::new(dir.path().to_path_buf());
    let orphan_count = inner.cleanup_orphans().expect("cleanup orphans");
    assert_eq!(orphan_count, 0, "no orphan objects after failed write");
}

#[test]
fn store_recovers_and_can_recommit_after_crash() {
    let dir = tempfile::tempdir().expect("create temp dir");

    // Phase 1: crash during commit (at cas_ref, the 3rd fault point).
    {
        let (store, injector) = open_with_faults(dir.path(), 3);

        let fo = common::test_fact(
            "test/place/camelot",
            "place",
            &["camelot"],
            PosBool::truth(),
            Grade::Asserted,
        );
        let fact_hash = store.write_object(&fo).expect("write fact");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            store.commit(
                vec!["atom-0".to_owned()],
                vec![fact_hash],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                String::new(),
            )
        }));
        assert!(result.is_err());
        injector.disable();
    }

    // Phase 2: recover — re-open, the store should be consistent.
    // The fact object is orphaned (written but not committed), but that's fine.
    verify_store_consistency(dir.path());

    // Phase 3: re-commit successfully — the orphaned fact object can be
    // referenced in a new commit.
    {
        let store = Lorestore::open(dir.path().to_path_buf()).expect("reopen store");

        // The fact object is still on disk (orphaned). Read it back, then commit.
        let _head = store.root_hash().expect("root").expect("has genesis");
        // The genesis is valid — we can now create a new child commit.

        // Write the same fact object again (idempotent — same hash).
        let fo = common::test_fact(
            "test/place/camelot",
            "place",
            &["camelot"],
            PosBool::truth(),
            Grade::Asserted,
        );
        let fact_hash = store.write_object(&fo).expect("write fact (idempotent)");

        // Commit — this time without faults.
        let commit_hash = store
            .commit(
                vec!["atom-0".to_owned()],
                vec![fact_hash],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                String::new(),
            )
            .expect("commit");

        // The root hash must have advanced.
        let root = store.root_hash().expect("root").expect("has root");
        assert_eq!(root.to_string(), commit_hash.to_string());

        // And the store must be consistent.
        verify_store_consistency(dir.path());
    }
}

#[test]
fn store_cleanup_removes_stale_temp_files() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // Write and commit a fact object to create a non-empty store.
    let fo = common::test_fact(
        "test/place/committed",
        "place",
        &["committed"],
        PosBool::truth(),
        Grade::Asserted,
    );
    let fact_hash = store.write_object(&fo).expect("write fact");
    let _commit_hash = store
        .commit(
            vec!["atom-0".to_owned()],
            vec![fact_hash],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit");

    // Simulate a stale .tmp file left behind by a crashed write.
    let inner = dli_lorestore::FilesystemStorage::new(dir.path().to_path_buf());
    let objects_dir = dir.path().join("objects");
    let sub_dir = objects_dir.join("ab");
    std::fs::create_dir_all(&sub_dir).expect("create objects/ab");
    let tmp_path = sub_dir.join(".ab.stale.tmp");
    std::fs::write(&tmp_path, b"stale").expect("write stale tmp");
    assert!(tmp_path.exists(), "stale tmp should exist before cleanup");

    // Cleanup should remove the stale .tmp file.
    let removed = inner.cleanup_orphans().expect("cleanup");
    assert!(removed > 0, "stale .tmp file should have been removed");
    assert!(
        !tmp_path.exists(),
        "stale .tmp file should be gone after cleanup"
    );
}
