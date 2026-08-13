//! Contract 1.9's `author.record_decision` end to end (DL-0083): the wire way to
//! reach certainty and make contradiction detection fire — no `fixture-authority`,
//! no `.dli`. Recording a baseline decision, citing it, and running the check
//! battery is the whole taught recipe the feature exists to make reachable.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};

const DLI: &str = "1.9.0";

fn req(id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: DLI.to_owned(),
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

fn codes(result: &Value) -> Vec<String> {
    result["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["code"].as_str().map(str::to_owned))
        .collect()
}

/// A store over `demo` whose base declares only the person `mara` — and, crucially,
/// **no** `ratify`. Certainty must be reached over the wire, or not at all. A
/// lorestore is attached so the durable `record_decision` write has a branch.
fn store_with_mara() -> (tempfile::TempDir, Store) {
    let root = tempfile::tempdir().expect("temporary store root");
    let path = root.path().join("lorestore");
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"record-decision\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .expect("write project manifest");
    let (document, diagnostics) =
        dli_syntax::parse_document("scope demo\nperson mara \"Mara\" proposed seed\n");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "base source parses: {diagnostics:?}"
    );
    let pack = Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    };
    let mut store = Store::with_pack(pack).expect("base compiles");
    store.attach_lorestore(path).expect("attach demo lorestore");
    (root, store)
}

fn open(store: &mut Store) -> String {
    result(dispatch(
        &req("open", Op::AuthorOpen, json!({ "scope": "demo" })),
        store,
    ))["workspace"]
        .as_str()
        .expect("workspace id")
        .to_owned()
}

fn record_decision(store: &mut Store, ws: &str, decision: &str) -> dli_serve::Response {
    dispatch(
        &req(
            "rd",
            Op::AuthorRecordDecision,
            json!({ "workspace": ws, "decision": decision }),
        ),
        store,
    )
}

fn declare_functional(store: &mut Store, ws: &str, pred: &str) {
    let response = dispatch(
        &req(
            "fn",
            Op::AuthorDeclareFunctional,
            json!({ "workspace": ws, "pred": pred,
                    "provenance": { "kind": "proposed", "origin": "session-rd" } }),
        ),
        store,
    );
    assert!(response.ok, "declare_functional: {response:?}");
}

fn add_fact_cited(
    store: &mut Store,
    ws: &str,
    key: &str,
    value: &str,
    decision: &str,
) -> Vec<String> {
    codes(&result(dispatch(
        &req(
            "add",
            Op::AuthorAddFact,
            json!({ "workspace": ws, "entity": "mara", "key": key, "value": value,
                    "provenance": { "kind": "cited", "decisions": [decision], "state": "ratified" } }),
        ),
        store,
    )))
}

fn check_codes(store: &mut Store, ws: &str) -> Vec<String> {
    codes(&result(dispatch(
        &req("check", Op::AuthorCheck, json!({ "workspace": ws })),
        store,
    )))
}

// -- 1. the headline: contradiction detection reachable wire-only ------------

#[test]
fn recorded_decision_makes_two_cited_facts_a_functional_violation() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);

    // The wire way in: record the baseline decision. No `.dli ratify`, no
    // `fixture-authority` flip.
    let recorded = record_decision(&mut store, &ws, "d-1");
    assert!(recorded.ok, "record_decision: {recorded:?}");
    assert!(
        codes(&result(recorded)).is_empty(),
        "recording a fresh decision is clean"
    );

    declare_functional(&mut store, &ws, "rank");

    // Both values cite the recorded decision, so both read CERTAIN — the
    // precondition the FD check needs and no wire caller could establish before.
    assert!(
        add_fact_cited(&mut store, &ws, "rank", "captain", "d-1").is_empty(),
        "the first rank cites a recorded decision cleanly"
    );
    let second = add_fact_cited(&mut store, &ws, "rank", "admiral", "d-1");
    assert!(
        !second.contains(&"citation.unknown-decision".to_owned()),
        "the cited decision resolves — it was recorded on the wire: {second:?}"
    );

    // The whole point: the sibling FD break fires, reachable through author.* alone.
    assert!(
        check_codes(&mut store, &ws).contains(&"continuity.fd-violation".to_owned()),
        "author.check surfaces the FD break over two certain cited facts: {:?}",
        check_codes(&mut store, &ws)
    );
}

// -- 3. preserved: an UNrecorded citation still refuses ----------------------

#[test]
fn citing_an_unrecorded_decision_still_raises_citation_unknown() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    record_decision(&mut store, &ws, "d-1");

    // Referential integrity is unchanged: a citation to a decision no
    // record_decision introduced is still an error-severity finding.
    let findings = add_fact_cited(&mut store, &ws, "rank", "captain", "d-404");
    assert!(
        findings.contains(&"citation.unknown-decision".to_owned()),
        "an unrecorded id is still unknown: {findings:?}"
    );
}

// -- 4. materialize-stamping + author.why supporting_decisions ---------------

