//! Contract 1.9's `author.record_decision` end to end (DL-0083): the wire way to
//! reach certainty and make contradiction detection fire — no `fixture-authority`,
//! no `.dli`. Recording a baseline decision, citing it, and running the check
//! battery is the whole taught recipe the feature exists to make reachable.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
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

/// Stage a PROPOSED (uncited) attribute fact on `mara` — the shape
/// `materialize {decision}` must lift to certain-under-`d`, and the shape that
/// stays `possible` while merely staged.
fn add_fact_proposed(store: &mut Store, ws: &str, key: &str, value: &str) -> Vec<String> {
    codes(&result(dispatch(
        &req(
            "add",
            Op::AuthorAddFact,
            json!({ "workspace": ws, "entity": "mara", "key": key, "value": value,
                    "provenance": { "kind": "proposed", "origin": "session-rd" } }),
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
            json!({ "workspace": ws, "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "captain"}]} }),
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

// -- 4b. decision conferral participates in the pre-CAS materialize gate ------

/// Two proposed values are individually possible, but accepting both under one
/// decision would make them certain and violate the functional declaration.
/// The exact post-stamp candidate must therefore be checked before the CAS.
#[test]
fn materialize_decision_refuses_the_certain_fd_violation_before_cas() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    declare_functional(&mut store, &ws, "rank");

    // Two uncited proposals — possible while staged, so neither the FD check nor
    // any other error-severity finding bars materialize.
    assert!(
        add_fact_proposed(&mut store, &ws, "rank", "captain").is_empty(),
        "a proposed value stages clean"
    );
    assert!(
        add_fact_proposed(&mut store, &ws, "rank", "admiral").is_empty(),
        "the sibling proposed value stages clean too"
    );

    let head_before = store.workspace_lorestore(&ws).unwrap().root_hash().unwrap();
    let refused = dispatch(
        &req(
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false, "decision": "d-1" }),
        ),
        &mut store,
    );
    assert!(!refused.ok, "the exact certain candidate must refuse");
    let error = refused.error.unwrap();
    assert_eq!(error.code, dli_serve::ErrorCode::MaterializeBlocked);
    assert!(
        error.data.unwrap()["blocking"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "continuity.fd-violation")
    );
    assert_eq!(
        store.workspace_lorestore(&ws).unwrap().root_hash().unwrap(),
        head_before,
        "the invalid stamped candidate never advances the durable ref"
    );
}

// -- 4c. no declaration corruption: a carriage is never stamped ---------------

/// `materialize {decision}` rests only FACTS on the decision. A declaration
/// carriage (here the `functional` declaration) is machinery — it must come back
/// with NO `standing_decision` and NO false `Ratified`, or a later
/// reconstruction would read a declaration as a ratified claim. The fact staged
/// alongside it is the positive control: it DID get accepted under d-1.
#[test]
fn materialize_decision_never_stamps_a_declaration_carriage() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    declare_functional(&mut store, &ws, "rank");
    assert!(
        add_fact_proposed(&mut store, &ws, "rank", "captain").is_empty(),
        "a proposed value stages clean"
    );

    let materialized = result(dispatch(
        &req(
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false, "decision": "d-1" }),
        ),
        &mut store,
    ));
    let facts = materialized["facts"]
        .as_array()
        .expect("materialize returns committed facts");

    // The carriage object is the one carrying an `expert_body`.
    let carriage = facts
        .iter()
        .find(|fact| !fact["expert_body"].is_null())
        .expect("the functional declaration was committed as a carriage");
    assert_ne!(
        carriage["provenance"]["standing_decision"],
        json!("d-1"),
        "a declaration carriage does not rest on a standing decision: {carriage}"
    );
    assert_ne!(
        carriage["provenance"]["marker"],
        json!("Ratified"),
        "a declaration carriage is not falsely ratified under the decision: {carriage}"
    );

    // The fact itself DID get accepted under d-1 — the conferral applies to
    // claims, not machinery.
    let fact = facts
        .iter()
        .find(|fact| fact["expert_body"].is_null())
        .expect("the proposed value was committed as a fact");
    assert_eq!(
        fact["provenance"]["standing_decision"],
        json!("d-1"),
        "the accepted fact records the promoting decision: {fact}"
    );
    assert_eq!(
        fact["provenance"]["marker"],
        json!("Ratified"),
        "the accepted fact reads ratified under the decision: {fact}"
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

fn materialize_rank_and_control(store: &mut Store, ws: &str) -> String {
    assert!(
        add_fact_proposed(store, ws, "rank", "captain").is_empty(),
        "the proposed fact is clean"
    );
    let materialized = result(dispatch(
        &req(
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false, "decision": "d-1" }),
        ),
        store,
    ));
    let claim_id = materialized["facts"]
        .as_array()
        .expect("committed facts")
        .iter()
        .find(|fact| {
            fact["atom"]["pred"] == "rank" && fact["atom"]["args"] == json!(["mara", "captain"])
        })
        .and_then(|fact| fact["claim_id"].as_str())
        .expect("a committed claim id")
        .to_owned();

    record_decision(store, ws, "d-independent");
    assert!(
        add_fact_cited(store, ws, "title", "commander", "d-independent").is_empty(),
        "the independent control is clean"
    );
    result(dispatch(
        &req(
            "mat-independent",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false }),
        ),
        store,
    ));
    claim_id
}

