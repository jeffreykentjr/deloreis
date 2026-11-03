#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Contract 1.7.0's `author.assert_same_as` end to end (WS-1, the
//! identity-closure kernel's wire surface): the declaration reaches the
//! staged world, the merge answers under either alias, the four stage-time
//! findings fire correctly, the post-merge FD check blocks materialize, and
//! `author.defeat` un-merges a durably materialized identity claim through
//! the wire — the closing piece of the defeasible-identity story the kernel
//! (PR-1a) proved only at the storage-round-trip level.
//!
//! Also here: the version gate (the op is `unknown-op` below `1.7.0`, and the
//! `1.7.0` capability list is `1.6.0`'s plus exactly this one op) and the
//! idempotency/digest fences every mutating `author.*` tool shares.

use serde_json::{Value, json};

use dli_ast::decl::{Decl, Document, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::language::RatifyBody;
use dli_ast::span::Span;
use dli_serve::author::store::Store;
use dli_serve::author::{dispatch, workspace::Mode};
use dli_serve::{Op, Request};
use dli_world::pack::Pack;

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

fn ident(text: &str) -> Ident {
    Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

/// A base declaring `D-1` and two people, `nara` and `moira`, plus a certain
/// `rank(nara, gold)` — enough for a clean same-as with an immediate
/// observable merge consequence.
fn officer_base() -> Store {
    let source = "scope storybook\n\
        ratify D-1\n\
        person nara \"Nara\"  ratified D-1\n\
        person moira \"Moira\"  ratified D-1\n\
        functional rank  ratified D-1\n\
        rank(nara, gold)  ratified D-1\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.is_empty(),
        "base source parses: {diagnostics:?}"
    );
    Store::with_pack(Pack {
        name: "storybook".to_owned(),
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

fn assert_same_as(
    store: &mut Store,
    dli: &str,
    ws: &str,
    left: &str,
    right: &str,
    provenance: &Value,
) -> dli_serve::Response {
    dispatch(
        &req(
            dli,
            "same-as",
            Op::AuthorAssertSameAs,
            json!({ "workspace": ws, "left": left, "right": right, "provenance": provenance }),
        ),
        store,
    )
}

fn proposed(origin: &str) -> Value {
    json!({ "kind": "proposed", "origin": origin })
}

fn cited(decision: &str) -> Value {
    json!({ "kind": "cited", "decisions": [decision], "state": "ratified" })
}

fn check_codes(store: &mut Store, dli: &str, ws: &str) -> Vec<String> {
    let response = dispatch(
        &req(dli, "check", Op::AuthorCheck, json!({ "workspace": ws })),
        store,
    );
    codes(result(&response))
}

// -- the happy path: staging, rendering, closure ------------------------------

#[test]
fn assert_same_as_stages_cleanly_and_renders_the_declaration() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    let response = assert_same_as(&mut store, "1.7.0", &ws, "nara", "moira", &proposed("s"));
    assert!(
        codes(result(&response)).is_empty(),
        "{:?}",
        result(&response)
    );
    assert_eq!(
        result(&response)["rendered"],
        json!(["same-as nara = moira proposed s"])
    );
    assert!(
        result(&response)["workspace_digest"]
            .as_str()
            .is_some_and(|d| d.starts_with("jcs:sha256:"))
    );
}

#[test]
fn the_declaration_stages_a_same_as_decl() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    assert_same_as(&mut store, "1.7.0", &ws, "nara", "moira", &proposed("s"));
    let workspace = store.get(&ws).expect("workspace is open");
    assert_eq!(workspace.mode, Mode::Scope);
    assert_eq!(
        workspace.decls.len(),
        1,
        "one staged declaration: {:?}",
        workspace.decls
    );
    assert!(
        matches!(workspace.decls[0], Decl::SameAs(_)),
        "staged as a same-as decl: {:?}",
        workspace.decls[0]
    );
}

#[test]
fn the_merge_answers_a_staged_query_under_the_alias() {
    // A same-as declaration's merge is visible to `author.query` immediately —
    // fully re-derived from the staged world, no materialize needed.
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    assert_same_as(&mut store, "1.7.0", &ws, "nara", "moira", &cited("D-1"));
    let queried = dispatch(
        &req(
            "1.7.0",
            "q",
            Op::AuthorQuery,
            json!({ "workspace": ws, "goal_atom": {"pred":"rank", "args":[{"Sym":"moira"},{"Sym":"gold"}]}, "layer": "all" }),
        ),
        &mut store,
    );
    let rows = result(&queried)["rows"].as_array().expect("rows");
    assert!(
        !rows.is_empty() && rows[0]["status"] == "certain",
        "the alias inherits nara's certain rank: {:?}",
        result(&queried)
    );
}

// -- the four stage-time findings ---------------------------------------------

#[test]
fn unknown_endpoints_are_flagged_on_both_sides() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    let response = assert_same_as(&mut store, "1.7.0", &ws, "ghost", "phantom", &proposed("s"));
    let cs = codes(result(&response));
    assert_eq!(
        cs.iter().filter(|c| *c == "entity.unknown").count(),
        2,
        "both unresolved endpoints are flagged: {cs:?}"
    );
}

#[test]
fn a_self_merge_is_a_warning_not_a_block() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    let response = assert_same_as(&mut store, "1.7.0", &ws, "nara", "nara", &proposed("s"));
    let findings = result(&response)["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0]["code"], "identity.self-merge");
    assert_eq!(findings[0]["severity"], "warn");
}

#[test]
fn an_endpoint_naming_a_declared_era_is_refused() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    // Stage an era, then try to same-as it with a known person.
    dispatch(
        &req(
            "1.7.0",
            "era",
            Op::AuthorAddEntity,
            json!({ "workspace": ws, "kind": "era", "id": "first-tides", "name": "First Tides",
                    "provenance": { "kind": "cited", "decisions": ["D-1"], "state": "ratified" } }),
        ),
        &mut store,
    );
    let response = assert_same_as(
        &mut store,
        "1.7.0",
        &ws,
        "nara",
        "first-tides",
        &proposed("s"),
    );
    let cs = codes(result(&response));
    assert!(
        cs.contains(&"identity.temporal-unsupported".to_owned()),
        "{cs:?}"
    );
    let finding = result(&response)["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "identity.temporal-unsupported")
        .unwrap();
    assert_eq!(finding["severity"], "error");
}

#[test]
fn a_sort_mismatch_is_a_warning_not_a_block() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    dispatch(
        &req(
            "1.7.0",
            "place",
            Op::AuthorAddEntity,
            json!({ "workspace": ws, "kind": "place", "id": "camelot", "name": "Camelot",
                    "provenance": { "kind": "cited", "decisions": ["D-1"], "state": "ratified" } }),
        ),
        &mut store,
    );
    let response = assert_same_as(&mut store, "1.7.0", &ws, "nara", "camelot", &proposed("s"));
    let cs = codes(result(&response));
    assert!(cs.contains(&"identity.sort-mismatch".to_owned()), "{cs:?}");
    let finding = result(&response)["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "identity.sort-mismatch")
        .unwrap();
    assert_eq!(finding["severity"], "warn");
}

// -- the post-merge FD check ---------------------------------------------------

#[test]
fn a_post_merge_fd_violation_surfaces_on_check_and_blocks_materialize() {
    // nara is ranked gold; give moira a DIFFERENT certain rank, then merge them.
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    dispatch(
        &req(
            "1.7.0",
            "silver",
            Op::AuthorAddFact,
            json!({ "workspace": ws, "entity": "moira", "key": "rank", "value": "silver",
                    "provenance": { "kind": "cited", "decisions": ["D-1"], "state": "ratified" } }),
        ),
        &mut store,
    );
    assert_same_as(&mut store, "1.7.0", &ws, "nara", "moira", &cited("D-1"));

    assert!(
        check_codes(&mut store, "1.7.0", &ws).contains(&"continuity.fd-violation".to_owned()),
        "author.check surfaces the post-merge FD break: {:?}",
        check_codes(&mut store, "1.7.0", &ws)
    );

    let blocked = dispatch(
        &req(
            "1.7.0",
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

// -- author.why names the ruling ----------------------------------------------

#[test]
fn author_why_names_the_identity_ruling() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    assert_same_as(&mut store, "1.7.0", &ws, "nara", "moira", &cited("D-1"));
    let why = dispatch(
        &req(
            "1.7.0",
            "why",
            Op::AuthorWhy,
            json!({ "workspace": ws, "atom": "rank(moira, gold)" }),
        ),
        &mut store,
    );
    let antichain = result(&why)["why"]["antichain"]
        .as_array()
        .expect("the merged copy derives, so `why` carries a positive antichain");
    let names: Vec<String> = antichain
        .iter()
        .flat_map(|witness| witness.as_array().unwrap())
        .map(|claim| claim.as_str().unwrap_or_default().to_owned())
        .collect();
    // `why` resolves claim ids to rendered names via the support index; D-1
    // backs both nara's own rank and the identity link, so at minimum the
    // antichain is non-empty and the rendered support mentions the ruling.
    assert!(!names.is_empty(), "{:?}", result(&why));
    let rendered = result(&why)["why"]["rendered"]
        .as_array()
        .expect("rendered witnesses")
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(
        rendered.contains("D-1"),
        "the identity ruling's citation is named in the support: {rendered}"
    );
}

// -- author.diff shows the merged fact gated on the identity variable --------

#[test]
fn author_diff_rows_show_the_staged_same_as_declaration() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    assert_same_as(
        &mut store,
        "1.7.0",
        &ws,
        "nara",
        "moira",
        &proposed("origin-tag"),
    );
    let diff = dispatch(
        &req("1.7.0", "d", Op::AuthorDiff, json!({ "workspace": ws })),
        &mut store,
    );
    let rendered = result(&diff)["rendered"]
        .as_array()
        .expect("rendered lines");
    assert_eq!(rendered.len(), 1, "{rendered:?}");
    assert!(
        rendered[0]
            .as_str()
            .unwrap_or_default()
            .contains("same-as nara = moira"),
        "{rendered:?}"
    );
}

// -- the version gate -----------------------------------------------------

/// `author.assert_same_as` specifically: present at `1.7.0`, absent below it.
/// The exhaustive "1.6.0 plus exactly the campaign's ops, in append order"
/// property — now two ops, `assert_same_as` then `add_rule` (WS-2) — lives in
/// `add_rule.rs`'s `the_1_7_0_capability_list_is_1_6_0_plus_both_campaign_ops`,
/// so it has one place to stay current as the campaign's op count moves.

// -- idempotency and digest fences --------------------------------------------

#[test]
fn a_stale_digest_fence_refuses() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    let mut stale = req(
        "1.7.0",
        "same-as",
        Op::AuthorAssertSameAs,
        json!({ "workspace": ws, "left": "nara", "right": "moira", "provenance": proposed("s") }),
    );
    stale.expected_digest = Some("jcs:sha256:deadbeef".to_owned());
    let response = dispatch(&stale, &mut store);
    assert!(!response.ok);
    assert_eq!(
        response.error.expect("error").code,
        dli_serve::ErrorCode::StaleFence
    );
}

#[test]
fn an_idempotent_retry_replays_and_stages_nothing() {
    let mut store = officer_base();
    let ws = open_scope(&mut store, "1.7.0", "storybook");
    let mut call = req(
        "1.7.0",
        "same-as",
        Op::AuthorAssertSameAs,
        json!({ "workspace": ws, "left": "nara", "right": "moira", "provenance": proposed("s") }),
    );
    call.idempotency_key = Some("idem-same-as-1".to_owned());
    let first = dispatch(&call, &mut store);
    let second = dispatch(&call, &mut store);
    assert_eq!(result(&second), result(&first));

    let diff = dispatch(
        &req("1.7.0", "d", Op::AuthorDiff, json!({ "workspace": ws })),
        &mut store,
    );
    assert_eq!(
        result(&diff)["rendered"].as_array().map(Vec::len),
        Some(1),
        "the retry staged nothing new: {:?}",
        result(&diff)
    );
}

// -- the durable-defeat-un-merges wire test -----------------------------------
//
// The closing piece of the defeasible-identity story: PR-1a proved defeat
// un-merges at the in-memory valuation-flip level (`same_as_honesty.rs`) and
// at the storage-round-trip level (`same_as_decl_parity.rs`, which manually
// applied a captured `Ref.valuation` overlay onto a freshly compiled world,
// narrating that the automatic APPLICATION of that overlay is a consumer's
// job). This is that consumer: `author.query`'s `staged_world` reads the
// workspace's durable base through the SAME `Ref.valuation` overlay
// `author.defeat` writes, end to end, through the real wire.

fn ledger_pack() -> Pack {
    fn id(text: &str) -> Ident {
        ident(text)
    }
    Pack {
        name: "demo".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("demo"),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Ratify(RatifyDecl {
                    decision: id("D-1"),
                    body: RatifyBody::default(),
                }),
                Decl::Ratify(RatifyDecl {
                    decision: id("D-2"),
                    body: RatifyBody::default(),
                }),
                Decl::Ratify(RatifyDecl {
                    decision: id("D-3"),
                    body: RatifyBody::default(),
                }),
            ],
        }],
    }
}

