//! LS-5 oracle (contract 0.6.0): the query and review contract.
//!
//! Drives a seeded change-set through the wire —
//! `author.open → author.add_* → author.status/diff → author.materialize →
//! author.query` — then proves:
//!
//! 1. **Byte-identical replay**: loading the lorestore identified by the
//!    materialize `root_hash`, compiling it, and re-running the query yields
//!    rows that serialize byte-for-byte equal to the wire response rows, and
//!    a valuation equal to the response's full valuation entries.
//! 2. **Accept/reject**: `materialize` is the accept decision (the staged
//!    delta becomes a commit; a repeat is an idempotent no-op), and
//!    `author.discard` with `decl` is the per-declaration reject (the
//!    rejected claim never reaches the store).
//! 3. **The fence survives materialize** (audit A4): the returned
//!    `workspace_digest` stays in the fence-valid `jcs:sha256:` regime and a
//!    follow-up call fenced with it succeeds; the commit hash travels in its
//!    own `root_hash` field.
//! 4. **Review standing shows gates** (audit B3): a proposed fact's
//!    `diff_rows` standing carries its `h.` proposal-bundle variable — a
//!    gated claim never renders as unconditional.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;

fn crate_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn copy_tree(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).expect("read fixture project") {
        let entry = entry.expect("entry");
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            fs::create_dir(&to).expect("mkdir");
            copy_tree(&from, &to);
        } else {
            fs::copy(&from, &to).expect("copy");
        }
    }
}

fn send(frame: &Value, sessions: &mut Sessions, store: &mut Store) -> Value {
    let response = respond(&frame.to_string(), sessions, store).expect("frame answered");
    serde_json::to_value(&response).expect("response serializes")
}

fn ok_result(response: &Value) -> &Value {
    assert_eq!(
        response["ok"],
        Value::Bool(true),
        "call must succeed: {response}"
    );
    &response["result"]
}

