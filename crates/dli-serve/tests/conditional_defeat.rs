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
        writeln!(&mut source, "declared-{decision}(mara) ratified {decision}").unwrap();
    }
    source.push_str(concat!(
        "person mara \"Mara\" ratified d11\n",
        "p(mara) ratified d1\n",
        "independent-seed(mara) ratified d11\n",
        "redundant-seed(mara) ratified d11\n",
        "rule independent-rule ratified d10\n",
        "  independent(mara) :- independent-seed(mara)\n",
        "rule redundant-from-p ratified d10\n",
        "  redundant(mara) :- p(mara)\n",
        "rule redundant-live ratified d10\n",
        "  redundant(mara) :- redundant-seed(mara)\n",
    ));
    let mut predecessor = "p".to_owned();
    for level in 1..derivation_depth {
        let step = format!("step-{level}");
        writeln!(
            &mut source,
            "rule chain-{level} ratified d10\n  {step}(mara) :- {predecessor}(mara)"
        )
        .unwrap();
        predecessor = step;
    }
    for conclusion in 1..=conclusion_count {
        writeln!(
            &mut source,
            "rule conclusion-{conclusion} ratified d10\n  q-{conclusion}(mara) :- {predecessor}(mara)"
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
            "1.13.0",
            id,
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": predicate, "args": [{"Sym": "mara"}]},
                "layer": "all"
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
            "1.13.0",
            id,
            Op::AuthorQuery,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]},
                "layer": "all"
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
            "1.13.0",
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
    let d1_claim = claim_id(&lorestore, None, "mara");
    let workspace = result(dispatch(
        &request("1.13.0", "open", Op::AuthorOpen, json!({"scope": "demo"})),
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

    let d2_claim = claim_id(&lorestore, Some("ratify"), "d2");
    defeat(&mut store, &workspace, "d3", &d2_claim, "d3", "d2");
    let restored = author_query(&mut store, &workspace, "q2");
    let restored_why = result(dispatch(
        &request(
            "1.13.0",
            "why2",
            Op::AuthorWhy,
            json!({
                "workspace": workspace,
                "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]}
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

    let d3_claim = claim_id(&lorestore, Some("ratify"), "d3");
    defeat(&mut store, &workspace, "d4", &d3_claim, "d4", "d3");
    assert!(
        author_query(&mut store, &workspace, "q3")["rows"]
            .as_array()
            .unwrap()
            .is_empty()
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

    // A fresh project/session reconstructs the attack graph from the commit and
    // reaches the same dead result without borrowing the live author workspace.
    let project = Project::load(root.path()).unwrap();
    let mut sessions = Sessions::from_project(Arc::new(project));
    let mut author = Store::default();
    let frame = json!({
        "dli": "1.13.0",
        "id": "reopen",
        "op": "dli.query",
        "params": {
            "document": "demo",
            "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]},
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
}

#[test]
fn contract_1_12_defeat_keeps_the_frozen_one_way_ref_semantics() {
    let (_root, mut store, path) = fixture();
    let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
    let claim = claim_id(&lorestore, None, "mara");
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
}

#[test]
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
                "1.13.0",
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
                claim_id(&lorestore, None, "mara")
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

        let project = Project::load(root.path()).unwrap();
        let mut sessions = Sessions::from_project(Arc::new(project));
        let mut author = Store::default();
        let frame = json!({
            "dli": "1.13.0", "id": format!("reopen-{name}"), "op": "dli.query",
            "params": {"document": "demo", "goal_atom": {"pred": "rank", "args": [{"Sym": "mara"}, {"Sym": "gold"}]}, "layer": "all"}
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
                    "dli": "1.13.0", "id": format!("export-{format}"),
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
                "1.13.0",
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
        let mut carrier = claim_id(&lorestore, None, "mara");
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
    let claim = claim_id(&lorestore, None, "mara");
    let workspace = result(dispatch(
        &request(
            "1.13.0",
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
        "1.13.0",
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
        let claim = claim_id(&lorestore, None, "mara");
        let workspace = result(dispatch(
            &request(
                "1.13.0",
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
                    "1.13.0",
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
                let query = query_predicate(
                    &mut author,
                    &workspace,
                    &format!("before-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &format!("q-{conclusion}"),
                );
                assert_eq!(
                    query["rows"][0]["status"], "certain",
                    "derivation depth {derivation_depth}, shared conclusion {conclusion}/{conclusion_count}: {query}"
                );
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
                let query = query_predicate(
                    &mut author,
                    &workspace,
                    &format!("retracted-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &format!("q-{conclusion}"),
                );
                assert_eq!(
                    query["rows"],
                    json!([]),
                    "one decision attack retracts every result at derivation depth {derivation_depth}: {query}"
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
                let query = query_predicate(
                    &mut author,
                    &workspace,
                    &format!("restored-{derivation_depth}-{conclusion_count}-{conclusion}"),
                    &format!("q-{conclusion}"),
                );
                assert_eq!(
                    query["rows"][0]["status"], "certain",
                    "defeating the attacker restores all {conclusion_count} results through {derivation_depth} real rule edges: {query}"
                );
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
            "1.13.0",
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
    for (case, permutation) in permutations.into_iter().enumerate() {
        let declarations: Vec<_> = permutation.iter().map(|index| edges[*index].0).collect();
        let (_root, mut author, path) = propagation_fixture(1, 1, &declarations);
        let lorestore = dli_lorestore::Lorestore::open(path).unwrap();
        let workspace = result(dispatch(
            &request(
                "1.13.0",
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
}
