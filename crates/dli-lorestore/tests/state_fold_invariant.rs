//! LS-2 oracle: `state ≡ fold(commit log)`.
//!
//! The set of all objects reachable from the head ref must equal the union of
//! objects named by all commits in the chain from root to head. This is the
//! fundamental persistence invariant — a torn write, a missing object, or a
//! ref pointing at a commit that names nonexistent objects would violate it.
//!
//! Each commit names:
//!   - fact objects (via `commit.facts`)
//!   - lineage edges (via `commit.lineage`)
//!   - evidence envelopes (via `commit.evidence`)
//!   - the commit object itself
//!   - the interner is stored *inside* the commit (not as a separate object)
//!
//! The tests cover a fixed multi-commit example plus a seeded-random property
//! run (`state_equals_fold_property`), which verifies **both directions**:
//!   1. Every object named by every commit in the DAG exists and reads back.
//!   2. Every object present on disk is named by the DAG — set equality, no
//!      unnamed survivors (orphans are permitted only in the dedicated orphan
//!      test, which tracks them explicitly).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

mod common;

use std::collections::HashSet;

use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_lorestore::{FactObject, Lorestore};

#[test]
fn state_equals_fold_of_commit_log_single_commit() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // Write one fact object and commit it.
    let fo = common::test_fact(
        "test/place/camelot",
        "place",
        &["camelot"],
        PosBool::truth(),
        Grade::Asserted,
    );
    let fact_hash = store.write_object(&fo).expect("write fact");
    let commit_hash = store
        .commit(
            vec!["c1".to_owned()],
            vec![fact_hash.clone()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit");

    // Walk the DAG from the head.
    let head = store.root_hash().expect("root hash").expect("has root");
    assert_eq!(head.to_string(), commit_hash.to_string());

    let chain = store.walk_dag(&head, None).expect("walk dag");
    assert_eq!(chain.len(), 2, "genesis + 1 child commit");

    // Collect all object hashes named by the DAG.
    let mut named: HashSet<String> = HashSet::new();
    named.insert(head.to_string()); // the head commit
    for commit in &chain {
        named.insert(commit.content_hash().expect("hash").to_string());
        for fact in &commit.facts {
            named.insert(fact.to_string());
        }
        for lineage in &commit.lineage {
            named.insert(lineage.to_string());
        }
        for evidence in &commit.evidence {
            named.insert(evidence.to_string());
        }
    }

    // Every named object must be readable from the store.
    for hash_str in &named {
        let hash = dli_lorestore::ContentHash(hash_str.clone());
        let obj: Option<serde_json::Value> = store.read_object(&hash).expect("read object");
        assert!(
            obj.is_some(),
            "object {hash_str} named by DAG must exist in store"
        );
    }
}

#[test]
fn state_equals_fold_of_commit_log_multi_commit() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // Commit 3 rounds of facts. The interner grows append-only across the
    // rounds — a whole-replacement interner is refused by `commit` (audit A6).
    let mut all_fact_hashes: Vec<Vec<dli_lorestore::ContentHash>> = Vec::new();
    let mut interner: Vec<String> = Vec::new();
    for round in 0..3 {
        let fo = common::test_fact(
            &format!("test/place/location-{round}"),
            "place",
            &[&format!("loc-{round}")],
            PosBool::truth(),
            Grade::Asserted,
        );
        let fact_hash = store.write_object(&fo).expect("write fact");
        interner.push(format!("atom-{round}"));
        let _commit_hash = store
            .commit(
                interner.clone(),
                vec![fact_hash.clone()],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                String::new(),
            )
            .expect("commit");
        all_fact_hashes.push(vec![fact_hash]);
    }

    // Walk the DAG from the head.
    let head = store.root_hash().expect("root hash").expect("has root");
    let chain = store.walk_dag(&head, None).expect("walk dag");

    // Should be genesis + 3 child commits = 4 commits.
    assert_eq!(chain.len(), 4, "genesis + 3 child commits");

    // Collect all object hashes named by the DAG.
    let mut named_objects: HashSet<dli_lorestore::ContentHash> = HashSet::new();
    named_objects.insert(head.clone());
    for commit in &chain {
        named_objects.insert(commit.content_hash().expect("hash").clone());
        for fact in &commit.facts {
            named_objects.insert(fact.clone());
        }
    }

    // Every named object must be readable from the store.
    for hash in &named_objects {
        let obj: Option<serde_json::Value> = store.read_object(hash).expect("read object");
        assert!(
            obj.is_some(),
            "object {hash} named by DAG must exist in store"
        );
    }

    // The set of fact hashes named by the DAG must match what we committed.
    let mut dag_fact_hashes: HashSet<dli_lorestore::ContentHash> = HashSet::new();
    for commit in &chain {
        for fact in &commit.facts {
            dag_fact_hashes.insert(fact.clone());
        }
    }
    let mut committed_fact_hashes: HashSet<dli_lorestore::ContentHash> = HashSet::new();
    for round in &all_fact_hashes {
        for h in round {
            committed_fact_hashes.insert(h.clone());
        }
    }
    assert_eq!(
        dag_fact_hashes, committed_fact_hashes,
        "DAG fact hashes must equal committed fact hashes"
    );

    // All fact objects must be readable with the expected content.
    for hash in &committed_fact_hashes {
        let obj: Option<FactObject> = store.read_object(hash).expect("read fact object");
        assert!(obj.is_some(), "fact object must be readable");
    }
}

