//! Contract 1.13's atomic bulk decision recorder (GH #106).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_serve::author::dispatch;
use dli_serve::author::store::Store;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};
use std::time::Instant;

const DLI: &str = "1.13.0";

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

fn mara_pack() -> Pack {
    let (document, diagnostics) =
        dli_syntax::parse_document("scope demo\nperson mara \"Mara\" proposed seed\n");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error())
    );
    Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    }
}

fn store_with_mara() -> (tempfile::TempDir, Store) {
    let root = tempfile::tempdir().expect("temporary store root");
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"record-decisions\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .expect("write project manifest");
    let mut store = Store::with_pack(mara_pack()).expect("base compiles");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .expect("attach lorestore");
    (root, store)
}

fn term(text: &str) -> Value {
    text.strip_prefix('$')
        .map_or_else(|| json!({"Sym": text}), |var| json!({"Var": var}))
}

fn atom(pred: &str, args: &[&str]) -> Value {
    json!({"pred": pred, "args": args.iter().map(|arg| term(arg)).collect::<Vec<_>>()})
}

fn lit(pred: &str, args: &[&str]) -> Value {
    json!({"atom": atom(pred, args), "negated": false})
}

fn cited(decision: &str) -> Value {
    json!({"kind":"cited", "decisions":[decision], "state":"ratified"})
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

fn bulk(store: &mut Store, ws: &str, records: &Value) -> Value {
    result(dispatch(
        &req(
            "bulk",
            Op::AuthorRecordDecisions,
            json!({ "workspace": ws, "records": records }),
        ),
        store,
    ))
}

fn head(store: &Store, ws: &str) -> dli_lorestore::ContentHash {
    store
        .workspace_lorestore(ws)
        .expect("workspace lorestore")
        .root_hash()
        .expect("read root")
        .expect("root exists")
}

#[test]
fn forward_and_cyclic_same_batch_citations_are_validated_as_a_whole() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let value = bulk(
        &mut store,
        &ws,
        &json!([
            {"decision":"d-1", "provenance":{
                "kind":"cited", "decisions":["d-2"], "state":"ratified"}},
            {"decision":"d-2", "provenance":{
                "kind":"cited", "decisions":["d-1"], "state":"ratified"}}
        ]),
    );
    assert_eq!(value["findings"], json!([]));
    assert_eq!(value["requested"], 2);
    assert_eq!(value["already_known"], 0);
    assert_eq!(value["recorded"], 2);
    let decisions = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .decisions_at(&head(&store, &ws))
        .expect("decisions");
    assert!(decisions.ends_with(&["d-1".to_owned(), "d-2".to_owned()]));
}

#[test]
fn duplicate_and_unknown_citation_findings_are_atomic_and_identify_records() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let before = head(&store, &ws);
    let value = bulk(
        &mut store,
        &ws,
        &json!([
            {"decision":"d-dup"},
            {"decision":"d-dup"},
            {"decision":"d-external", "provenance":{
                "kind":"cited", "decisions":["missing"], "state":"ratified"}}
        ]),
    );
    assert_eq!(value["recorded"], 0);
    let findings = value["findings"].as_array().expect("findings");
    assert!(findings.iter().any(|finding| {
        finding["code"] == "decision.duplicate-in-batch"
            && finding["data"]["record_index"] == 1
            && finding["data"]["first_index"] == 0
    }));
    assert!(findings.iter().any(|finding| {
        finding["code"] == "citation.unknown-decision"
            && finding["data"]["record_index"] == 2
            && finding["data"]["decision"] == "d-external"
    }));
    assert_eq!(head(&store, &ws), before);
    assert_eq!(store.base_update_stats().prepared_compiles, 0);
    assert_eq!(store.base_update_stats().prepared_installs, 0);
}

