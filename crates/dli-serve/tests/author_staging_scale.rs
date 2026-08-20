//! Non-gating scale evidence for GH #99's `author.add_fact` workload.
//!
//! Wall clock is evidence, never a gate verdict (DL-0013), so the test is
//! ignored by default. It deliberately exercises the real dispatch and durable
//! staging paths at the four issue sizes, with unique proposal origins so the
//! workspace interner grows on every mutation. Run in release mode:
//!
//! `cargo test -p dli-serve --release --test author_staging_scale -- --ignored --nocapture`

#![allow(clippy::expect_used, clippy::print_stderr, clippy::unwrap_used)]

use std::time::Instant;

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};

fn request(id: String, op: Op, params: Value) -> Request {
    Request {
        dli: "1.12.0".to_owned(),
        id,
        op,
        params,
        expected_digest: None,
        idempotency_key: None,
    }
}

fn scale_store() -> (tempfile::TempDir, Store, String) {
    let root = tempfile::tempdir().expect("temporary scale store");
    let (document, diagnostics) =
        dli_syntax::parse_document("scope scale\nperson world \"World\" proposed seed\n");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "scale base parses: {diagnostics:?}"
    );
    let mut store = Store::with_pack(Pack {
        name: "scale".to_owned(),
        documents: vec![document],
    })
    .expect("scale base compiles");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .expect("attach scale lorestore");
    let opened = dispatch(
        &request("open".to_owned(), Op::AuthorOpen, json!({"scope": "scale"})),
        &mut store,
    );
    assert!(opened.ok, "open scale workspace: {opened:?}");
    let workspace = opened
        .result
        .as_ref()
        .and_then(|result| result["workspace"].as_str())
        .expect("workspace id")
        .to_owned();
    (root, store, workspace)
}

#[test]
#[ignore = "non-gating wall-clock scale evidence; run explicitly in release mode"]
fn add_fact_scales_through_65k_with_a_growing_interner() {
    for count in [1_024usize, 4_096, 16_384, 65_536] {
        let (_root, mut store, workspace) = scale_store();
        let stage_start = Instant::now();
        for index in 0..count {
            let response = dispatch(
                &request(
                    format!("fact-{index}"),
                    Op::AuthorAddFact,
                    json!({
                        "workspace": workspace,
                        "entity": "world",
                        "key": "holds",
                        "value": format!("value-{index}"),
                        "provenance": {
                            "kind": "proposed",
                            "origin": format!("scale-{index}")
                        }
                    }),
                ),
                &mut store,
            );
            assert!(response.ok, "fact {index}/{count} failed: {response:?}");
            assert!(
                response
                    .result
                    .as_ref()
                    .and_then(|result| result["findings"].as_array())
                    .is_some_and(Vec::is_empty),
                "fact {index}/{count} raised a finding: {response:?}"
            );
        }
        let stage_elapsed = stage_start.elapsed();
        let check_start = Instant::now();
        let checked = dispatch(
            &request(
                "check".to_owned(),
                Op::AuthorCheck,
                json!({"workspace": workspace}),
            ),
            &mut store,
        );
        let check_elapsed = check_start.elapsed();
        assert!(
            checked.ok,
            "aggregate preflight failed at {count}: {checked:?}"
        );

        let workspace_state = store.get(&workspace).expect("workspace remains open");
        assert_eq!(workspace_state.decls.len(), count);
        assert!(
            workspace_state.interner.len() >= count,
            "every unique origin is interned"
        );
        eprintln!(
            "issue99 facts={count} stage={stage_elapsed:?} ns_per_fact={} preflight={check_elapsed:?}",
            stage_elapsed.as_nanos() / count as u128
        );
    }
}
