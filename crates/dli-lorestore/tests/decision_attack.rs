//! Schema-v5 conditional decision attacks: storage, fold, reopen, merge, and
//! legacy-upcast compatibility.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use dli_ast::decl::{Decl, RatifyDecl};
use dli_ast::ident::Ident;
use dli_ast::language::RatifyBody;
use dli_ast::span::Span;
use dli_core::{Atom, Valuation};
use dli_lorestore::{ClaimId, Commit, DecisionAttack, Lorestore, MAIN_REF, Ref};

fn decisions() -> Vec<String> {
    ["d1", "d2", "d3"].map(str::to_owned).to_vec()
}

fn carriers(store: &Lorestore) -> (Vec<String>, Vec<dli_lorestore::ContentHash>) {
    let mut interner = dli_core::Interner::new();
    let mut counter = 0;
    let mut hashes = Vec::new();
    for decision in decisions() {
        interner.intern(&decision);
        let decl = Decl::Ratify(RatifyDecl {
            decision: Ident {
                text: decision,
                span: Span::synthetic(),
            },
            body: RatifyBody::default(),
        });
        for object in
            dli_lorestore::lower::lower_decl(&decl, "attack-test", &mut interner, &mut counter)
        {
            hashes.push(store.write_object(&object).unwrap());
        }
    }
    (interner.names().to_vec(), hashes)
}

fn write_attack(
    store: &Lorestore,
    facts: &[dli_lorestore::ContentHash],
    attacker: &str,
    target: &str,
) -> dli_lorestore::ContentHash {
    let subject_fact_hash = facts
        .iter()
        .find(|hash| {
            store
                .read_object::<dli_lorestore::FactObject>(hash)
                .unwrap()
                .unwrap()
                .expert_body
                .as_ref()
                .is_some_and(|body| body.name == target)
        })
        .unwrap()
        .clone();
    let carrier: dli_lorestore::FactObject =
        store.read_object(&subject_fact_hash).unwrap().unwrap();
    store
        .write_object(&DecisionAttack::new(
            carrier.claim_id,
            subject_fact_hash,
            attacker,
            target,
        ))
        .expect("write attack")
}

