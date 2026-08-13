#![allow(clippy::unwrap_used, clippy::expect_used)]
//! WS-3 branch-scoped wire writes (contract 1.7.0): `author.open`'s `branch`
//! param, write routing (`materialize`, `set_standing`/`defeat`) to a target
//! branch instead of always `main`, `author.merge`'s `dry_run`/`expected_ref`,
//! and `author.branch`'s `delete` action — proven end to end through the real
//! `respond` wire path (mirrors `ls6_serve.rs`'s harness; LS-6 already proves
//! the merge conflict engine itself exhaustively at the lorestore layer).
//!
//! Priority: branch isolation/leakage is the failure mode this file exists to
//! catch (the campaign brief names it explicitly) — most tests here assert a
//! NEGATIVE ("main never moved", "the sibling never saw it") alongside the
//! positive ("the branch itself did").

use std::path::Path;
use std::sync::Arc;

use dli_lorestore::{ContentHash, FactObject};
use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_world::{KilledTokens, Pack};
use serde_json::{Value, json};

/// The window this whole file negotiates at — every branch-write field is
/// gated ≥ 1.7.0.
const V: &str = "1.7.0";
/// A pre-1.7.0 accepted version, for the "new params are refused below the
/// floor" cases.
const V_OLD: &str = "1.6.0";

// ── harness — mirrors ls6_serve.rs's, duplicated per this crate's per-file
//    integration-test convention (no shared `tests/common` module here) ─────

fn drive(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let response = respond(&frame.to_string(), sessions, store).expect("a response");
    serde_json::to_value(&response).expect("serialize")
}

#[allow(dead_code)]
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

/// A store over a bespoke functional-predicate base, with a lorestore
/// attached — mirrors `ls6_serve.rs`'s `functional_store`: `mara`'s `home` is
/// DECIDED (certain) under `d-1`, so it is exactly the shape the certain-
/// conflict and functional-dependency tests need.
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

fn open_workspace(scope: &str, sessions: &mut Sessions, store: &mut Store) -> String {
    open_workspace_at(V, scope, sessions, store)
}

fn open_workspace_at(dli: &str, scope: &str, sessions: &mut Sessions, store: &mut Store) -> String {
    let resp = drive(
        &json!({ "dli": dli, "id": "open", "op": "author.open", "params": { "scope": scope } }),
        sessions,
        store,
    );
    assert!(resp["ok"].as_bool().unwrap(), "open ok: {resp}");
    resp["result"]["workspace"].as_str().unwrap().to_owned()
}

/// Open a workspace against a named branch (WS-3, ≥ 1.7.0).
fn open_branch_workspace(
    scope: &str,
    branch: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) -> Value {
    drive(
        &json!({ "dli": V, "id": "open-branch", "op": "author.open",
                 "params": { "scope": scope, "branch": branch } }),
        sessions,
        store,
    )
}

fn create_branch(
    ws: &str,
    name: &str,
    from: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) -> Value {
    let resp = drive(
        &json!({ "dli": V, "id": "create", "op": "author.branch",
                 "params": { "workspace": ws, "action": "create", "name": name, "from": from } }),
        sessions,
        store,
    );
    assert!(resp["ok"].as_bool().unwrap(), "branch create ok: {resp}");
    resp
}

fn add_proposed_relation(
    ws: &str,
    relation: &str,
    from_arg: &str,
    to_arg: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) -> Value {
    drive(
        &json!({ "dli": V, "id": "add", "op": "author.add_relation",
                 "params": { "workspace": ws, "relation": relation, "from": from_arg, "to": to_arg,
                             "provenance": { "kind": "proposed", "origin": "demo" } } }),
        sessions,
        store,
    )
}

fn materialize(ws: &str, sessions: &mut Sessions, store: &mut Store) -> Value {
    drive(
        &json!({ "dli": V, "id": "mat", "op": "author.materialize", "params": { "workspace": ws } }),
        sessions,
        store,
    )
}

fn status(ws: &str, sessions: &mut Sessions, store: &mut Store) -> Value {
    drive(
        &json!({ "dli": V, "id": "status", "op": "author.status", "params": { "workspace": ws } }),
        sessions,
        store,
    )
}

/// Add and materialize a place entity on `ws` — call this on the seed/main
/// workspace BEFORE branching, so every branch created `from: "main"`
/// inherits it as known, avoiding `entity.unknown` on the new relations these
/// tests go on to stage (the base only declares `mara` via its one `home`
/// fact; nothing these tests newly reference is known otherwise).
fn seed_place(ws: &str, id: &str, sessions: &mut Sessions, store: &mut Store) {
    let added = drive(
        &json!({ "dli": V, "id": "seed-entity", "op": "author.add_entity",
                 "params": { "workspace": ws, "kind": "place", "id": id, "name": id,
                             "provenance": { "kind": "proposed", "origin": "demo" } } }),
        sessions,
        store,
    );
    assert!(added["ok"].as_bool().unwrap(), "seed place {id}: {added}");
    let mat = materialize(ws, sessions, store);
    assert!(
        mat["ok"].as_bool().unwrap(),
        "materialize seed place {id}: {mat}"
    );
}

