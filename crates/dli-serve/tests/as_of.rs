#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 1.9's `author.as_of` (`docs/decisions.md` DL-0082).
//! Mirrors `crates/dli-serve/tests/declare_inertial.rs`'s harness.
//!
//! `author.as_of` is the ONE inertia-aware read (DL-0081's v1 read boundary,
//! lifted by DL-0082): it projects `check::slot_state_at` directly, never
//! `dli_eval::world::World::query_with`, and it is a read (routed through
//! `read`, never `mutate`) — it stages nothing and carries no idempotency
//! fence.

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
        name: "storybook".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles")
}

/// A base declaring decisions `d-1`/`d-2` (both ratified), the person
/// `joren`, events `dawn`/`noon`, and `dawn before noon` — the exact
/// `commons`/`enclosed` scenario `docs/decisions.md` DL-0081/DL-0082 and the
/// design spec both use, with a SECOND ratified decision so the
/// defeat-safety test can defeat one value's citation without touching the
/// other's.
fn weir_base() -> Store {
    base(concat!(
        "scope storybook\n",
        "ratify d-1\n",
        "ratify d-2\n",
        "person joren \"Joren\" decided d-1\n",
        "event dawn decided d-1\n",
        "event noon decided d-1\n",
        "before dawn -> noon decided d-1\n",
    ))
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

/// Stage `inertial <pred>`, citing `d-1` (ratified, so capping is active).
fn declare_pred_inertial(
    store: &mut Store,
    dli: &str,
    ws: &str,
    pred: &str,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "in",
            Op::AuthorDeclareInertial,
            json!({ "workspace": ws, "pred": pred,
                    "provenance": { "kind": "cited", "decisions": ["d-1"], "state": "ratified" } }),
        ),
        store,
    )
}

/// Stage `joren.status = <value> since <point>`, citing `decision` (ratified).
fn add_status(
    store: &mut Store,
    dli: &str,
    ws: &str,
    value: &str,
    point: &str,
    decision: &str,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "st",
            Op::AuthorAddState,
            json!({
                "workspace": ws,
                "id": "joren.status",
                "value": value,
                "time": { "kind": "since", "point": point },
                "provenance": { "kind": "cited", "decisions": [decision], "state": "ratified" }
            }),
        ),
        store,
    )
}

fn as_of(
    store: &mut Store,
    dli: &str,
    ws: &str,
    pred: &str,
    subject: &str,
    at: &str,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "a",
            Op::AuthorAsOf,
            json!({ "workspace": ws, "pred": pred, "subject": subject, "at": at }),
        ),
        store,
    )
}

// -- op reachability -----------------------------------------------------------

#[test]
fn as_of_is_answered_at_contract_1_9_0() {
    let mut sessions = Sessions::new();
    let mut store = weir_base();
    let ws = open_scope(&mut store, "2.0.0", "storybook");
    declare_pred_inertial(&mut store, "2.0.0", &ws, "status");
    add_status(&mut store, "2.0.0", &ws, "commons", "dawn", "d-1");
    let response = dli_serve::serve::dispatch(
        &req(
            "2.0.0",
            "a",
            Op::AuthorAsOf,
            json!({ "workspace": ws, "pred": "status", "subject": "joren", "at": "dawn" }),
        ),
        &mut sessions,
        &mut store,
    );
    assert!(response.ok, "1.9.0 answers the op: {response:?}");
    assert_eq!(result(&response)["holdings"][0]["value"], "commons");
}

// -- the three as-of cases ------------------------------------------------------

/// The capped value, its successor, and the unbounded tail: `commons since
/// dawn`, `enclosed since noon` (dawn before noon) on an inertial slot. At
/// `dawn`, `commons` is capped at `noon` by `enclosed`; at `noon`,
/// `enclosed` holds unbounded.
#[test]
fn wire_as_of_reports_the_capped_value_the_successor_and_the_unbounded_tail() {
    let mut store = weir_base();
    let ws = open_scope(&mut store, "1.9.0", "storybook");
    declare_pred_inertial(&mut store, "1.9.0", &ws, "status");
    add_status(&mut store, "1.9.0", &ws, "commons", "dawn", "d-1");
    add_status(&mut store, "1.9.0", &ws, "enclosed", "noon", "d-1");

    let at_dawn = as_of(&mut store, "1.9.0", &ws, "status", "joren", "dawn");
    let r = result(&at_dawn);
    assert_eq!(r["inertial"], true, "{r:?}");
    assert_eq!(r["capping"], true, "{r:?}");
    assert_eq!(r["contested"], false, "{r:?}");
    let holdings = r["holdings"].as_array().unwrap();
    assert_eq!(holdings.len(), 1, "{holdings:?}");
    assert_eq!(holdings[0]["value"], "commons");
    assert_eq!(holdings[0]["support"], "certain");
    assert_eq!(holdings[0]["projection"], "holds-certainly");
    assert_eq!(holdings[0]["since"], "dawn");
    assert_eq!(
        holdings[0]["until"], "noon",
        "the derived boundary is the defeater's own since: {holdings:?}"
    );
    assert_eq!(holdings[0]["defeater"]["value"], "enclosed");
    assert_eq!(holdings[0]["defeater"]["since"], "noon");

    let at_noon = as_of(&mut store, "1.9.0", &ws, "status", "joren", "noon");
    let r = result(&at_noon);
    let holdings = r["holdings"].as_array().unwrap();
    assert_eq!(holdings.len(), 1, "{holdings:?}");
    assert_eq!(holdings[0]["value"], "enclosed");
    assert_eq!(
        holdings[0]["until"],
        Value::Null,
        "unbounded — no later certain defeater: {holdings:?}"
    );
    assert_eq!(holdings[0]["defeater"], Value::Null);
}

