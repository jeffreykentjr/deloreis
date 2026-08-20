//! Deterministic acceptance oracles for GH #107's request-side compile cache.

#![allow(clippy::expect_used, clippy::similar_names, clippy::unwrap_used)]

use dli_serve::author::store::Store;
use dli_serve::author::workspace::CompileCacheStats;
use dli_serve::author::{dispatch, workspace::Mode};
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};

fn request(id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: "1.13.0".to_owned(),
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

fn fixture() -> (tempfile::TempDir, Store, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope demo\nratify d1\nperson mara \"Mara\" ratified d1\nfact mara rank gold ratified d1\n",
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "fixture parses: {diagnostics:?}"
    );
    let mut store = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let path = root.path().join("lorestore/demo");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    (root, store, path)
}

fn open(store: &mut Store) -> String {
    result(dispatch(
        &request("open", Op::AuthorOpen, json!({"scope": "demo"})),
        store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn open_main_branch(store: &mut Store) -> String {
    result(dispatch(
        &request(
            "fresh-main",
            Op::AuthorOpen,
            json!({"scope": "demo", "branch": "main"}),
        ),
        store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn query(store: &mut Store, workspace: &str, id: &str) -> Value {
    result(dispatch(
        &request(
            id,
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]},
                "layer": "all",
                "why": true
            }),
        ),
        store,
    ))
}

fn why(store: &mut Store, workspace: &str, id: &str) -> Value {
    result(dispatch(
        &request(
            id,
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]}
            }),
        ),
        store,
    ))
}

fn query_value(store: &mut Store, workspace: &str, id: &str, key: &str, value: &str) -> Value {
    result(dispatch(
        &request(
            id,
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": key, "args": [{"Sym": "mara"}, {"Sym": value}]},
                "layer": "all",
                "why": true
            }),
        ),
        store,
    ))
}

fn why_value(store: &mut Store, workspace: &str, id: &str, key: &str, value: &str) -> Value {
    result(dispatch(
        &request(
            id,
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": key, "args": [{"Sym": "mara"}, {"Sym": value}]}
            }),
        ),
        store,
    ))
}

fn claim_id(store: &dli_lorestore::Lorestore, ratify: bool, name: &str) -> String {
    let head = store.root_hash().unwrap().unwrap();
    store
        .fact_hashes_from(&head)
        .unwrap()
        .into_iter()
        .find_map(|hash| {
            let object = store
                .read_object::<dli_lorestore::FactObject>(&hash)
                .unwrap()?;
            if ratify {
                object
                    .expert_body
                    .as_ref()
                    .is_some_and(|body| {
                        body.kind == dli_lorestore::ExpertDeclKind::Ratify && body.name == name
                    })
                    .then_some(object.claim_id.0)
            } else {
                (object.atom.pred == "rank"
                    && object.atom.args.first().is_some_and(|arg| arg == name))
                .then_some(object.claim_id.0)
            }
        })
        .unwrap()
}

fn defeat(
    store: &mut Store,
    workspace: &str,
    id: &str,
    claim_id: &str,
    attacker: &str,
    target: &str,
) {
    result(dispatch(
        &request(
            id,
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": claim_id,
                "decision": attacker,
                "defeated_decision": target
            }),
        ),
        store,
    ));
}