#[test]
fn state_equals_fold_of_commit_log_orphan_cleanup() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // Write facts but commit only some — the un-committed ones are orphans
    // that exist on disk but are not named by any commit.
    let fo1 = common::test_fact(
        "test/place/committed",
        "place",
        &["committed"],
        PosBool::truth(),
        Grade::Asserted,
    );
    let fo2 = common::test_fact(
        "test/place/orphan",
        "place",
        &["orphan"],
        PosBool::truth(),
        Grade::Asserted,
    );

    let hash1 = store.write_object(&fo1).expect("write committed");
    let hash2 = store.write_object(&fo2).expect("write orphan");

    // Only commit hash1, not hash2.
    let _commit_hash = store
        .commit(
            vec!["orphan-test".to_owned()],
            vec![hash1.clone()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        )
        .expect("commit");

    // Walk the DAG and collect named objects.
    let head = store.root_hash().expect("root").expect("has root");
    let chain = store.walk_dag(&head, None).expect("walk");

    let mut named: HashSet<dli_lorestore::ContentHash> = HashSet::new();
    named.insert(head.clone());
    for commit in &chain {
        named.insert(commit.content_hash().expect("hash").clone());
        for fact in &commit.facts {
            named.insert(fact.clone());
        }
    }

    // hash2 is on disk but should NOT be in the named set.
    assert!(named.contains(&hash1), "committed fact must be named");
    assert!(
        !named.contains(&hash2),
        "orphan fact must NOT be named by DAG"
    );

    // But it IS readable (it's just garbage, not corruption).
    let orphan: Option<FactObject> = store.read_object(&hash2).expect("read orphan");
    assert!(
        orphan.is_some(),
        "orphan object should still be on disk (garbage, not torn)"
    );
}

#[test]
fn state_equals_fold_of_commit_log_empty_store() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    let head = store
        .root_hash()
        .expect("root hash")
        .expect("has genesis root");
    let chain = store.walk_dag(&head, None).expect("walk dag");

    // Only the genesis commit.
    assert_eq!(chain.len(), 1, "only genesis commit");

    // Genesis must have no facts.
    assert!(chain[0].facts.is_empty(), "genesis has no facts");

    // The genesis commit must be readable.
    let genesis_bytes: Option<serde_json::Value> = store.read_object(&head).expect("read genesis");
    assert!(genesis_bytes.is_some(), "genesis commit must be readable");
}

/// The property form (audit A11): seeded-random commit sequences, verified in
/// **both directions** — every DAG-named object reads back, and every on-disk
/// object is DAG-named. Deterministic (fixed seeds, no clock, no OS
/// randomness), so the gate replays byte-identically.
#[test]
fn state_equals_fold_property() {
    // A tiny deterministic LCG — no external randomness enters the gate.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0
        }
    }

    for seed in [1_u64, 7, 42, 1999] {
        let mut rng = Lcg(seed);
        let dir = tempfile::tempdir().expect("create temp dir");
        let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

        let mut interner: Vec<String> = Vec::new();
        let mut named: HashSet<String> = HashSet::new();
        // The genesis commit is named by the DAG.
        named.insert(
            store
                .root_hash()
                .expect("root ok")
                .expect("genesis")
                .to_string(),
        );

        let rounds = 2 + (rng.next() % 5) as usize;
        for round in 0..rounds {
            // 1..=3 facts per round, occasionally re-writing an earlier fact
            // (idempotent CAS — the object must still be named exactly once
            // on disk).
            let fact_count = 1 + (rng.next() % 3) as usize;
            let mut fact_hashes = Vec::new();
            for f in 0..fact_count {
                let key = if rng.next().is_multiple_of(4) && round > 0 {
                    format!("seed{seed}/repeat/shared")
                } else {
                    format!("seed{seed}/round{round}/fact{f}")
                };
                let fo = common::test_fact(
                    &key,
                    "place",
                    &[&format!("loc-{round}-{f}")],
                    PosBool::truth(),
                    Grade::Asserted,
                );
                let hash = store.write_object(&fo).expect("write fact");
                named.insert(hash.to_string());
                fact_hashes.push(hash);
            }
            interner.push(format!("seed{seed}-atom-{round}"));
            let commit_hash = store
                .commit(
                    interner.clone(),
                    fact_hashes,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    String::new(),
                )
                .expect("commit");
            named.insert(commit_hash.to_string());
        }

        // Direction 1: every DAG-named object reads back.
        let head = store.root_hash().expect("root ok").expect("head");
        let chain = store.walk_dag(&head, None).expect("walk");
        let mut dag_named: HashSet<String> = HashSet::new();
        for commit in &chain {
            dag_named.insert(commit.content_hash().expect("hash").to_string());
            for fact in &commit.facts {
                dag_named.insert(fact.to_string());
                let read: Option<FactObject> = store.read_object(fact).expect("read ok");
                assert!(read.is_some(), "seed {seed}: DAG-named fact must read back");
            }
        }
        assert_eq!(
            dag_named, named,
            "seed {seed}: the DAG must name exactly the objects the run created"
        );

        // Direction 2: every on-disk object is DAG-named — set equality.
        let mut on_disk: HashSet<String> = HashSet::new();
        let objects_dir = dir.path().join("objects");
        for prefix in std::fs::read_dir(&objects_dir).expect("objects dir") {
            let prefix = prefix.expect("entry").path();
            if !prefix.is_dir() {
                continue;
            }
            let prefix_name = prefix
                .file_name()
                .and_then(|n| n.to_str())
                .expect("utf8")
                .to_owned();
            for file in std::fs::read_dir(&prefix).expect("prefix dir") {
                let file = file.expect("entry").path();
                let stem = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".json"))
                    .expect("object filename");
                on_disk.insert(format!("sha256:{prefix_name}{stem}"));
            }
        }
        assert_eq!(
            on_disk, dag_named,
            "seed {seed}: state ≡ fold(commit log) — no unnamed survivors, no missing objects"
        );
    }
}
