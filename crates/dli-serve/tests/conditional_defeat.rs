//! Contract 1.13 conditional defeat: nested restoration on the author wire,
//! durable reopen, and preservation of the released 1.12 one-way behavior.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fmt::Write as _;
use std::sync::Arc;

use dli_serve::author::{dispatch, store::Store};
use dli_serve::project::Project;
use dli_serve::serve::respond;
use dli_serve::session::Sessions;
use dli_serve::{Op, Request};
use dli_world::pack::Pack;
use serde_json::{Value, json};

fn request(version: &str, id: &str, op: Op, params: Value) -> Request {
    Request {
        dli: version.to_owned(),
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
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"conditional-defeat\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope demo\nratify d1\nperson nara \"Nara\" ratified d1\nfact nara rank gold ratified d1\n",
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
    let base = root.path().join("lorestore/demo");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    (root, store, base)
}

fn propagation_fixture(
    derivation_depth: usize,
    conclusion_count: usize,
    decision_order: &[&str],
) -> (tempfile::TempDir, Store, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"conditional-propagation\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .unwrap();
    let mut source = "scope demo\nratify d1\nratify d10\nratify d11\n".to_owned();
    for decision in decision_order {
        writeln!(&mut source, "ratify {decision}").unwrap();
        writeln!(&mut source, "declared-{decision}(nara) ratified {decision}").unwrap();
    }
    for level in 1..derivation_depth {
        writeln!(&mut source, "ratify d{}", 20 + level).unwrap();
    }
    source.push_str("ratify d19\n");
    source.push_str(concat!(
        "person nara \"Nara\" ratified d11\n",
        "p(nara) ratified d1\n",
        "independent-seed(nara) ratified d11\n",
        "redundant-seed(nara) ratified d11\n",
        "rule independent-rule ratified d10\n",
        "  independent(nara) :- independent-seed(nara)\n",
        "rule redundant-from-p ratified d10\n",
        "  redundant(nara) :- p(nara)\n",
        "rule redundant-live ratified d10\n",
        "  redundant(nara) :- redundant-seed(nara)\n",
    ));
    let mut predecessor = "p".to_owned();
    for level in 1..derivation_depth {
        let step = format!("step-{level}");
        writeln!(
            &mut source,
            "rule chain-{level} ratified d{}\n  {step}(nara) :- {predecessor}(nara)",
            20 + level
        )
        .unwrap();
        predecessor = step;
    }
    for conclusion in 1..=conclusion_count {
        writeln!(
            &mut source,
            "rule conclusion-{conclusion} ratified d19\n  q-{conclusion}(nara) :- {predecessor}(nara)"
        )
        .unwrap();
    }
    let (document, diagnostics) = dli_syntax::parse_document(&source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "propagation fixture parses: {diagnostics:?}\n{source}"
    );
    let mut store = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let base = root.path().join("lorestore/demo");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    (root, store, base)
}

fn propagation_decision_path(derivation_depth: usize) -> Vec<String> {
    let mut decisions = vec!["d1".to_owned(), "d19".to_owned()];
    decisions.extend((1..derivation_depth).map(|level| format!("d{}", 20 + level)));
    decisions.sort();
    decisions
}

fn claim_id(store: &dli_lorestore::Lorestore, kind: Option<&str>, name: &str) -> String {
    let head = store.root_hash().unwrap().unwrap();
    store
        .fact_hashes_from(&head)
        .unwrap()
        .into_iter()
        .find_map(|hash| {
            let object = store
                .read_object::<dli_lorestore::FactObject>(&hash)
                .unwrap()?;
            match kind {
                Some("ratify")
                    if object.expert_body.as_ref().is_some_and(|body| {
                        body.kind == dli_lorestore::ExpertDeclKind::Ratify && body.name == name
                    }) =>
                {
                    Some(object.claim_id.0)
                }
                None if object.atom.pred == "rank"
                    && object.atom.args.first().is_some_and(|arg| arg == name) =>
                {
                    Some(object.claim_id.0)
                }
                _ => None,
            }
        })
        .unwrap_or_else(|| panic!("stored {kind:?} claim `{name}` exists"))
}

fn predicate_claim_id(store: &dli_lorestore::Lorestore, predicate: &str) -> String {
    let head = store.root_hash().unwrap().unwrap();
    store
        .fact_hashes_from(&head)
        .unwrap()
        .into_iter()
        .find_map(|hash| {
            store
                .read_object::<dli_lorestore::FactObject>(&hash)
                .unwrap()
                .filter(|object| object.atom.pred == predicate)
                .map(|object| object.claim_id.0)
        })
        .unwrap_or_else(|| panic!("stored predicate `{predicate}` exists"))
}

fn query_predicate(store: &mut Store, workspace: &str, id: &str, predicate: &str) -> Value {
    result(dispatch(
        &request(
            "2.0.0",
            id,
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": predicate, "args": [{"Sym": "nara"}]},
                "layer": "all"
            }),
        ),
        store,
    ))
}

fn why_predicate(store: &mut Store, workspace: &str, id: &str, predicate: &str) -> Value {
    result(dispatch(
        &request(
            "2.0.0",
            id,
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": predicate, "args": [{"Sym": "nara"}]}
            }),
        ),
        store,
    ))
}

fn assert_named_standings(store: &dli_lorestore::Lorestore, expected: &[(&str, bool)]) {
    let reference = store.branch_ref(dli_lorestore::MAIN_REF).unwrap().unwrap();
    let names = store.interner_at(&reference.commit).unwrap();
    let effective = store
        .effective_valuation_at(&reference.commit, &reference.valuation)
        .unwrap();
    for (name, value) in expected {
        let atom = dli_core::Atom(
            u32::try_from(
                names
                    .iter()
                    .position(|candidate| candidate == name)
                    .unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(
            effective.valuation.get(atom),
            Some(*value),
            "standing for exact decision name `{name}`"
        );
        assert!(!effective.unassigned.contains(&atom));
    }
}

fn author_query(store: &mut Store, workspace: &str, id: &str) -> Value {
    result(dispatch(
        &request(
            "2.0.0",
            id,
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]},
                "layer": "all"
            }),
        ),
        store,
    ))
}

fn author_why(store: &mut Store, workspace: &str, id: &str) -> Value {
    result(dispatch(
        &request(
            "2.0.0",
            id,
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        store,
    ))
}

fn defeat(
    store: &mut Store,
    workspace: &str,
    id: &str,
    target_claim: &str,
    attacker: &str,
    target: &str,
) {
    let defeated = result(dispatch(
        &request(
            "2.0.0",
            id,
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": target_claim,
                "decision": attacker,
                "defeated_decision": target
            }),
        ),
        store,
    ));
    assert_eq!(defeated["findings"], json!([]));
}

