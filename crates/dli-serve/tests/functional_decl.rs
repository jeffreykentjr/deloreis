#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 1.6's `author.declare_functional` end to end (issue 49): the
//! declaration reaches the staged world, the FD check reads it, and two sibling
//! *staged* facts on the declared predicate block materialize with
//! `continuity.fd-violation` — a conflict neither add-time guard can see, because
//! neither fact contradicts the base.
//!
//! Also here: the version gate (the op is `unknown-op` below `1.6.0`) and the
//! add-time guard's two issue-49 repairs — a `Term::Str` value is compared
//! against canon like any other ground value, and comparison is on the
//! normalized key, so cosmetic drift is not a contradiction.

use serde_json::{Value, json};

use dli_serve::author::store::Store;
use dli_serve::author::{dispatch, workspace::Mode};
use dli_serve::session::Sessions;
use dli_serve::{Op, Request};

// -- harness -----------------------------------------------------------------

/// Build a request frame at a given contract version.
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

/// The finding codes in a result, in order.
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

/// Compile a base world from `.dli` source. `with_pack` (not `with_base`) so the
/// base's `ratify` declarations stay reachable — a citation resolves against the
/// base *documents*, and a store built from a bare compiled world has none.
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

/// A base declaring the decision `D-0001` and the officer `lyra`, so a staged
/// fact citing `D-0001` reads certain and its subject is known.
fn officer_base() -> Store {
    base("scope storybook\nratify D-0001\nperson lyra \"Lyra\"  ratified D-0001\n")
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

/// Stage `functional <pred>`.
fn declare_functional(store: &mut Store, dli: &str, ws: &str, pred: &str) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "fn",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": pred,
                    "provenance": { "kind": "proposed", "origin": "session-fd" } }),
        ),
        store,
    )
}

/// Stage `<entity>.<key> = <value>` citing the ratified decision `D-0001`, so
/// the staged fact reads certain.
fn add_certain_fact(
    store: &mut Store,
    dli: &str,
    ws: &str,
    entity: &str,
    key: &str,
    value: &str,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "add",
            Op::AuthorAddFact,
            json!({ "workspace": ws, "entity": entity, "key": key, "value": value,
                    "provenance": { "kind": "cited", "decisions": ["D-0001"], "state": "ratified" } }),
        ),
        store,
    )
}

fn check_codes(store: &mut Store, dli: &str, ws: &str) -> Vec<String> {
    let response = dispatch(
        &req(dli, "check", Op::AuthorCheck, json!({ "workspace": ws })),
        store,
    );
    codes(result(&response))
}

// -- the declaration reaches the check ---------------------------------------

#[test]
fn declared_functional_predicate_makes_two_staged_facts_a_violation() {
    // Neither fact contradicts the base — canon has no `rank` at all — so the
    // add-time guard is silent on both. The conflict is between the *siblings*,
    // and only the declaration makes a first-seen predicate judgeable.
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.6.0", "storybook");

    let declared = declare_functional(&mut store, "1.6.0", &ws, "rank");
    assert!(
        codes(result(&declared)).is_empty(),
        "the declaration itself is clean: {:?}",
        codes(result(&declared))
    );

    let first = add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "captain");
    assert!(
        codes(result(&first)).is_empty(),
        "the first rank contradicts nothing: {:?}",
        codes(result(&first))
    );
    let second = add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "admiral");
    assert!(
        !codes(result(&second)).contains(&"fact.contradiction".to_owned()),
        "add-time sees no base conflict — the sibling clash is the check's job: {:?}",
        codes(result(&second))
    );

    assert!(
        check_codes(&mut store, "1.6.0", &ws).contains(&"continuity.fd-violation".to_owned()),
        "author.check surfaces the sibling FD break: {:?}",
        check_codes(&mut store, "1.6.0", &ws)
    );
}

#[test]
fn the_violation_blocks_materialize() {
    // And it is an *error*, so the accept decision is refused rather than
    // committing canon that already breaks its own dependency.
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    declare_functional(&mut store, "1.6.0", &ws, "rank");
    add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "captain");
    add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "admiral");

    let blocked = dispatch(
        &req(
            "1.6.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(!blocked.ok, "materialize is refused: {blocked:?}");
    let error = blocked.error.expect("blocked carries an error");
    assert_eq!(error.code, dli_serve::ErrorCode::MaterializeBlocked);
    let blocking = error.data.expect("blocking data");
    assert_eq!(blocking["blocking"], json!(["continuity.fd-violation"]));
}

#[test]
fn without_the_declaration_the_same_two_staged_facts_pass() {
    // The control: `rank` is first-seen and nothing else witnesses it, so canon
    // cannot tell a slip from a genuine multi-valued relation. Without the
    // declaration the session is clean and materialize succeeds — the declaration
    // is what carries the judgment.
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "captain");
    add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "admiral");

    assert!(
        !check_codes(&mut store, "1.6.0", &ws).contains(&"continuity.fd-violation".to_owned()),
        "undeclared and unwitnessed is not a violation: {:?}",
        check_codes(&mut store, "1.6.0", &ws)
    );
    let materialized = dispatch(
        &req(
            "1.6.0",
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws }),
        ),
        &mut store,
    );
    assert!(materialized.ok, "materialize succeeds: {materialized:?}");
}

