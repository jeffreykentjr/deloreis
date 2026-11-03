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