/// The whole-ref digest of a named branch, read directly off the lorestore —
/// a legitimate test affordance (mirrors `ls6_serve.rs`'s direct lorestore
/// access for seeding), used here only to observe state, never to author it.
fn branch_ref_digest(store: &Store, ws_for_lorestore: &str, branch: &str) -> String {
    let lorestore = store
        .workspace_lorestore(ws_for_lorestore)
        .expect("lorestore attached");
    lorestore
        .branch_ref(branch)
        .expect("read ref")
        .expect("ref present")
        .digest()
        .expect("digest")
        .0
}

/// Find a stored fact's `claim_id` by its ground atom — a legitimate test
/// affordance for exercising `author.defeat` against a fact the base itself
/// asserted (mirrors `ls6_serve.rs`'s direct-lorestore fact construction).
fn claim_id_for(
    lorestore: &dli_lorestore::Lorestore,
    head: &ContentHash,
    pred: &str,
    args: &[&str],
) -> String {
    let want: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    for hash in lorestore.fact_hashes_from(head).expect("fact hashes") {
        let obj: FactObject = lorestore
            .read_object(&hash)
            .expect("read object")
            .expect("object present");
        if obj.atom.pred == pred && obj.atom.args == want {
            return obj.claim_id.0.clone();
        }
    }
    panic!("no fact object found for {pred}({args:?})");
}

// ── 1. branch reads see the branch head, not main ───────────────────────────

#[test]
fn branch_workspace_reads_see_branch_head_not_main() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "highlands", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    assert!(opened["ok"].as_bool().unwrap(), "{opened}");
    let branch_ws = opened["result"]["workspace"].as_str().unwrap().to_owned();

    add_proposed_relation(
        &branch_ws,
        "likes",
        "mara",
        "highlands",
        &mut sessions,
        &mut store,
    );
    let mat = materialize(&branch_ws, &mut sessions, &mut store);
    assert!(mat["ok"].as_bool().unwrap(), "{mat}");
    let rival_head = mat["result"]["root_hash"].as_str().unwrap().to_owned();

    // A FRESH workspace opened against "rival" pins that new (diverged) head.
    let reopened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let reopened_ws = reopened["result"]["workspace"].as_str().unwrap().to_owned();
    let branch_status = status(&reopened_ws, &mut sessions, &mut store);
    assert_eq!(
        branch_status["result"]["root_hash"], rival_head,
        "the branch workspace pins the branch's own (diverged) head: {branch_status}"
    );

    // A fresh main-scope (branchless) workspace pins the ORIGINAL, undiverged
    // main head — not rival's.
    let main_ws = open_workspace("canon", &mut sessions, &mut store);
    let main_status = status(&main_ws, &mut sessions, &mut store);
    assert_ne!(
        main_status["result"]["root_hash"], rival_head,
        "main's head is untouched by the branch's own write: {main_status}"
    );
}

// ── 2. writes to a branch are invisible to base and siblings ────────────────

#[test]
fn writes_to_branch_invisible_to_base_and_siblings() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "highlands", &mut sessions, &mut store);
    let before = create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);
    let main_head_before = before["result"]["heads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["name"] == "main")
        .unwrap()["commit"]
        .clone();

    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let branch_ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
    add_proposed_relation(
        &branch_ws,
        "likes",
        "mara",
        "highlands",
        &mut sessions,
        &mut store,
    );
    let mat = materialize(&branch_ws, &mut sessions, &mut store);
    assert!(mat["ok"].as_bool().unwrap(), "{mat}");

    // A sibling workspace opened on plain "canon" (main) never staged
    // anything on the branch and does not see its write.
    let sibling_ws = open_workspace("canon", &mut sessions, &mut store);
    let sibling_status = status(&sibling_ws, &mut sessions, &mut store);
    assert_eq!(
        sibling_status["result"]["root_hash"], main_head_before,
        "a main-scoped sibling still pins main's untouched head: {sibling_status}"
    );

    let listed = drive(
        &json!({ "dli": V, "id": "list", "op": "author.branch",
                 "params": { "workspace": seed_ws, "action": "list" } }),
        &mut sessions,
        &mut store,
    );
    let heads_after = listed["result"]["heads"].as_array().unwrap();
    let main_head_after =
        heads_after.iter().find(|h| h["name"] == "main").unwrap()["commit"].clone();
    let rival_head_after =
        heads_after.iter().find(|h| h["name"] == "rival").unwrap()["commit"].clone();

    assert_eq!(
        main_head_after, main_head_before,
        "main's ref never moved from the branch's own write"
    );
    assert_ne!(
        rival_head_after, main_head_before,
        "rival's ref DID advance"
    );
}

// ── 3. conflicting values evaluate correctly, independently, per branch ─────

