//! LS-2 oracle: cross-platform root-hash determinism.
//!
//! The store's root commit hash must be byte-identical across platforms — the
//! same operation sequence must yield the same root hash wherever the store
//! runs. This is guaranteed by JCS canonicalization (RFC 8785), the
//! absorption-normalized DNF of `PosBool`, and the lexicographic ordering of
//! `BTreeSet` — none of which depend on the OS.
//!
//! Honest scope (audit A11): CI exercises this on Linux only, and the store's
//! directory-fsync durability discipline is Unix-only (`sync_dir` is a no-op
//! elsewhere). The determinism *claim* is platform-independent by
//! construction; the *verification* covers the platforms CI actually runs.
//!
//! The test writes a small fixed set of fact objects, commits them, and checks
//! the root hash against a hard-coded expected value. If the hash drifts, the
//! gate catches a platform-specific serialization bug.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

mod common;

use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_lorestore::{FactObject, Lorestore};

/// The expected root commit hash for the deterministic write sequence below.
/// This value is computed from the content alone — if it ever changes, a
/// serialization invariant was broken (floats, unsorted keys, platform path
/// separators in content).
// Pinned under schema v2 (LS-9): the schema_version field participates in the
// commit/object bytes, so the LS-9 bump from v1 to v2 re-pinned this golden.
const EXPECTED_ROOT_HASH: &str =
    "sha256:5e160542a1de285a7a730537d051a18dac04c2ed4a21e9e498f5bbe1d3e3f432";

/// Build the deterministic sequence of fact objects used by this test.
fn deterministic_facts() -> Vec<FactObject> {
    vec![
        FactObject::new(
            dli_lorestore::ClaimId::from_label("test/place/camelot"),
            dli_eval::world::GroundAtom {
                pred: "place".to_owned(),
                args: vec!["camelot".to_owned()],
            },
            PosBool::truth(),
            PosBool::FALSE,
            Vec::new(),
            Grade::Asserted,
        ),
        FactObject::new(
            dli_lorestore::ClaimId::from_label("test/place/trefpen"),
            dli_eval::world::GroundAtom {
                pred: "place".to_owned(),
                args: vec!["trefpen".to_owned()],
            },
            PosBool::pos(dli_core::atom::Atom(0)),
            PosBool::FALSE,
            Vec::new(),
            Grade::Asserted,
        ),
        FactObject::new(
            dli_lorestore::ClaimId::from_label("test/role/camelot/hub"),
            dli_eval::world::GroundAtom {
                pred: "role".to_owned(),
                args: vec!["camelot".to_owned(), "hub".to_owned()],
            },
            PosBool::truth(),
            PosBool::FALSE,
            Vec::new(),
            Grade::Asserted,
        ),
    ]
}

#[test]
fn root_hash_is_deterministic_across_writes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // Write the deterministic fact objects.
    let facts = deterministic_facts();
    let mut fact_hashes = Vec::with_capacity(facts.len());
    for fo in &facts {
        let hash = store.write_object(fo).expect("write fact object");
        fact_hashes.push(hash);
    }

    // Commit them with an empty interner (no event-variable names needed for
    // these positive-only atoms).
    let root_hash = store
        .commit(
            Vec::new(),
            fact_hashes,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit");

    // The root hash must be deterministic — content-addressed and platform-
    // independent.
    assert_eq!(
        root_hash.to_string(),
        EXPECTED_ROOT_HASH,
        "root hash must be deterministic across platforms"
    );

    // Re-opening the store must yield the same root hash.
    let store2 = Lorestore::open(dir.path().to_path_buf()).expect("reopen store");
    let root2 = store2.root_hash().expect("root hash").expect("has a root");
    assert_eq!(
        root2.to_string(),
        EXPECTED_ROOT_HASH,
        "root hash must survive a reopen"
    );
}

#[test]
fn root_hash_changes_when_content_changes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // First commit: one fact.
    let fo1 = deterministic_facts().into_iter().next().unwrap();
    let hash1 = store.write_object(&fo1).expect("write");
    let root1 = store
        .commit(
            Vec::new(),
            vec![hash1],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit");
    assert_ne!(root1.to_string(), EXPECTED_ROOT_HASH);

    // Second commit: add a different fact.
    let fo2 = FactObject::new(
        dli_lorestore::ClaimId::from_label("test/place/gwern"),
        dli_eval::world::GroundAtom {
            pred: "place".to_owned(),
            args: vec!["gwern".to_owned()],
        },
        PosBool::truth(),
        PosBool::FALSE,
        Vec::new(),
        Grade::Asserted,
    );
    let hash2 = store.write_object(&fo2).expect("write");
    let root2 = store
        .commit(
            Vec::new(),
            vec![hash2],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit");
    assert_ne!(
        root1.to_string(),
        root2.to_string(),
        "different content must yield different root"
    );
}

#[test]
fn root_hash_is_stable_for_identical_re_writes() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // Writing the same object twice is idempotent — the content hash is the same.
    let fo = deterministic_facts().into_iter().next().unwrap();
    let h1 = store.write_object(&fo).expect("write 1");
    let h2 = store.write_object(&fo).expect("write 2");
    assert_eq!(
        h1.to_string(),
        h2.to_string(),
        "idempotent writes must produce same hash"
    );

    // Committing the same hash twice (in separate commits) must produce
    // different root hashes (different commit objects — parent differs).
    let root1 = store
        .commit(
            Vec::new(),
            vec![h1],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit 1");
    let root2 = store
        .commit(
            Vec::new(),
            vec![h2],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit 2");
    assert_ne!(
        root1.to_string(),
        root2.to_string(),
        "commits must chain — same facts, different commit, different root"
    );
}

#[test]
fn genesis_root_hash_is_fixed() {
    // The genesis commit (empty) has a deterministic hash.
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");
    let root = store.root_hash().expect("root hash").expect("has root");

    // Re-open: must be the same.
    let dir2 = std::path::PathBuf::from(dir.path());
    let store2 = Lorestore::open(dir2).expect("reopen");
    let root2 = store2.root_hash().expect("root hash").expect("has root");
    assert_eq!(root.to_string(), root2.to_string());
}