#[test]
fn attack_round_trips_and_missing_or_unknown_edges_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    let (_, facts) = carriers(&store);
    let subject = facts[0].clone();
    let carrier: dli_lorestore::FactObject = store.read_object(&subject).unwrap().unwrap();
    let attack = DecisionAttack::new(carrier.claim_id, subject, "d2", "d1");
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

    let unknown = write_attack(&store, &facts, "unknown", "d1");
    assert!(
        store
            .commit_with_attacks(
                decisions(),
                facts,
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
fn corrupt_endpoint_and_carrier_refusals_leave_the_ref_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    let (interner, facts) = carriers(&store);
    store
        .commit(
            interner.clone(),
            facts.clone(),
            Vec::new(),
            Vec::new(),
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();
    let before = store.branch_ref(MAIN_REF).unwrap().unwrap();
    let before_digest = before.digest().unwrap().to_string();
    let d2_hash = facts
        .iter()
        .find(|hash| {
            store
                .read_object::<dli_lorestore::FactObject>(hash)
                .unwrap()
                .unwrap()
                .expert_body
                .as_ref()
                .is_some_and(|body| body.name == "d2")
        })
        .unwrap()
        .clone();
    let d2: dli_lorestore::FactObject = store.read_object(&d2_hash).unwrap().unwrap();

    let cases = [
        DecisionAttack::new(d2.claim_id.clone(), d2_hash.clone(), "d2", "d1"),
        DecisionAttack::new(ClaimId::from_label("phantom"), d2_hash, "d2", "d2"),
    ];
    for attack in cases {
        let hash = store.write_object(&attack).unwrap();
        let error = store
            .commit_on_branch_with_attacks_at_ref(
                MAIN_REF,
                Some(&before_digest),
                interner.clone(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![hash],
                decisions(),
                "attack-test".to_owned(),
            )
            .unwrap_err();
        assert!(error.to_string().contains("carrier"), "{error}");
        assert_eq!(store.branch_ref(MAIN_REF).unwrap().unwrap(), before);
    }

    let mut phantom_carrier = d2.clone();
    phantom_carrier.claim_id = ClaimId::from_label("orphan-revision");
    let phantom_hash = store.write_object(&phantom_carrier).unwrap();
    let phantom_attack = store
        .write_object(&DecisionAttack::new(
            phantom_carrier.claim_id,
            phantom_hash,
            "d1",
            "d2",
        ))
        .unwrap();
    let error = store
        .commit_on_branch_with_attacks_at_ref(
            MAIN_REF,
            Some(&before_digest),
            interner.clone(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![phantom_attack],
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("candidate fact fold"), "{error}");
    assert_eq!(store.branch_ref(MAIN_REF).unwrap().unwrap(), before);

    let valid = write_attack(&store, &facts, "d2", "d1");
    let mut duplicate = interner;
    duplicate.push("d1".to_owned());
    let error = store
        .commit_on_branch_with_attacks_at_ref(
            MAIN_REF,
            Some(&before_digest),
            duplicate,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![valid],
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("not uniquely interned"),
        "{error}"
    );
    assert_eq!(store.branch_ref(MAIN_REF).unwrap().unwrap(), before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn raw_commit_cas_and_reopened_reads_reject_every_invalid_attack_shape() {
    let dir = tempfile::tempdir().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    let (interner, facts) = carriers(&store);
    store
        .commit(
            interner.clone(),
            facts.clone(),
            Vec::new(),
            Vec::new(),
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();
    store.create_branch("probe", MAIN_REF).unwrap();
    let before = store.branch_ref(MAIN_REF).unwrap().unwrap();
    let before_digest = before.digest().unwrap().to_string();

    let carrier_for = |target: &str| {
        let hash = facts
            .iter()
            .find(|hash| {
                store
                    .read_object::<dli_lorestore::FactObject>(hash)
                    .unwrap()
                    .unwrap()
                    .expert_body
                    .as_ref()
                    .is_some_and(|body| body.name == target)
            })
            .unwrap()
            .clone();
        let object: dli_lorestore::FactObject = store.read_object(&hash).unwrap().unwrap();
        (hash, object)
    };
    let (d1_hash, d1) = carrier_for("d1");
    let (d2_hash, d2) = carrier_for("d2");
    let mut phantom = d2.clone();
    phantom.claim_id = ClaimId::from_label("phantom-carrier");
    let phantom_hash = store.write_object(&phantom).unwrap();

    let cases = [
        (
            DecisionAttack::new(d2.claim_id.clone(), d2_hash.clone(), "d2", "d1"),
            interner.clone(),
        ),
        (
            DecisionAttack::new(ClaimId::from_label("mismatched-claim"), d2_hash, "d1", "d2"),
            interner.clone(),
        ),
        (
            DecisionAttack::new(phantom.claim_id, phantom_hash, "d1", "d2"),
            interner.clone(),
        ),
        (
            DecisionAttack::new(d1.claim_id.clone(), d1_hash.clone(), "d3", "d1"),
            interner[..2].to_vec(),
        ),
        (DecisionAttack::new(d1.claim_id, d1_hash, "d2", "d1"), {
            let mut duplicate = interner.clone();
            duplicate.push("d1".to_owned());
            duplicate
        }),
    ];

    let mut corrupt_heads = Vec::new();
    for (attack, names) in cases {
        let attack_hash = store.write_object(&attack).unwrap();
        let raw = Commit::child(
            before.commit.clone(),
            names,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            decisions(),
            "attack-test".to_owned(),
        )
        .with_attacks(vec![attack_hash]);
        let raw_hash = store.write_object(&raw).unwrap();
        let proposed = Ref::new(MAIN_REF, raw_hash.clone());
        assert!(
            store.cas_main_ref(Some(&before_digest), &proposed).is_err(),
            "raw invalid commit must be refused before main CAS"
        );
        assert_eq!(store.branch_ref(MAIN_REF).unwrap().unwrap(), before);
        assert!(store.decision_attacks_from(&raw_hash).is_err());
        assert!(
            store
                .effective_valuation_at(&raw_hash, &Valuation::new())
                .is_err(),
            "invalid attack must never produce an overlay"
        );
        corrupt_heads.push(raw_hash);
    }

    let probe = store.branch_ref("probe").unwrap().unwrap();
    for corrupt_head in &corrupt_heads {
        let corrupt = Ref::new("refs/heads/probe", corrupt_head.clone());
        assert!(
            store
                .cas_branch_ref(
                    "probe",
                    Some(&probe.digest().unwrap().to_string()),
                    &corrupt,
                )
                .is_err()
        );
        assert_eq!(store.branch_ref("probe").unwrap().unwrap(), probe);
    }

    // Simulate a store produced by an older/bypassing writer: reopening can
    // observe its ref, but every attack-derived read still fails closed.
    let corrupt_ref = Ref::new(MAIN_REF, corrupt_heads[1].clone());
    std::fs::write(
        dir.path().join("refs/main"),
        serde_json::to_vec(&corrupt_ref).unwrap(),
    )
    .unwrap();
    drop(store);
    let reopened = Lorestore::open(dir.path().to_path_buf()).unwrap();
    let corrupt_head = reopened.branch_ref(MAIN_REF).unwrap().unwrap().commit;
    assert!(reopened.decision_attacks_from(&corrupt_head).is_err());
    assert!(
        reopened
            .effective_valuation_at(&corrupt_head, &Valuation::new())
            .is_err()
    );
}

#[test]
fn folded_graph_reopens_and_keeps_legacy_ref_assignments_fixed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let store = Lorestore::open(root.clone()).unwrap();
    let (interner, facts) = carriers(&store);
    let d2_d1 = write_attack(&store, &facts, "d2", "d1");
    let d3_d2 = write_attack(&store, &facts, "d3", "d2");
    let head = store
        .commit_with_attacks(
            interner,
            facts,
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
    assert_eq!(effective.valuation.get(Atom(0)), Some(true));
    assert_eq!(effective.valuation.get(Atom(1)), Some(false));
    assert_eq!(effective.valuation.get(Atom(2)), Some(true));

    let mut legacy = Valuation::new();
    legacy.set(Atom(0), false);
    let fixed = reopened.effective_valuation_at(&head, &legacy).unwrap();
    assert_eq!(
        fixed.valuation.get(Atom(0)),
        Some(false),
        "legacy false is immutable"
    );
    assert_eq!(fixed.valuation.get(Atom(1)), Some(false));
    assert_eq!(fixed.valuation.get(Atom(2)), Some(true));

    // The build-independent corpus lanes round-trip by value: fact objects,
    // attack objects, interner, and decisions reproduce the same root.
    let imported_dir = tempfile::tempdir().unwrap();
    let imported = Lorestore::open(imported_dir.path().to_path_buf()).unwrap();
    let imported_facts = reopened
        .fact_hashes_from(&head)
        .unwrap()
        .into_iter()
        .map(|hash| {
            let object: dli_lorestore::FactObject = reopened.read_object(&hash).unwrap().unwrap();
            imported.write_object(&object).unwrap()
        })
        .collect();
    let imported_attacks = reopened
        .decision_attacks_from(&head)
        .unwrap()
        .into_iter()
        .map(|attack| imported.write_object(&attack).unwrap())
        .collect();
    let imported_head = imported
        .commit_with_attacks(
            reopened.interner_at(&head).unwrap(),
            imported_facts,
            Vec::new(),
            Vec::new(),
            imported_attacks,
            reopened.decisions_at(&head).unwrap(),
            "attack-test".to_owned(),
        )
        .unwrap();
    assert_eq!(imported_head, head);
    assert_eq!(
        imported
            .effective_valuation_at(&imported_head, &Valuation::new())
            .unwrap(),
        effective
    );
}

#[test]
fn branch_merge_unions_attacks_before_grounded_labelling() {
    let dir = tempfile::tempdir().unwrap();
    let store = Lorestore::open(dir.path().to_path_buf()).unwrap();
    let (interner, facts) = carriers(&store);
    store
        .commit(
            interner.clone(),
            facts.clone(),
            Vec::new(),
            Vec::new(),
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();
    store.create_branch("left", MAIN_REF).unwrap();
    store.create_branch("right", MAIN_REF).unwrap();

    let left_edge = write_attack(&store, &facts, "d2", "d1");
    let left_ref = store.branch_ref("left").unwrap().unwrap();
    store
        .commit_on_branch_with_attacks_at_ref(
            "left",
            Some(&left_ref.digest().unwrap().to_string()),
            interner.clone(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![left_edge],
            decisions(),
            "attack-test".to_owned(),
        )
        .unwrap();

    let right_edge = write_attack(&store, &facts, "d3", "d2");
    let right_ref = store.branch_ref("right").unwrap().unwrap();
    store
        .commit_on_branch_with_attacks_at_ref(
            "right",
            Some(&right_ref.digest().unwrap().to_string()),
            interner,
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
    assert_eq!(effective.valuation.get(Atom(0)), Some(true), "d1 restores");
    assert_eq!(effective.valuation.get(Atom(1)), Some(false));
    assert_eq!(effective.valuation.get(Atom(2)), Some(true));
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
