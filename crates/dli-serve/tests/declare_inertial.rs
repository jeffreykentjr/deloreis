#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 1.8's `author.declare_inertial` (`docs/decisions.md` DL-0081).
//! Mirrors `crates/dli-serve/tests/functional_decl.rs`'s harness.
//!
//! Two layers: the Phase-1 plumbing coverage (op reachability, staging, and the
//! four guard-census arms `decl_kind` / `free_text_fields` / `decl_annotation` /
//! `sanitize_decl` a missing wildcard match would silently skip), and the
//! Phase-2 behavior coverage — the three outcomes (transition / contested /
//! clash) reached wire-only via `declare_inertial` → `add_state` → `check` →
//! `materialize`.

use serde_json::{Value, json};

use dli_serve::author::store::Store;
use dli_serve::author::{dispatch, workspace::Mode};
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
        name: "tpth".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles")
}

/// A base declaring the decision `D-0001` and the place `gwern`, so a staged
/// declaration citing `D-0001` reads certain.
fn gwern_base() -> Store {
    base("scope tpth\nratify D-0001\nplace gwern \"Gwern\"  ratified D-0001\n")
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

/// Stage `inertial <pred>`.
fn declare_inertial(store: &mut Store, dli: &str, ws: &str, pred: &str) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "in",
            Op::AuthorDeclareInertial,
            json!({ "workspace": ws, "pred": pred,
                    "provenance": { "kind": "proposed", "origin": "session-inertia" } }),
        ),
        store,
    )
}

// -- op reachability -----------------------------------------------------------

#[test]
fn declare_inertial_is_unknown_before_contract_1_8_0() {
    // The op is advertised and answered only from `1.8.0` up — every earlier
    // accepted version, including 1.7.0 (the wire-surface-completion campaign
    // window), keeps the surface it shipped with.
    for dli in ["0.5.1", "1.0.0", "1.6.0", "1.7.0"] {
        let mut sessions = Sessions::new();
        let mut store = Store::default();
        let response = dli_serve::serve::dispatch(
            &req(
                dli,
                "in",
                Op::AuthorDeclareInertial,
                json!({ "workspace": "ws-1", "pred": "water-status",
                        "provenance": { "kind": "proposed", "origin": "s" } }),
            ),
            &mut sessions,
            &mut store,
        );
        assert!(!response.ok, "{dli} must refuse the op: {response:?}");
        let error = response.error.expect("refusal carries an error");
        assert_eq!(
            error.code,
            dli_serve::ErrorCode::UnknownOp,
            "{dli} refuses as unknown-op"
        );
        assert!(
            error.message.contains("1.8.0"),
            "{dli}'s refusal names the version that introduced it: {}",
            error.message
        );
    }
}

#[test]
fn declare_inertial_is_answered_at_contract_1_8_0() {
    let mut sessions = Sessions::new();
    let mut store = gwern_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    let response = dli_serve::serve::dispatch(
        &req(
            "1.8.0",
            "in",
            Op::AuthorDeclareInertial,
            json!({ "workspace": ws, "pred": "water-status",
                    "provenance": { "kind": "proposed", "origin": "s" } }),
        ),
        &mut sessions,
        &mut store,
    );
    assert!(response.ok, "1.8.0 answers the op: {response:?}");
    assert!(codes(result(&response)).is_empty());
}

#[test]
fn the_1_8_0_capability_list_carries_declare_inertial_not_1_7_0() {
    let mut sessions = Sessions::new();
    let mut store = Store::default();

    let at_1_7_0 = dli_serve::serve::dispatch(
        &req("1.7.0", "v", Op::Version, json!({})),
        &mut sessions,
        &mut store,
    );
    let caps_1_7_0 = result(&at_1_7_0)["capabilities"]
        .as_array()
        .expect("capabilities");
    assert!(
        !caps_1_7_0.contains(&json!("author.declare_inertial")),
        "1.7.0 must not advertise the 1.8.0 op: {caps_1_7_0:?}"
    );

    let at_1_8_0 = dli_serve::serve::dispatch(
        &req("1.8.0", "v", Op::Version, json!({})),
        &mut sessions,
        &mut store,
    );
    let caps_1_8_0 = result(&at_1_8_0)["capabilities"]
        .as_array()
        .expect("capabilities");
    assert!(
        caps_1_8_0.contains(&json!("author.declare_inertial")),
        "1.8.0 advertises the new op: {caps_1_8_0:?}"
    );
}

// -- the staged declaration ----------------------------------------------------

#[test]
fn the_declaration_stages_an_inertial_decl() {
    let mut store = gwern_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    declare_inertial(&mut store, "1.8.0", &ws, "water-status");
    let workspace = store.get(&ws).expect("workspace is open");
    assert_eq!(workspace.mode, Mode::Scope);
    assert_eq!(workspace.base_name, "tpth");
    assert_eq!(
        workspace.decls.len(),
        1,
        "one staged declaration: {:?}",
        workspace.decls
    );
    assert!(
        matches!(workspace.decls[0], dli_ast::decl::Decl::Inertial(_)),
        "staged as an inertial decl: {:?}",
        workspace.decls[0]
    );
}

