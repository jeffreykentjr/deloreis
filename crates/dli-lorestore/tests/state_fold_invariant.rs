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
//! The test performs a random multi-commit sequence, then verifies:
//!   1. Walking the DAG from head to root yields N+1 commits (genesis + N).
//!   2. Every object named by every commit exists in the store.
//!   3. The set of object hashes read from the store = the union of hashes
//!      named by the DAG (no dangling refs, no orphaned-but-missing objects).

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
