//! LS-6 serve-level integration: `author.branch` and `author.merge` end-to-end
//! through the real `respond` path (contract 0.6.1).
//!
//! The branch/merge conflict engine is proven exhaustively at the lorestore layer
//! (`dli-lorestore/tests/merge.rs`); here we prove the *wire* plumbing: the ops
//! dispatch through Door T, drive the workspace's durable lorestore, and answer
//! with the typed contract shape — including a real conflict report surfaced all
//! the way to JSON.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;

use dli_core::atom::Atom;
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_lorestore::{ClaimId, FactObject};
use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_world::{KilledTokens, Pack};
use serde_json::{Value, json};

const V: &str = "0.6.1";

// ── a project store with a lorestore, mirroring serve.rs / the fixture harness ──

fn fixture_project_store() -> (tempfile::TempDir, Sessions, Store) {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/dli-worker/fixture-project");
    let temp = tempfile::tempdir().expect("temp project");
    copy_tree(&source, temp.path());
    let project = Arc::new(Project::load(temp.path()).expect("fixture project loads"));
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let mut store = Store::with_packs(packs).expect("fixture project compiles");
    store
        .attach_lorestore(temp.path().join(".dli-lorestore"))
        .expect("attach lorestore");
    (temp, Sessions::from_project(Arc::clone(&project)), store)
}

fn copy_tree(source: &Path, destination: &Path) {
    for entry in std::fs::read_dir(source).expect("read project") {
        let entry = entry.expect("entry");
        let to = destination.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            std::fs::create_dir(&to).expect("mkdir");
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).expect("copy");
        }
    }
}

/// A store over a bespoke functional-predicate base, with a lorestore attached.
fn functional_store() -> (tempfile::TempDir, Sessions, Store) {
    let source = "scope canon\n\nratify d-1\n\nhome(mara, shallows) decided d-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "base parses clean: {diagnostics:?}"
    );
    let pack = Pack {
        name: "canon".to_owned(),
        documents: vec![document],
    };
    let mut store =
        Store::with_packs([("canon".to_owned(), pack, KilledTokens::default())]).expect("compiles");
    let temp = tempfile::tempdir().expect("temp");
    store
        .attach_lorestore(temp.path().join(".dli-lorestore"))
        .expect("attach lorestore");
    (temp, Sessions::new(), store)
}

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let response = respond(&frame.to_string(), sessions, store).expect("a response");
    serde_json::to_value(&response).expect("serialize")
}

fn open_workspace(scope: &str, sessions: &mut Sessions, store: &mut Store) -> String {
    let resp = drive(
        &json!({ "dli": V, "id": "open", "op": "author.open", "params": { "scope": scope } }),
        sessions,
        store,
    );
    assert!(resp["ok"].as_bool().unwrap(), "open ok: {resp}");
    resp["result"]["workspace"].as_str().unwrap().to_owned()
}

// ── plumbing: create / list / clean merge ───────────────────────────────────