#[test]
fn direct_defeat_and_nested_restoration_reuse_one_symbolic_compile() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let workspace = open(&mut store);
    let d1_claim = claim_id(&lorestore, false, "mara");

    let live_query = query(&mut store, &workspace, "q-live");
    let live_why = why(&mut store, &workspace, "why-live");
    assert_eq!(live_query["rows"][0]["status"], "certain");
    assert_eq!(
        live_why["active_supporting_decision_paths"],
        json!([["d1"]])
    );
    assert_eq!(
        store.get(&workspace).unwrap().compile_cache_stats(),
        CompileCacheStats {
            compiles: 1,
            hits: 1,
            misses: 1,
            evictions: 0,
        }
    );

    defeat(&mut store, &workspace, "defeat-d1", &d1_claim, "d2", "d1");
    let blocked_query = query(&mut store, &workspace, "q-blocked");
    let blocked_why = why(&mut store, &workspace, "why-blocked");
    assert!(blocked_query["rows"].as_array().unwrap().is_empty());
    assert_eq!(
        blocked_why["blocking_defeats"],
        json!([{"defeated_decision":"d1", "defeating_decision":"d2"}])
    );

    let d2_claim = claim_id(&lorestore, true, "d2");
    defeat(&mut store, &workspace, "defeat-d2", &d2_claim, "d3", "d2");
    let restored_query = query(&mut store, &workspace, "q-restored");
    let restored_why = why(&mut store, &workspace, "why-restored");
    assert_eq!(restored_query["rows"][0]["status"], "certain");
    assert_eq!(
        restored_why["active_supporting_decision_paths"],
        json!([["d1"]])
    );
    assert_eq!(restored_why["blocking_defeats"], json!([]));

    assert_eq!(
        store.get(&workspace).unwrap().compile_cache_stats(),
        CompileCacheStats {
            compiles: 1,
            hits: 5,
            misses: 1,
            evictions: 0,
        },
        "both grounded standing transitions valuate the original saturation"
    );
    let work = store.get(&workspace).unwrap().read_work_stats();
    assert_eq!(work.attack_endpoint_resolutions, 6, "0 + 2 + 4");
    assert_eq!(work.attack_incidence_visits, 1);
    assert_eq!(work.full_name_scans, 0);
    assert_eq!(work.remap_builds, 0);
    assert_eq!(
        work.valuation_copies, 9,
        "six reads plus three why overlays"
    );
}

#[test]
fn workspaces_have_independent_bounded_entries() {
    let mut store = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: Vec::new(),
    })
    .unwrap();
    store.open(Mode::Scope, "demo".to_owned(), None).unwrap();
    store.open(Mode::Scope, "demo".to_owned(), None).unwrap();

    query(&mut store, "ws-1", "left-1");
    query(&mut store, "ws-1", "left-2");
    query(&mut store, "ws-2", "right-1");
    assert_eq!(store.get("ws-1").unwrap().compile_cache_stats().compiles, 1);
    assert_eq!(store.get("ws-1").unwrap().compile_cache_stats().hits, 1);
    assert_eq!(store.get("ws-2").unwrap().compile_cache_stats().compiles, 1);
    assert_eq!(store.get("ws-2").unwrap().compile_cache_stats().hits, 0);
}

#[test]
fn record_add_discard_materialize_and_reload_invalidate_at_structural_boundaries() {
    let (_root, mut store, _path) = fixture();
    let workspace = open(&mut store);
    query(&mut store, &workspace, "initial");

    result(dispatch(
        &request(
            "record-one",
            Op::AuthorRecordDecision,
            json!({"workspace": workspace, "decision": "d-recorded"}),
        ),
        &mut store,
    ));
    query(&mut store, &workspace, "after-record");

    result(dispatch(
        &request(
            "record-bulk",
            Op::AuthorRecordDecisions,
            json!({"workspace": workspace, "records": [{"decision": "d-bulk"}]}),
        ),
        &mut store,
    ));
    query(&mut store, &workspace, "after-bulk");

    result(dispatch(
        &request(
            "add",
            Op::AuthorAddFact,
            json!({
                "workspace": workspace,
                "entity": "mara",
                "key": "title",
                "value": "warden",
                "provenance": {
                    "kind": "cited", "decisions": ["d-recorded"], "state": "ratified"
                }
            }),
        ),
        &mut store,
    ));
    let status = result(dispatch(
        &request("status", Op::AuthorStatus, json!({"workspace": workspace})),
        &mut store,
    ));
    let key = status["staged"][0]["key"].as_str().unwrap().to_owned();
    result(dispatch(
        &request(
            "discard",
            Op::AuthorDiscard,
            json!({"workspace": workspace, "decl": key}),
        ),
        &mut store,
    ));
    query(&mut store, &workspace, "after-discard");

    result(dispatch(
        &request(
            "add-again",
            Op::AuthorAddFact,
            json!({
                "workspace": workspace,
                "entity": "mara",
                "key": "title",
                "value": "keeper",
                "provenance": {
                    "kind": "cited", "decisions": ["d-bulk"], "state": "ratified"
                }
            }),
        ),
        &mut store,
    ));
    result(dispatch(
        &request(
            "materialize",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false}),
        ),
        &mut store,
    ));
    query(&mut store, &workspace, "after-materialize");
    store.reload_workspace_base(&workspace).unwrap();
    query(&mut store, &workspace, "after-explicit-reload");

    let stats = store.get(&workspace).unwrap().compile_cache_stats();
    assert_eq!(stats.compiles, 6);
    assert_eq!(stats.misses, 6);
    assert_eq!(stats.evictions, 5);
    assert_eq!(stats.hits, 1);
}