#[test]
fn declaring_inertial_alone_raises_no_findings() {
    let mut store = gwern_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    let declared = declare_inertial(&mut store, "1.8.0", &ws, "water-status");
    assert!(
        codes(result(&declared)).is_empty(),
        "the declaration itself is clean: {:?}",
        codes(result(&declared))
    );
    let materialized = dispatch(
        &req(
            "1.8.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(materialized.ok, "materialize succeeds: {materialized:?}");
}

// -- guard-census: the four wildcarded arms -----------------------------------
//
// Each of these tests would FAIL if the corresponding arm in
// `crates/dli-serve/src/author/tools.rs` (`decl_kind`, `free_text_fields`,
// `decl_annotation`) or `crates/dli-serve/src/author/render.rs`
// (`sanitize_decl`) were missing for `Decl::Inertial` — every one of those
// four match sites has a `_`/`other` wildcard, so a missing arm compiles
// clean and silently skips the guard (plan B.3, user pin).

/// `decl_kind`: the staged declaration reports kind `"inertial"` in
/// `author.status`'s structural rows.
#[test]
fn the_staged_decl_reports_kind_inertial() {
    let mut store = gwern_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    declare_inertial(&mut store, "1.8.0", &ws, "water-status");
    let status = dispatch(
        &req("1.8.0", "st", Op::AuthorStatus, json!({ "workspace": ws })),
        &mut store,
    );
    let staged = result(&status)["staged"]
        .as_array()
        .expect("staged rows (contract >= 1.3.0)");
    assert_eq!(staged.len(), 1, "one staged row: {staged:?}");
    assert_eq!(
        staged[0]["kind"],
        json!("inertial"),
        "the row names the decl's kind: {staged:?}"
    );
}

/// `free_text_fields`: a control character smuggled into the predicate name
/// must raise `field.control-char` — the raw text lands in the durable
/// carriage body, and a newline there could forge a second declaration line
/// in the review diff.
#[test]
fn a_control_char_in_pred_is_refused() {
    let mut store = gwern_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    let response = declare_inertial(&mut store, "1.8.0", &ws, "water\nstatus");
    assert!(
        codes(result(&response)).contains(&"field.control-char".to_owned()),
        "a control character in `pred` must be caught: {:?}",
        codes(result(&response))
    );
}

/// `decl_annotation`: a citation to a decision the ledger does not declare
/// must raise `citation.unknown-decision` like any other cited add —
/// otherwise a carriage-only decl's provenance escapes the referential-
/// integrity check every other declaration is held to.
#[test]
fn a_cited_unknown_decision_raises_a_finding() {
    let mut store = gwern_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    let response = dispatch(
        &req(
            "1.8.0",
            "in",
            Op::AuthorDeclareInertial,
            json!({ "workspace": ws, "pred": "water-status",
                    "provenance": { "kind": "cited", "decisions": ["D-9999"], "state": "ratified" } }),
        ),
        &mut store,
    );
    assert!(
        codes(result(&response)).contains(&"citation.unknown-decision".to_owned()),
        "an unknown cited decision must be caught: {:?}",
        codes(result(&response))
    );
}

/// `sanitize_decl`: even though the control character above is caught as an
/// error-severity finding, the mutation still stages the declaration (the
/// ordinary stage-then-block-materialize shape) and its rendered surface
/// must be scrubbed — one line out, no raw newline able to forge a second
/// line in the review diff (`author.diff`, the supervision surface).
#[test]
fn a_tainted_decl_is_scrubbed_on_render() {
    let mut store = gwern_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    let added = declare_inertial(&mut store, "1.8.0", &ws, "water\nstatus");
    let added = result(&added);
    let rendered = added["rendered"].as_array().expect("rendered");
    assert_eq!(rendered.len(), 1, "one rendered line: {rendered:?}");
    assert!(
        !rendered[0].as_str().unwrap_or_default().contains('\n'),
        "the control character must not survive into the rendered line: {rendered:?}"
    );

    // The supervision surface (author.diff) is scrubbed the same way.
    let diff = dispatch(
        &req("1.8.0", "d", Op::AuthorDiff, json!({ "workspace": ws })),
        &mut store,
    );
    let diff_rendered = result(&diff)["rendered"].as_array().expect("rendered");
    assert_eq!(diff_rendered.len(), 1, "one diff line: {diff_rendered:?}");
    assert!(
        !diff_rendered[0].as_str().unwrap_or_default().contains('\n'),
        "the diff surface is scrubbed too: {diff_rendered:?}"
    );
}

// -- Phase 2: the three outcomes, reached wire-only ---------------------------

/// A base declaring `d-1`, two events `dawn`/`noon`, and `dawn before noon`, so
/// a staged state citing `d-1` reads certain and its `since` anchor resolves.
fn weir_base() -> Store {
    base(concat!(
        "scope tpth\n",
        "ratify d-1\n",
        "person gwern \"Gwern\" decided d-1\n",
        "event dawn decided d-1\n",
        "event noon decided d-1\n",
        "before dawn -> noon decided d-1\n",
    ))
}

/// Stage `<subject>.status = <value> since <point>` citing `d-1` (certain).
fn add_status(store: &mut Store, ws: &str, value: &str, point: &str) -> dli_serve::Response {
    dispatch(
        &req(
            "1.8.0",
            "st",
            Op::AuthorAddState,
            json!({
                "workspace": ws,
                "id": "gwern.status",
                "value": value,
                "time": { "kind": "since", "point": point },
                "provenance": { "kind": "cited", "decisions": ["d-1"], "state": "ratified" }
            }),
        ),
        store,
    )
}

fn cited_ratified() -> Value {
    json!({ "kind": "cited", "decisions": ["d-1"], "state": "ratified" })
}

/// Declare `status` inertial citing `d-1` (so the declaration reads certain and
/// capping is active).
fn declare_status_inertial(store: &mut Store, ws: &str) -> dli_serve::Response {
    dispatch(
        &req(
            "1.8.0",
            "in",
            Op::AuthorDeclareInertial,
            json!({ "workspace": ws, "pred": "status", "provenance": cited_ratified() }),
        ),
        store,
    )
}

fn check_codes(store: &mut Store, ws: &str) -> Vec<String> {
    codes(result(&dispatch(
        &req("1.8.0", "ck", Op::AuthorCheck, json!({ "workspace": ws })),
        store,
    )))
}

/// Transition (silent): `commons since dawn` then `enclosed since noon`
/// (dawn before noon) on an inertial slot raises no continuity finding, and
/// `author.materialize` accepts it.
#[test]
fn wire_inertial_ordered_transition_is_silent_and_materializes() {
    let mut store = weir_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    declare_status_inertial(&mut store, &ws);
    add_status(&mut store, &ws, "commons", "dawn");
    add_status(&mut store, &ws, "enclosed", "noon");
    let codes = check_codes(&mut store, &ws);
    assert!(
        !codes.iter().any(|c| c.starts_with("continuity.fluent")),
        "an ordered inertial transition is silent: {codes:?}"
    );
    let materialized = dispatch(
        &req(
            "1.8.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(
        materialized.ok,
        "materialize accepts the transition: {materialized:?}"
    );
}

/// Clash (error): two distinct values `since dawn` (the same point) certainly
/// co-hold — `author.check` reports `continuity.fluent-contradiction` and
/// `author.materialize` refuses.
#[test]
fn wire_inertial_same_point_clash_blocks_materialize() {
    let mut store = weir_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    declare_status_inertial(&mut store, &ws);
    add_status(&mut store, &ws, "commons", "dawn");
    add_status(&mut store, &ws, "enclosed", "dawn");
    assert!(
        check_codes(&mut store, &ws).contains(&"continuity.fluent-contradiction".to_owned()),
        "same-point distinct values clash on the wire"
    );
    let materialized = dispatch(
        &req(
            "1.8.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(
        !materialized.ok,
        "the clash bars materialize: {materialized:?}"
    );
}

/// Contested (warn, non-blocking): two distinct values whose `since` anchors
/// are unordered warn `continuity.fluent-order-undeclared`, but the warn does
/// not block `author.materialize`.
#[test]
fn wire_inertial_unordered_transition_warns_but_materializes() {
    // A base whose two events are unordered (no `before`).
    let mut store = base(concat!(
        "scope tpth\n",
        "ratify d-1\n",
        "person gwern \"Gwern\" decided d-1\n",
        "event dawn decided d-1\n",
        "event noon decided d-1\n",
    ));
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    declare_status_inertial(&mut store, &ws);
    add_status(&mut store, &ws, "commons", "dawn");
    add_status(&mut store, &ws, "enclosed", "noon");
    let codes = check_codes(&mut store, &ws);
    assert!(
        codes.contains(&"continuity.fluent-order-undeclared".to_owned()),
        "unordered anchors are contested: {codes:?}"
    );
    assert!(
        !codes.contains(&"continuity.fluent-contradiction".to_owned()),
        "a contest is never a clash: {codes:?}"
    );
    let materialized = dispatch(
        &req(
            "1.8.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(
        materialized.ok,
        "a warn does not block materialize: {materialized:?}"
    );
}

/// The eval query path is provably untouched (plan B.0 hard invariant): derived
/// inertial boundaries never leak into `author.query` answers — the query still
/// returns the raw extensional values, both of them.
#[test]
fn wire_inertial_query_returns_extensional_values() {
    let mut store = weir_base();
    let ws = open_scope(&mut store, "1.8.0", "tpth");
    declare_status_inertial(&mut store, &ws);
    add_status(&mut store, &ws, "commons", "dawn");
    add_status(&mut store, &ws, "enclosed", "noon");
    let queried = dispatch(
        &req(
            "1.8.0",
            "q",
            Op::AuthorQuery,
            json!({ "workspace": ws, "goal": "status(gwern, ?value)" }),
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