/// Contested: two distinct values `since dawn` (the SAME point) certainly
/// co-hold — `contested:true`, both `holds-certainly`. Separately, two
/// distinct values whose anchors are unordered (no `before` between them)
/// read `order-indeterminate`, never `contested` (a contest is a
/// `continuity.fluent-order-undeclared` finding, not a wire clash flag).
#[test]
fn wire_as_of_reports_contested_and_order_indeterminate() {
    let mut store = weir_base();
    let ws = open_scope(&mut store, "1.9.0", "storybook");
    declare_pred_inertial(&mut store, "1.9.0", &ws, "status");
    add_status(&mut store, "1.9.0", &ws, "commons", "dawn", "d-1");
    add_status(&mut store, "1.9.0", &ws, "enclosed", "dawn", "d-1");
    let response = as_of(&mut store, "1.9.0", &ws, "status", "joren", "dawn");
    let r = result(&response);
    assert_eq!(r["contested"], true, "{r:?}");
    let holdings = r["holdings"].as_array().unwrap();
    assert_eq!(holdings.len(), 2, "{holdings:?}");
    assert!(
        holdings
            .iter()
            .all(|h| h["projection"] == "holds-certainly"),
        "{holdings:?}"
    );

    let mut store2 = base(concat!(
        "scope storybook\n",
        "ratify d-1\n",
        "person joren \"Joren\" decided d-1\n",
        "event e1 decided d-1\n",
        "event e2 decided d-1\n",
    ));
    let ws2 = open_scope(&mut store2, "1.9.0", "storybook");
    declare_pred_inertial(&mut store2, "1.9.0", &ws2, "status");
    add_status(&mut store2, "1.9.0", &ws2, "commons", "e1", "d-1");
    add_status(&mut store2, "1.9.0", &ws2, "enclosed", "e2", "d-1");
    let response = as_of(&mut store2, "1.9.0", &ws2, "status", "joren", "e1");
    let r = result(&response);
    assert_eq!(
        r["contested"], false,
        "an undeclared order is a settleable ambiguity, not a clash: {r:?}"
    );
    let holdings = r["holdings"].as_array().unwrap();
    assert!(
        holdings
            .iter()
            .any(|h| h["projection"] == "order-indeterminate"),
        "{holdings:?}"
    );
}

/// Non-inertial byte-identity (risk: non-inertial drift): the SAME two open
/// spans WITHOUT `declare_inertial` — no capping, both open spans cover
/// `noon` at once (the plain `state_at` clash, exactly as
/// `declare_inertial.rs`'s `non_inertial_open_transition_still_clashes`
/// pins at the check layer).
#[test]
fn wire_as_of_matches_plain_projection_when_not_inertial() {
    let mut store = weir_base();
    let ws = open_scope(&mut store, "1.9.0", "storybook");
    add_status(&mut store, "1.9.0", &ws, "commons", "dawn", "d-1");
    add_status(&mut store, "1.9.0", &ws, "enclosed", "noon", "d-1");
    let response = as_of(&mut store, "1.9.0", &ws, "status", "joren", "noon");
    let r = result(&response);
    assert_eq!(r["inertial"], false, "{r:?}");
    assert_eq!(r["capping"], false, "{r:?}");
    assert_eq!(
        r["contested"], true,
        "without inertia the two open values clash forward, as ever: {r:?}"
    );
    let holdings = r["holdings"].as_array().unwrap();
    assert_eq!(holdings.len(), 2, "{holdings:?}");
    for holding in holdings {
        assert_eq!(
            holding["until"],
            Value::Null,
            "an uncapped open span is unbounded: {holding:?}"
        );
        assert_eq!(holding["defeater"], Value::Null);
    }
}