#[test]
fn cosmetic_drift_on_a_declared_predicate_is_not_a_violation() {
    // Sub-claim 1 through the wire: `captain` and `Captain` normalize alike, so a
    // restatement is not a second value even under the strictest declaration.
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    declare_functional(&mut store, "1.6.0", &ws, "rank");
    add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "captain");
    add_certain_fact(&mut store, "1.6.0", &ws, "lyra", "rank", "Captain");

    assert!(
        !check_codes(&mut store, "1.6.0", &ws).contains(&"continuity.fd-violation".to_owned()),
        "case drift is one value: {:?}",
        check_codes(&mut store, "1.6.0", &ws)
    );
}

#[test]
fn the_declaration_stages_a_functional_decl() {
    // The staged declaration is a `functional` decl — not silently dropped, and
    // not smuggled in as some other kind.
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    declare_functional(&mut store, "1.6.0", &ws, "rank");
    let workspace = store.get(&ws).expect("workspace is open");
    assert_eq!(workspace.mode, Mode::Scope);
    assert_eq!(workspace.base_name, "storybook");
    assert_eq!(
        workspace.decls.len(),
        1,
        "one staged declaration: {:?}",
        workspace.decls
    );
    assert!(
        matches!(workspace.decls[0], dli_ast::decl::Decl::Functional(_)),
        "staged as a functional decl: {:?}",
        workspace.decls[0]
    );
}

// -- the version gate --------------------------------------------------------

#[test]
fn declare_functional_is_answered_at_contract_1_6_0() {
    // The same frame at `1.6.0` routes to the tool: no `unknown-op`, a real
    // workspace answer.
    let mut sessions = Sessions::new();
    let mut store = officer_base();
    let ws = open_scope(&mut store, "2.0.0", "storybook");
    let response = dli_serve::serve::dispatch(
        &req(
            "2.0.0",
            "fn",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "rank",
                    "provenance": { "kind": "proposed", "origin": "s" } }),
        ),
        &mut sessions,
        &mut store,
    );
    assert!(response.ok, "1.6.0 answers the op: {response:?}");
    assert!(codes(result(&response)).is_empty());
}

// -- the add-time guard's issue-49 repairs ------------------------------------

#[test]
fn a_str_value_contradicting_canon_is_flagged() {
    // The guard used to compare only `Term::Sym` values, so a multiword value —
    // which the author tools render as a `Term::Str` — slipped past the base
    // contradiction check entirely.
    let mut store = base(
        "scope storybook\nratify D-0001\nplace joren \"Joren\"  ratified D-0001\njoren.water-status = tidal  ratified D-0001\n",
    );
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    let response = dispatch(
        &req(
            "1.6.0",
            "add",
            Op::AuthorAddFact,
            json!({ "workspace": ws, "entity": "joren", "key": "water-status", "value": "gone dry",
                    "provenance": { "kind": "proposed", "origin": "s" } }),
        ),
        &mut store,
    );
    assert!(
        codes(result(&response)).contains(&"fact.contradiction".to_owned()),
        "a multiword (Term::Str) value contradicts canon like any other: {:?}",
        codes(result(&response))
    );
}

#[test]
fn a_cosmetically_drifted_value_does_not_contradict_canon() {
    // …and the same comparison is on the normalized key, so restating canon's own
    // value in another casing is a restatement, not a contradiction.
    let mut store = base(
        "scope storybook\nratify D-0001\nplace joren \"Joren\"  ratified D-0001\njoren.water-status = tidal  ratified D-0001\n",
    );
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    let response = dispatch(
        &req(
            "1.6.0",
            "add",
            Op::AuthorAddFact,
            json!({ "workspace": ws, "entity": "joren", "key": "water-status", "value": "Tidal",
                    "provenance": { "kind": "proposed", "origin": "s" } }),
        ),
        &mut store,
    );
    assert!(
        !codes(result(&response)).contains(&"fact.contradiction".to_owned()),
        "`Tidal` is canon's own `tidal`: {:?}",
        codes(result(&response))
    );
}

/// A `functional` declaration citing a decision the ledger does not hold must
/// raise `citation.unknown-decision` like any other cited add — otherwise a
/// constraint takes effect for every later author citing authority that does
/// not exist (a carriage-only decl has no support variable to fail closed on).
#[test]
fn declare_functional_with_an_unknown_citation_is_a_finding() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    let response = dispatch(
        &req(
            "1.6.0",
            "fn",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "rank",
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

/// A control character smuggled into the predicate name must raise
/// `field.control-char` — the raw text lands in the durable carriage body, and a
/// newline there could forge a second declaration line in the review diff.
#[test]
fn declare_functional_control_char_in_pred_is_refused() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.6.0", "storybook");
    let response = dispatch(
        &req(
            "1.6.0",
            "fn",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": "ra\nnk",
                    "provenance": { "kind": "proposed", "origin": "session-fd" } }),
        ),
        &mut store,
    );
    assert!(
        codes(result(&response)).contains(&"field.control-char".to_owned()),
        "a control character in `pred` must be caught: {:?}",
        codes(result(&response))
    );
}