#[test]
fn materialize_installs_the_decision_stamped_base_before_returning() {
    let (_root, mut store, _path) = fixture();
    let workspace = open(&mut store);
    result(dispatch(
        &request(
            "add-proposal",
            Op::AuthorAddFact,
            json!({
                "workspace": workspace,
                "entity": "mara",
                "key": "title",
                "value": "silver",
                "provenance": {"kind": "proposed", "origin": "cache-materialize"}
            }),
        ),
        &mut store,
    ));
    assert_eq!(
        query_value(&mut store, &workspace, "before", "title", "silver")["rows"][0]["status"],
        "possible"
    );

    result(dispatch(
        &request(
            "materialize",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false, "decision": "d-accept"}),
        ),
        &mut store,
    ));
    let before_read = store.get(&workspace).unwrap().compile_cache_stats();
    let immediate = query_value(&mut store, &workspace, "immediate", "title", "silver");
    let explanation = why_value(&mut store, &workspace, "immediate-why", "title", "silver");
    assert_eq!(immediate["rows"][0]["status"], "certain");
    assert_eq!(
        explanation["active_supporting_decision_paths"],
        json!([["d-accept"]])
    );
    assert_eq!(explanation["blocked_supports"], json!([]));

    let after_read = store.get(&workspace).unwrap().compile_cache_stats();
    assert_eq!(after_read.compiles, before_read.compiles + 1);
    assert_eq!(after_read.misses, before_read.misses + 1);
    assert_eq!(after_read.evictions, before_read.evictions + 1);
    assert_eq!(after_read.hits, before_read.hits + 1);

    store.reload_workspace_base(&workspace).unwrap();
    let before_reloaded_read = store.get(&workspace).unwrap().compile_cache_stats();
    assert_eq!(
        query_value(&mut store, &workspace, "after-reload", "title", "silver",)["rows"][0]["status"],
        "certain"
    );
    let after_reloaded_read = store.get(&workspace).unwrap().compile_cache_stats();
    assert_eq!(after_reloaded_read.compiles, before_reloaded_read.compiles);
    assert_eq!(after_reloaded_read.hits, before_reloaded_read.hits + 1);
}

