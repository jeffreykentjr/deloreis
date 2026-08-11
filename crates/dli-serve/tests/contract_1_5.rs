//! Contract 1.5 durable standing operations.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::Arc;

use dli_ast::decl::{Decl, Document, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::language::RatifyBody;
use dli_ast::span::Span;
use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};

fn id(text: &str) -> Ident {
    Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

fn pack() -> Pack {
    Pack {
        name: "demo".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("demo"),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Ratify(RatifyDecl {
                    decision: id("D-1"),
                    body: RatifyBody::default(),
                }),
                Decl::Ratify(RatifyDecl {
                    decision: id("D-other"),
                    body: RatifyBody::default(),
                }),
            ],
        }],
    }
}

fn request(id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: "1.5.0".to_owned(),
        id: id.to_owned(),
        op,
        params,
        expected_digest: None,
        idempotency_key: None,
    }
}

fn result(response: dli_serve::Response) -> Value {
    assert!(response.ok, "request failed: {response:?}");
    response.result.expect("successful request has a result")
}

fn temp_store() -> (tempfile::TempDir, Store, PathBuf) {
    let root = tempfile::tempdir().expect("temporary store root");
    let path = root.path().join("lorestore");
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"contract-1-5\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .expect("write project manifest");
    let mut store = Store::with_pack(pack()).expect("compile demo pack");
    store
        .attach_lorestore(path.clone())
        .expect("attach demo lorestore");
    (root, store, path.join("demo"))
}

fn corpus_export(root: &std::path::Path) -> dli_serve::Response {
    let project = Project::load(root).expect("reload store-backed project");
    let mut sessions = Sessions::from_project(Arc::new(project));
    let mut author = Store::default();
    let frame = json!({
        "dli": "1.5.0",
        "id": "canon",
        "op": "dli.corpus.export",
        "params": {"document": "demo", "format": "canon"}
    });
    respond(
        &serde_json::to_string(&frame).expect("serialize frame"),
        &mut sessions,
        &mut author,
    )
    .expect("canon export answers")
}

fn canon(root: &std::path::Path) -> Value {
    let response = corpus_export(root);
    assert!(response.ok, "canon export failed: {response:?}");
    response.result.expect("canon export result")["canon"].clone()
}

