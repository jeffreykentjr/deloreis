//! Schema-v5 conditional decision attacks: storage, fold, reopen, merge, and
//! legacy-upcast compatibility.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use dli_core::{Atom, Valuation};
use dli_lorestore::{ClaimId, DecisionAttack, Lorestore, MAIN_REF};

fn decisions() -> Vec<String> {
    ["d1", "d2", "d3"].map(str::to_owned).to_vec()
}

fn write_attack(store: &Lorestore, attacker: &str, target: &str) -> dli_lorestore::ContentHash {
    store
        .write_object(&DecisionAttack::new(
            ClaimId::from_label(&format!("claim/{target}")),
            attacker,
            target,
        ))
        .expect("write attack")
}

#[test]
fn attack_round_trips_and_missing_or_unknown_edges_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    let attack = DecisionAttack::new(ClaimId::from_label("claim/d1"), "d2", "d1");
    let hash = store.write_object(&attack).unwrap();
    assert_eq!(
        store.read_object::<DecisionAttack>(&hash).unwrap(),
        Some(attack)
    );

    let missing = dli_lorestore::ContentHash("sha256:not-present".to_owned());
    assert!(
        store
            .commit_with_attacks(
                decisions(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![missing],
                decisions(),
                "attack-test".to_owned(),
            )
            .unwrap_err()
            .to_string()
            .contains("decision attack not in store")
    );

    let unknown = write_attack(&store, "unknown", "d1");
    assert!(
        store
            .commit_with_attacks(
                decisions(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![unknown],
                decisions(),
                "attack-test".to_owned(),
            )
            .unwrap_err()
            .to_string()
            .contains("unknown decision `unknown`")
    );
}

#[test]
fn folded_graph_reopens_and_keeps_legacy_ref_assignments_fixed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = Lorestore::open(root.clone()).unwrap();
    let d2_d1 = write_attack(&store, "d2", "d1");
    let d3_d2 = write_attack(&store, "d3", "d2");
    let head = store
        .commit_with_attacks(
            decisions(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![d2_d1, d3_d2],
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();
    drop(store);

    let reopened = Lorestore::open(root).unwrap();
    assert_eq!(reopened.decision_attacks_from(&head).unwrap().len(), 2);
    let effective = reopened
        .effective_valuation_at(&head, &Valuation::new())
        .unwrap();
    assert_eq!(effective.get(Atom(0)), Some(true));
    assert_eq!(effective.get(Atom(1)), Some(false));
    assert_eq!(effective.get(Atom(2)), Some(true));

    let mut legacy = Valuation::new();
    legacy.set(Atom(0), false);
    let fixed = reopened.effective_valuation_at(&head, &legacy).unwrap();
    assert_eq!(fixed.get(Atom(0)), Some(false), "legacy false is immutable");
    assert_eq!(fixed.get(Atom(1)), Some(false));
    assert_eq!(fixed.get(Atom(2)), Some(true));
}

#[test]
fn branch_merge_unions_attacks_before_grounded_labelling() {
    let dir = tempfile::tempdir().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    store
        .commit(
            decisions(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();
    store.create_branch("left", MAIN_REF).unwrap();
    store.create_branch("right", MAIN_REF).unwrap();

    let left_edge = write_attack(&store, "d2", "d1");
    let left_ref = store.branch_ref("left").unwrap().unwrap();
    store
        .commit_on_branch_with_attacks_at_ref(
            "left",
            Some(&left_ref.digest().unwrap().to_string()),
            decisions(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![left_edge],
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();

    let right_edge = write_attack(&store, "d3", "d2");
    let right_ref = store.branch_ref("right").unwrap().unwrap();
    store
        .commit_on_branch_with_attacks_at_ref(
            "right",
            Some(&right_ref.digest().unwrap().to_string()),
            decisions(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![right_edge],
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();

    let report = store.merge_with("left", "right", true, None).unwrap();
    assert_eq!(
        store
            .decision_attack_hashes_from(&report.merge_commit)
            .unwrap()
            .len(),
        2
    );
    let merged_ref = store.branch_ref("left").unwrap().unwrap();
    let effective = store
        .effective_valuation_at(&report.merge_commit, &merged_ref.valuation)
        .unwrap();
    assert_eq!(effective.get(Atom(0)), Some(true), "d1 restores");
    assert_eq!(effective.get(Atom(1)), Some(false));
    assert_eq!(effective.get(Atom(2)), Some(true));
}

#[test]
fn schema_v4_commit_upcasts_with_an_empty_attack_lane() {
    let dir = tempfile::tempdir().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    let old = serde_json::json!({
        "schema_version": 4,
        "authority": null,
        "parent": null,
        "facts": [],
        "lineage": [],
        "evidence": [],
        "interner": [],
        "pack_name": "legacy"
    });
    let hash = store.write_object(&old).unwrap();
    let lifted: dli_lorestore::Commit = store.read_object(&hash).unwrap().unwrap();
    assert_eq!(lifted.schema_version, 5);
    assert!(lifted.attacks.is_empty());
}