/// Defeat-safety / valuation-lazy (risk: recompile): after the silent
/// transition, `author.defeat` the defeater's OWN citing decision (`d-2`,
/// distinct from `commons`'s `d-1`) — `author.as_of` at `noon`, in the SAME
/// workspace, flips back to the re-extended `commons`, no recompile.
#[test]
fn wire_as_of_flips_back_after_defeating_the_defeater() {
    let root = tempfile::tempdir().expect("temporary lorestore root");
    let mut store = weir_base();
    store
        .attach_lorestore(root.path().join("lorestore"))
        .expect("attach a fresh lorestore");
    let ws = open_scope(&mut store, "1.9.0", "storybook");
    declare_pred_inertial(&mut store, "1.9.0", &ws, "status");
    add_status(&mut store, "1.9.0", &ws, "commons", "dawn", "d-1");
    add_status(&mut store, "1.9.0", &ws, "enclosed", "noon", "d-2");

    let before = as_of(&mut store, "1.9.0", &ws, "status", "joren", "noon");
    assert_eq!(result(&before)["holdings"][0]["value"], "enclosed");

    let materialized = dispatch(
        &req(
            "1.9.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false }),
        ),
        &mut store,
    );
    assert!(materialized.ok, "{materialized:?}");
    let facts = result(&materialized)["facts"].as_array().expect("facts");
    let enclosed_claim_id = facts
        .iter()
        .find(|fact| {
            fact["atom"]["pred"] == "status" && fact["atom"]["args"] == json!(["joren", "enclosed"])
        })
        .and_then(|fact| fact["claim_id"].as_str())
        .expect("the durably stored enclosed-status claim id")
        .to_owned();

    let defeated = dispatch(
        &req(
            "1.9.0",
            "def",
            Op::AuthorDefeat,
            json!({
                "workspace": ws,
                "claim_id": enclosed_claim_id,
                "decision": "d-1",
                "defeated_decision": "d-2"
            }),
        ),
        &mut store,
    );
    assert!(defeated.ok, "{defeated:?}");

    // The SAME wire read, no recompile of anything by this test — `staged_world`
    // applies the durable `Ref.valuation` overlay `author.defeat` just wrote.
    let after = as_of(&mut store, "1.9.0", &ws, "status", "joren", "noon");
    let r = result(&after);
    let holdings = r["holdings"].as_array().unwrap();
    assert_eq!(holdings.len(), 1, "{holdings:?}");
    assert_eq!(
        holdings[0]["value"], "commons",
        "defeating the defeater re-extends the earlier value: {r:?}"
    );
    assert_eq!(holdings[0]["until"], Value::Null);
    assert_eq!(holdings[0]["defeater"], Value::Null);
}

// -- eval-path non-leak (DL-0081 hard invariant) --------------------------------

/// `author.query` on the SAME staged world still returns BOTH raw
/// extensional values, unprojected — `author.as_of` is the only
/// inertia-aware read. Extends
/// `declare_inertial.rs::wire_inertial_query_returns_extensional_values`
/// with an `author.as_of` call alongside it, in the same test, so a future
/// change that made `author.query` inertia-aware would fail HERE, next to
/// the `as_of` call it must never resemble.
#[test]
fn wire_as_of_does_not_leak_into_author_query() {
    let mut store = weir_base();
    let ws = open_scope(&mut store, "1.9.0", "storybook");
    declare_pred_inertial(&mut store, "1.9.0", &ws, "status");
    add_status(&mut store, "1.9.0", &ws, "commons", "dawn", "d-1");
    add_status(&mut store, "1.9.0", &ws, "enclosed", "noon", "d-1");

    // The inertia-aware read sees ONE certain value at `noon`.
    let projected = as_of(&mut store, "1.9.0", &ws, "status", "joren", "noon");
    let holdings = result(&projected)["holdings"].as_array().unwrap();
    assert_eq!(holdings.len(), 1, "{holdings:?}");
    assert_eq!(holdings[0]["value"], "enclosed");

    // The eval query path, on the exact same staged world, still answers
    // BOTH raw extensional values. `goal_atom` (the typed successor), not the
    // rendered `goal` string, so this test does not depend on a `syntax`-
    // enabled build.
    let queried = dispatch(
        &req(
            "1.9.0",
            "q",
            Op::AuthorQuery,
            json!({
                "workspace": ws,
                "goal_atom": { "pred": "status", "args": [{ "Sym": "joren" }, { "Var": "value" }] }
            }),
        ),
        &mut store,
    );
    let rows = result(&queried)["rows"].as_array().expect("rows");
    let values: Vec<&str> = rows
        .iter()
        .filter_map(|r| r["bindings"]["value"].as_str())
        .collect();
    assert!(
        values.contains(&"commons") && values.contains(&"enclosed"),
        "the eval query answers both extensional values, unprojected: {values:?}"
    );
}