#[test]
fn conflicting_values_evaluate_correctly_in_their_own_branches() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "highlands", &mut sessions, &mut store);
    seed_place(&seed_ws, "lowlands", &mut sessions, &mut store);
    create_branch(&seed_ws, "branch-a", "main", &mut sessions, &mut store);
    create_branch(&seed_ws, "branch-b", "main", &mut sessions, &mut store);

    // `likes` carries no canon value at all (only `home` is base-decided), so
    // each branch's own proposed ruling for it is uncontradicted locally,
    // however much the two branches' rulings would disagree if ever brought
    // together.
    for (branch, dest) in [("branch-a", "highlands"), ("branch-b", "lowlands")] {
        let opened = open_branch_workspace("canon", branch, &mut sessions, &mut store);
        let ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
        let added = add_proposed_relation(&ws, "likes", "mara", dest, &mut sessions, &mut store);
        assert!(added["ok"].as_bool().unwrap(), "{branch}: {added}");
        let mat = materialize(&ws, &mut sessions, &mut store);
        assert!(
            mat["ok"].as_bool().unwrap(),
            "{branch} materializes its own ruling for kes cleanly: {mat}"
        );
    }

    // Re-opening each branch fresh and checking it is clean proves the two
    // branches' rulings never cross-contaminated each other.
    for branch in ["branch-a", "branch-b"] {
        let reopened = open_branch_workspace("canon", branch, &mut sessions, &mut store);
        let ws = reopened["result"]["workspace"].as_str().unwrap().to_owned();
        let checked = drive(
            &json!({ "dli": V, "id": "check", "op": "author.check", "params": { "workspace": ws } }),
            &mut sessions,
            &mut store,
        );
        assert!(checked["ok"].as_bool().unwrap(), "{checked}");
        assert_eq!(
            checked["result"]["ok"], true,
            "{branch} evaluates its own ruling with no contradiction: {checked}"
        );
    }
}

// ── 4. the certain-class conflict, constructed wire-only ────────────────────