fn assert_persisted_compile_skew(root: &std::path::Path) {
    // Mutation tooth: this fixture is useful only when the persisted and
    // independently recompiled positions genuinely differ. Directly copying
    // the numeric overlay would therefore target another name.
    let durable =
        dli_lorestore::Lorestore::open(root.join("lorestore/demo")).expect("open durable store");
    let head = durable
        .root_hash()
        .expect("read durable head")
        .expect("head exists");
    let durable_names = durable.interner_at(&head).expect("durable interner");
    let recompiled = durable.compile_at(&head).expect("recompiled world");
    let durable_pos = durable_names
        .iter()
        .position(|name| name == "d-1")
        .expect("durable decision position");
    let compiled_pos = recompiled
        .model
        .interner()
        .names()
        .iter()
        .position(|name| name == "d-1")
        .expect("compiled decision position");
    assert_ne!(
        durable_pos, compiled_pos,
        "the regression must exercise cross-interner skew"
    );
}

fn defeat_rank(store: &mut Store, ws: &str, claim_id: &str) {
    let defeated = dispatch(
        &req(
            "def",
            Op::AuthorDefeat,
            json!({ "workspace": ws, "claim_id": claim_id, "decision": "d-2", "defeated_decision": "d-1" }),
        ),
        store,
    );
    assert!(defeated.ok, "defeat succeeds: {defeated:?}");
}