#[test]
fn mixed_known_and_new_records_preserve_order_and_all_known_is_zero_write() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    bulk(&mut store, &ws, &json!([{"decision":"known"}]));
    let value = bulk(
        &mut store,
        &ws,
        &json!([
            {"decision":"new-a"},
            {"decision":"known"},
            {"decision":"new-b"}
        ]),
    );
    assert_eq!(value["requested"], 3);
    assert_eq!(value["already_known"], 1);
    assert_eq!(value["recorded"], 2);
    let after_mixed = head(&store, &ws);
    let decisions = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .decisions_at(&after_mixed)
        .expect("decisions");
    assert!(decisions.ends_with(&["known".to_owned(), "new-a".to_owned(), "new-b".to_owned()]));

    let stats = store.base_update_stats();
    let all_known = bulk(
        &mut store,
        &ws,
        &json!([
            {"decision":"new-b"},
            {"decision":"known"},
            {"decision":"new-a"}
        ]),
    );
    assert_eq!(all_known["already_known"], 3);
    assert_eq!(all_known["recorded"], 0);
    assert_eq!(head(&store, &ws), after_mixed);
    assert_eq!(store.base_update_stats(), stats);
}

#[test]
fn one_thousand_twenty_four_records_use_one_commit_and_one_prepared_compile() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let before = head(&store, &ws);
    let before_depth = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .walk_dag(&before, None)
        .expect("walk before")
        .len();
    let records = (0..1024)
        .map(|index| json!({"decision": format!("d-{index:04}")}))
        .collect::<Vec<_>>();
    let value = bulk(&mut store, &ws, &Value::Array(records));
    assert_eq!(value["recorded"], 1024);
    let after = head(&store, &ws);
    let after_depth = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .walk_dag(&after, None)
        .expect("walk after")
        .len();
    assert_eq!(after_depth, before_depth + 1);
    let stats = store.base_update_stats();
    assert_eq!(stats.prepared_compiles, 1);
    assert_eq!(stats.prepared_installs, 1);
    assert_eq!(stats.reloads, 0);
}

#[test]
fn one_record_has_the_same_semantics_as_record_decision() {
    let (_single_root, mut single) = store_with_mara();
    let (_bulk_root, mut batched) = store_with_mara();
    let single_ws = open(&mut single);
    let bulk_ws = open(&mut batched);

    result(dispatch(
        &req(
            "single",
            Op::AuthorRecordDecision,
            json!({"workspace":single_ws, "decision":"d-one"}),
        ),
        &mut single,
    ));
    let recorded = bulk(&mut batched, &bulk_ws, &json!([{"decision":"d-one"}]));
    assert_eq!(recorded["requested"], 1);
    assert_eq!(recorded["already_known"], 0);
    assert_eq!(recorded["recorded"], 1);

    for (store, workspace) in [(&mut single, &single_ws), (&mut batched, &bulk_ws)] {
        result(dispatch(
            &req(
                "fact",
                Op::AuthorAddFact,
                json!({
                    "workspace":workspace,
                    "entity":"mara", "key":"rank", "value":"captain",
                    "provenance":cited("d-one")
                }),
            ),
            store,
        ));
    }

    let single_head = head(&single, &single_ws);
    let bulk_head = head(&batched, &bulk_ws);
    assert_eq!(
        single
            .workspace_lorestore(&single_ws)
            .expect("single lorestore")
            .decisions_at(&single_head)
            .expect("single decisions"),
        batched
            .workspace_lorestore(&bulk_ws)
            .expect("bulk lorestore")
            .decisions_at(&bulk_head)
            .expect("bulk decisions")
    );
    assert_eq!(
        single
            .workspace_lorestore(&single_ws)
            .expect("single lorestore")
            .interner_at(&single_head)
            .expect("single interner"),
        batched
            .workspace_lorestore(&bulk_ws)
            .expect("bulk lorestore")
            .interner_at(&bulk_head)
            .expect("bulk interner")
    );

    let why = |store: &mut Store, workspace: &str| {
        result(dispatch(
            &req(
                "why",
                Op::AuthorWhy,
                json!({"workspace":workspace, "goal_atom":atom("rank", &["mara", "captain"])}),
            ),
            store,
        ))
    };
    let single_why = why(&mut single, &single_ws);
    let bulk_why = why(&mut batched, &bulk_ws);
    assert_eq!(single_why["why"], bulk_why["why"]);
    assert_eq!(single_why["why_not"], bulk_why["why_not"]);
    assert_eq!(
        single_why["supporting_decisions"],
        bulk_why["supporting_decisions"]
    );
}