/// WS-3's named resolution to "the authoring seam never mints certain content": on a
/// branch, `author.defeat` the base's certain value citing a NEW decision,
/// assert the rival value cited to it, and materialize — the old value is no
/// longer certain THERE, so the branch stays internally consistent (unlike
/// `ls6_serve::proposed_contradiction_is_still_blocked_upstream`'s bare
/// `proposed` add, which a single branch's own continuity guard still
/// refuses).
///
/// Two independent branches run the identical recipe — same defeated
/// decision (`d-1`), same NAME for the new citing decision (`d-2`) — each
/// asserting its OWN rival value. Two branches minting *different-named*
/// decisions independently from the same point cannot later merge with each
/// other at all (`reconcile_interners`' prefix-preservation guard refuses
/// colliding positional atoms — DL-0041 §LS-6, an unrelated, real hazard);
/// reusing the SAME name sidesteps that while keeping the two branches'
/// certain values genuinely independent (each cites `d-2` for a DIFFERENT
/// object — `mara`'s home — so `d-2` being shared is not itself a conflict,
/// it is what lets both rulings default true together once merged). Merging
/// them DIRECTLY with each other (not via `main`) keeps the merge base the
/// ORIGINAL single-value commit — the shape the merge's functional-dependency
/// inference needs to recognize `home` as functional at all
/// (`certain_layer_conflicts`, `dli_core::fd::inferred_functional`): merging
/// through an intermediate that already carries one branch's new value would
/// make the merge base itself look already-multi-valued, silencing the
/// inference.
#[test]
fn certain_conflict_constructible_wire_only() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "highlands", &mut sessions, &mut store);
    seed_place(&seed_ws, "lowlands", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);
    create_branch(&seed_ws, "counter", "main", &mut sessions, &mut store);

    let claim_id = {
        let lorestore = store
            .workspace_lorestore(&seed_ws)
            .expect("lorestore attached");
        let head = lorestore
            .root_hash()
            .expect("root hash")
            .expect("head present");
        claim_id_for(lorestore, &head, "home", &["mara", "shallows"])
    };

    // Each branch defeats d-1 (citing the SAME new decision name, "d-2") and
    // asserts its own rival value, materializing cleanly on the branch.
    let apply_defeat_and_rival_value = |branch: &str,
                                        dest: &str,
                                        sessions: &mut Sessions,
                                        store: &mut Store| {
        let opened_1 = open_branch_workspace("canon", branch, sessions, store);
        let ws_1 = opened_1["result"]["workspace"].as_str().unwrap().to_owned();
        let defeat = drive(
            &json!({ "dli": V, "id": "defeat", "op": "author.defeat",
                         "params": { "workspace": ws_1, "claim_id": claim_id.clone(),
                                     "decision": "d-2", "defeated_decision": "d-1" } }),
            sessions,
            store,
        );
        assert!(defeat["ok"].as_bool().unwrap(), "{branch}: {defeat}");

        // Re-open: the reconstructed branch head now carries the
        // `Decl::Ratify(d-2)` the defeat minted, so a FRESH workspace can
        // cite it (a workspace's own `workspace_decisions` reads its base
        // docs at open time — it does not re-poll the lorestore
        // mid-session).
        let opened_2 = open_branch_workspace("canon", branch, sessions, store);
        let ws_2 = opened_2["result"]["workspace"].as_str().unwrap().to_owned();
        let added = drive(
            &json!({ "dli": V, "id": "add", "op": "author.add_relation",
                         "params": { "workspace": ws_2, "relation": "home", "from": "mara", "to": dest,
                                     "provenance": { "kind": "cited", "decisions": ["d-2"], "state": "ratified" } } }),
            sessions,
            store,
        );
        assert!(
            added["ok"].as_bool().unwrap(),
            "{branch}: citing the freshly-minted d-2: {added}"
        );

        let mat = materialize(&ws_2, sessions, store);
        assert!(
            mat["ok"].as_bool().unwrap(),
            "{branch} materializes cleanly — the old value is no longer certain there: {mat}"
        );
    };

    apply_defeat_and_rival_value("rival", "highlands", &mut sessions, &mut store);
    apply_defeat_and_rival_value("counter", "lowlands", &mut sessions, &mut store);

    // Merge "counter" directly into "rival" — NOT through "main" — so the
    // merge base stays the ORIGINAL, single-value commit.
    let merged = drive(
        &json!({ "dli": V, "id": "merge", "op": "author.merge",
                 "params": { "workspace": seed_ws, "source": "counter", "target": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert!(merged["ok"].as_bool().unwrap(), "{merged}");
    let conflicts = merged["result"]["conflicts"].as_array().unwrap();
    assert!(
        conflicts.iter().any(|c| c["kind"] == "certain"),
        "a certain-layer conflict, constructed entirely wire-only: {merged}"
    );
    assert!(
        merged["result"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "merge.functional-dependency"),
        "{merged}"
    );
}

// ── 5. `proposed_contradiction_is_still_blocked_upstream` lives in
//    ls6_serve.rs (the renamed, rewritten supersession of :232) — not
//    duplicated here.

// ── 6. materialize on a branch is fenced by the branch's own ref digest ─────

#[test]
fn materialize_on_branch_fenced_by_ref_digest() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "highlands", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
    add_proposed_relation(&ws, "likes", "mara", "highlands", &mut sessions, &mut store);

    // Externally advance "rival" behind the workspace's back — a no-op
    // content-wise (same interner/decisions/pack), just to move the ref.
    {
        let lorestore = store.workspace_lorestore(&ws).expect("lorestore attached");
        let head = lorestore
            .branch_head("rival")
            .expect("head")
            .expect("present");
        let commit: dli_lorestore::Commit = lorestore
            .read_object(&head)
            .expect("read")
            .expect("present");
        lorestore
            .commit_on_branch(
                "rival",
                commit.interner.clone(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                commit.decisions.clone(),
                "canon".to_owned(),
            )
            .expect("external commit advances rival");
    }

    let mat = materialize(&ws, &mut sessions, &mut store);
    assert_eq!(
        mat["error"]["code"], "stale-fence",
        "an external write on the branch fences the workspace's own materialize: {mat}"
    );
}

// ── 7. set_standing/defeat route to the workspace's branch; main untouched ──

#[test]
fn set_standing_and_defeat_route_to_workspace_branch() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    let main_ref_before = branch_ref_digest(&store, &seed_ws, "main");
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let claim_id = {
        let lorestore = store
            .workspace_lorestore(&seed_ws)
            .expect("lorestore attached");
        let head = lorestore
            .root_hash()
            .expect("root hash")
            .expect("head present");
        claim_id_for(lorestore, &head, "home", &["mara", "shallows"])
    };

    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
    let defeat = drive(
        &json!({ "dli": V, "id": "defeat", "op": "author.defeat",
                 "params": { "workspace": ws, "claim_id": claim_id,
                             "decision": "d-2", "defeated_decision": "d-1" } }),
        &mut sessions,
        &mut store,
    );
    assert!(defeat["ok"].as_bool().unwrap(), "{defeat}");

    let main_ref_after = branch_ref_digest(&store, &seed_ws, "main");
    assert_eq!(
        main_ref_before, main_ref_after,
        "defeat on a branch workspace never touches main's ref"
    );

    let rival_head = {
        let lorestore = store
            .workspace_lorestore(&seed_ws)
            .expect("lorestore attached");
        lorestore
            .branch_ref("rival")
            .expect("ref")
            .expect("present")
            .commit
            .to_string()
    };
    assert_eq!(
        defeat["result"]["root_hash"], rival_head,
        "the defeat committed to rival, not main: {defeat}"
    );
}

// ── 8. branch delete leaves no residue ───────────────────────────────────────

#[test]
fn branch_delete_leaves_no_residue() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let deleted = drive(
        &json!({ "dli": V, "id": "del", "op": "author.branch",
                 "params": { "workspace": seed_ws, "action": "delete", "name": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert!(deleted["ok"].as_bool().unwrap(), "{deleted}");
    assert_eq!(
        deleted["result"]["branches"],
        json!(["main"]),
        "rival is no longer listed: {deleted}"
    );

    // A fresh open against the deleted branch now refuses — no ref left to
    // reconstruct from.
    let reopen = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    assert!(!reopen["ok"].as_bool().unwrap(), "{reopen}");
    assert_eq!(reopen["error"]["code"], "unknown-doc", "{reopen}");

    // Re-creating a branch under the SAME name works cleanly — no leftover
    // ref or object collides with the fresh one.
    let recreated = drive(
        &json!({ "dli": V, "id": "recreate", "op": "author.branch",
                 "params": { "workspace": seed_ws, "action": "create", "name": "rival", "from": "main" } }),
        &mut sessions,
        &mut store,
    );
    assert!(recreated["ok"].as_bool().unwrap(), "{recreated}");
}

// ── 9. branch delete refuses main, and refuses while a workspace is open ────

#[test]
fn branch_delete_refuses_main_and_open_workspaces() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);

    let del_main = drive(
        &json!({ "dli": V, "id": "del-main", "op": "author.branch",
                 "params": { "workspace": seed_ws, "action": "delete", "name": "main" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(del_main["error"]["code"], "schema-violation", "{del_main}");

    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);
    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    assert!(opened["ok"].as_bool().unwrap(), "{opened}");

    let del_targeted = drive(
        &json!({ "dli": V, "id": "del-open", "op": "author.branch",
                 "params": { "workspace": seed_ws, "action": "delete", "name": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        del_targeted["error"]["code"], "conflict",
        "refused while an open workspace targets it: {del_targeted}"
    );
}

// ── 10. merge dry_run commits nothing, and matches the later real merge ─────

#[test]
fn merge_dry_run_commits_nothing_and_matches_the_later_real_merge() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "highlands", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
    add_proposed_relation(&ws, "likes", "mara", "highlands", &mut sessions, &mut store);
    let mat = materialize(&ws, &mut sessions, &mut store);
    assert!(mat["ok"].as_bool().unwrap(), "{mat}");

    let main_ref_before = branch_ref_digest(&store, &seed_ws, "main");

    let dry = drive(
        &json!({ "dli": V, "id": "dry", "op": "author.merge",
                 "params": { "workspace": seed_ws, "source": "rival", "dry_run": true } }),
        &mut sessions,
        &mut store,
    );
    assert!(dry["ok"].as_bool().unwrap(), "{dry}");
    let dry_commit = dry["result"]["merge_commit"].clone();

    let main_ref_after_dry = branch_ref_digest(&store, &seed_ws, "main");
    assert_eq!(
        main_ref_before, main_ref_after_dry,
        "the dry run advances nothing"
    );

    let real = drive(
        &json!({ "dli": V, "id": "real", "op": "author.merge",
                 "params": { "workspace": seed_ws, "source": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert!(real["ok"].as_bool().unwrap(), "{real}");
    assert_eq!(
        real["result"]["merge_commit"], dry_commit,
        "the real merge writes the SAME content-addressed commit the dry run reported: {real}"
    );

    let main_ref_after_real = branch_ref_digest(&store, &seed_ws, "main");
    assert_ne!(
        main_ref_before, main_ref_after_real,
        "the real merge DOES advance main"
    );
}

// ── 11. merge's expected_ref fences a stale target snapshot ─────────────────

#[test]
fn merge_expected_ref_stale_fence() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let merged = drive(
        &json!({ "dli": V, "id": "merge", "op": "author.merge",
                 "params": { "workspace": seed_ws, "source": "rival",
                             "expected_ref": "sha256:0000000000000000000000000000000000000000000000000000000000000000" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(merged["error"]["code"], "stale-fence", "{merged}");
}

// ── 12. every new param is a schema violation below 1.7.0 ───────────────────

#[test]
fn new_params_below_1_7_0_are_schema_violations() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let open_branch = drive(
        &json!({ "dli": V_OLD, "id": "1", "op": "author.open",
                 "params": { "scope": "canon", "branch": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        open_branch["error"]["code"], "schema-violation",
        "{open_branch}"
    );

    let merge_dry = drive(
        &json!({ "dli": V_OLD, "id": "2", "op": "author.merge",
                 "params": { "workspace": seed_ws, "source": "rival", "dry_run": true } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        merge_dry["error"]["code"], "schema-violation",
        "{merge_dry}"
    );

    let merge_ref = drive(
        &json!({ "dli": V_OLD, "id": "3", "op": "author.merge",
                 "params": { "workspace": seed_ws, "source": "rival", "expected_ref": "x" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        merge_ref["error"]["code"], "schema-violation",
        "{merge_ref}"
    );

    let branch_ref_param = drive(
        &json!({ "dli": V_OLD, "id": "4", "op": "author.branch",
                 "params": { "workspace": seed_ws, "action": "list", "expected_ref": "x" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        branch_ref_param["error"]["code"], "schema-violation",
        "{branch_ref_param}"
    );

    let branch_delete = drive(
        &json!({ "dli": V_OLD, "id": "5", "op": "author.branch",
                 "params": { "workspace": seed_ws, "action": "delete", "name": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(
        branch_delete["error"]["code"], "schema-violation",
        "{branch_delete}"
    );
}

// ── 13. idempotent merge replay ──────────────────────────────────────────────

#[test]
fn idempotent_merge_replay() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let first = drive(
        &json!({ "dli": V, "id": "1", "op": "author.merge",
                 "idempotency_key": "merge-once",
                 "params": { "workspace": seed_ws, "source": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert!(first["ok"].as_bool().unwrap(), "{first}");
    let first_commit = first["result"]["merge_commit"].clone();
    let main_ref_after_first = branch_ref_digest(&store, &seed_ws, "main");

    // A retry with the SAME key and payload replays the cached result — no
    // second merge, no second ref advance.
    let retry = drive(
        &json!({ "dli": V, "id": "1", "op": "author.merge",
                 "idempotency_key": "merge-once",
                 "params": { "workspace": seed_ws, "source": "rival" } }),
        &mut sessions,
        &mut store,
    );
    assert!(retry["ok"].as_bool().unwrap(), "{retry}");
    assert_eq!(
        retry["result"]["merge_commit"], first_commit,
        "the retry replays the cached result: {retry}"
    );
    let main_ref_after_retry = branch_ref_digest(&store, &seed_ws, "main");
    assert_eq!(
        main_ref_after_first, main_ref_after_retry,
        "the retry staged/committed nothing new"
    );

    // The same key with a DIFFERENT payload is a conflict, never a silent
    // replay (R3/M9 — the general idempotency-key discipline, now reachable
    // for branch/merge since WS-3 moved them onto the `mutate` wrapper).
    let conflicting = drive(
        &json!({ "dli": V, "id": "1", "op": "author.merge",
                 "idempotency_key": "merge-once",
                 "params": { "workspace": seed_ws, "source": "rival", "dry_run": true } }),
        &mut sessions,
        &mut store,
    );
    assert_eq!(conflicting["error"]["code"], "conflict", "{conflicting}");
}

// ── 14. workspace digest: target_branch moves it; branchless stays identical ─

#[test]
fn workspace_digest_reflects_target_branch_and_stays_identical_when_branchless() {
    let (_t, mut sessions, mut store) = functional_store();

    let plain_a = open_workspace("canon", &mut sessions, &mut store);
    let plain_b = open_workspace("canon", &mut sessions, &mut store);
    let digest_a =
        status(&plain_a, &mut sessions, &mut store)["result"]["workspace_digest"].clone();
    let digest_b =
        status(&plain_b, &mut sessions, &mut store)["result"]["workspace_digest"].clone();
    assert_eq!(
        digest_a, digest_b,
        "two branchless opens of the same scope digest identically — the pre-1.7.0 shape"
    );

    create_branch(&plain_a, "rival", "main", &mut sessions, &mut store);
    let branched = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let branched_ws = branched["result"]["workspace"].as_str().unwrap().to_owned();
    let digest_branched =
        status(&branched_ws, &mut sessions, &mut store)["result"]["workspace_digest"].clone();
    assert_ne!(
        digest_branched, digest_a,
        "a branch-mode open's digest differs from a branchless one — target_branch moves the fence"
    );
}

// ── 15. discard clears the branch-base cache ─────────────────────────────────

/// Black-box proxy for the internal `Store::branch_bases` cache-clearing
/// property: after discard, a fresh `author.open` against the SAME branch
/// must reconstruct from the branch's CURRENT head, never reuse anything left
/// behind by the discarded workspace's own compile.
#[test]
fn discard_clears_branch_base_cache() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "highlands", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
    add_proposed_relation(&ws, "likes", "mara", "highlands", &mut sessions, &mut store);
    let mat = materialize(&ws, &mut sessions, &mut store);
    assert!(mat["ok"].as_bool().unwrap(), "{mat}");
    let advanced_head = mat["result"]["root_hash"].clone();

    let discarded = drive(
        &json!({ "dli": V, "id": "discard", "op": "author.discard",
                 "params": { "workspace": ws } }),
        &mut sessions,
        &mut store,
    );
    assert!(discarded["ok"].as_bool().unwrap(), "{discarded}");

    let reopened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let reopened_ws = reopened["result"]["workspace"].as_str().unwrap().to_owned();
    let reopened_status = status(&reopened_ws, &mut sessions, &mut store);
    assert_eq!(
        reopened_status["result"]["root_hash"], advanced_head,
        "reopen after discard sees the advanced head, not a stale cache: {reopened_status}"
    );
}

// ── 16/17. regression: standing changes read their pre-image from the
//    WORKSPACE's own branch, not always `main` (red-team SEV-A) ────────────

/// Add a fact cited to a KNOWN decision, with the given standing state,
/// returning the response (so the caller can pull the claim's identity out of
/// the branch's own store afterward).
#[allow(clippy::too_many_arguments)]
fn add_cited_relation(
    ws: &str,
    relation: &str,
    from_arg: &str,
    to_arg: &str,
    decision: &str,
    state: &str,
    sessions: &mut Sessions,
    store: &mut Store,
) -> Value {
    drive(
        &json!({ "dli": V, "id": "add-cited", "op": "author.add_relation",
                 "params": { "workspace": ws, "relation": relation, "from": from_arg, "to": to_arg,
                             "provenance": { "kind": "cited", "decisions": [decision], "state": state } } }),
        sessions,
        store,
    )
}

/// `author.defeat` and `author.set_standing` must read their PRE-IMAGE (the
/// claim lookup, the known-decisions set, the interner) from the WORKSPACE's
/// own target branch — not unconditionally from `main`, which is what
/// `Lorestore::root_hash()` always returns. A claim materialized ONLY on a
/// branch (never on main) must still be defeatable/promotable from a
/// workspace opened against that branch: before the fix, the pre-image read
/// walked main's fact set, found nothing with this `claim_id`, and answered
/// `unknown-doc` — the claim was invisible to its own branch's standing ops.
#[test]
fn defeat_and_set_standing_see_a_branch_authored_claim_main_never_had() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "festivals", &mut sessions, &mut store);
    seed_place(&seed_ws, "spring", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws = opened["result"]["workspace"].as_str().unwrap().to_owned();

    // Two branch-only claims, materialized only on "rival": one ratified (for
    // `defeat`), one provisional (for `set_standing`). Both cite the BASE
    // decision `d-1`, which is known everywhere, so this isolates the
    // pre-image bug from the separate branch-only-decision case below.
    let ratified = add_cited_relation(
        &ws,
        "likes",
        "mara",
        "festivals",
        "d-1",
        "ratified",
        &mut sessions,
        &mut store,
    );
    assert!(ratified["ok"].as_bool().unwrap(), "{ratified}");
    let provisional = add_cited_relation(
        &ws,
        "likes",
        "mara",
        "spring",
        "d-1",
        "provisional",
        &mut sessions,
        &mut store,
    );
    assert!(provisional["ok"].as_bool().unwrap(), "{provisional}");
    let mat = materialize(&ws, &mut sessions, &mut store);
    assert!(mat["ok"].as_bool().unwrap(), "{mat}");

    let (ratified_claim, provisional_claim) = {
        let lorestore = store.workspace_lorestore(&ws).expect("lorestore attached");
        let branch_head = lorestore
            .branch_ref("rival")
            .expect("read ref")
            .expect("ref present")
            .commit;
        (
            claim_id_for(lorestore, &branch_head, "likes", &["mara", "festivals"]),
            claim_id_for(lorestore, &branch_head, "likes", &["mara", "spring"]),
        )
    };

    // This claim exists ONLY on "rival" — confirm main truly never had it, so
    // a pass here cannot be an accident of main==branch (unlike the existing
    // `set_standing_and_defeat_route_to_workspace_branch`, which only ever
    // touches a BASE claim on a non-diverged branch).
    {
        let lorestore = store
            .workspace_lorestore(&seed_ws)
            .expect("lorestore attached");
        let main_head = lorestore
            .root_hash()
            .expect("root hash")
            .expect("head present");
        let main_hashes = lorestore.fact_hashes_from(&main_head).expect("hashes");
        let on_main = main_hashes.iter().any(|hash| {
            lorestore
                .read_object::<FactObject>(hash)
                .expect("read")
                .is_some_and(|obj| obj.claim_id.0 == ratified_claim)
        });
        assert!(
            !on_main,
            "the branch-only claim is genuinely absent from main"
        );
    }

    let defeated = drive(
        &json!({ "dli": V, "id": "defeat", "op": "author.defeat",
                 "params": { "workspace": ws, "claim_id": ratified_claim,
                             "decision": "d-5", "defeated_decision": "d-1" } }),
        &mut sessions,
        &mut store,
    );
    assert!(
        defeated["ok"].as_bool().unwrap(),
        "defeat sees a branch-only claim from its own branch workspace: {defeated}"
    );

    let promoted = drive(
        &json!({ "dli": V, "id": "promote", "op": "author.set_standing",
                 "params": { "workspace": ws, "claim_id": provisional_claim, "decision": "d-1" } }),
        &mut sessions,
        &mut store,
    );
    assert!(
        promoted["ok"].as_bool().unwrap(),
        "set_standing sees a branch-only claim from its own branch workspace: {promoted}"
    );
}

/// The known-decisions set a standing change validates `decision`/
/// `defeated_decision` against must also come from the branch's own
/// pre-image: a decision minted ONLY on a branch (via an earlier `defeat` on
/// that same branch) must be citable there, not answered
/// `citation.unknown-decision` because the check consulted main's decision
/// list instead.
#[test]
fn set_standing_can_cite_a_decision_that_only_exists_on_the_branch() {
    let (_t, mut sessions, mut store) = functional_store();
    let seed_ws = open_workspace("canon", &mut sessions, &mut store);
    seed_place(&seed_ws, "autumn", &mut sessions, &mut store);
    create_branch(&seed_ws, "rival", "main", &mut sessions, &mut store);

    let claim_id = {
        let lorestore = store
            .workspace_lorestore(&seed_ws)
            .expect("lorestore attached");
        let head = lorestore
            .root_hash()
            .expect("root hash")
            .expect("head present");
        claim_id_for(lorestore, &head, "home", &["mara", "shallows"])
    };

    let opened_1 = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws_1 = opened_1["result"]["workspace"].as_str().unwrap().to_owned();
    let defeat = drive(
        &json!({ "dli": V, "id": "defeat", "op": "author.defeat",
                 "params": { "workspace": ws_1, "claim_id": claim_id,
                             "decision": "d-2", "defeated_decision": "d-1" } }),
        &mut sessions,
        &mut store,
    );
    assert!(defeat["ok"].as_bool().unwrap(), "{defeat}");

    // Re-open: the branch head now carries `Decl::Ratify(d-2)`, so a fresh
    // workspace can cite it (same "workspace_decisions reads base docs at
    // open time" rule the certain-conflict test above relies on).
    let opened_2 = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws_2 = opened_2["result"]["workspace"].as_str().unwrap().to_owned();
    let added = add_cited_relation(
        &ws_2,
        "likes",
        "mara",
        "autumn",
        "d-2",
        "provisional",
        &mut sessions,
        &mut store,
    );
    assert!(
        added["ok"].as_bool().unwrap(),
        "citing the branch-only d-2: {added}"
    );
    let mat = materialize(&ws_2, &mut sessions, &mut store);
    assert!(mat["ok"].as_bool().unwrap(), "{mat}");

    let provisional_claim = {
        let lorestore = store
            .workspace_lorestore(&ws_2)
            .expect("lorestore attached");
        let branch_head = lorestore
            .branch_ref("rival")
            .expect("read ref")
            .expect("ref present")
            .commit;
        claim_id_for(lorestore, &branch_head, "likes", &["mara", "autumn"])
    };

    let opened_3 = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let ws_3 = opened_3["result"]["workspace"].as_str().unwrap().to_owned();
    let promoted = drive(
        &json!({ "dli": V, "id": "promote", "op": "author.set_standing",
                 "params": { "workspace": ws_3, "claim_id": provisional_claim, "decision": "d-2" } }),
        &mut sessions,
        &mut store,
    );
    assert!(
        promoted["ok"].as_bool().unwrap(),
        "set_standing cites the branch-only decision d-2, not citation.unknown-decision: {promoted}"
    );
}

// ── inertia (DL-0081, 1.8.0) is branch-local ────────────────────────────────

/// The contract window the inertia op negotiates at (≥ 1.8.0).
const V8: &str = "1.8.0";

/// A base whose two open `status` values clash WITHOUT an inertial declaration
/// (`commons since dawn`, `enclosed since noon`, dawn before noon): on `main`
/// they certainly co-hold at noon, so `continuity.fluent-contradiction` fires.
/// Declaring `status` inertial on a branch silences it — and that silencing
/// must stay branch-local.
fn inertia_branch_store() -> (tempfile::TempDir, Sessions, Store) {
    let source = concat!(
        "scope canon\n\n",
        "ratify d-1\n",
        "person river \"River\" decided d-1\n",
        "event dawn decided d-1\n",
        "event noon decided d-1\n",
        "before dawn -> noon decided d-1\n",
        "state river.status = commons since dawn decided d-1\n",
        "state river.status = enclosed since noon decided d-1\n",
    );
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

/// The `author.check` finding codes on a workspace, at contract 1.8.0.
fn check_codes(ws: &str, sessions: &mut Sessions, store: &mut Store) -> Vec<String> {
    let resp = drive(
        &json!({ "dli": V8, "id": "ck", "op": "author.check", "params": { "workspace": ws } }),
        sessions,
        store,
    );
    resp["result"]["findings"]
        .as_array()
        .map(|f| {
            f.iter()
                .filter_map(|x| x["code"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Inertia declared and materialized on a branch is invisible to `main` and to
/// a sibling branch: the branch silences the base clash, but a fresh `main`
/// (and a sibling) still sees the un-capped `continuity.fluent-contradiction`.
#[test]
fn inertial_declaration_is_branch_local() {
    let (_t, mut sessions, mut store) = inertia_branch_store();

    // Baseline: main itself sees the un-capped clash.
    let main_ws = open_workspace_at(V8, "canon", &mut sessions, &mut store);
    assert!(
        check_codes(&main_ws, &mut sessions, &mut store)
            .contains(&"continuity.fluent-contradiction".to_owned()),
        "main's two open values clash without an inertial declaration"
    );

    // Branch off main, declare `status` inertial on the branch, materialize.
    create_branch(&main_ws, "rival", "main", &mut sessions, &mut store);
    create_branch(&main_ws, "other", "main", &mut sessions, &mut store);
    let opened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let branch_ws = opened["result"]["workspace"].as_str().unwrap().to_owned();
    let declared = drive(
        &json!({ "dli": V8, "id": "in", "op": "author.declare_inertial",
                 "params": { "workspace": branch_ws, "pred": "status",
                             "provenance": { "kind": "cited", "decisions": ["d-1"], "state": "ratified" } } }),
        &mut sessions,
        &mut store,
    );
    assert!(
        declared["ok"].as_bool().unwrap(),
        "declare_inertial on branch: {declared}"
    );
    let mat = materialize(&branch_ws, &mut sessions, &mut store);
    assert!(
        mat["ok"].as_bool().unwrap(),
        "the branch silences its own clash and materializes: {mat}"
    );

    // The branch's durable head no longer clashes (capping active).
    let reopened = open_branch_workspace("canon", "rival", &mut sessions, &mut store);
    let rival_ws = reopened["result"]["workspace"].as_str().unwrap().to_owned();
    assert!(
        !check_codes(&rival_ws, &mut sessions, &mut store)
            .contains(&"continuity.fluent-contradiction".to_owned()),
        "the branch's inertial declaration caps the transition"
    );

    // A fresh main workspace and a sibling branch still see the clash — the
    // declaration never leaked off the branch it was made on (#68 lesson).
    let fresh_main = open_workspace_at(V8, "canon", &mut sessions, &mut store);
    assert!(
        check_codes(&fresh_main, &mut sessions, &mut store)
            .contains(&"continuity.fluent-contradiction".to_owned()),
        "main never saw the branch's inertial declaration"
    );
    let sibling = open_branch_workspace("canon", "other", &mut sessions, &mut store);
    let sibling_ws = sibling["result"]["workspace"].as_str().unwrap().to_owned();
    assert!(
        check_codes(&sibling_ws, &mut sessions, &mut store)
            .contains(&"continuity.fluent-contradiction".to_owned()),
        "a sibling branch never saw it either"
    );
}