fn assert_live_defeat_and_control(store: &mut Store, ws: &str) {
    // Bring the committed, now-defeated fact into the workspace's staged view,
    // then read its standing. The defeated object is still reachable in the
    // append-only store — history is preserved, not erased.
    store
        .reload_workspace_base(ws)
        .expect("the workspace reloads after the defeat");
    let why = result(dispatch(
        &req(
            "why",
            Op::AuthorWhy,
            json!({ "workspace": ws, "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "captain"}]} }),
        ),
        store,
    ));
    // Known (not deleted) but dead — its sole durable support rests on a
    // decision the defeat ruled false. Materialize reconciles the original
    // proposed declaration into that committed object, so no duplicate staged
    // proposal survives to make the fact merely possible.
    assert_eq!(
        why["why_not"]["known"],
        json!(true),
        "the defeated fact is still reachable, not erased: {why}"
    );
    assert_eq!(
        why["why_not"]["status"],
        json!("dead"),
        "the false named decision removes the reconciled durable support: {why}"
    );
    let independent = result(dispatch(
        &req(
            "why-independent",
            Op::AuthorWhy,
            json!({ "workspace": ws, "goal_atom": {"pred": "title", "args": [{"Sym": "mara"}, {"Sym": "commander"}]} }),
        ),
        store,
    ));
    assert_eq!(
        independent["why_not"]["status"], "certain",
        "name remapping must not falsify the atom occupying the durable position: {independent}"
    );
}

fn session_query(
    sessions: &mut Sessions,
    author: &mut Store,
    id: &str,
    pred: &str,
    value: &str,
) -> dli_serve::Response {
    let frame = json!({
        "dli": "1.9.0", "id": id, "op": "dli.query",
        "params": {
            "document": "demo",
            "goal_atom": {"pred": pred, "args": [{"Sym": "mara"}, {"Sym": value}]},
            "layer": "all"
        }
    });
    respond(
        &serde_json::to_string(&frame).expect("serialize query"),
        sessions,
        author,
    )
    .expect("query answers")
}

fn assert_reopened_defeat_and_control(root: &std::path::Path) {
    // Reopen through the project/session path. This is a second independently
    // compiled receiver and used to repeat the same positional corruption.
    let project = Project::load(root).expect("reload store-backed project");
    let mut sessions = Sessions::from_project(Arc::new(project));
    let mut reopened_author = Store::default();
    let reopened_defeated = session_query(
        &mut sessions,
        &mut reopened_author,
        "reopen-rank",
        "rank",
        "captain",
    );
    assert_eq!(
        reopened_defeated.result.expect("query result")["rows"],
        json!([]),
        "the reopened session excludes the defeated row"
    );
    let reopened_independent = session_query(
        &mut sessions,
        &mut reopened_author,
        "reopen-title",
        "title",
        "commander",
    );
    assert_eq!(
        reopened_independent.result.expect("query result")["rows"][0]["status"],
        "certain"
    );
}

#[test]
fn defeating_the_recorded_decision_leaves_reconciled_facts_dead() {
    let (root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let claim_id = materialize_rank_and_control(&mut store, &ws);
    assert_persisted_compile_skew(root.path());
    defeat_rank(&mut store, &ws, &claim_id);
    assert_live_defeat_and_control(&mut store, &ws);
    assert_reopened_defeat_and_control(root.path());
}

#[test]
fn project_load_refuses_an_out_of_range_persisted_valuation_atom() {
    let (root, _store) = store_with_mara();
    let durable = dli_lorestore::Lorestore::open(root.path().join("lorestore/demo"))
        .expect("open durable store");
    let current = durable
        .branch_ref(dli_lorestore::MAIN_REF)
        .expect("read main ref")
        .expect("main exists");
    let expected = current.digest().expect("digest current ref").to_string();
    let mut corrupt = current.valuation.clone();
    corrupt.set(dli_core::Atom(u32::MAX), false);
    let replacement = dli_lorestore::Ref::with_valuation(
        dli_lorestore::branch_ref_name(dli_lorestore::MAIN_REF),
        current.commit,
        corrupt,
    );
    assert!(
        durable
            .cas_branch_ref(dli_lorestore::MAIN_REF, Some(&expected), &replacement)
            .expect("write corrupt test ref")
    );

    let error = Project::load(root.path()).expect_err("corrupt valuation must fail closed");
    assert!(
        error.to_string().contains("outside its source interner"),
        "typed remap refusal names the corruption: {error}"
    );
}

fn corrupt_main_ref(durable: &dli_lorestore::Lorestore) -> String {
    let current = durable
        .branch_ref(dli_lorestore::MAIN_REF)
        .expect("read main ref")
        .expect("main exists");
    let expected = current.digest().expect("digest current ref").to_string();
    let mut corrupt = current.valuation.clone();
    corrupt.set(dli_core::Atom(u32::MAX), false);
    let replacement = dli_lorestore::Ref::with_valuation(
        dli_lorestore::branch_ref_name(dli_lorestore::MAIN_REF),
        current.commit,
        corrupt,
    );
    assert!(
        durable
            .cas_branch_ref(dli_lorestore::MAIN_REF, Some(&expected), &replacement)
            .expect("write corrupt test ref")
    );
    replacement
        .digest()
        .expect("corrupt ref digest")
        .to_string()
}

fn file_count(path: &std::path::Path) -> usize {
    std::fs::read_dir(path)
        .expect("read store directory")
        .map(|entry| entry.expect("directory entry").path())
        .map(|path| if path.is_dir() { file_count(&path) } else { 1 })
        .sum()
}

#[test]
fn corrupt_ref_preflight_blocks_record_decision_before_any_write() {
    let (root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let durable = dli_lorestore::Lorestore::open(root.path().join("lorestore/demo"))
        .expect("open durable store");
    let corrupt_digest = corrupt_main_ref(&durable);
    let workspace_before = {
        let workspace = store.get(&ws).expect("workspace");
        (
            workspace.digest.clone(),
            workspace.base_root_hash.clone(),
            workspace.base_ref_digest.clone(),
            workspace.base_valuation.clone(),
        )
    };
    let files_before = file_count(root.path().join("lorestore/demo").as_path());

    let response = record_decision(&mut store, &ws, "d-never-written");
    assert!(!response.ok, "corrupt ref is refused: {response:?}");
    assert_eq!(
        response.error.expect("typed refusal").code,
        dli_serve::ErrorCode::EvidenceCorrupt
    );
    assert_eq!(
        durable
            .branch_ref(dli_lorestore::MAIN_REF)
            .expect("read main")
            .expect("main exists")
            .digest()
            .expect("digest")
            .to_string(),
        corrupt_digest,
        "record_decision must not advance the corrupt ref"
    );
    let workspace_after = store.get(&ws).expect("workspace");
    assert_eq!(
        workspace_before,
        (
            workspace_after.digest.clone(),
            workspace_after.base_root_hash.clone(),
            workspace_after.base_ref_digest.clone(),
            workspace_after.base_valuation.clone(),
        ),
        "record_decision must not advance workspace state"
    );
    assert_eq!(
        file_count(root.path().join("lorestore/demo").as_path()),
        files_before,
        "preflight runs before writing even an orphan object"
    );
}

#[test]
fn corrupt_ref_preflight_blocks_defeat_before_any_write() {
    let (root, mut store) = store_with_mara();
    let ws = open(&mut store);
    assert!(add_fact_proposed(&mut store, &ws, "rank", "captain").is_empty());
    let materialized = result(dispatch(
        &req(
            "mat",
            Op::AuthorMaterialize,
            json!({ "workspace": ws, "rendered": false, "decision": "d-1" }),
        ),
        &mut store,
    ));
    let claim_id = materialized["facts"]
        .as_array()
        .expect("facts")
        .iter()
        .find(|fact| fact["atom"]["pred"] == "rank")
        .and_then(|fact| fact["claim_id"].as_str())
        .expect("rank claim")
        .to_owned();
    let durable = dli_lorestore::Lorestore::open(root.path().join("lorestore/demo"))
        .expect("open durable store");
    let corrupt_digest = corrupt_main_ref(&durable);
    let workspace_before = {
        let workspace = store.get(&ws).expect("workspace");
        (
            workspace.digest.clone(),
            workspace.base_root_hash.clone(),
            workspace.base_ref_digest.clone(),
            workspace.base_valuation.clone(),
        )
    };
    let files_before = file_count(root.path().join("lorestore/demo").as_path());

    let response = dispatch(
        &req(
            "defeat-corrupt",
            Op::AuthorDefeat,
            json!({ "workspace": ws, "claim_id": claim_id,
                    "decision": "d-2", "defeated_decision": "d-1" }),
        ),
        &mut store,
    );
    assert!(!response.ok, "corrupt ref is refused: {response:?}");
    assert_eq!(
        response.error.expect("typed refusal").code,
        dli_serve::ErrorCode::EvidenceCorrupt
    );
    assert_eq!(
        durable
            .branch_ref(dli_lorestore::MAIN_REF)
            .expect("read main")
            .expect("main exists")
            .digest()
            .expect("digest")
            .to_string(),
        corrupt_digest,
        "defeat must not advance the corrupt ref"
    );
    let workspace_after = store.get(&ws).expect("workspace");
    assert_eq!(
        workspace_before,
        (
            workspace_after.digest.clone(),
            workspace_after.base_root_hash.clone(),
            workspace_after.base_ref_digest.clone(),
            workspace_after.base_valuation.clone(),
        ),
        "defeat must not advance workspace state"
    );
    assert_eq!(
        file_count(root.path().join("lorestore/demo").as_path()),
        files_before,
        "preflight runs before writing even an orphan object"
    );
}

#[test]
fn new_workspace_observes_a_recorded_decision_without_restart() {
    let (_root, mut store) = store_with_mara();
    let writer = open(&mut store);
    let old = open(&mut store);
    let recorded = record_decision(&mut store, &writer, "d-new");
    assert!(recorded.ok, "{recorded:?}");
    let fresh = open(&mut store);
    let new_codes = add_fact_cited(&mut store, &fresh, "rank", "captain", "d-new");
    assert!(
        !new_codes.contains(&"citation.unknown-decision".to_owned()),
        "{new_codes:?}"
    );
    let old_codes = add_fact_cited(&mut store, &old, "rank", "captain", "d-new");
    assert!(
        old_codes.contains(&"citation.unknown-decision".to_owned()),
        "{old_codes:?}"
    );
}
