//! Entity-sort queries at the authoring interface.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use dli_serve::{
    Op, Request,
    author::{dispatch, store::Store},
};
use dli_world::pack::Pack;
use serde_json::{Value, json};

fn call(store: &mut Store, op: Op, params: Value) -> Value {
    let response = dispatch(
        &Request {
            dli: "2.0.0".into(),
            id: "sort".into(),
            op,
            params,
            expected_digest: None,
            idempotency_key: None,
        },
        store,
    );
    assert!(response.ok, "{response:?}");
    response.result.unwrap()
}

#[test]
fn author_queries_entity_and_era_chain_with_explanations() {
    let (document, findings) = dli_syntax::parse_document(
        "scope story\nratify d-1\nperson wren \"Wren\" ratified d-1\nera dawn < noon < dusk ratified d-1\n",
    );
    assert!(findings.iter().all(|d| !d.class.is_error()), "{findings:?}");
    let mut store = Store::with_pack(Pack {
        name: "story".into(),
        documents: vec![document],
    })
    .unwrap();
    let workspace = call(&mut store, Op::AuthorOpen, json!({"scope":"story"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    for goal in ["person(wren)", "era(noon)", "era(dusk)"] {
        let answer = call(
            &mut store,
            Op::AuthorQuery,
            json!({"workspace":workspace, "goal":goal, "why":true}),
        );
        let rows = answer["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{goal}: {answer}");
        assert_eq!(rows[0]["status"], "certain", "{goal}: {answer}");
        assert!(
            rows[0]["why"].to_string().contains("d-1"),
            "{goal}: {answer}"
        );
    }
}

#[test]
fn standing_change_that_cannot_replay_is_refused_before_commit() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"era-standing\"\n\n[documents.story]\nstore = \"lorestore/story\"\n",
    )
    .unwrap();
    let (document, findings) =
        dli_syntax::parse_document("scope story\nratify d-1\nera dawn < noon provisional d-1\n");
    assert!(findings.iter().all(|d| !d.class.is_error()), "{findings:?}");
    let mut store = Store::with_pack(Pack {
        name: "story".into(),
        documents: vec![document],
    })
    .unwrap();
    store
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    let workspace = call(&mut store, Op::AuthorOpen, json!({"scope":"story"}))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let lorestore = dli_lorestore::Lorestore::open(root.path().join("lorestore/story")).unwrap();
    let old = lorestore.root_hash().unwrap().unwrap();
    for name in ["dawn", "noon"] {
        let member = lorestore
            .fact_hashes_from(&old)
            .unwrap()
            .iter()
            .map(|hash| {
                lorestore
                    .read_object::<dli_lorestore::FactObject>(hash)
                    .unwrap()
                    .unwrap()
            })
            .find(|object| object.atom.pred == "era" && object.atom.args == [name])
            .unwrap();
        let request = Request {
            dli: "2.0.0".into(),
            id: "promote".into(),
            op: Op::AuthorSetStanding,
            params: json!({"workspace":workspace,"claim_id":member.claim_id.0,"decision":"d-1"}),
            expected_digest: None,
            idempotency_key: None,
        };
        let response = dispatch(&request, &mut store);
        assert!(
            !response.ok,
            "an unrenderable standing change must not commit: {response:?}"
        );
        assert!(
            response
                .error
                .unwrap()
                .message
                .contains("cannot be replayed")
        );
        assert_eq!(lorestore.root_hash().unwrap().unwrap(), old);
    }
    drop(store);
    dli_serve::project::Project::load(root.path())
        .expect("the original committed project must reopen");
}