#[test]
fn branch_create_list_and_clean_merge_through_the_wire() {
    let (_t, mut sessions, mut store) = fixture_project_store();
    let ws = open_workspace("saltmarket", &mut sessions, &mut store);

    let created = drive(
        &json!({ "dli": V, "id": "1", "op": "author.branch",
                 "params": { "workspace": ws, "action": "create", "name": "feature" } }),
        &mut sessions,
        &mut store,
    );
    assert!(created["ok"].as_bool().unwrap(), "{created}");
    assert_eq!(created["result"]["created"], "feature");
    assert_eq!(
        created["result"]["branches"],
        json!(["main", "feature"]),
        "main first, then the new branch"
    );

    let listed = drive(
        &json!({ "dli": V, "id": "2", "op": "author.branch",
                 "params": { "workspace": ws, "action": "list" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(listed["result"]["branches"], json!(["main", "feature"]));

    let merged = drive(
        &json!({ "dli": V, "id": "3", "op": "author.merge",
                 "params": { "workspace": ws, "source": "feature" } }),
        &mut sessions,
        &mut store,
    );
    assert!(merged["ok"].as_bool().unwrap(), "{merged}");
    assert!(
        merged["result"]["merge_commit"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"),
        "returns the merge commit hash"
    );
    assert_eq!(merged["result"]["conflicts"], json!([]), "clean merge");
    assert_eq!(merged["result"]["stratifies"], true);
    assert_eq!(merged["result"]["standing_changes"], json!([]));
}

// ── a real conflict, surfaced through the wire ──────────────────────────────

#[test]
fn merge_conflict_surfaces_through_the_wire() {
    let (_t, mut sessions, mut store) = functional_store();
    let ws = open_workspace("canon", &mut sessions, &mut store);

    // Create a rival branch and commit an incompatible `home` for mara directly
    // on the lorestore — the wire has no branch-scoped write yet, so this stands
    // in for one; the merge itself goes through the full serve path.
    {
        let lorestore = store.workspace_lorestore(&ws).expect("lorestore attached");
        lorestore.create_branch("rival", "main").expect("branch");
        // `d-1` is the sole decision, interned at index 0.
        let rival_fact = FactObject::new(
            ClaimId::from_label("rival/home"),
            dli_eval::world::GroundAtom {
                pred: "home".to_owned(),
                args: vec!["mara".to_owned(), "highlands".to_owned()],
            },
            PosBool::pos(Atom(0)),
            PosBool::FALSE,
            Vec::new(),
            Grade::Asserted,
        );
        let hash = lorestore.write_object(&rival_fact).expect("write");
        let (interner, decisions) = {
            let head = lorestore.branch_head("rival").unwrap().unwrap();
            let commit: dli_lorestore::Commit = lorestore.read_object(&head).unwrap().unwrap();
            (commit.interner, commit.decisions)
        };
        lorestore
            .commit_on_branch(
                "rival",
                interner,
                vec![hash],
                Vec::new(),
                Vec::new(),
                decisions,
                "canon".to_owned(),
            )
            .expect("commit rival");
    }

    let merged = drive(
        &json!({ "dli": V, "id": "1", "op": "author.merge",
                 "params": { "workspace": ws, "source": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert!(merged["ok"].as_bool().unwrap(), "{merged}");
    let conflicts = merged["result"]["conflicts"].as_array().unwrap();
    assert!(
        conflicts.iter().any(|c| c["kind"] == "certain"),
        "a certain-layer functional-dependency conflict reaches the wire: {merged}"
    );
    assert!(
        merged["result"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "merge.functional-dependency"),
        "the graded finding rides along: {merged}"
    );
}

// ── why a conflict cannot be forced from the wire (structural finding) ──────

/// A regression test pinning the LS-6 scope boundary: a merge **conflict cannot
/// be driven through pure wire ops**. Two independent guards make it so.
///
/// 1. Door T never mints certain content — facts are always `proposed`
///    (`refuse_cited` blocks citations, DL-0014/DL-0043) — and no wire op sets a
///    branch valuation. Every conflict class needs one or the other.
/// 2. Even the *attempt* is caught upstream: materializing a value that
///    contradicts the canon functional slot is **blocked by the workspace's own
///    continuity check** (`factcheck.contradicted`) before it can reach a
///    branch, so the merge layer never sees a conflicting fact.
///
/// So the conflict-report shape is pinned by the digest-checked
/// `author.merge.response.json` schema and proven end-to-end by
/// `merge_conflict_surfaces_through_the_wire` (which seeds divergence via the
/// lorestore, a legitimate test affordance), not by a wire-only golden fixture —
/// which is not constructible.
#[test]
fn wire_added_contradiction_is_blocked_before_it_can_conflict() {
    let (_t, mut sessions, mut store) = functional_store();
    let ws = open_workspace("canon", &mut sessions, &mut store);

    drive(
        &json!({ "dli": V, "id": "1", "op": "author.branch",
                 "params": { "workspace": ws, "action": "create", "name": "rival" } }),
        &mut sessions,
        &mut store,
    );

    // A *proposed* second `home` value for mara contradicts the canon functional
    // slot `home(mara, shallows)`.
    drive(
        &json!({ "dli": V, "id": "2", "op": "author.add_relation",
                 "params": { "workspace": ws, "relation": "home", "from": "mara", "to": "highlands",
                             "provenance": { "kind": "proposed", "origin": "demo" } } }),
        &mut sessions,
        &mut store,
    );
    // Materialize is refused by the workspace continuity guard — the conflicting
    // value never reaches a branch, so no merge conflict can be authored.
    let mat = drive(
        &json!({ "dli": V, "id": "3", "op": "author.materialize",
                 "params": { "workspace": ws } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        mat["error"]["code"], "materialize-blocked",
        "the contradicting value is barred upstream, before any merge: {mat}"
    );
    assert!(
        mat["error"]["data"]["blocking"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "factcheck.contradicted"),
        "blocked specifically on the functional-slot contradiction: {mat}"
    );
}

// ── version negotiation: branch/merge are 0.6.1+ capabilities ───────────────

#[test]
fn branch_merge_capabilities_are_gated_by_version() {
    let (_t, mut sessions, mut store) = fixture_project_store();
    for (version, expect_present) in [
        ("0.5.1", false),
        ("0.6.0", false),
        ("0.6.1", true),
        ("0.6.2", true),
        ("0.6.3", true),
    ] {
        let resp = drive(
            &json!({ "dli": version, "id": "v", "op": "dli.version", "params": {} }),
            &mut sessions,
            &mut store,
        );
        let caps = resp["result"]["capabilities"].as_array().unwrap();
        let has_branch = caps.iter().any(|c| c == "author.branch");
        let has_merge = caps.iter().any(|c| c == "author.merge");
        assert_eq!(
            has_branch, expect_present,
            "author.branch capability at {version}"
        );
        assert_eq!(
            has_merge, expect_present,
            "author.merge capability at {version}"
        );
        assert_eq!(resp["result"]["contract_version"], version);
    }
}
