#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 1.10's `author.entity_candidates` — bounded coreference candidates
//! (Read B). Mirrors `crates/dli-serve/tests/predicates.rs`'s harness.
//!
//! Coverage: op reachability (advertised and answered only from `1.10.0` up),
//! nearest-first ranking with distances, the canonical-representative fold (an
//! aliased class collapses to one candidate, so a surfaced name keys the way the
//! FD/continuity check would), the engine ceiling (a huge `limit` is clamped, not
//! honored — bounded is a property of the engine, not the caller), and the
//! guard-census determination (the read stages no `Decl`).

use serde_json::{Value, json};

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::session::Sessions;
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

fn base(source: &str) -> Store {
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.is_empty(),
        "base source parses: {diagnostics:?}"
    );
    Store::with_pack(dli_world::pack::Pack {
        name: "tpth".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles")
}

fn open_scope(store: &mut Store, dli: &str, scope: &str) -> String {
    let response = dispatch(
        &req(dli, "open", Op::AuthorOpen, json!({ "scope": scope })),
        store,
    );
    result(&response)["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned()
}

fn candidates(
    store: &mut Store,
    dli: &str,
    ws: &str,
    surface: &str,
    limit: u64,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "ec",
            Op::AuthorEntityCandidates,
            json!({ "workspace": ws, "surface": surface, "limit": limit }),
        ),
        store,
    )
}

fn names(result: &Value) -> Vec<String> {
    result["candidates"]
        .as_array()
        .expect("a candidates array")
        .iter()
        .map(|row| row["name"].as_str().unwrap_or_default().to_owned())
        .collect()
}

fn distances(result: &Value) -> Vec<u64> {
    result["candidates"]
        .as_array()
        .expect("a candidates array")
        .iter()
        .map(|row| row["distance"].as_u64().unwrap_or_default())
        .collect()
}

// -- op reachability ---------------------------------------------------------

#[test]
fn entity_candidates_is_unknown_before_contract_1_10_0() {
    for dli in ["0.5.1", "1.0.0", "1.8.0", "1.9.0"] {
        let mut sessions = Sessions::new();
        let mut store = Store::default();
        let response = dli_serve::serve::dispatch(
            &req(
                dli,
                "ec",
                Op::AuthorEntityCandidates,
                json!({ "workspace": "ws-1", "surface": "mara", "limit": 5 }),
            ),
            &mut sessions,
            &mut store,
        );
        assert!(!response.ok, "{dli} must refuse the op: {response:?}");
        let error = response.error.expect("refusal carries an error");
        assert_eq!(error.code, dli_serve::ErrorCode::UnknownOp);
        assert!(
            error.message.contains("1.10.0"),
            "{dli}'s refusal names the version that introduced it: {}",
            error.message
        );
    }
}

// -- ranking -----------------------------------------------------------------

#[test]
fn candidates_are_nearest_first_with_distances() {
    let mut store = base(concat!(
        "scope tpth\n",
        "ratify D-0001\n",
        "place mara \"Mara\"  ratified D-0001\n",
        "place mira \"Mira\"  ratified D-0001\n",
        "place keeper \"Keeper\"  ratified D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "tpth");
    let out = candidates(&mut store, "1.10.0", &ws, "mara", 5);
    let out = result(&out);
    let names = names(out);
    assert_eq!(names.first().map(String::as_str), Some("mara"), "{names:?}");
    assert!(
        names.contains(&"mira".to_owned()),
        "near match present: {names:?}"
    );
    let distances = distances(out);
    assert_eq!(distances.first(), Some(&0), "exact match is distance 0");
    assert!(
        distances.windows(2).all(|w| w[0] <= w[1]),
        "distances are non-decreasing (nearest first): {distances:?}"
    );
}

/// The canonical-representative fold: an aliased class collapses to one
/// candidate, so the surfaced name keys the way `continuity.fd-violation` would
/// and the `limit` budget is never spent twice on one class. `same-as` merges
/// `mira` into `mara` (lexicographic-min representative), so only `mara`
/// surfaces.
#[test]
fn an_aliased_class_collapses_to_one_representative() {
    let mut store = base(concat!(
        "scope tpth\n",
        "ratify D-0001\n",
        "place mara \"Mara\"  ratified D-0001\n",
        "place mira \"Mira\"  ratified D-0001\n",
        "same-as mara = mira decided D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "tpth");
    let out = candidates(&mut store, "1.10.0", &ws, "mira", 5);
    let names = names(result(&out));
    assert!(
        names.contains(&"mara".to_owned()),
        "the canonical representative surfaces: {names:?}"
    );
    assert!(
        !names.contains(&"mira".to_owned()),
        "the aliased member does not surface separately: {names:?}"
    );
}

/// Bounded-by-design: a caller `limit` far larger than the entity set is capped
/// at the engine ceiling, so it can never turn into a full entity dump. With 110
/// entities present and `limit` at `usize::MAX`, the read returns the ceiling
/// (100), not all 110.
#[test]
fn an_over_large_limit_is_clamped_to_the_ceiling() {
    use std::fmt::Write as _;
    let mut source = String::from("scope tpth\nratify D-0001\n");
    for i in 0..110 {
        let _ = writeln!(source, "place e{i:03} \"E{i:03}\"  ratified D-0001");
    }
    let mut store = base(&source);
    let ws = open_scope(&mut store, "1.10.0", "tpth");
    let out = candidates(&mut store, "1.10.0", &ws, "e000", u64::MAX);
    let count = result(&out)["candidates"]
        .as_array()
        .expect("candidates")
        .len();
    assert_eq!(
        count, 100,
        "an over-large limit is clamped to the engine ceiling, not the full set of 110"
    );
}

// -- guard-census determination ----------------------------------------------

#[test]
fn the_read_stages_nothing() {
    let mut store = base(concat!(
        "scope tpth\n",
        "ratify D-0001\n",
        "place mara \"Mara\"  ratified D-0001\n",
    ));
    let ws = open_scope(&mut store, "1.10.0", "tpth");
    let declared = dispatch(
        &req(
            "1.10.0",
            "df",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "current-role",
                    "provenance": { "kind": "proposed", "origin": "test" } }),
        ),
        &mut store,
    );
    assert!(declared.ok, "stage a baseline decl: {declared:?}");
    let staged_before = store.get(&ws).expect("workspace open").decls.len();

    let read = candidates(&mut store, "1.10.0", &ws, "mara", 5);
    assert!(read.ok, "the read answers: {read:?}");

    let staged_after = store.get(&ws).expect("workspace open").decls.len();
    assert_eq!(
        staged_before, staged_after,
        "a pure read must neither stage nor drop a declaration"
    );
}