#[test]
fn stale_main_materialize_installs_the_exact_current_parent_fold() {
    let (_root, mut store, _path) = fixture();
    let stale = open(&mut store);
    let sibling = open(&mut store);

    result(dispatch(
        &request(
            "sibling-add",
            Op::AuthorAddFact,
            json!({
                "workspace": sibling,
                "entity": "mara",
                "key": "title",
                "value": "keeper",
                "provenance": {"kind": "cited", "decisions": ["d1"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    result(dispatch(
        &request(
            "sibling-materialize",
            Op::AuthorMaterialize,
            json!({"workspace": sibling, "rendered": false}),
        ),
        &mut store,
    ));

    result(dispatch(
        &request(
            "stale-add",
            Op::AuthorAddFact,
            json!({
                "workspace": stale,
                "entity": "mara",
                "key": "color",
                "value": "blue",
                "provenance": {"kind": "cited", "decisions": ["d1"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    result(dispatch(
        &request(
            "stale-materialize",
            Op::AuthorMaterialize,
            json!({"workspace": stale, "rendered": false}),
        ),
        &mut store,
    ));

    let mut immediate_query = query_value(&mut store, &stale, "stale-query", "title", "keeper");
    let immediate_why = why_value(&mut store, &stale, "stale-why", "title", "keeper");
    let fresh = open_main_branch(&mut store);
    let mut fresh_query = query_value(&mut store, &fresh, "fresh-query", "title", "keeper");
    immediate_query.as_object_mut().unwrap().remove("root_hash");
    fresh_query.as_object_mut().unwrap().remove("root_hash");
    assert_eq!(
        immediate_query, fresh_query,
        "the stale caller must install the complete current-parent fold"
    );
    assert_eq!(
        immediate_why,
        why_value(&mut store, &fresh, "fresh-why", "title", "keeper"),
        "immediate explanation must match a fresh main workspace"
    );
}

#[test]
fn stale_main_materialize_remaps_workspace_local_support_by_exact_name() {
    let (_root, mut store, _path) = fixture();
    let stale = open(&mut store);
    let sibling = open(&mut store);

    result(dispatch(
        &request(
            "sibling-decision",
            Op::AuthorRecordDecision,
            json!({"workspace": sibling, "decision": "d-sibling"}),
        ),
        &mut store,
    ));
    result(dispatch(
        &request(
            "stale-proposal",
            Op::AuthorAddFact,
            json!({
                "workspace": stale,
                "entity": "mara",
                "key": "color",
                "value": "blue",
                "provenance": {"kind": "proposed", "origin": "stale-skew"}
            }),
        ),
        &mut store,
    ));
    result(dispatch(
        &request(
            "stale-materialize-skew",
            Op::AuthorMaterialize,
            json!({"workspace": stale, "rendered": false}),
        ),
        &mut store,
    ));

    let immediate = query_value(&mut store, &stale, "stale-skew-query", "color", "blue");
    assert_eq!(
        immediate["rows"][0]["status"], "possible",
        "the stale proposal support must not be retargeted to the sibling's true decision"
    );
    let immediate_why = why_value(&mut store, &stale, "stale-skew-why", "color", "blue");
    let fresh = open_main_branch(&mut store);
    let fresh_why = why_value(&mut store, &fresh, "fresh-skew-why", "color", "blue");
    assert_eq!(immediate_why, fresh_why);
    assert_eq!(immediate_why["why_not"]["status"], "possible");
    assert!(
        immediate_why["why"]["rendered"]
            .as_array()
            .unwrap()
            .iter()
            .any(|name| name.as_str() == Some("h.stale-skew")),
        "the explanation keeps the workspace-local hypothesis name: {immediate_why}"
    );
}

#[test]
fn stale_main_materialize_checks_the_exact_merged_candidate_before_cas() {
    let root = tempfile::tempdir().unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(concat!(
        "scope demo\n",
        "ratify d1\n",
        "person mara \"Mara\" ratified d1\n",
        "functional rank\n",
    ));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "fixture parses: {diagnostics:?}"
    );
    let mut store = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let path = root.path().join("lorestore/demo");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    let first = open(&mut store);
    let stale = open(&mut store);

    for (id, workspace, value) in [
        ("first-rank", first.as_str(), "gold"),
        ("stale-rank", stale.as_str(), "silver"),
    ] {
        result(dispatch(
            &request(
                id,
                Op::AuthorAddFact,
                json!({
                    "workspace": workspace,
                    "entity": "mara",
                    "key": "rank",
                    "value": value,
                    "provenance": {"kind": "cited", "decisions": ["d1"], "state": "ratified"}
                }),
            ),
            &mut store,
        ));
    }
    result(dispatch(
        &request(
            "first-materialize",
            Op::AuthorMaterialize,
            json!({"workspace": first, "rendered": false}),
        ),
        &mut store,
    ));
    let pinned_check = result(dispatch(
        &request("stale-check", Op::AuthorCheck, json!({"workspace": stale})),
        &mut store,
    ));
    assert_eq!(pinned_check["ok"], true, "the stale pre-image is clean");

    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let head_before = lorestore.root_hash().unwrap();
    let status_before = result(dispatch(
        &request(
            "status-before",
            Op::AuthorStatus,
            json!({"workspace": stale}),
        ),
        &mut store,
    ));
    let refused = dispatch(
        &request(
            "stale-invalid-merge",
            Op::AuthorMaterialize,
            json!({"workspace": stale, "rendered": false}),
        ),
        &mut store,
    );
    assert!(!refused.ok, "the merged FD violation must refuse");
    let error = refused.error.unwrap();
    assert_eq!(error.code, dli_serve::ErrorCode::MaterializeBlocked);
    assert!(
        error.data.unwrap()["blocking"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "continuity.fd-violation")
    );
    assert_eq!(lorestore.root_hash().unwrap(), head_before);
    assert_eq!(
        result(dispatch(
            &request(
                "status-after",
                Op::AuthorStatus,
                json!({"workspace": stale}),
            ),
            &mut store,
        )),
        status_before,
        "refused candidate leaves the live workspace unchanged"
    );
}

#[test]
fn exact_candidate_oracle_does_not_self_report_a_staged_rule_as_duplicate() {
    let (_root, mut store, _path) = fixture();
    let workspace = open(&mut store);
    result(dispatch(
        &request(
            "add-rule",
            Op::AuthorAddRule,
            json!({
                "workspace": workspace,
                "name": "honorific",
                "head": {"pred": "honored", "args": [{"Var": "x"}]},
                "body": [{
                    "atom": {"pred": "rank", "args": [{"Var": "x"}, {"Sym": "gold"}]},
                    "negated": false
                }],
                "provenance": {"kind": "cited", "decisions": ["d1"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    let materialized = dispatch(
        &request(
            "materialize-rule",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false}),
        ),
        &mut store,
    );
    assert!(
        materialized.ok,
        "candidate checking must compare the staged rule to the target parent, not to itself: \
         {materialized:?}"
    );
}

#[test]
fn cache_enabled_and_forced_miss_match_active_blocked_and_restored_wire_bytes() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let workspace = open(&mut store);
    let d1_claim = claim_id(&lorestore, false, "mara");

    let assert_equal = |store: &mut Store, label: &str| {
        let enabled_query = query(store, &workspace, &format!("{label}-query-enabled"));
        let enabled_why = why(store, &workspace, &format!("{label}-why-enabled"));
        store
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(false);
        let missed_query = query(store, &workspace, &format!("{label}-query-miss"));
        let missed_why = why(store, &workspace, &format!("{label}-why-miss"));
        store
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(true);
        assert_eq!(enabled_query, missed_query, "{label} query JSON");
        assert_eq!(enabled_why, missed_why, "{label} why JSON");
        enabled_why
    };

    let active = assert_equal(&mut store, "active");
    assert_eq!(active["active_supporting_decision_paths"], json!([["d1"]]));
    assert_eq!(active["blocking_defeats"], json!([]));
    assert_eq!(active["blocked_supports"], json!([]));

    defeat(&mut store, &workspace, "defeat-d1", &d1_claim, "d2", "d1");
    let blocked = assert_equal(&mut store, "blocked");
    assert_eq!(
        blocked["blocking_defeats"],
        json!([{"defeated_decision":"d1", "defeating_decision":"d2"}])
    );
    assert_eq!(blocked["active_supporting_decision_paths"], json!([]));
    assert_eq!(blocked["blocked_supports"].as_array().unwrap().len(), 1);

    let d2_claim = claim_id(&lorestore, true, "d2");
    defeat(&mut store, &workspace, "defeat-d2", &d2_claim, "d3", "d2");
    let restored = assert_equal(&mut store, "restored");
    assert_eq!(
        restored["active_supporting_decision_paths"],
        json!([["d1"]])
    );
    assert_eq!(restored["blocking_defeats"], json!([]));
    assert_eq!(restored["blocked_supports"], json!([]));
}