fn ledger_request(id: &str, op: Op, params: Value) -> Request {
    req("2.0.0", id, op, params)
}

fn ledger_result(response: dli_serve::Response) -> Value {
    assert!(response.ok, "request failed: {response:?}");
    response.result.expect("successful request has a result")
}

#[test]
#[allow(clippy::too_many_lines)]
fn durable_defeat_of_a_same_as_claim_un_merges_the_wire_query() {
    let root = tempfile::tempdir().expect("temporary store root");
    let lorestore_path = root.path().join("lorestore");
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"contract-1-7\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .expect("write project manifest");
    let mut store = Store::with_pack(ledger_pack()).expect("compile demo pack");
    store
        .attach_lorestore(lorestore_path.clone())
        .expect("attach demo lorestore");

    let opened = ledger_result(dispatch(
        &ledger_request("open", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ));
    let workspace = opened["workspace"].as_str().unwrap().to_owned();

    // Two people, one ranked, both cited to D-1 so they read certain.
    for (id, name) in [("nara", "Nara"), ("moira", "Moira")] {
        let added = ledger_result(dispatch(
            &ledger_request(
                "add",
                Op::AuthorAddEntity,
                json!({
                    "workspace": workspace, "kind": "person", "id": id, "name": name,
                    "provenance": {"kind": "cited", "decisions": ["D-1"], "state": "ratified"}
                }),
            ),
            &mut store,
        ));
        assert!(
            added["findings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["severity"] != "error"),
            "{added:?}"
        );
    }
    ledger_result(dispatch(
        &ledger_request(
            "rank",
            Op::AuthorAddFact,
            json!({
                "workspace": workspace, "entity": "nara", "key": "rank", "value": "gold",
                "provenance": {"kind": "cited", "decisions": ["D-1"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    // The identity claim cites a DIFFERENT decision (D-3) than the base facts
    // (D-1) — the whole point of this test is to defeat the identity alone
    // and leave the surface fact's own citation untouched.
    let same_as_added = ledger_result(dispatch(
        &ledger_request(
            "same-as",
            Op::AuthorAssertSameAs,
            json!({
                "workspace": workspace, "left": "nara", "right": "moira",
                "provenance": {"kind": "cited", "decisions": ["D-3"], "state": "ratified"}
            }),
        ),
        &mut store,
    ));
    assert!(
        same_as_added["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["severity"] != "error"),
        "{same_as_added:?}"
    );

    let materialized = ledger_result(dispatch(
        &ledger_request(
            "materialize",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false}),
        ),
        &mut store,
    ));
    let same_as_claim_id = materialized["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fact| {
            fact["atom"]["pred"] == "same-as" && fact["atom"]["args"] == json!(["nara", "moira"])
        })
        .and_then(|fact| fact["claim_id"].as_str())
        .expect("the durably stored same-as claim id")
        .to_owned();

    // Before defeat: the merge is visible through the wire query, reading the
    // durable base (no staged declarations remain after a clean materialize).
    let before = ledger_result(dispatch(
        &ledger_request(
            "q1",
            Op::AuthorQuery,
            json!({ "workspace": workspace, "goal_atom": {"pred":"rank", "args":[{"Sym":"moira"},{"Sym":"gold"}]}, "layer": "all" }),
        ),
        &mut store,
    ));
    let before_rows = before["rows"].as_array().expect("rows");
    assert!(
        !before_rows.is_empty() && before_rows[0]["status"] == "certain",
        "before defeat, the merged consequence is certain through the wire: {before:?}"
    );

    // Defeat the identity claim: D-2 supersedes D-3 (the same-as's OWN
    // citation), leaving D-1 (the base facts' citation) untouched.
    let defeated = ledger_result(dispatch(
        &ledger_request(
            "defeat",
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": same_as_claim_id,
                "decision": "D-2",
                "defeated_decision": "D-3"
            }),
        ),
        &mut store,
    ));
    assert_eq!(defeated["findings"], json!([]), "{defeated:?}");

    // After defeat: the SAME wire query, no recompile of anything by this
    // test — `author.query`'s `staged_world` applies the durable
    // conditional attack `author.defeat` just wrote, and the merged
    // consequence is dead on this very next read.
    let after = ledger_result(dispatch(
        &ledger_request(
            "q2",
            Op::AuthorQuery,
            json!({ "workspace": workspace, "goal_atom": {"pred":"rank", "args":[{"Sym":"moira"},{"Sym":"gold"}]}, "layer": "all" }),
        ),
        &mut store,
    ));
    assert_eq!(
        after["rows"],
        json!([]),
        "the durable defeat un-merges the identity through the wire: {after:?}"
    );

    // The surface fact is untouched: nara's own rank is still certain.
    let surface = ledger_result(dispatch(
        &ledger_request(
            "q3",
            Op::AuthorQuery,
            json!({ "workspace": workspace, "goal_atom": {"pred":"rank", "args":[{"Sym":"nara"},{"Sym":"gold"}]}, "layer": "all" }),
        ),
        &mut store,
    ));
    let surface_rows = surface["rows"].as_array().expect("rows");
    assert!(
        !surface_rows.is_empty() && surface_rows[0]["status"] == "certain",
        "nara's own fact never depended on the identity ruling: {surface:?}"
    );

    // The identity's ruling is defeated by a durable conditional attack;
    // the unrelated surface fact's ruling is untouched.
    let lorestore =
        dli_lorestore::Lorestore::open(lorestore_path.join("demo")).expect("reopen lorestore");
    let reference = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .expect("read main ref")
        .expect("main ref exists");
    let attacks = lorestore
        .decision_attacks_from(&reference.commit)
        .expect("read attacks");
    assert!(
        attacks
            .iter()
            .any(|attack| attack.attacker == "D-2" && attack.target == "D-3")
    );
    assert!(attacks.iter().all(|attack| attack.target != "D-1"));
}