#[test]
fn bulk_decisions_drive_fact_rule_why_and_defeat() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    bulk(
        &mut store,
        &ws,
        &json!([
            {"decision":"d-fact"},
            {"decision":"d-rule"},
            {"decision":"d-defeat"}
        ]),
    );
    result(dispatch(
        &req(
            "fact",
            Op::AuthorAddFact,
            json!({
                "workspace":ws, "entity":"mara", "key":"rank", "value":"captain",
                "provenance":cited("d-fact")
            }),
        ),
        &mut store,
    ));
    result(dispatch(
        &req(
            "rule",
            Op::AuthorAddRule,
            json!({
                "workspace":ws, "name":"honorific",
                "head":atom("honored", &["$x"]),
                "body":[lit("rank", &["$x", "captain"])],
                "provenance":cited("d-rule")
            }),
        ),
        &mut store,
    ));

    let why = result(dispatch(
        &req(
            "why-before",
            Op::AuthorWhy,
            json!({"workspace":ws, "goal_atom":atom("honored", &["mara"])}),
        ),
        &mut store,
    ));
    let supporting = why["supporting_decisions"]
        .as_array()
        .expect("why returns supporting decisions");
    assert!(supporting.contains(&json!("d-fact")), "fact support: {why}");
    assert!(supporting.contains(&json!("d-rule")), "rule support: {why}");

    let materialized = result(dispatch(
        &req(
            "materialize",
            Op::AuthorMaterialize,
            json!({"workspace":ws, "rendered":false}),
        ),
        &mut store,
    ));
    let fact_claim_id = materialized["facts"]
        .as_array()
        .expect("materialized facts")
        .iter()
        .find(|fact| fact["atom"]["pred"] == "rank")
        .and_then(|fact| fact["claim_id"].as_str())
        .unwrap_or_else(|| panic!("materialized rank claim: {materialized}"))
        .to_owned();
    let defeated = result(dispatch(
        &req(
            "defeat",
            Op::AuthorDefeat,
            json!({
                "workspace":ws,
                "claim_id":fact_claim_id,
                "decision":"d-defeat",
                "defeated_decision":"d-fact"
            }),
        ),
        &mut store,
    ));
    assert_eq!(defeated["findings"], json!([]));

    let after = result(dispatch(
        &req(
            "query-after",
            Op::AuthorQuery,
            json!({
                "workspace":ws,
                "goal_atom":atom("honored", &["mara"]),
                "layer":"all"
            }),
        ),
        &mut store,
    ));
    assert_eq!(
        after["rows"],
        json!([]),
        "defeat removes derivation: {after}"
    );
}

#[test]
fn stale_workspace_digest_refuses_the_complete_batch() {
    let (_root, mut store) = store_with_mara();
    let opened = result(dispatch(
        &req("open-stale", Op::AuthorOpen, json!({"scope":"demo"})),
        &mut store,
    ));
    let ws = opened["workspace"].as_str().expect("workspace").to_owned();
    let stale_digest = opened["workspace_digest"]
        .as_str()
        .expect("open digest")
        .to_owned();
    result(dispatch(
        &req(
            "move-digest",
            Op::AuthorAddFact,
            json!({
                "workspace":ws, "entity":"mara", "key":"color", "value":"red",
                "provenance":{"kind":"proposed", "origin":"stale-test"}
            }),
        ),
        &mut store,
    ));
    let before = head(&store, &ws);
    let mut stale = req(
        "stale-bulk",
        Op::AuthorRecordDecisions,
        json!({"workspace":ws, "records":[{"decision":"d-never"}]}),
    );
    stale.expected_digest = Some(stale_digest);
    let response = dli_serve::serve::respond(
        &serde_json::to_string(&stale).expect("stale envelope json"),
        &mut dli_serve::session::Sessions::default(),
        &mut store,
    )
    .expect("stale envelope receives a response");
    assert!(!response.ok, "stale request is refused: {response:?}");
    assert_eq!(
        response.error.expect("typed refusal").code,
        dli_serve::ErrorCode::StaleFence
    );
    assert_eq!(head(&store, &ws), before, "the branch is untouched");
}

