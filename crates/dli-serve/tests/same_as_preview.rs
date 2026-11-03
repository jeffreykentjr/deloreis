#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Read C — previewing an `assert_same_as` merge consequence WITHOUT committing,
//! through the existing wire surface. No new op: the incremental-canonicalization
//! audit found this already covered by the branch/staged workflow, and this test
//! is the standing proof plus the per-preview cost measurement.
//!
//! The per-mention preview loop, over one reused workspace:
//!
//! 1. `author.assert_same_as` — stage the proposed identity (a cited, ratified
//!    same-as, so the merge is certain and a real conflict surfaces).
//! 2. `author.check` — the full battery over the staged world; a merge that puts
//!    two distinct certain values on one functional slot surfaces as
//!    `continuity.fd-violation`. THIS is the preview signal.
//! 3. `author.discard {decl}` — roll back just that one staged declaration.
//!
//! Measured cost: **3 wire calls per mention**, plus O(1) amortized setup (one
//! `author.open` and a one-time `author.status` to learn the same-as's structural
//! key, which is stable within a contract version). Nothing is ever materialized,
//! so the preview leaves nothing observable moved — pinned below.

use serde_json::{Value, json};

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request};

// -- harness -----------------------------------------------------------------

fn req(dli: &str, id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: dli.to_owned(),
        id: id.to_owned(),
        op,
        params,
        expected_digest: None,
        idempotency_key: None,
    }
}

fn result(response: &dli_serve::Response) -> &Value {
    assert!(response.ok, "expected ok, got error: {:?}", response.error);
    response
        .result
        .as_ref()
        .expect("ok response carries a result")
}

fn codes(result: &Value) -> Vec<String> {
    result["findings"]
        .as_array()
        .map(|findings| {
            findings
                .iter()
                .map(|f| f["code"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn base(source: &str) -> Store {
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.is_empty(),
        "base source parses: {diagnostics:?}"
    );
    Store::with_pack(dli_world::pack::Pack {
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles")
}

/// A base where `current-role` is functional and two distinct entities hold two
/// distinct certain values — so nothing conflicts until a same-as merges them.
fn base_with_latent_conflict() -> Store {
    base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "place lyra \"Lyra\"  ratified D-0001\n",
        "place mira \"Mira\"  ratified D-0001\n",
        "functional current-role decided D-0001\n",
        "current-role(lyra, keeper) decided D-0001\n",
        "current-role(mira, warden) decided D-0001\n",
    ))
}

fn open_scope(store: &mut Store, ws_scope: &str) -> String {
    let response = dispatch(
        &req(
            "1.10.0",
            "open",
            Op::AuthorOpen,
            json!({ "scope": ws_scope }),
        ),
        store,
    );
    result(&response)["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned()
}

fn stage_same_as(store: &mut Store, ws: &str) -> dli_serve::Response {
    dispatch(
        &req(
            "1.10.0",
            "sa",
            Op::AuthorAssertSameAs,
            json!({
                "workspace": ws, "left": "lyra", "right": "mira",
                "provenance": { "kind": "cited", "decisions": ["D-0001"], "state": "ratified" }
            }),
        ),
        store,
    )
}

fn check(store: &mut Store, ws: &str) -> dli_serve::Response {
    dispatch(
        &req("1.10.0", "ck", Op::AuthorCheck, json!({ "workspace": ws })),
        store,
    )
}

fn discard_decl(store: &mut Store, ws: &str, key: &str) -> dli_serve::Response {
    dispatch(
        &req(
            "1.10.0",
            "dc",
            Op::AuthorDiscard,
            json!({ "workspace": ws, "decl": key }),
        ),
        store,
    )
}

/// The structural key of the one staged `same-as` row, via `author.status`.
fn same_as_key(store: &mut Store, ws: &str) -> String {
    let status = dispatch(
        &req("1.10.0", "st", Op::AuthorStatus, json!({ "workspace": ws })),
        store,
    );
    result(&status)["staged"]
        .as_array()
        .expect("staged rows (contract >= 1.3.0)")
        .iter()
        .find(|row| row["kind"] == json!("same-as"))
        .expect("a staged same-as row")["key"]
        .as_str()
        .expect("a structural key")
        .to_owned()
}

fn staged_count(store: &mut Store, ws: &str) -> usize {
    let status = dispatch(
        &req("1.10.0", "st", Op::AuthorStatus, json!({ "workspace": ws })),
        store,
    );
    result(&status)["staged"].as_array().map_or(0, Vec::len)
}

// -- the preview loop --------------------------------------------------------

#[test]
fn a_same_as_merge_conflict_previews_then_rolls_back_at_three_calls_per_mention() {
    let mut store = base_with_latent_conflict();
    let ws = open_scope(&mut store, "storybook");

    // Amortized setup: learn the same-as's stable structural key once, then reset.
    let staged = stage_same_as(&mut store, &ws);
    assert!(staged.ok, "stage the proposed identity: {staged:?}");
    let key = same_as_key(&mut store, &ws);
    assert!(
        discard_decl(&mut store, &ws, &key).ok,
        "reset after discovery"
    );
    assert_eq!(
        staged_count(&mut store, &ws),
        0,
        "reset leaves nothing staged"
    );

    // The per-mention preview loop: exactly three wire calls each.
    for mention in 0..3 {
        // (1) stage
        let staged = stage_same_as(&mut store, &ws);
        assert!(staged.ok, "mention {mention}: stage: {staged:?}");
        // (2) check — the merge conflict surfaces, without committing
        let checked = check(&mut store, &ws);
        assert!(
            codes(result(&checked))
                .iter()
                .any(|c| c.contains("fd-violation")),
            "mention {mention}: the merge previews as an fd-violation: {:?}",
            codes(result(&checked))
        );
        // (3) discard — roll back just this same-as
        let rolled = discard_decl(&mut store, &ws, &key);
        assert!(rolled.ok, "mention {mention}: discard: {rolled:?}");
    }

    // Nothing accumulated and nothing committed: the workspace is empty of staged
    // decls and clean again, and the base was never materialized.
    assert_eq!(
        staged_count(&mut store, &ws),
        0,
        "the preview loop leaves nothing staged"
    );
    let final_check = check(&mut store, &ws);
    assert!(
        !codes(result(&final_check))
            .iter()
            .any(|c| c.contains("fd-violation")),
        "with the same-as rolled back, no merge conflict remains: {:?}",
        codes(result(&final_check))
    );
}

/// The clean case: a same-as whose merge does NOT collide previews clean, so the
/// signal is the conflict itself, not the mere presence of a staged same-as.
#[test]
fn a_non_colliding_same_as_previews_clean() {
    let mut store = base(concat!(
        "scope storybook\n",
        "ratify D-0001\n",
        "place lyra \"Lyra\"  ratified D-0001\n",
        "place mira \"Mira\"  ratified D-0001\n",
        "functional current-role decided D-0001\n",
        "current-role(lyra, keeper) decided D-0001\n",
    ));
    let ws = open_scope(&mut store, "storybook");
    assert!(stage_same_as(&mut store, &ws).ok);
    let checked = check(&mut store, &ws);
    assert!(
        !codes(result(&checked))
            .iter()
            .any(|c| c.contains("fd-violation")),
        "a merge that collides with no second value previews clean: {:?}",
        codes(result(&checked))
    );
}
