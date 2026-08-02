//! Audit A9 oracle: the `LineageEdge` — the third leg of the three-way
//! identity split — exercised end-to-end: construct, hash, store, name in a
//! commit, read back. Also pins the hash-projection discipline: the edge's
//! content hash covers `authority`/`from`/`to`/`support`/`defeaters` only —
//! minting a claim id or bumping the schema version never destabilizes the
//! address, exactly as for `FactObject`. (First *production* emission of
//! edges belongs to the merge engine, LS-6.)

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use dli_core::atom::Atom;
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_lorestore::{Authority, ClaimId, LineageEdge, Lorestore};

#[test]
fn lineage_edge_round_trips_and_hashes_by_projection() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = Lorestore::open(dir.path().to_path_buf()).expect("open store");

    // Two versions of one claim.
    let older = common::test_fact(
        "edge/claim",
        "standing",
        &["keep", "contested"],
        PosBool::truth(),
        Grade::Asserted,
    );
    let newer = common::test_fact(
        "edge/claim",
        "standing",
        &["keep", "fortified"],
        PosBool::truth(),
        Grade::Asserted,
    );
    let from = store.write_object(&older).expect("write older");
    let to = store.write_object(&newer).expect("write newer");

    // The supersession is itself contested: it holds under event variable 0
    // (the ruling that accepted the retcon) and is defeated under variable 1.
    let edge = LineageEdge::new(
        Authority::named("second-continuity"),
        ClaimId::from_label("edge/claim"),
        from.clone(),
        to.clone(),
        PosBool::pos(Atom(0)),
        PosBool::pos(Atom(1)),
    );

    // Projection discipline: claim id and schema version are outside the hash.
    let base_hash = edge.content_hash().expect("edge hashes");
    let mut reminted = edge.clone();
    reminted.claim_id = ClaimId::from_label("edge/claim/reminted");
    assert_eq!(
        reminted.content_hash().expect("edge hashes"),
        base_hash,
        "minting a claim id must never destabilize the edge's address"
    );
    let mut upcast = edge.clone();
    upcast.schema_version = 2;
    assert_eq!(
        upcast.content_hash().expect("edge hashes"),
        base_hash,
        "a schema-version bump must never destabilize the edge's address"
    );
    let mut different = edge.clone();
    different.support = PosBool::pos(Atom(2));
    assert_ne!(
        different.content_hash().expect("edge hashes"),
        base_hash,
        "a different supersession judgment is a different edge"
    );

    // Store the edge, name it in a commit, read it back.
    let edge_hash = store.write_object(&edge).expect("write edge");
    let commit_hash = store
        .commit(
            vec!["accept-retcon".to_owned(), "reject-retcon".to_owned()],
            vec![from, to],
            vec![edge_hash.clone()],
            Vec::new(),
            Vec::new(),
            "edges".to_owned(),
        )
        .expect("commit names the edge");

    let read: LineageEdge = store
        .read_object(&edge_hash)
        .expect("read ok")
        .expect("edge present");
    assert_eq!(read, edge, "the edge reads back byte-faithful");

    let chain = store
        .walk_dag(&commit_hash, None)
        .expect("walk from the new head");
    assert!(
        chain.first().is_some_and(|c| c.lineage == vec![edge_hash]),
        "the head commit names the lineage edge"
    );
}