#[test]
fn ls5_review_contract_replays_byte_identical_and_drives_accept_reject() {
    let temp = tempfile::tempdir().expect("isolated project");
    let root = temp.path();
    copy_tree(
        &crate_path().join("../../contracts/dli-worker/fixture-project"),
        root,
    );

    let project = Arc::new(Project::load(root).expect("fixture project loads"));
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let mut sessions = Sessions::from_project(Arc::clone(&project));
    let mut store = Store::with_packs(packs).expect("fixture project compiles");
    let lorestore_root = root.join(".dli-lorestore");
    store
        .attach_lorestore(lorestore_root.clone())
        .expect("attach imports the base packs");

    // --- open a 0.6.0 workspace over the storybook scope ---
    let open = send(
        &json!({"dli":"2.0.0","id":"open","op":"author.open","params":{"scope":"storybook"}}),
        &mut sessions,
        &mut store,
    );
    let ws = ok_result(&open)["workspace"]
        .as_str()
        .expect("ws id")
        .to_owned();

    // --- stage a proposed entity and fact ---
    for (id, op, params) in [
        (
            "e1",
            "author.add_entity",
            json!({"workspace": ws, "kind":"person","id":"nara","name":"Nara",
                   "provenance": {"kind":"proposed","origin":"ls5-review"}}),
        ),
        (
            "f1",
            "author.add_fact",
            json!({"workspace": ws, "entity":"nara","key":"home","value":"riverside",
                   "provenance": {"kind":"proposed","origin":"ls5-review"}}),
        ),
    ] {
        let response = send(
            &json!({"dli":"2.0.0","id":id,"op":op,"params":params}),
            &mut sessions,
            &mut store,
        );
        ok_result(&response);
    }

    // --- status (0.6.0): pinned base commit + gated standing (audit B3) ---
    let status = send(
        &json!({"dli":"2.0.0","id":"st","op":"author.status","params":{"workspace": ws}}),
        &mut sessions,
        &mut store,
    );
    let status = ok_result(&status);
    let pinned = status["root_hash"].as_str().expect("pinned base commit");
    assert!(
        pinned.starts_with("sha256:"),
        "the pinned base is a lorestore commit hash"
    );
    let diff_rows = status["diff_rows"].as_array().expect("structured rows");
    assert!(!diff_rows.is_empty(), "the staged change produces rows");
    let gated = diff_rows.iter().any(|row| {
        row["after"]["support"].as_array().is_some_and(|monomials| {
            monomials.iter().any(|m| {
                m.as_array().is_some_and(|lits| {
                    lits.iter()
                        .any(|l| l.as_str().is_some_and(|s| s.contains("+h.")))
                })
            })
        })
    });
    assert!(
        gated,
        "a proposed fact's standing must carry its h. gate, never render unconditional: {diff_rows:?}"
    );

    // --- materialize (0.6.0, JSON-first): the accept decision ---
    let materialize = send(
        &json!({"dli":"2.0.0","id":"mz","op":"author.materialize","params":{"workspace": ws}}),
        &mut sessions,
        &mut store,
    );
    let materialize = ok_result(&materialize).clone();
    let commit_hash = materialize["root_hash"]
        .as_str()
        .expect("commit hash")
        .to_owned();
    assert!(commit_hash.starts_with("sha256:"), "store-address regime");
    let workspace_digest = materialize["workspace_digest"]
        .as_str()
        .expect("fence digest")
        .to_owned();
    assert!(
        workspace_digest.starts_with("jcs:sha256:"),
        "the workspace digest stays in the fence-valid regime (audit A4)"
    );
    let facts = materialize["facts"].as_array().expect("JSON-first facts");
    assert!(
        !facts.is_empty(),
        "the committed fact objects are the return"
    );
    assert_eq!(
        materialize["rendered"].as_array().map(Vec::len),
        Some(0),
        "rendering is the opt-in on 0.6.0, not the default"
    );

    // --- the fence survives materialize (audit A4) ---
    let fenced = send(
        &json!({"dli":"2.0.0","id":"f2","op":"author.add_fact",
                "expected_digest": workspace_digest,
                "params":{"workspace": ws, "entity":"nara","key":"trade","value":"salt",
                          "provenance": {"kind":"proposed","origin":"ls5-review"}}}),
        &mut sessions,
        &mut store,
    );
    ok_result(&fenced);

    // --- repeat materialize with the new delta, then an idempotent no-op ---
    let second = send(
        &json!({"dli":"2.0.0","id":"mz2","op":"author.materialize","params":{"workspace": ws}}),
        &mut sessions,
        &mut store,
    );
    let second_hash = ok_result(&second)["root_hash"]
        .as_str()
        .expect("second commit")
        .to_owned();
    assert_ne!(second_hash, commit_hash, "a new delta advances the branch");
    let noop = send(
        &json!({"dli":"2.0.0","id":"mz3","op":"author.materialize","params":{"workspace": ws}}),
        &mut sessions,
        &mut store,
    );
    let noop = ok_result(&noop);
    assert_eq!(
        noop["root_hash"].as_str(),
        Some(second_hash.as_str()),
        "an empty delta re-materialize is an idempotent no-op (audit B9)"
    );
    assert_eq!(noop["facts"].as_array().map(Vec::len), Some(0));

    // --- query (0.6.0): the replay triple ---
    let query = send(
        &json!({"dli":"2.0.0","id":"q1","op":"author.query",
                "params":{"workspace": ws, "goal_atom":{"pred":"home","args":[{"Sym":"nara"},{"Var":"where"}]}}}),
        &mut sessions,
        &mut store,
    );
    let query = ok_result(&query).clone();
    assert!(
        query["root_hash"]
            .as_str()
            .is_some_and(|d| d.starts_with("jcs:sha256:")),
        "author.query anchors to the workspace digest — staged state, not a store address"
    );
    let response_valuation = query["valuation"]
        .as_array()
        .expect("full valuation")
        .clone();
    let mut response_rows = query["rows"].as_array().expect("rows").clone();
    assert!(
        !response_rows.is_empty(),
        "the staged fact answers the goal"
    );

    // --- REPLAY: load the store the materialize root_hash names, recompile,
    //     and re-derive both halves byte-identically ---
    let ls = dli_lorestore::Lorestore::open(lorestore_root.join("storybook"))
        .expect("open the per-base store");
    assert_eq!(
        ls.root_hash().expect("head ok").expect("head").to_string(),
        second_hash,
        "the branch head is the last accepted commit"
    );
    let compiled = ls
        .compile()
        .expect("the store compiles base + accepted deltas");

    let interner = compiled.model.interner();
    let mut replay_valuation: Vec<Value> = compiled
        .model
        .world
        .valuation
        .0
        .iter()
        .filter_map(|(atom, value)| {
            interner
                .resolve(*atom)
                .map(|name| json!({"atom": name, "value": value}))
        })
        .collect();
    replay_valuation.sort_by(|a, b| {
        a["atom"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["atom"].as_str().unwrap_or_default())
    });
    assert_eq!(
        Value::Array(replay_valuation),
        Value::Array(response_valuation),
        "replay: the compiled world's full valuation matches the response's"
    );

    let goal = parse_goal();
    let opts = dli_eval::query::QueryOpts {
        layer: dli_eval::query::Layer::default(),
        as_of: None,
        fork: None,
        why: false,
    };
    let replay_answer = compiled
        .model
        .world
        .query_with(&dli_eval::query::Query { goal }, &opts)
        .expect("replay query evaluates");
    let expected_claim = facts
        .iter()
        .find(|fact| fact["atom"]["pred"] == "home")
        .and_then(|fact| fact["claim_id"].as_str())
        .expect("committed home claim");
    assert_eq!(response_rows[0]["claim_ids"], json!([expected_claim]));
    for row in &mut response_rows {
        row.as_object_mut().expect("query row").remove("claim_ids");
    }
    let replay_rows = serde_json::to_value(&replay_answer.rows).expect("rows serialize");
    assert_eq!(
        serde_json::to_vec(&replay_rows).expect("bytes"),
        serde_json::to_vec(&Value::Array(response_rows)).expect("bytes"),
        "replay: the semantic row fields match the recompiled answer"
    );

    // --- REJECT: a second workspace discards a staged claim per-decl; the
    //     rejected claim never reaches the store ---
    let open2 = send(
        &json!({"dli":"2.0.0","id":"open2","op":"author.open","params":{"scope":"storybook"}}),
        &mut sessions,
        &mut store,
    );
    let ws2 = ok_result(&open2)["workspace"]
        .as_str()
        .expect("ws2")
        .to_owned();
    let rejected = send(
        &json!({"dli":"2.0.0","id":"r1","op":"author.add_fact",
                "params":{"workspace": ws2, "entity":"nara","key":"rank","value":"reckless",
                          "provenance": {"kind":"proposed","origin":"ls5-review"}}}),
        &mut sessions,
        &mut store,
    );
    ok_result(&rejected);
    let status = send(
        &json!({"dli":"2.0.0","id":"r-status","op":"author.status","params":{"workspace":ws2}}),
        &mut sessions,
        &mut store,
    );
    let staged_key = ok_result(&status)["staged"][0]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    let discard = send(
        &json!({"dli":"2.0.0","id":"r2","op":"author.discard",
                "params":{"workspace": ws2, "decl": staged_key}}),
        &mut sessions,
        &mut store,
    );
    assert_eq!(ok_result(&discard)["discarded"], Value::Bool(true));
    let after_reject = send(
        &json!({"dli":"2.0.0","id":"r3","op":"author.materialize","params":{"workspace": ws2}}),
        &mut sessions,
        &mut store,
    );
    let after_reject = ok_result(&after_reject);
    assert_eq!(
        after_reject["facts"].as_array().map(|facts| facts
            .iter()
            .filter(|fo| fo["atom"]["pred"] == json!("rank"))
            .count()),
        Some(0),
        "the rejected claim never reaches the store"
    );
}

/// The replay goal `home(nara, ?where)` — constructed exactly as the wire's
/// goal parser lowers it (a `?name` argument is a query variable named
/// without its marker), so the replayed query is the SAME query.
fn parse_goal() -> dli_ast::term::AtomExpr {
    dli_ast::term::AtomExpr {
        pred: dli_ast::ident::Ident::synthetic("home"),
        args: vec![
            dli_ast::term::Term::Sym(dli_ast::ident::Ident::synthetic("nara")),
            dli_ast::term::Term::Var(dli_ast::ident::Ident::synthetic("where")),
        ],
    }
}