#[test]
#[allow(clippy::too_many_lines)]
fn contract_1_5_promotes_and_defeats_without_deleting_the_object() {
    let (root, mut store, path) = temp_store();
    let opened = result(dispatch(
        &request("open", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ));
    let workspace = opened["workspace"].as_str().unwrap().to_owned();

    let added = result(dispatch(
        &request(
            "add",
            Op::AuthorAddEntity,
            json!({
                "workspace": workspace,
                "kind": "person",
                "id": "mara",
                "name": "Mara",
                "provenance": {
                    "kind": "cited",
                    "decisions": ["D-1"],
                    "state": "provisional"
                }
            }),
        ),
        &mut store,
    ));
    assert!(
        added["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["severity"] != "error")
    );

    let materialized = result(dispatch(
        &request(
            "materialize",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false}),
        ),
        &mut store,
    ));
    let claim_id = materialized["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|fact| {
            fact["provenance"]["marker"]
                .as_str()
                .is_some()
                .then(|| fact["claim_id"].as_str())
                .flatten()
        })
        .expect("materialize returns the stored claim id")
        .to_owned();
    assert_eq!(
        claim_id, "sha256:5d572986b5ab96893c9a1a4c6543b9731f8a72d8198c13d52e394521a5f39c68",
        "the fixture claim identity is part of the deterministic contract"
    );
    assert_eq!(
        materialized["facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["claim_id"].as_str() == Some(&claim_id))
            .unwrap()["provenance"]["marker"],
        "Provisional",
        "materialize must preserve the requested standing: {materialized}"
    );
    let before = materialized["root_hash"].as_str().unwrap().to_owned();
    let provisional_canon = canon(root.path());
    assert!(
        provisional_canon["characters"]["person"]
            .as_array()
            .expect("person rows")
            .iter()
            .any(|row| row["id"] == "mara" && row["status"] == "provisional"),
        "1.5 cited provisional authoring must reach canon: {provisional_canon}"
    );

    let unknown = result(dispatch(
        &request(
            "unknown",
            Op::AuthorSetStanding,
            json!({
                "workspace": workspace,
                "claim_id": claim_id,
                "decision": "D-unknown"
            }),
        ),
        &mut store,
    ));
    assert_eq!(unknown["findings"][0]["code"], "citation.unknown-decision");
    assert_eq!(unknown["root_hash"], before);

    let unrelated = dispatch(
        &request(
            "unrelated",
            Op::AuthorSetStanding,
            json!({
                "workspace": workspace,
                "claim_id": claim_id,
                "decision": "D-other"
            }),
        ),
        &mut store,
    );
    assert!(!unrelated.ok, "an unrelated decision must not promote");
    assert_eq!(
        unrelated
            .error
            .expect("promotion refusal has an error")
            .code,
        dli_serve::ErrorCode::SchemaViolation
    );

    let promoted = result(dispatch(
        &request(
            "promote",
            Op::AuthorSetStanding,
            json!({
                "workspace": workspace,
                "claim_id": claim_id,
                "decision": "D-1"
            }),
        ),
        &mut store,
    ));
    assert_ne!(promoted["root_hash"], before);
    let promoted_store = dli_lorestore::Lorestore::open(path.clone()).expect("reopen lorestore");
    let promoted_head = promoted_store
        .root_hash()
        .expect("read promoted head")
        .expect("promoted head exists");
    let promoted_fact = promoted_store
        .fact_hashes_from(&promoted_head)
        .expect("read promoted facts")
        .into_iter()
        .find_map(|hash| {
            promoted_store
                .read_object::<dli_lorestore::FactObject>(&hash)
                .expect("read promoted fact")
                .filter(|fact| fact.claim_id.0 == claim_id)
        })
        .expect("promoted fact remains reachable");
    assert_eq!(
        promoted_fact
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.standing_decision.as_deref()),
        Some("D-1")
    );

    let defeated = result(dispatch(
        &request(
            "defeat",
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": claim_id,
                "decision": "D-2",
                "defeated_decision": "D-1"
            }),
        ),
        &mut store,
    ));
    assert_eq!(defeated["findings"], json!([]));
    let queried = result(dispatch(
        &request(
            "query",
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal": "person(mara)",
                "layer": "all"
            }),
        ),
        &mut store,
    ));
    assert_eq!(
        queried["rows"],
        json!([]),
        "a defeated claim is excluded from every query layer: {queried}"
    );
    assert!(
        queried["valuation"]
            .as_array()
            .expect("valuation rows")
            .iter()
            .any(|entry| entry["atom"] == "D-1" && entry["value"] == false),
        "the author session sees the durable defeat overlay: {queried}"
    );

    let lorestore = dli_lorestore::Lorestore::open(path).expect("reopen lorestore");
    let reference = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .expect("read main ref")
        .expect("main ref exists");
    let interner = lorestore
        .interner_at(&reference.commit)
        .expect("read interner");
    let decision_atom = dli_core::Atom(
        u32::try_from(
            interner
                .iter()
                .position(|name| name == "D-1")
                .unwrap_or_else(|| panic!("defeated decision is interned: {interner:?}")),
        )
        .expect("interner index fits in an atom"),
    );
    assert_eq!(reference.valuation.get(decision_atom), Some(false));
    let defeated_fact = lorestore
        .fact_hashes_from(&reference.commit)
        .expect("read folded facts")
        .into_iter()
        .find_map(|hash| {
            lorestore
                .read_object::<dli_lorestore::FactObject>(&hash)
                .expect("read fact")
                .filter(|fact| fact.claim_id.0 == claim_id)
        })
        .expect("defeat keeps the object reachable");
    assert_eq!(
        defeated_fact
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.defeated_by.as_deref()),
        Some("D-2")
    );
    let defeated_canon = canon(root.path());
    assert!(
        defeated_canon["characters"]["person"]
            .as_array()
            .expect("person rows")
            .iter()
            .all(|row| row["id"] != "mara"),
        "canon excludes the dead claim while the object remains stored: {defeated_canon}"
    );
}

#[test]
fn canon_refuses_a_commit_whose_decision_list_disagrees_with_stored_ratify_objects() {
    let (root, _store, path) = temp_store();
    let lorestore = dli_lorestore::Lorestore::open(path).expect("reopen lorestore");
    let current = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .expect("read main ref")
        .expect("main ref exists");
    let current_digest = current.digest().expect("hash current ref").to_string();
    let decisions = lorestore
        .decisions_at(&current.commit)
        .expect("read current decisions");
    assert!(
        decisions.iter().any(|decision| decision == "D-1"),
        "the seed commit carries the ratify declaration"
    );
    let interner = lorestore
        .interner_at(&current.commit)
        .expect("read current interner");
    let pack_name = lorestore
        .head_pack_name()
        .expect("read current pack name")
        .expect("head names its pack");

    let corrupt = dli_lorestore::Commit::child(
        current.commit,
        interner,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        pack_name,
    );
    let corrupt_hash = lorestore
        .write_object(&corrupt)
        .expect("persist deliberately corrupt commit");
    let corrupt_ref = dli_lorestore::Ref::with_valuation(
        dli_lorestore::MAIN_REF,
        corrupt_hash,
        current.valuation,
    );
    assert!(
        lorestore
            .cas_main_ref(Some(&current_digest), &corrupt_ref)
            .expect("point main at corrupt commit")
    );

    let response = corpus_export(root.path());
    assert!(!response.ok, "canon must fail closed: {response:?}");
    assert_eq!(
        response.error.expect("canon refusal has an error").code,
        dli_serve::ErrorCode::EvidenceCorrupt
    );
}