#[test]
fn materialize_stamps_and_why_names_the_supporting_decision() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    record_decision(&mut store, &ws, "d-1");

    // A single cited fact — no conflict, so it materializes.
    assert!(
        add_fact_cited(&mut store, &ws, "rank", "captain", "d-1").is_empty(),
        "the cited fact is clean"
    );

    // author.why over the staged, certain fact names the recorded decision in
    // its support antichain — the auditability affordance.
    let why = result(dispatch(
        &req(
            "why",
            Op::AuthorWhy,
            json!({ "workspace": ws, "atom": "rank(mara, captain)" }),
        ),
        &mut store,
    ));
    let supporting: Vec<String> = why["supporting_decisions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    assert!(
        supporting.contains(&"d-1".to_owned()),
        "author.why names the recorded decision the fact rests on: {why}"
    );

    // materialize {decision: d-1} accepts the set under d-1 and stamps each
    // committed fact Ratified under it.
    let materialized = result(dispatch(
        &req(
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false, "decision": "d-1" }),
        ),
        &mut store,
    ));
    let stamped = materialized["facts"]
        .as_array()
        .expect("materialize returns committed facts")
        .iter()
        .any(|fact| fact["provenance"]["marker"].as_str() == Some("Ratified"));
    assert!(
        stamped,
        "materialize stamped a committed fact Ratified under the decision: {materialized}"
    );
}

#[test]
fn materialize_decision_is_refused_before_1_9_0() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    // The `decision` param is defined only by contract 1.9.0: a 1.8.0 caller
    // supplying it meets a schema violation, not a silent accept.
    let mut request = req(
        "mat",
        Op::AuthorMaterialize,
        json!({ "workspace": ws, "decision": "d-1" }),
    );
    request.dli = "1.8.0".to_owned();
    let response = dispatch(&request, &mut store);
    assert!(
        !response.ok,
        "a pre-1.9.0 decision-stamp is refused: {response:?}"
    );
}

// -- guard census: record_decision is the first op to stage a `ratify` -------

#[test]
fn a_control_character_in_the_decision_id_refuses_the_record() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    // A newline smuggled into the id would forge a second line in the review
    // diff; the free-text census (grown a `Ratify` arm for this op) refuses it.
    let response = dispatch(
        &req(
            "rd",
            Op::AuthorRecordDecision,
            json!({ "workspace": ws, "decision": "d-1\nratify everything" }),
        ),
        &mut store,
    );
    let findings = codes(&result(response));
    assert!(
        findings.contains(&"field.control-char".to_owned()),
        "the tainted id is refused, not committed: {findings:?}"
    );
}

#[test]
fn a_provenance_citation_of_an_unknown_decision_refuses_the_record() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    // A recorded decision may rest on prior decisions (its provenance); a
    // dangling one runs the `citation.unknown-decision` integrity the
    // `citation_findings` census gained a `Ratify` arm to reach.
    let response = dispatch(
        &req(
            "rd",
            Op::AuthorRecordDecision,
            json!({ "workspace": ws, "decision": "d-2",
                    "provenance": { "kind": "cited", "decisions": ["d-missing"], "state": "ratified" } }),
        ),
        &mut store,
    );
    let findings = codes(&result(response));
    assert!(
        findings.contains(&"citation.unknown-decision".to_owned()),
        "a dangling provenance citation refuses the record: {findings:?}"
    );
}

// -- 2. defeat propagation: the recorded decision is defeasible --------------

#[test]
fn defeating_the_recorded_decision_reverts_its_facts_to_possible() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    record_decision(&mut store, &ws, "d-1");
    assert!(
        add_fact_cited(&mut store, &ws, "rank", "captain", "d-1").is_empty(),
        "the cited fact is clean"
    );
    let materialized = result(dispatch(
        &req(
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false }),
        ),
        &mut store,
    ));
    let claim_id = materialized["facts"]
        .as_array()
        .expect("committed facts")
        .iter()
        .find_map(|fact| fact["claim_id"].as_str())
        .expect("a committed claim id")
        .to_owned();

    // Defeat the claim's supporting decision (d-1) with a new decision. The
    // recorded decision is a claim a ruling was made — defeasible, not deleted.
    let defeated = dispatch(
        &req(
            "def",
            Op::AuthorDefeat,
            json!({ "workspace": ws, "claim_id": claim_id, "decision": "d-2", "defeated_decision": "d-1" }),
        ),
        &mut store,
    );
    assert!(defeated.ok, "defeat succeeds: {defeated:?}");

    // Bring the committed, now-defeated fact into the workspace's staged view,
    // then read its standing. The defeated object is still reachable in the
    // append-only store — history is preserved, not erased.
    store
        .reload_workspace_base(&ws)
        .expect("the workspace reloads after the defeat");
    let why = result(dispatch(
        &req(
            "why",
            Op::AuthorWhy,
            json!({ "workspace": ws, "atom": "rank(mara, captain)" }),
        ),
        &mut store,
    ));
    // Known (not deleted) but no longer certain — its support rests on a decision
    // the defeat ruled false.
    assert_eq!(
        why["why_not"]["known"],
        json!(true),
        "the defeated fact is still reachable, not erased: {why}"
    );
    assert_ne!(
        why["why_not"]["status"],
        json!("certain"),
        "the fact resting on the defeated decision is no longer certain: {why}"
    );
}