#[test]
fn older_contract_rejects_bulk_without_moving_the_single_operation() {
    let (_root, mut store) = store_with_mara();
    let ws = open(&mut store);
    let mut request = req(
        "old",
        Op::AuthorRecordDecisions,
        json!({"workspace":ws, "records":[{"decision":"d-1"}]}),
    );
    request.dli = "1.12.0".to_owned();
    let line = serde_json::to_string(&request).expect("request json");
    let response = dli_serve::serve::respond(
        &line,
        &mut dli_serve::session::Sessions::default(),
        &mut store,
    )
    .expect("response");
    assert!(!response.ok, "pre-floor bulk op is refused");

    let mut single = req(
        "single",
        Op::AuthorRecordDecision,
        json!({"workspace":ws, "decision":"d-1"}),
    );
    single.dli = "1.12.0".to_owned();
    assert!(
        dispatch(&single, &mut store).ok,
        "single operation is unchanged"
    );
}

#[test]
fn branch_routing_and_idempotent_retry_preserve_isolation() {
    let (_root, mut store) = store_with_mara();
    let main_before = store
        .lorestore_for("demo")
        .expect("demo lorestore")
        .root_hash()
        .expect("main head")
        .expect("main exists");
    store
        .lorestore_for("demo")
        .expect("demo lorestore")
        .create_branch("side", "main")
        .expect("create side branch");
    let ws = result(dispatch(
        &req(
            "open-side",
            Op::AuthorOpen,
            json!({"scope":"demo", "branch":"side"}),
        ),
        &mut store,
    ))["workspace"]
        .as_str()
        .expect("workspace")
        .to_owned();
    let mut request = req(
        "bulk-side",
        Op::AuthorRecordDecisions,
        json!({"workspace":ws, "records":[{"decision":"d-side"}]}),
    );
    request.idempotency_key = Some("bulk-side-key".to_owned());
    let first = dispatch(&request, &mut store);
    assert!(first.ok, "first batch succeeds: {first:?}");
    let side_after = store
        .workspace_lorestore(&ws)
        .expect("lorestore")
        .branch_ref("side")
        .expect("side ref")
        .expect("side exists")
        .commit;
    let replay = dispatch(&request, &mut store);
    assert_eq!(replay, first, "same idempotency key replays exact response");
    assert_eq!(
        store
            .workspace_lorestore(&ws)
            .expect("lorestore")
            .branch_ref("side")
            .expect("side ref")
            .expect("side exists")
            .commit,
        side_after
    );
    assert_eq!(
        store
            .lorestore_for("demo")
            .expect("demo lorestore")
            .root_hash()
            .expect("main head")
            .expect("main exists"),
        main_before,
        "side batch never advances main"
    );
}

/// Release-only scale evidence at every requested campaign size. The printed
/// phases make request construction and the atomic store transaction visible
/// separately; counters pin the one-compile/one-install/no-reload invariant.
#[test]
#[ignore = "release scale harness"]
#[allow(clippy::print_stderr)]
fn release_scale_harness_reports_phase_timings_and_counters() {
    for count in [1_024_usize, 4_096, 16_384, 65_536] {
        let setup_started = Instant::now();
        let (_root, mut store) = store_with_mara();
        let ws = open(&mut store);
        let before = head(&store, &ws);
        let setup_elapsed = setup_started.elapsed();

        let build_started = Instant::now();
        let records = (0..count)
            .map(|index| json!({"decision": format!("d-{index:05}")}))
            .collect::<Vec<_>>();
        let build_elapsed = build_started.elapsed();

        let dispatch_started = Instant::now();
        let value = bulk(&mut store, &ws, &Value::Array(records));
        let dispatch_elapsed = dispatch_started.elapsed();
        assert_eq!(value["recorded"], count);
        let after = head(&store, &ws);
        let commit_delta = store
            .workspace_lorestore(&ws)
            .expect("lorestore")
            .walk_dag(&after, Some(&before))
            .expect("walk delta")
            .len()
            - 1;
        let stats = store.base_update_stats();
        assert_eq!(commit_delta, 1);
        assert_eq!(stats.prepared_compiles, 1);
        assert_eq!(stats.prepared_installs, 1);
        assert_eq!(stats.reloads, 0);
        eprintln!(
            "record_decisions count={count} setup_ms={} build_ms={} dispatch_ms={} commits={commit_delta} prepared_compiles={} prepared_installs={} reloads={}",
            setup_elapsed.as_millis(),
            build_elapsed.as_millis(),
            dispatch_elapsed.as_millis(),
            stats.prepared_compiles,
            stats.prepared_installs,
            stats.reloads,
        );
    }
}
