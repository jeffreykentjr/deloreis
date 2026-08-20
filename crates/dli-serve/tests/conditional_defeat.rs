//! Contract 1.13 conditional defeat: nested restoration on the author wire,
//! durable reopen, and preservation of the released 1.12 one-way behavior.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use dli_serve::author::{dispatch, store::Store};
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};

fn request(version: &str, id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: version.to_owned(),
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

fn fixture() -> (tempfile::TempDir, Store, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"conditional-defeat\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope demo\nratify d1\nperson mara \"Mara\" ratified d1\nfact mara rank gold ratified d1\n",
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "fixture parses: {diagnostics:?}"
    );
    let mut store = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let base = root.path().join("lorestore/demo");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    (root, store, base)
}

fn claim_id(store: &dli_lorestore::Lorestore, kind: Option<&str>, name: &str) -> String {
    let head = store.root_hash().unwrap().unwrap();
    store
        .fact_hashes_from(&head)
        .unwrap()
        .into_iter()
        .find_map(|hash| {
            let object = store
                .read_object::<dli_lorestore::FactObject>(&hash)
                .unwrap()?;
            match kind {
                Some("ratify")
                    if object.expert_body.as_ref().is_some_and(|body| {
                        body.kind == dli_lorestore::ExpertDeclKind::Ratify && body.name == name
                    }) =>
                {
                    Some(object.claim_id.0)
                }
                None if object.atom.pred == "rank"
                    && object.atom.args.first().is_some_and(|arg| arg == name) =>
                {
                    Some(object.claim_id.0)
                }
                _ => None,
            }
        })
        .unwrap_or_else(|| panic!("stored {kind:?} claim `{name}` exists"))
}

fn author_query(store: &mut Store, workspace: &str, id: &str) -> Value {
    result(dispatch(
        &request(
            "1.13.0",
            id,
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]},
                "layer": "all"
            }),
        ),
        store,
    ))
}

fn defeat(
    store: &mut Store,
    workspace: &str,
    id: &str,
    target_claim: &str,
    attacker: &str,
    target: &str,
) {
    let defeated = result(dispatch(
        &request(
            "1.13.0",
            id,
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": target_claim,
                "decision": attacker,
                "defeated_decision": target
            }),
        ),
        store,
    ));
    assert_eq!(defeated["findings"], json!([]));
}

#[test]
#[allow(clippy::too_many_lines)]
fn nested_defeat_restores_and_reopens_without_false_ref_bits() {
    let (root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path.clone()).unwrap();
    let d1_claim = claim_id(&lorestore, None, "mara");
    let workspace = result(dispatch(
        &request("1.13.0", "open", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();

    defeat(&mut store, &workspace, "d2", &d1_claim, "d2", "d1");
    let head_after_first = lorestore.root_hash().unwrap().unwrap();
    let original = lorestore
        .fact_hashes_from(&head_after_first)
        .unwrap()
        .into_iter()
        .find_map(|hash| {
            lorestore
                .read_object::<dli_lorestore::FactObject>(&hash)
                .unwrap()
                .filter(|object| object.claim_id.0 == d1_claim)
        })
        .unwrap();
    assert_eq!(
        original
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.defeated_by.as_deref()),
        None,
        "conditional defeat does not stamp one-way defeated metadata"
    );
    assert!(
        author_query(&mut store, &workspace, "q1")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let d2_claim = claim_id(&lorestore, Some("ratify"), "d2");
    defeat(&mut store, &workspace, "d3", &d2_claim, "d3", "d2");
    let restored = author_query(&mut store, &workspace, "q2");
    let restored_why = result(dispatch(
        &request(
            "1.13.0",
            "why2",
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut store,
    ));
    let debug_head = lorestore.root_hash().unwrap().unwrap();
    let debug_pack = lorestore.reconstruct_at(&debug_head).unwrap();
    let debug_decls: Vec<String> = debug_pack
        .documents
        .iter()
        .flat_map(|document| document.decls.iter())
        .map(|decl| format!("{decl:?}"))
        .collect();
    assert_eq!(
        restored["rows"][0]["status"], "certain",
        "nested defeat restores the original support: query={restored}, why={restored_why}, decls={debug_decls:?}"
    );

    let d3_claim = claim_id(&lorestore, Some("ratify"), "d3");
    defeat(&mut store, &workspace, "d4", &d3_claim, "d4", "d3");
    assert!(
        author_query(&mut store, &workspace, "q3")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let reference = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .unwrap()
        .unwrap();
    assert!(
        reference.valuation.0.is_empty(),
        "conditional standing is derived, never persisted as false Ref bits"
    );
    assert_eq!(
        lorestore
            .decision_attacks_from(&reference.commit)
            .unwrap()
            .len(),
        3
    );

    // A fresh project/session reconstructs the attack graph from the commit and
    // reaches the same dead result without borrowing the live author workspace.
    let project = Project::load(root.path()).unwrap();
    let mut sessions = Sessions::from_project(Arc::new(project));
    let mut author = Store::default();
    let frame = json!({
        "dli": "1.13.0",
        "id": "reopen",
        "op": "dli.query",
        "params": {
            "document": "demo",
            "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]},
            "layer": "all"
        }
    });
    let reopened = respond(&frame.to_string(), &mut sessions, &mut author).unwrap();
    assert!(reopened.ok, "reopened query: {reopened:?}");
    assert!(
        reopened.result.unwrap()["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn contract_1_12_defeat_keeps_the_frozen_one_way_ref_semantics() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let claim = claim_id(&lorestore, None, "mara");
    let workspace = result(dispatch(
        &request("1.12.0", "open", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    result(dispatch(
        &request(
            "1.12.0",
            "defeat",
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": claim,
                "decision": "d2",
                "defeated_decision": "d1"
            }),
        ),
        &mut store,
    ));
    let reference = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .unwrap()
        .unwrap();
    assert_eq!(
        lorestore.decision_attacks_from(&reference.commit).unwrap(),
        []
    );
    let names = lorestore.interner_at(&reference.commit).unwrap();
    let d1 = dli_core::Atom(
        u32::try_from(names.iter().position(|name| name == "d1").unwrap())
            .expect("decision atom index fits u32"),
    );
    assert_eq!(reference.valuation.get(d1), Some(false));
}