#[test]
#[allow(clippy::too_many_lines)]
fn nested_defeat_restores_and_reopens_without_false_ref_bits() {
    let (root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path.clone()).unwrap();
    let d1_claim = claim_id(&lorestore, None, "nara");
    let workspace = result(dispatch(
        &request("2.0.0", "open", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();

    defeat(&mut store, &workspace, "d2", &d1_claim, "d2", "d1");
    let head_after_first = lorestore.root_hash().unwrap().unwrap();
    let original = lorestore
        .fact_hashes_from(&head_after_first)
        .unwrap()
        .into_iter()
        .find_map(|hash| {
            lorestore
                .read_object::<dli_lorestore::FactObject>(&hash)
                .unwrap()
                .filter(|object| object.claim_id.0 == d1_claim)
        })
        .unwrap();
    assert_eq!(
        original
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.defeated_by.as_deref()),
        None,
        "conditional defeat does not stamp one-way defeated metadata"
    );
    assert!(
        author_query(&mut store, &workspace, "q1")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let blocked_why = result(dispatch(
        &request(
            "2.0.0",
            "why1",
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut store,
    ));
    assert_eq!(blocked_why["active_supporting_decision_paths"], json!([]));
    assert_eq!(
        blocked_why["blocking_defeats"],
        json!([{"defeated_decision": "d1", "defeating_decision": "d2"}])
    );
    assert_eq!(
        blocked_why["blocked_supports"],
        json!([{
            "supporting_decision_path": ["d1"],
            "blocking_defeats": [{"defeated_decision": "d1", "defeating_decision": "d2"}]
        }])
    );

    let d2_claim = claim_id(&lorestore, Some("ratify"), "d2");
    defeat(&mut store, &workspace, "d3", &d2_claim, "d3", "d2");
    let restored = author_query(&mut store, &workspace, "q2");
    let restored_why = result(dispatch(
        &request(
            "2.0.0",
            "why2",
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut store,
    ));
    let debug_head = lorestore.root_hash().unwrap().unwrap();
    let debug_pack = lorestore.reconstruct_at(&debug_head).unwrap();
    let debug_decls: Vec<String> = debug_pack
        .documents
        .iter()
        .flat_map(|document| document.decls.iter())
        .map(|decl| format!("{decl:?}"))
        .collect();
    assert_eq!(
        restored["rows"][0]["status"], "certain",
        "nested defeat restores the original support: query={restored}, why={restored_why}, decls={debug_decls:?}"
    );
    assert_eq!(
        restored_why["active_supporting_decision_paths"],
        json!([["d1"]])
    );
    assert_eq!(restored_why["blocking_defeats"], json!([]));
    assert_eq!(restored_why["blocked_supports"], json!([]));

    let d3_claim = claim_id(&lorestore, Some("ratify"), "d3");
    defeat(&mut store, &workspace, "d4", &d3_claim, "d4", "d3");
    assert!(
        author_query(&mut store, &workspace, "q3")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let reblocked_why = result(dispatch(
        &request(
            "2.0.0",
            "why3",
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut store,
    ));
    assert_eq!(reblocked_why["active_supporting_decision_paths"], json!([]));
    assert_eq!(
        reblocked_why["blocking_defeats"],
        json!([{"defeated_decision": "d1", "defeating_decision": "d2"}])
    );

    let reference = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .unwrap()
        .unwrap();
    assert!(
        reference.valuation.0.is_empty(),
        "conditional standing is derived, never persisted as false Ref bits"
    );
    assert_eq!(
        lorestore
            .decision_attacks_from(&reference.commit)
            .unwrap()
            .len(),
        3
    );

    // Pin the project to this exact attacked commit. A fresh project/session
    // must explain that snapshot, never the ambient tip or the original pack.
    std::fs::write(
        root.path().join("deloreis.toml"),
        format!(
            "version = 1\nname = \"conditional-defeat\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\nhead = \"{}\"\n",
            reference.commit
        ),
    )
    .unwrap();
    // A fresh pinned project/session reconstructs the attack graph from the
    // commit and reaches the same dead result without borrowing the live author
    // workspace.
    let project = Arc::new(Project::load(root.path()).unwrap());
    let packs = project.documents().map(|(id, document)| {
        (
            id.to_owned(),
            document.pack.clone(),
            document.killed.clone(),
        )
    });
    let mut author = Store::with_packs(packs).unwrap();
    for (id, document) in project.documents() {
        if let (Some(origin), Some(commit)) = (&document.store_origin, &document.store_commit) {
            author
                .register_store_origin(
                    id.to_owned(),
                    origin.clone(),
                    document.store_pinned,
                    commit.clone(),
                    document.store_valuation.clone(),
                    document.store_unassigned.clone(),
                    document.store_ref_digest.clone(),
                )
                .expect("store standing maps by exact name");
        }
    }
    author
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    let mut sessions = Sessions::from_project(Arc::clone(&project));
    let frame = json!({
        "dli": "2.0.0",
        "id": "reopen",
        "op": "dli.query",
        "params": {
            "document": "demo",
            "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]},
            "layer": "all"
        }
    });
    let reopened = respond(&frame.to_string(), &mut sessions, &mut author).unwrap();
    assert!(reopened.ok, "reopened query: {reopened:?}");
    assert!(
        reopened.result.unwrap()["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let reopened_open = json!({
        "dli": "2.0.0", "id": "author-reopen", "op": "author.open",
        "params": {"scope": "demo"}
    });
    let reopened_open = respond(&reopened_open.to_string(), &mut sessions, &mut author).unwrap();
    let reopened_workspace = reopened_open.result.unwrap()["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let reopened_why = result(dispatch(
        &request(
            "2.0.0",
            "author-why-reopen",
            Op::AuthorWhy,
            json!({
                "workspace": reopened_workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut author,
    ));
    assert_eq!(reopened_why["active_supporting_decision_paths"], json!([]));
    assert_eq!(
        reopened_why["blocking_defeats"],
        json!([{"defeated_decision": "d1", "defeating_decision": "d2"}])
    );
    author
        .get(&reopened_workspace)
        .unwrap()
        .set_compile_cache_enabled(false);
    let reopened_miss = result(dispatch(
        &request(
            "2.0.0",
            "author-why-reopen-miss",
            Op::AuthorWhy,
            json!({
                "workspace": reopened_workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut author,
    ));
    author
        .get(&reopened_workspace)
        .unwrap()
        .set_compile_cache_enabled(true);
    assert_eq!(reopened_miss, reopened_why, "pinned-session forced miss");
}

#[test]
fn stale_sibling_bulk_installs_the_current_attack_valuation() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let target_claim = claim_id(&lorestore, None, "nara");

    let first = result(dispatch(
        &request("2.0.0", "open-a", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let stale = result(dispatch(
        &request("2.0.0", "open-b", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();

    defeat(&mut store, &first, "defeat", &target_claim, "d2", "d1");
    assert!(
        author_query(&mut store, &first, "first-query")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let before_bulk = store
        .workspace_lorestore(&stale)
        .unwrap()
        .branch_ref(dli_lorestore::MAIN_REF)
        .unwrap()
        .unwrap();
    let before_names = store
        .workspace_lorestore(&stale)
        .unwrap()
        .interner_at(&before_bulk.commit)
        .unwrap();
    let before_effective = store
        .workspace_lorestore(&stale)
        .unwrap()
        .effective_valuation_at(&before_bulk.commit, &before_bulk.valuation)
        .unwrap();
    let effective_by_name = dli_core::valuation::NamedValuation::from_indexed(
        &before_effective.valuation,
        &before_names,
    )
    .unwrap();
    assert_eq!(effective_by_name.0.get("d1"), Some(&false));

    let bulk = result(dispatch(
        &request(
            "2.0.0",
            "bulk",
            Op::AuthorRecordDecisions,
            json!({"workspace": stale, "records": [{"decision": "d3"}]}),
        ),
        &mut store,
    ));
    assert_eq!(bulk["recorded"], 1);
    assert_eq!(
        store.get(&stale).unwrap().base_valuation.0.get("d1"),
        Some(&false)
    );

    let stale_rows = author_query(&mut store, &stale, "stale-query")["rows"].clone();
    let fresh = result(dispatch(
        &request(
            "2.0.0",
            "open-c",
            Op::AuthorOpen,
            json!({"scope": "demo", "branch": "main"}),
        ),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let fresh_rows = author_query(&mut store, &fresh, "fresh-query")["rows"].clone();
    assert_eq!(
        stale_rows, fresh_rows,
        "bulk install must match its committed branch head"
    );
}

#[test]
fn duplicate_projected_blocked_paths_still_report_the_global_blocker() {
    let root = tempfile::tempdir().unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(concat!(
        "scope demo\n",
        "ratify d1\n",
        "ratify d2\n",
        "ratify d10\n",
        "person nara \"Nara\" proposed seed-person\n",
        "p(nara) ratified d1\n",
        "left(nara) proposed seed-left\n",
        "right(nara) proposed seed-right\n",
        "rule left-path ratified d10\n",
        "  q(nara) :- p(nara), left(nara)\n",
        "rule right-path ratified d10\n",
        "  q(nara) :- p(nara), right(nara)\n",
    ));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let mut author = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let path = root.path().join("lorestore/demo");
    author
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();

    // Resolve both independent proposal gates true so both symbolic paths are
    // certain before the same decision attack blocks them.
    let current = lorestore.head_ref().unwrap().unwrap();
    let token = current.digest().unwrap().to_string();
    let names = lorestore.interner_at(&current.commit).unwrap();
    let mut valuation = current.valuation;
    let proposal_atoms = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.starts_with("h."))
        .map(|(index, _)| dli_core::Atom(u32::try_from(index).unwrap()))
        .collect::<Vec<_>>();
    assert!(proposal_atoms.len() >= 2, "{names:?}");
    for atom in proposal_atoms {
        valuation.set(atom, true);
    }
    let updated = dli_lorestore::Ref::with_valuation("main", current.commit, valuation);
    assert!(lorestore.cas_main_ref(Some(&token), &updated).unwrap());

    let p_claim = predicate_claim_id(&lorestore, "p");
    let workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-projected",
            Op::AuthorOpen,
            json!({"scope": "demo", "branch": "main"}),
        ),
        &mut author,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    defeat(
        &mut author,
        &workspace,
        "block-projected",
        &p_claim,
        "d2",
        "d1",
    );

    let query = query_predicate(&mut author, &workspace, "query-projected", "q");
    assert!(query["rows"].as_array().unwrap().is_empty(), "{query}");
    let why = why_predicate(&mut author, &workspace, "why-projected", "q");
    assert_eq!(
        why["blocking_defeats"],
        json!([{"defeated_decision": "d1", "defeating_decision": "d2"}]),
        "every symbolic path is blocked by the same attack: {why}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn author_why_is_pinned_to_each_branch_workspace_root() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let claim = claim_id(&lorestore, None, "nara");
    let main_workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-main",
            Op::AuthorOpen,
            json!({"scope": "demo"}),
        ),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    result(dispatch(
        &request(
            "2.0.0",
            "create-rival",
            Op::AuthorBranch,
            json!({
                "workspace": main_workspace,
                "action": "create",
                "name": "rival",
                "from": "main"
            }),
        ),
        &mut store,
    ));
    let rival_workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-rival",
            Op::AuthorOpen,
            json!({"scope": "demo", "branch": "rival"}),
        ),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    defeat(
        &mut store,
        &rival_workspace,
        "rival-defeat",
        &claim,
        "d2",
        "d1",
    );

    let rival_why = result(dispatch(
        &request(
            "2.0.0",
            "why-rival",
            Op::AuthorWhy,
            json!({
                "workspace": rival_workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut store,
    ));
    assert_eq!(rival_why["active_supporting_decision_paths"], json!([]));
    assert_eq!(
        rival_why["blocking_defeats"],
        json!([{"defeated_decision": "d1", "defeating_decision": "d2"}])
    );
    store
        .get(&rival_workspace)
        .unwrap()
        .set_compile_cache_enabled(false);
    let rival_miss = author_why(&mut store, &rival_workspace, "why-rival-miss");
    store
        .get(&rival_workspace)
        .unwrap()
        .set_compile_cache_enabled(true);
    assert_eq!(rival_miss, rival_why, "rival branch forced miss");

    let main_why = result(dispatch(
        &request(
            "2.0.0",
            "why-main",
            Op::AuthorWhy,
            json!({
                "workspace": main_workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut store,
    ));
    assert_eq!(
        main_why["active_supporting_decision_paths"],
        json!([["d1"]])
    );
    assert_eq!(main_why["blocking_defeats"], json!([]));
    assert_eq!(main_why["blocked_supports"], json!([]));
    store
        .get(&main_workspace)
        .unwrap()
        .set_compile_cache_enabled(false);
    let main_miss = author_why(&mut store, &main_workspace, "why-main-miss");
    store
        .get(&main_workspace)
        .unwrap()
        .set_compile_cache_enabled(true);
    assert_eq!(main_miss, main_why, "main sibling forced miss");
}

#[test]
fn contract_1_12_defeat_keeps_the_frozen_one_way_ref_semantics() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let claim = claim_id(&lorestore, None, "nara");
    let workspace = result(dispatch(
        &request("1.12.0", "open", Op::AuthorOpen, json!({"scope": "demo"})),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    result(dispatch(
        &request(
            "1.12.0",
            "defeat",
            Op::AuthorDefeat,
            json!({
                "workspace": workspace,
                "claim_id": claim,
                "decision": "d2",
                "defeated_decision": "d1"
            }),
        ),
        &mut store,
    ));
    let reference = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .unwrap()
        .unwrap();
    assert_eq!(
        lorestore.decision_attacks_from(&reference.commit).unwrap(),
        []
    );
    let names = lorestore.interner_at(&reference.commit).unwrap();
    let d1 = dli_core::Atom(
        u32::try_from(names.iter().position(|name| name == "d1").unwrap())
            .expect("decision atom index fits u32"),
    );
    assert_eq!(reference.valuation.get(d1), Some(false));
    let legacy_why = result(dispatch(
        &request(
            "1.12.0",
            "why",
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}
            }),
        ),
        &mut store,
    ));
    for field in [
        "active_supporting_decision_paths",
        "blocking_defeats",
        "blocked_supports",
    ] {
        assert!(
            legacy_why.get(field).is_none(),
            "legacy field `{field}` absent"
        );
    }
    assert_eq!(
        serde_json::to_string(&legacy_why).unwrap(),
        r#"{"supporting_decisions":["d1"],"why":{"antichain":[[0]],"rendered":["d1"]},"why_not":{"atom":"rank(nara, gold)","blocking":[],"closest":[],"defeater":null,"known":true,"rendered":["dead — no admissible support"],"status":"dead"}}"#,
        "the released 1.12 result bytes remain frozen"
    );
}

#[test]
fn contract_1_13_unknown_atom_has_explicit_empty_explanation_arrays() {
    let (_root, mut store, _path) = fixture();
    let workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-unknown",
            Op::AuthorOpen,
            json!({"scope": "demo"}),
        ),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let why = why_predicate(&mut store, &workspace, "why-unknown", "never-known");
    assert_eq!(why["active_supporting_decision_paths"], json!([]));
    assert_eq!(why["blocking_defeats"], json!([]));
    assert_eq!(why["blocked_supports"], json!([]));
}

#[test]
#[allow(clippy::too_many_lines)]
fn self_mutual_and_odd_cycles_are_possible_end_to_end() {
    for (name, edges) in [
        ("self", vec![("d1", "d1")]),
        ("mutual", vec![("d2", "d1"), ("d1", "d2")]),
        ("odd", vec![("d2", "d1"), ("d3", "d2"), ("d1", "d3")]),
    ] {
        let (root, mut store, path) = fixture();
        let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
        let workspace = result(dispatch(
            &request(
                "2.0.0",
                &format!("open-{name}"),
                Op::AuthorOpen,
                json!({"scope": "demo"}),
            ),
            &mut store,
        ))["workspace"]
            .as_str()
            .unwrap()
            .to_owned();
        for (attacker, target) in edges {
            let carrier = if target == "d1" {
                claim_id(&lorestore, None, "nara")
            } else {
                claim_id(&lorestore, Some("ratify"), target)
            };
            defeat(
                &mut store,
                &workspace,
                &format!("{name}-{attacker}-{target}"),
                &carrier,
                attacker,
                target,
            );
        }
        let live = author_query(&mut store, &workspace, &format!("query-{name}"));
        assert_eq!(live["rows"][0]["status"], "possible", "{name}: {live}");
        let canon_adjacent_before = author_why(&mut store, &workspace, &format!("why-{name}"));

        let project = Project::load(root.path()).unwrap();
        let mut sessions = Sessions::from_project(Arc::new(project));
        let mut author = Store::default();
        let frame = json!({
            "dli": "2.0.0", "id": format!("reopen-{name}"), "op": "dli.query",
            "params": {"document": "demo", "goal_atom": {"pred": "rank", "args": [{"Sym": "nara"}, {"Sym": "gold"}]}, "layer": "all"}
        });
        let reopened = respond(&frame.to_string(), &mut sessions, &mut author).unwrap();
        assert_eq!(
            reopened.result.unwrap()["rows"][0]["status"],
            "possible",
            "{name}"
        );
        if name == "odd" {
            for format in ["objects", "canon"] {
                let export = json!({
                    "dli": "2.0.0", "id": format!("export-{format}"),
                    "op": "dli.corpus.export",
                    "params": {"document": "demo", "format": format}
                });
                let response = respond(&export.to_string(), &mut sessions, &mut author).unwrap();
                assert!(response.ok, "{response:?}");
                let value = response.result.unwrap();
                let attacks = if format == "canon" {
                    &value["canon"]["attacks"]
                } else {
                    &value["attacks"]
                };
                assert_eq!(attacks.as_array().unwrap().len(), 3);
                assert!(
                    attacks
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|edge| edge.get("subject_fact_hash").is_some())
                );
            }
            let canon_adjacent_after =
                author_why(&mut store, &workspace, "why-after-canon-exports");
            store
                .get(&workspace)
                .unwrap()
                .set_compile_cache_enabled(false);
            let canon_adjacent_miss = author_why(&mut store, &workspace, "why-after-canon-miss");
            store
                .get(&workspace)
                .unwrap()
                .set_compile_cache_enabled(true);
            for field in [
                "active_supporting_decision_paths",
                "blocking_defeats",
                "blocked_supports",
            ] {
                assert_eq!(
                    canon_adjacent_after[field], canon_adjacent_before[field],
                    "canon-adjacent reads do not move author.why field `{field}`"
                );
                assert_eq!(
                    canon_adjacent_miss[field], canon_adjacent_after[field],
                    "canon-adjacent forced miss preserves author.why field `{field}`"
                );
            }
        }
    }
}

#[test]
fn depths_one_two_four_and_eight_alternate_deterministically() {
    for depth in [1usize, 2, 4, 8] {
        let (_root, mut store, path) = fixture();
        let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
        let workspace = result(dispatch(
            &request(
                "2.0.0",
                &format!("open-{depth}"),
                Op::AuthorOpen,
                json!({"scope": "demo"}),
            ),
            &mut store,
        ))["workspace"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut target = "d1".to_owned();
        let mut carrier = claim_id(&lorestore, None, "nara");
        for step in 2..=(depth + 1) {
            let attacker = format!("d{step}");
            defeat(
                &mut store,
                &workspace,
                &format!("depth-{depth}-{step}"),
                &carrier,
                &attacker,
                &target,
            );
            target = attacker;
            carrier = claim_id(&lorestore, Some("ratify"), &target);
        }
        let rows = author_query(&mut store, &workspace, &format!("q-{depth}"))["rows"]
            .as_array()
            .unwrap()
            .clone();
        if depth % 2 == 0 {
            assert_eq!(rows[0]["status"], "certain", "depth {depth}");
        } else {
            assert!(rows.is_empty(), "depth {depth}: {rows:?}");
        }
    }
}

#[test]
fn defeat_idempotency_replay_does_not_advance_the_ref() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let claim = claim_id(&lorestore, None, "nara");
    let workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-idem",
            Op::AuthorOpen,
            json!({"scope": "demo"}),
        ),
        &mut store,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut frame = request(
        "2.0.0",
        "idem",
        Op::AuthorDefeat,
        json!({
            "workspace": workspace, "claim_id": claim, "decision": "d2", "defeated_decision": "d1"
        }),
    );
    frame.idempotency_key = Some("same-defeat".to_owned());
    let first = result(dispatch(&frame, &mut store));
    let head = lorestore
        .branch_ref(dli_lorestore::MAIN_REF)
        .unwrap()
        .unwrap();
    let second = result(dispatch(&frame, &mut store));
    assert_eq!(second, first);
    assert_eq!(
        lorestore
            .branch_ref(dli_lorestore::MAIN_REF)
            .unwrap()
            .unwrap(),
        head
    );
    assert_eq!(
        lorestore.decision_attacks_from(&head.commit).unwrap().len(),
        1
    );
}

#[test]
fn multiple_attackers_are_order_independent_end_to_end() {
    let mut projections = Vec::new();
    for order in [["d2", "d3"], ["d3", "d2"]] {
        let (_root, mut store, path) = fixture();
        let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
        let claim = claim_id(&lorestore, None, "nara");
        let workspace = result(dispatch(
            &request(
                "2.0.0",
                "open-order",
                Op::AuthorOpen,
                json!({"scope": "demo"}),
            ),
            &mut store,
        ))["workspace"]
            .as_str()
            .unwrap()
            .to_owned();
        for attacker in order {
            defeat(
                &mut store,
                &workspace,
                &format!("edge-{attacker}"),
                &claim,
                attacker,
                "d1",
            );
        }
        assert!(
            author_query(&mut store, &workspace, "order-query")["rows"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let head = lorestore.root_hash().unwrap().unwrap();
        projections.push(
            lorestore
                .decision_attacks_from(&head)
                .unwrap()
                .into_iter()
                .map(|attack| (attack.attacker, attack.target))
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(projections[0], projections[1]);
}

#[test]
#[allow(clippy::too_many_lines)]
fn real_rule_propagation_matrix_separates_derivation_depth_from_attack_depth() {
    // `derivation_depth` counts evaluator rule edges from p to each q. Attack
    // depth is independently one (d2 -> d1) and then two (d3 -> d2 -> d1).
    // The (1, 1) cell is the literal P -> Q acceptance case.
    for derivation_depth in [1usize, 2, 4, 8] {
        for conclusion_count in [1usize, 2, 5] {
            let (_root, mut author, path) =
                propagation_fixture(derivation_depth, conclusion_count, &["d2", "d3"]);
            let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
            let p_claim = predicate_claim_id(&lorestore, "p");
            let workspace = result(dispatch(
                &request(
                    "2.0.0",
                    "open-propagation",
                    Op::AuthorOpen,
                    json!({"scope": "demo"}),
                ),
                &mut author,
            ))["workspace"]
                .as_str()
                .unwrap()
                .to_owned();

            for conclusion in 1..=conclusion_count {
                let predicate = format!("q-{conclusion}");
                let query = query_predicate(
                    &mut author,
                    &workspace,
                    &format!("before-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &predicate,
                );
                assert_eq!(
                    query["rows"][0]["status"], "certain",
                    "derivation depth {derivation_depth}, shared conclusion {conclusion}/{conclusion_count}: {query}"
                );
                let why = why_predicate(
                    &mut author,
                    &workspace,
                    &format!("why-before-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &predicate,
                );
                assert_eq!(
                    why["active_supporting_decision_paths"],
                    json!([propagation_decision_path(derivation_depth)]),
                    "full active path at derivation depth {derivation_depth}: {why}"
                );
                assert_eq!(why["blocking_defeats"], json!([]));
                assert_eq!(why["blocked_supports"], json!([]));
            }

            defeat(
                &mut author,
                &workspace,
                "attack-depth-1",
                &p_claim,
                "d2",
                "d1",
            );
            assert_named_standings(&lorestore, &[("d1", false), ("d2", true)]);
            assert_eq!(
                lorestore
                    .decision_attacks_from(&lorestore.root_hash().unwrap().unwrap())
                    .unwrap()
                    .len(),
                1
            );
            for conclusion in 1..=conclusion_count {
                let predicate = format!("q-{conclusion}");
                let query = query_predicate(
                    &mut author,
                    &workspace,
                    &format!("retracted-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &predicate,
                );
                assert_eq!(
                    query["rows"],
                    json!([]),
                    "one decision attack retracts every result at derivation depth {derivation_depth}: {query}"
                );
                let why = why_predicate(
                    &mut author,
                    &workspace,
                    &format!("why-dead-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &predicate,
                );
                let blocker = json!({"defeated_decision": "d1", "defeating_decision": "d2"});
                assert_eq!(why["active_supporting_decision_paths"], json!([]));
                assert_eq!(why["blocking_defeats"], json!([blocker.clone()]));
                assert_eq!(
                    why["blocked_supports"],
                    json!([{
                        "supporting_decision_path": propagation_decision_path(derivation_depth),
                        "blocking_defeats": [blocker]
                    }]),
                    "full blocked path at derivation depth {derivation_depth}: {why}"
                );
            }
            for control in ["independent", "redundant"] {
                let query = query_predicate(
                    &mut author,
                    &workspace,
                    &format!("control-dead-{derivation_depth}-{conclusion_count}-{control}"),
                    control,
                );
                assert_eq!(
                    query["rows"][0]["status"], "certain",
                    "{control} is an independent/redundant live-support control: {query}"
                );
                let why = why_predicate(
                    &mut author,
                    &workspace,
                    &format!("why-control-{derivation_depth}-{conclusion_count}-{control}"),
                    control,
                );
                assert_eq!(
                    why["active_supporting_decision_paths"],
                    json!([["d10", "d11"]]),
                    "{control}: {why}"
                );
                assert_eq!(
                    why["blocking_defeats"],
                    json!([]),
                    "a live independent path keeps the conclusion unblocked: {why}"
                );
                if control == "redundant" {
                    assert_eq!(
                        why["blocked_supports"],
                        json!([{
                            "supporting_decision_path": ["d1", "d10"],
                            "blocking_defeats": [{
                                "defeated_decision": "d1",
                                "defeating_decision": "d2"
                            }]
                        }]),
                        "the blocked redundant alternative stays separate: {why}"
                    );
                } else {
                    assert_eq!(why["blocked_supports"], json!([]));
                }
            }

            let d2_claim = claim_id(&lorestore, Some("ratify"), "d2");
            defeat(
                &mut author,
                &workspace,
                "attack-depth-2",
                &d2_claim,
                "d3",
                "d2",
            );
            assert_named_standings(&lorestore, &[("d1", true), ("d2", false), ("d3", true)]);
            assert_eq!(
                lorestore
                    .decision_attacks_from(&lorestore.root_hash().unwrap().unwrap())
                    .unwrap()
                    .len(),
                2
            );
            for conclusion in 1..=conclusion_count {
                let predicate = format!("q-{conclusion}");
                let query = query_predicate(
                    &mut author,
                    &workspace,
                    &format!("restored-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &predicate,
                );
                assert_eq!(
                    query["rows"][0]["status"], "certain",
                    "defeating the attacker restores all {conclusion_count} results through {derivation_depth} real rule edges: {query}"
                );
                let why = why_predicate(
                    &mut author,
                    &workspace,
                    &format!("why-restored-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &predicate,
                );
                assert_eq!(
                    why["active_supporting_decision_paths"],
                    json!([propagation_decision_path(derivation_depth)])
                );
                assert_eq!(why["blocking_defeats"], json!([]));
                assert_eq!(why["blocked_supports"], json!([]));
            }
        }
    }
}

#[test]
fn multiple_attackers_require_every_live_attack_to_be_defeated() {
    let (_root, mut author, path) = propagation_fixture(2, 2, &["d2", "d3", "d4", "d5"]);
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-multi",
            Op::AuthorOpen,
            json!({"scope": "demo"}),
        ),
        &mut author,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let p_claim = predicate_claim_id(&lorestore, "p");
    defeat(&mut author, &workspace, "d2-d1", &p_claim, "d2", "d1");
    defeat(&mut author, &workspace, "d3-d1", &p_claim, "d3", "d1");
    assert_named_standings(&lorestore, &[("d1", false), ("d2", true), ("d3", true)]);

    let d2_claim = claim_id(&lorestore, Some("ratify"), "d2");
    defeat(&mut author, &workspace, "d4-d2", &d2_claim, "d4", "d2");
    assert_named_standings(
        &lorestore,
        &[("d1", false), ("d2", false), ("d3", true), ("d4", true)],
    );
    assert!(
        query_predicate(&mut author, &workspace, "still-retracted", "q-1")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let d3_claim = claim_id(&lorestore, Some("ratify"), "d3");
    defeat(&mut author, &workspace, "d5-d3", &d3_claim, "d5", "d3");
    assert_named_standings(
        &lorestore,
        &[
            ("d1", true),
            ("d2", false),
            ("d3", false),
            ("d4", true),
            ("d5", true),
        ],
    );
    for predicate in ["q-1", "q-2", "independent", "redundant"] {
        let query = query_predicate(&mut author, &workspace, "multi-restored", predicate);
        assert_eq!(query["rows"][0]["status"], "certain", "{query}");
    }
}

#[test]
fn all_redundant_paths_report_their_distinct_direct_blockers() {
    let (_root, mut author, path) = propagation_fixture(1, 1, &["d2", "d3"]);
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-distinct",
            Op::AuthorOpen,
            json!({"scope": "demo"}),
        ),
        &mut author,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    let p_claim = predicate_claim_id(&lorestore, "p");
    let redundant_seed_claim = predicate_claim_id(&lorestore, "redundant-seed");
    defeat(&mut author, &workspace, "distinct-d1", &p_claim, "d2", "d1");
    defeat(
        &mut author,
        &workspace,
        "distinct-d11",
        &redundant_seed_claim,
        "d3",
        "d11",
    );

    let why = why_predicate(&mut author, &workspace, "why-distinct", "redundant");
    assert_eq!(why["active_supporting_decision_paths"], json!([]));
    assert_eq!(
        why["blocking_defeats"],
        json!([
            {"defeated_decision": "d1", "defeating_decision": "d2"},
            {"defeated_decision": "d11", "defeating_decision": "d3"}
        ])
    );
    assert_eq!(
        why["blocked_supports"],
        json!([
            {
                "supporting_decision_path": ["d1", "d10"],
                "blocking_defeats": [
                    {"defeated_decision": "d1", "defeating_decision": "d2"}
                ]
            },
            {
                "supporting_decision_path": ["d10", "d11"],
                "blocking_defeats": [
                    {"defeated_decision": "d11", "defeating_decision": "d3"}
                ]
            }
        ])
    );
}

#[test]
fn possible_alternative_prevents_a_blocked_path_from_becoming_a_global_blocker() {
    let root = tempfile::tempdir().unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(concat!(
        "scope demo\n",
        "ratify d1\n",
        "ratify d2\n",
        "ratify d10\n",
        "person nara \"Nara\" proposed seed-person\n",
        "p(nara) ratified d1\n",
        "open-seed(nara) proposed seed-open\n",
        "rule from-p ratified d10\n",
        "  mixed(nara) :- p(nara)\n",
        "rule from-open ratified d10\n",
        "  mixed(nara) :- open-seed(nara)\n",
    ));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "mixed-standing fixture parses: {diagnostics:?}"
    );
    let mut author = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let path = root.path().join("lorestore/demo");
    author
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let p_claim = predicate_claim_id(&lorestore, "p");
    let workspace = result(dispatch(
        &request(
            "2.0.0",
            "open-mixed",
            Op::AuthorOpen,
            json!({"scope": "demo"}),
        ),
        &mut author,
    ))["workspace"]
        .as_str()
        .unwrap()
        .to_owned();
    defeat(
        &mut author,
        &workspace,
        "block-one-mixed-path",
        &p_claim,
        "d2",
        "d1",
    );

    let query = query_predicate(&mut author, &workspace, "query-mixed", "mixed");
    assert_eq!(query["rows"][0]["status"], "possible", "{query}");
    let why = why_predicate(&mut author, &workspace, "why-mixed", "mixed");
    assert_eq!(why["active_supporting_decision_paths"], json!([]));
    assert_eq!(
        why["blocking_defeats"],
        json!([]),
        "an open alternative means the conclusion is not globally attack-blocked: {why}"
    );
    assert_eq!(
        why["blocked_supports"],
        json!([{
            "supporting_decision_path": ["d1", "d10"],
            "blocking_defeats": [{
                "defeated_decision": "d1",
                "defeating_decision": "d2"
            }]
        }]),
        "the independently blocked alternative remains available: {why}"
    );
}

#[test]
fn six_legal_declaration_and_attack_orders_have_exact_named_standing() {
    let edges = [("d2", "d1"), ("d3", "d1"), ("d4", "d2")];
    let permutations = [
        [0usize, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut projections = Vec::new();
    let mut explanations = Vec::new();
    for (case, permutation) in permutations.into_iter().enumerate() {
        let declarations: Vec<_> = permutation.iter().map(|index| edges[*index].0).collect();
        let (_root, mut author, path) = propagation_fixture(1, 1, &declarations);
        let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
        let workspace = result(dispatch(
            &request(
                "2.0.0",
                &format!("open-order-{case}"),
                Op::AuthorOpen,
                json!({"scope": "demo"}),
            ),
            &mut author,
        ))["workspace"]
            .as_str()
            .unwrap()
            .to_owned();
        for index in permutation {
            let (attacker, target) = edges[index];
            let carrier = if target == "d1" {
                predicate_claim_id(&lorestore, "p")
            } else {
                claim_id(&lorestore, Some("ratify"), target)
            };
            defeat(
                &mut author,
                &workspace,
                &format!("order-{case}-{attacker}-{target}"),
                &carrier,
                attacker,
                target,
            );
        }
        assert_named_standings(
            &lorestore,
            &[("d1", false), ("d2", false), ("d3", true), ("d4", true)],
        );
        assert!(
            query_predicate(&mut author, &workspace, "ordered-q", "q-1")["rows"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let why = why_predicate(&mut author, &workspace, "ordered-why", "q-1");
        explanations.push(json!({
            "active": why["active_supporting_decision_paths"],
            "blocking": why["blocking_defeats"],
            "blocked": why["blocked_supports"]
        }));
        for control in ["independent", "redundant"] {
            assert_eq!(
                query_predicate(&mut author, &workspace, "ordered-control", control)["rows"][0]["status"],
                "certain"
            );
        }
        let head = lorestore.root_hash().unwrap().unwrap();
        projections.push(
            lorestore
                .decision_attacks_from(&head)
                .unwrap()
                .into_iter()
                .map(|attack| (attack.attacker, attack.target))
                .collect::<Vec<_>>(),
        );
    }
    assert!(projections.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(
        explanations.windows(2).all(|pair| pair[0] == pair[1]),
        "author.why is declaration/attack order independent: {explanations:#?}"
    );
}

fn skew_fixture() -> (tempfile::TempDir, Store, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("deloreis.toml"),
        "version = 1\nname = \"explanation-skew\"\n\n[documents.demo]\nstore = \"lorestore/demo\"\n",
    )
    .unwrap();
    let (document, diagnostics) =
        dli_syntax::parse_document("scope demo\nperson nara \"Nara\" proposed seed\n");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "skew fixture parses: {diagnostics:?}"
    );
    let mut store = Store::with_pack(Pack {
        name: "demo".to_owned(),
        documents: vec![document],
    })
    .unwrap();
    let base = root.path().join("lorestore/demo");
    store
        .attach_lorestore(root.path().join("lorestore"))
        .unwrap();
    (root, store, base)
}

fn ingest_accepted_fact(
    store: &mut Store,
    workspace: &str,
    id: &str,
    key: &str,
    value: &str,
    decision: &str,
) {
    let added = result(dispatch(
        &request(
            "2.0.0",
            &format!("add-accepted-{id}"),
            Op::AuthorAddFact,
            json!({
                "workspace": workspace,
                "entity": "nara",
                "key": key,
                "value": value,
                "provenance": {"kind": "proposed", "origin": format!("batch-{id}")}
            }),
        ),
        store,
    ));
    assert_eq!(
        added["findings"],
        json!([]),
        "stage accepted fact {id}: {added}"
    );
    let materialized = result(dispatch(
        &request(
            "2.0.0",
            &format!("materialize-accepted-{id}"),
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false, "decision": decision}),
        ),
        store,
    ));
    assert_eq!(
        materialized["findings"],
        json!([]),
        "materialize accepted fact {id}: {materialized}"
    );
    store
        .reload_workspace_base(workspace)
        .unwrap_or_else(|error| panic!("reload after accepted fact {id}: {error}"));
}

fn stage_cited_fact(
    store: &mut Store,
    workspace: &str,
    id: &str,
    key: &str,
    value: &str,
    decision: &str,
) {
    let added = result(dispatch(
        &request(
            "2.0.0",
            &format!("add-{id}"),
            Op::AuthorAddFact,
            json!({
                "workspace": workspace,
                "entity": "nara",
                "key": key,
                "value": value,
                "provenance": {
                    "kind": "cited",
                    "decisions": [decision],
                    "state": "ratified"
                }
            }),
        ),
        store,
    ));
    assert_eq!(
        added["findings"],
        json!([]),
        "stage cited fact {id}: {added}"
    );
}

fn materialize_cited_facts(store: &mut Store, workspace: &str) {
    let materialized = result(dispatch(
        &request(
            "2.0.0",
            "materialize-cited-facts",
            Op::AuthorMaterialize,
            json!({"workspace": workspace, "rendered": false}),
        ),
        store,
    ));
    assert_eq!(
        materialized["findings"],
        json!([]),
        "materialize cited facts: {materialized}"
    );
    store
        .reload_workspace_base(workspace)
        .unwrap_or_else(|error| panic!("reload after cited facts: {error}"));
}

fn explanation_fields(why: &Value) -> Value {
    json!({
        "active_supporting_decision_paths": why["active_supporting_decision_paths"],
        "blocking_defeats": why["blocking_defeats"],
        "blocked_supports": why["blocked_supports"]
    })
}

fn why_attribute(store: &mut Store, workspace: &str, id: &str, key: &str, value: &str) -> Value {
    result(dispatch(
        &request(
            "2.0.0",
            id,
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {
                    "pred": key,
                    "args": [{"Sym": "nara"}, {"Sym": value}]
                }
            }),
        ),
        store,
    ))
}

#[test]
#[allow(clippy::too_many_lines)]
fn six_persisted_compile_skews_keep_exact_name_explanations_byte_identical() {
    let batches = [
        ("support", "rank", "captain", "d1"),
        ("oath", "oath", "iron", "d-alpha"),
        ("banner", "banner", "white", "d-omega"),
    ];
    let permutations = [
        [0usize, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let expected_active = json!({
        "active_supporting_decision_paths": [["d1"]],
        "blocking_defeats": [],
        "blocked_supports": []
    });
    let expected_blocked = json!({
        "active_supporting_decision_paths": [],
        "blocking_defeats": [{
            "defeated_decision": "d1",
            "defeating_decision": "d2"
        }],
        "blocked_supports": [{
            "supporting_decision_path": ["d1"],
            "blocking_defeats": [{
                "defeated_decision": "d1",
                "defeating_decision": "d2"
            }]
        }]
    });
    let mut observed_bytes = Vec::new();

    for (case, permutation) in permutations.into_iter().enumerate() {
        let (_root, mut author, path) = skew_fixture();
        let workspace = result(dispatch(
            &request(
                "2.0.0",
                &format!("open-skew-{case}"),
                Op::AuthorOpen,
                json!({"scope": "demo"}),
            ),
            &mut author,
        ))["workspace"]
            .as_str()
            .unwrap()
            .to_owned();
        for declaration in permutation {
            let (id, key, value, decision) = batches[declaration];
            ingest_accepted_fact(
                &mut author,
                &workspace,
                &format!("{case}-{id}"),
                key,
                value,
                decision,
            );
        }
        stage_cited_fact(
            &mut author,
            &workspace,
            &format!("{case}-verdict"),
            "verdict",
            "accepted",
            "d1",
        );
        materialize_cited_facts(&mut author, &workspace);

        let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
        let head = lorestore.root_hash().unwrap().unwrap();
        let durable_names = lorestore.interner_at(&head).unwrap();
        let recompiled = lorestore.compile_at(&head).unwrap();
        let compiled_names = recompiled.model.interner().names();
        let durable_d1 = durable_names.iter().position(|name| name == "d1").unwrap();
        let compiled_d1 = compiled_names.iter().position(|name| name == "d1").unwrap();
        let durable_decision_positions: Vec<_> = ["d1", "d-alpha", "d-omega"]
            .into_iter()
            .map(|decision| {
                durable_names
                    .iter()
                    .position(|name| name == decision)
                    .unwrap()
            })
            .collect();
        let compiled_decision_positions: Vec<_> = ["d1", "d-alpha", "d-omega"]
            .into_iter()
            .map(|decision| {
                compiled_names
                    .iter()
                    .position(|name| name == decision)
                    .unwrap()
            })
            .collect();
        assert_ne!(
            durable_decision_positions, compiled_decision_positions,
            "case {case} must exercise persisted/recompiled decision-position skew: durable={durable_names:?}, compiled={compiled_names:?}"
        );
        assert_eq!(durable_names[durable_d1], "d1");
        assert_eq!(compiled_names[compiled_d1], "d1");

        let active = explanation_fields(&why_attribute(
            &mut author,
            &workspace,
            &format!("why-active-{case}"),
            "verdict",
            "accepted",
        ));
        assert_eq!(active, expected_active, "active case {case}");
        author
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(false);
        let active_miss = explanation_fields(&why_attribute(
            &mut author,
            &workspace,
            &format!("why-active-miss-{case}"),
            "verdict",
            "accepted",
        ));
        author
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(true);
        assert_eq!(active_miss, active, "active forced miss case {case}");
        let verdict_claim = predicate_claim_id(&lorestore, "verdict");
        defeat(
            &mut author,
            &workspace,
            &format!("block-{case}"),
            &verdict_claim,
            "d2",
            "d1",
        );
        let blocked = explanation_fields(&why_attribute(
            &mut author,
            &workspace,
            &format!("why-blocked-{case}"),
            "verdict",
            "accepted",
        ));
        assert_eq!(blocked, expected_blocked, "blocked case {case}");
        author
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(false);
        let blocked_miss = explanation_fields(&why_attribute(
            &mut author,
            &workspace,
            &format!("why-blocked-miss-{case}"),
            "verdict",
            "accepted",
        ));
        author
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(true);
        assert_eq!(blocked_miss, blocked, "blocked forced miss case {case}");

        let d2_claim = claim_id(&lorestore, Some("ratify"), "d2");
        defeat(
            &mut author,
            &workspace,
            &format!("restore-{case}"),
            &d2_claim,
            "d3",
            "d2",
        );
        let restored = explanation_fields(&why_attribute(
            &mut author,
            &workspace,
            &format!("why-restored-{case}"),
            "verdict",
            "accepted",
        ));
        assert_eq!(restored, expected_active, "restored case {case}");
        author
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(false);
        let restored_miss = explanation_fields(&why_attribute(
            &mut author,
            &workspace,
            &format!("why-restored-miss-{case}"),
            "verdict",
            "accepted",
        ));
        author
            .get(&workspace)
            .unwrap()
            .set_compile_cache_enabled(true);
        assert_eq!(restored_miss, restored, "restored forced miss case {case}");
        observed_bytes.push([
            serde_json::to_vec(&active).unwrap(),
            serde_json::to_vec(&blocked).unwrap(),
            serde_json::to_vec(&restored).unwrap(),
        ]);
    }

    assert!(
        observed_bytes.windows(2).all(|pair| pair[0] == pair[1]),
        "exact-name author.why fields must be byte-identical across all six real ingestion orders"
    );
}

#[test]
fn fresh_workspace_observes_defeat_without_changing_existing_snapshot() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let target = claim_id(&lorestore, None, "nara");
    let open = |store: &mut Store, id: &str| {
        result(dispatch(
            &request("2.0.0", id, Op::AuthorOpen, json!({"scope": "demo"})),
            store,
        ))["workspace"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let writer = open(&mut store, "writer");
    let old = open(&mut store, "old");
    defeat(&mut store, &writer, "defeat", &target, "d2", "d1");
    let fresh = open(&mut store, "fresh");
    assert!(
        author_query(&mut store, &fresh, "new-query")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !author_query(&mut store, &old, "old-query")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
