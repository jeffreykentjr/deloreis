//! The authoring seam's read/self-check tools (`AUTH-2`) end-to-end tests: the five `author.*`
//! reads driven through the real [`dispatch`] seam.
//!
//! The acceptance proof (the `end_to_end_self_check_session` test): an agent
//! authors a non-trivial artifact, discovers a defect with the read tools,
//! resolves it, re-checks clean, and materializes — **all through `author.*`
//! tools, with zero raw `.dli` text and zero batch `dli.*` ops.** This test *is*
//! the acceptance proof of the agent interface: the whole author → self-check →
//! fix → materialize loop is closeable through tools alone.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::{Value, json};

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request, Response};
use dli_world::pack::Pack;

// -- harness -----------------------------------------------------------------

fn req(id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: "0.5.1".to_owned(),
        id: id.to_owned(),
        op,
        params,
        expected_digest: None,
        idempotency_key: None,
    }
}

fn send(store: &mut Store, id: &str, op: Op, params: Value) -> Response {
    dispatch(&req(id, op, params), store)
}

/// The `result` of a successful response, or panic with the error.
fn result(response: &Response) -> &Value {
    assert!(response.ok, "expected ok, got error: {:?}", response.error);
    response.result.as_ref().expect("ok carries a result")
}

/// The error code of a failed response.
fn error_code(response: &Response) -> String {
    let body = response.error.as_ref().expect("an error body");
    serde_json::to_value(body.code)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

/// The finding codes in a result's `findings` array, in order.
fn finding_codes(result: &Value) -> Vec<String> {
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

/// A base store with one ratified place, built through the read-complete
/// `with_pack` constructor so the reads evaluate `base + staged`.
fn base_store() -> Store {
    let source = "scope tpth\nratify D-0001\nplace saltmere \"Saltmere\"  ratified D-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "base parses: {diagnostics:?}");
    Store::with_pack(Pack {
        name: "tpth".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles")
}

fn open_scope(store: &mut Store, scope: &str) -> String {
    let response = send(store, "open", Op::AuthorOpen, json!({ "scope": scope }));
    result(&response)["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned()
}

fn proposed() -> Value {
    json!({ "kind": "proposed", "origin": "session-7c9" })
}

fn add_entity(store: &mut Store, ws: &str, id: &str) -> Response {
    send(
        store,
        "add-entity",
        Op::AuthorAddEntity,
        json!({ "workspace": ws, "kind": "person", "id": id, "name": id, "provenance": proposed() }),
    )
}

fn add_relation(store: &mut Store, ws: &str, relation: &str, from: &str, to: &str) -> Response {
    send(
        store,
        "add-relation",
        Op::AuthorAddRelation,
        json!({ "workspace": ws, "relation": relation, "from": from, "to": to, "provenance": proposed() }),
    )
}

// -- the acceptance proof ----------------------------------------------------

/// THE central completeness proof: an agent authors an entity, its trait, and a
/// relation to an initially-unknown entity; uses `author.findings`/`query`/`why`
/// to DISCOVER and understand the `entity.unknown` defect; resolves it by adding
/// the entity; re-checks clean via `author.check`; and materializes — every step
/// an `author.*` tool, no `.dli` text and no `dli.*` op anywhere.
#[test]
#[allow(clippy::too_many_lines)]
fn end_to_end_self_check_session() {
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");

    // 1. Author the artifact: a person, her tongue, and two relations — one to a
    //    known base entity (saltmere), one to an entity not yet introduced.
    add_entity(&mut store, &ws, "mara");
    send(
        &mut store,
        "add-fact",
        Op::AuthorAddFact,
        json!({ "workspace": ws, "entity": "mara", "key": "tongue", "value": "low-saltish", "provenance": proposed() }),
    );
    // A relation to the ratified base place resolves — proof the staged world
    // sees the base's entities.
    let dwells = add_relation(&mut store, &ws, "dwells", "mara", "saltmere");
    assert!(
        !finding_codes(result(&dwells)).contains(&"entity.unknown".to_owned()),
        "a relation to a known base entity raises no unknown-entity finding"
    );
    // A relation to an entity the agent has not introduced stages, but flags.
    let serves = add_relation(&mut store, &ws, "serves", "mara", "the-warden");
    assert!(
        finding_codes(result(&serves)).contains(&"entity.unknown".to_owned()),
        "a relation to an unintroduced entity flags entity.unknown"
    );

    // 2. SELF-CHECK: discover the blocker through the read tools alone.
    //    author.findings — the standing pre-flight of what materialize refuses on.
    let findings = send(
        &mut store,
        "findings",
        Op::AuthorFindings,
        json!({ "workspace": ws }),
    );
    let findings = result(&findings);
    assert_eq!(
        findings["materializable"],
        Value::Bool(false),
        "an open entity.unknown makes the workspace non-materializable"
    );
    let error_codes: Vec<String> = findings["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        error_codes.contains(&"entity.unknown".to_owned()),
        "the blocker is surfaced as an error-severity finding: {error_codes:?}"
    );
    // The repair is typed: the candidate ids the reference plausibly meant.
    let repair = findings["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "entity.unknown")
        .and_then(|f| f.get("repair"))
        .expect("entity.unknown carries a typed repair");
    assert_eq!(repair["kind"], "resolve-entity");

    // author.query — the staged relation is queryable over the in-progress work,
    // even before the reference resolves. A `?` argument enumerates matches.
    let query = send(
        &mut store,
        "query",
        Op::AuthorQuery,
        json!({ "workspace": ws, "goal": "serves(mara, ?who)", "why": true }),
    );
    let rows = result(&query)["rows"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 1, "one staged serves-row over the staged world");
    assert_eq!(rows[0]["atom"], "serves(mara, the-warden)");
    assert_eq!(
        rows[0]["status"], "possible",
        "a proposed claim is possible, never certain"
    );
    assert!(
        rows[0].get("why").is_some(),
        "why:true attaches the support antichain"
    );

    let diff = send(
        &mut store,
        "diff",
        Op::AuthorDiff,
        json!({ "workspace": ws }),
    );
    assert!(
        result(&diff)["consequences"]["newly_possible"]
            .as_array()
            .is_some_and(|changes| {
                changes.iter().any(|change| {
                    change["atom"]["pred"] == "serves"
                        && change["atom"]["args"] == json!(["mara", "the-warden"])
                        && change["before"]["status"] == "dead"
                        && change["after"]["status"] == "possible"
                })
            }),
        "the supervision diff carries staged derived consequences"
    );
    assert!(
        result(&diff)["workspace_digest"].is_string(),
        "the supervision diff carries the workspace fence"
    );

    // author.why — why the staged claim can't be certain yet: it is proposed, so
    // it awaits a ruling (an open hypothesis stands between it and certainty).
    let why = send(
        &mut store,
        "why",
        Op::AuthorWhy,
        json!({ "workspace": ws, "atom": "serves(mara, the-warden)" }),
    );
    let why_not = &result(&why)["why_not"];
    assert_eq!(
        why_not["known"],
        Value::Bool(true),
        "the staged atom is derived"
    );
    assert_eq!(why_not["status"], "possible");
    assert!(
        !why_not["blocking"].as_array().unwrap().is_empty(),
        "an unruled variable blocks certainty"
    );

    // 3. RESOLVE: introduce the missing entity — through a tool, not text.
    add_entity(&mut store, &ws, "the-warden");

    // 4. RE-CHECK clean via author.check: no error-severity findings remain.
    let check = send(
        &mut store,
        "check",
        Op::AuthorCheck,
        json!({ "workspace": ws }),
    );
    let check = result(&check);
    assert_eq!(
        check["ok"],
        Value::Bool(true),
        "after resolving the unknown entity the draft is clean of errors: {:?}",
        finding_codes(check)
    );
    assert!(
        !finding_codes(check).contains(&"entity.unknown".to_owned()),
        "the entity.unknown finding is cleared"
    );

    // author.findings now reports the workspace materializable.
    let findings = send(
        &mut store,
        "findings2",
        Op::AuthorFindings,
        json!({ "workspace": ws }),
    );
    assert_eq!(result(&findings)["materializable"], Value::Bool(true));

    // 5. MATERIALIZE: the loop closes — file bytes as data, no error refusal.
    let materialize = send(
        &mut store,
        "materialize",
        Op::AuthorMaterialize,
        json!({ "workspace": ws }),
    );
    let materialize = result(&materialize);
    assert!(
        finding_codes(materialize).is_empty(),
        "materialize returns no blocking findings"
    );
    let rendered = materialize["rendered"].as_array().unwrap();
    assert!(
        rendered
            .iter()
            .any(|line| line.as_str().unwrap().contains("scope tpth")),
        "materialize returns the canonical scoped surface"
    );
}

// -- per-tool coverage over a staged (uncommitted) world ---------------------

#[test]
fn query_enumerates_over_the_staged_world() {
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");
    add_entity(&mut store, &ws, "mara");
    add_entity(&mut store, &ws, "fen");
    add_relation(&mut store, &ws, "serves", "mara", "saltmere");
    add_relation(&mut store, &ws, "serves", "fen", "saltmere");

    // A variable in the second slot enumerates; a variable in the first slot
    // enumerates the other way — both read the staged relation.
    let by_target = send(
        &mut store,
        "q1",
        Op::AuthorQuery,
        json!({ "workspace": ws, "goal": "serves(?who, saltmere)" }),
    );
    let rows = result(&by_target)["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "two servers of saltmere are staged");
}

#[test]
fn query_certain_layer_hides_proposed_rows() {
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");
    add_entity(&mut store, &ws, "mara");
    add_relation(&mut store, &ws, "serves", "mara", "saltmere");

    // The proposed claim is possible, never certain: the certain floor drops it.
    let certain = send(
        &mut store,
        "q-cert",
        Op::AuthorQuery,
        json!({ "workspace": ws, "goal": "serves(mara, saltmere)", "layer": "certain" }),
    );
    assert!(
        result(&certain)["rows"].as_array().unwrap().is_empty(),
        "no proposed claim survives the certain floor"
    );
    let all = send(
        &mut store,
        "q-all",
        Op::AuthorQuery,
        json!({ "workspace": ws, "goal": "serves(mara, saltmere)" }),
    );
    assert_eq!(result(&all)["rows"].as_array().unwrap().len(), 1);
}

#[test]
fn query_bad_goal_is_a_schema_violation() {
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");
    let response = send(
        &mut store,
        "q-bad",
        Op::AuthorQuery,
        json!({ "workspace": ws, "goal": "not an atom" }),
    );
    assert!(!response.ok);
    assert_eq!(error_code(&response), "schema-violation");
    assert!(
        response
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("pred(args)"),
        "the error message is self-explanatory, not bare"
    );
}

#[test]
fn query_unknown_predicate_is_empty_not_error() {
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");
    let response = send(
        &mut store,
        "q-unknown",
        Op::AuthorQuery,
        json!({ "workspace": ws, "goal": "flies(mara)" }),
    );
    // Asked and answered, nothing matched — a diagnosed absence, not a fault.
    assert!(response.ok);
    assert!(result(&response)["rows"].as_array().unwrap().is_empty());
}

#[test]
fn findings_partitions_by_severity_with_repairs() {
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");
    add_entity(&mut store, &ws, "mara");
    add_relation(&mut store, &ws, "serves", "mara", "the-warden");

    let findings = send(
        &mut store,
        "f",
        Op::AuthorFindings,
        json!({ "workspace": ws }),
    );
    let findings = result(&findings);
    let errors = findings["errors"].as_array().unwrap();
    let warnings = findings["warnings"].as_array().unwrap();
    assert!(
        errors.iter().any(|f| f["code"] == "entity.unknown"),
        "the unknown-entity blocker is an error"
    );
    assert!(
        warnings.iter().any(|f| f["code"] == "name.unvetted"),
        "the unvetted name is a warning, not a blocker"
    );
    assert_eq!(findings["materializable"], Value::Bool(false));
}

#[test]
fn why_reports_unknown_for_an_underived_atom() {
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");
    let why = send(
        &mut store,
        "why-unknown",
        Op::AuthorWhy,
        json!({ "workspace": ws, "atom": "serves(nobody, nowhere)" }),
    );
    let why_not = &result(&why)["why_not"];
    assert_eq!(
        why_not["known"],
        Value::Bool(false),
        "the world derives no such atom"
    );
}

#[test]
fn check_flags_a_contradiction_against_the_base() {
    // The base rules saltmere's `kind` certain; a staged fact setting it otherwise
    // contradicts canon, and author.check surfaces it as an error.
    let source = "scope tpth\nratify D-0001\nplace saltmere \"Saltmere\"  ratified D-0001\nsaltmere.region = tidewater  ratified D-0001\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "base parses: {diagnostics:?}");
    let mut store = Store::with_pack(Pack {
        name: "tpth".to_owned(),
        documents: vec![document],
    })
    .expect("base compiles");
    let ws = open_scope(&mut store, "tpth");
    send(
        &mut store,
        "contradict",
        Op::AuthorAddFact,
        json!({ "workspace": ws, "entity": "saltmere", "key": "region", "value": "highfell", "provenance": proposed() }),
    );

    let check = send(
        &mut store,
        "check",
        Op::AuthorCheck,
        json!({ "workspace": ws }),
    );
    let check = result(&check);
    assert_eq!(
        check["ok"],
        Value::Bool(false),
        "the contradiction bars the draft"
    );
    assert!(
        finding_codes(check)
            .iter()
            .any(|code| code == "fact.contradiction" || code == "factcheck.contradicted"),
        "a contradiction against a certain base fact is surfaced: {:?}",
        finding_codes(check)
    );
}

#[test]
fn list_workspaces_recovers_open_context() {
    let mut store = base_store();
    let scope_ws = open_scope(&mut store, "tpth");
    let fork_response = send(
        &mut store,
        "open-fork",
        Op::AuthorOpen,
        json!({ "fork": "coast-runs-north" }),
    );
    let fork_ws = result(&fork_response)["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    add_entity(&mut store, &scope_ws, "mara");

    let list = send(&mut store, "list", Op::AuthorListWorkspaces, json!({}));
    let workspaces = result(&list)["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 2, "both open workspaces are listed");

    let scope_row = workspaces
        .iter()
        .find(|w| w["workspace"] == scope_ws.as_str())
        .expect("the scope workspace is listed");
    assert_eq!(scope_row["mode"], "scope");
    assert_eq!(scope_row["base"], "tpth");
    assert_eq!(scope_row["staged"], 1, "one declaration is staged");
    assert!(!scope_row["workspace_digest"].as_str().unwrap().is_empty());

    let fork_row = workspaces
        .iter()
        .find(|w| w["workspace"] == fork_ws.as_str())
        .expect("the fork workspace is listed");
    assert_eq!(fork_row["mode"], "fork");
}

#[test]
fn read_tools_reject_an_unknown_workspace() {
    let mut store = base_store();
    for (op, params) in [
        (
            Op::AuthorQuery,
            json!({ "workspace": "ws-none", "goal": "serves(a, b)" }),
        ),
        (Op::AuthorCheck, json!({ "workspace": "ws-none" })),
        (
            Op::AuthorWhy,
            json!({ "workspace": "ws-none", "atom": "serves(a, b)" }),
        ),
        (Op::AuthorFindings, json!({ "workspace": "ws-none" })),
    ] {
        let response = send(&mut store, "missing", op, params);
        assert!(!response.ok, "an unknown workspace is refused for {op:?}");
        assert_eq!(error_code(&response), "unknown-doc");
    }
}

#[test]
fn read_tools_reject_unknown_fields() {
    // H2: an unknown top-level field is a schema-violation, never silently dropped.
    let mut store = base_store();
    let ws = open_scope(&mut store, "tpth");
    let response = send(
        &mut store,
        "unknown-field",
        Op::AuthorFindings,
        json!({ "workspace": ws, "evil": 1 }),
    );
    assert!(!response.ok);
    assert_eq!(error_code(&response), "schema-violation");
}
