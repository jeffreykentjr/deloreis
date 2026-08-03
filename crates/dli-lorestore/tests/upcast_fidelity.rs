//! LS-9 upcast-fidelity oracle.
//!
//! A schema-v1 **string-bodied** `rule` carriage — the shape stored before LS-9
//! structured the rule interior — must, on read, upcast to the v2 typed carriage
//! and compile to the identical world. Two levels of proof:
//!
//! 1. *Object fidelity* — a hand-built v1 carriage upcasts to a fact object
//!    byte-identical (by content hash) to the v2 carriage the front end now
//!    produces, and its embedded declaration deepens to the typed `RuleBody`.
//!    The naive read (no upcast) is shown to fail, so the upcast is load-bearing.
//! 2. *Compiled fidelity* — a whole pack stored with its rule carriage
//!    downgraded to v1 on disk compiles, through `Lorestore::compile`, to the
//!    same facts / fixpoint / findings as a direct compile.
//!
//! Both fail against the pre-fix state: pre-LS-9 there is no v2 schema and no
//! typed `RuleBody`, so a string body is the only shape and nothing upcasts.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
use common::{render_derived, render_facts, render_findings};

use serde_json::Value;
use tempfile::TempDir;

use dli_ast::decl::Decl;
use dli_lorestore::lower::{declared_decisions, lower_pack};
use dli_lorestore::object::{ExpertDeclKind, FactObject};
use dli_lorestore::storage::{FilesystemStorage, Storage, read_typed, write_typed};
use dli_lorestore::store::Commit;
use dli_lorestore::upcast::upcast_value;
use dli_lorestore::{Lorestore, MAIN_REF};
use dli_world::{Pack, compile};

/// A tiny pack whose only expert carriage is a world-datalog `rule` with a
/// block body — the exact shape LS-9 structured.
fn wards_pack() -> Pack {
    let src = "scope wards\n\
               ratify D-0003\n\
               seated(a)  ratified D-0003\n\
               rule reach  ratified D-0003\n\
               \x20 reachable($g) :- seated($g)\n";
    let (document, diagnostics) = dli_syntax::parse_document(src);
    assert!(
        diagnostics.is_empty(),
        "pack must parse clean: {diagnostics:?}"
    );
    Pack {
        name: "wards".to_owned(),
        documents: vec![document],
    }
}

/// Is `fo` a `rule` expert carriage?
fn is_rule_carriage(fo: &FactObject) -> bool {
    fo.expert_body
        .as_ref()
        .is_some_and(|b| b.kind == ExpertDeclKind::Rule)
}

/// Rewrite a v2 rule-carriage fact object into the schema-v1 JSON a pre-LS-9
/// store held: `schema_version` 1, and the embedded declaration's body reverted
/// from the typed `RuleBody` to the old `Vec<Clause>` string body (exactly the
/// `RuleClause::clause` values, in order).
fn downgrade_to_v1(fo: &FactObject) -> Value {
    let mut value = serde_json::to_value(fo).expect("fact object serializes");
    value["schema_version"] = Value::from(1);

    let embedded = value["expert_body"]["body"][0]
        .as_str()
        .expect("carriage body is a JSON string")
        .to_owned();
    let decl_v2: Value = serde_json::from_str(&embedded).expect("embedded decl is JSON");
    let rule = decl_v2.get("Rule").expect("carriage is a rule decl");

    // The v1 body is the ordered normalized clause texts (the `clause` field of
    // each typed clause), with no `datalog` — the pre-LS-9 `Vec<Clause>`.
    let v1_body: Vec<Value> = rule
        .get("body")
        .and_then(|b| b.get("clauses"))
        .and_then(Value::as_array)
        .map(|clauses| clauses.iter().map(|c| c["clause"].clone()).collect())
        .unwrap_or_default();

    let mut rule_v1 = rule.clone();
    rule_v1["body"] = Value::Array(v1_body);
    let decl_v1 = serde_json::json!({ "Rule": rule_v1 });
    value["expert_body"]["body"][0] =
        Value::from(serde_json::to_string(&decl_v1).expect("v1 decl serializes"));
    value
}

#[test]
fn v1_rule_carriage_upcasts_to_the_exact_v2_object() {
    let pack = wards_pack();
    let (objects, _names) = lower_pack(&pack);
    let v2_carriage = objects
        .iter()
        .find(|fo| is_rule_carriage(fo))
        .expect("the pack carries a rule");

    let v1_value = downgrade_to_v1(v2_carriage);

    // The v1 embedded declaration must NOT decode as a current `Decl` — the
    // string body (a JSON array) is not the typed `RuleBody` (a struct). This is
    // what makes the upcast load-bearing rather than cosmetic.
    let v1_embedded = v1_value["expert_body"]["body"][0].as_str().unwrap();
    assert!(
        serde_json::from_str::<Decl>(v1_embedded).is_err(),
        "a v1 string-bodied rule decl must not decode as a typed v2 Decl without upcast"
    );

    // Upcast v1 → current, then decode: it must equal the v2 carriage exactly
    // (same content address), and its interior must be the typed `RuleBody`.
    let lifted = upcast_value(1, v1_value).expect("v1 carriage upcasts");
    let lifted_fo: FactObject = serde_json::from_value(lifted).expect("lifted carriage decodes");

    assert_eq!(
        lifted_fo.content_hash().expect("hash"),
        v2_carriage.content_hash().expect("hash"),
        "the upcast v1 carriage must be byte-identical to the v2 carriage"
    );
    assert_eq!(lifted_fo.schema_version, dli_lorestore::SCHEMA_VERSION);

    let embedded = &lifted_fo.expert_body.as_ref().unwrap().body[0];
    let Decl::Rule(rule) = serde_json::from_str(embedded).expect("lifted decl decodes") else {
        panic!("carriage is a rule");
    };
    let clause = &rule.body.clauses[0];
    let datalog = clause
        .datalog
        .as_ref()
        .expect("the world-rule clause is structured after upcast");
    assert_eq!(datalog.head.pred.text, "reachable");
    assert_eq!(datalog.body.len(), 1);
    assert_eq!(datalog.body[0].atom.pred.text, "seated");
}

/// A tiny pack whose only expert carriage is a `practice` block — the shape LS-9
/// structured into a typed `PracticeBody`.
fn practice_pack() -> Pack {
    let src = "scope magic\n\
               cycle tide(4) proposed\n\
               practice spoken-proof proposed {\n\
               \x20 kind ground = door | field\n\
               \x20 resource debt : linear conserved\n\
               \x20 error fizzle refines L2 premise-not-available where absent\n\
               \x20 cost bodily | material\n\
               }\n";
    let (document, diagnostics) = dli_syntax::parse_document(src);
    assert!(
        diagnostics.is_empty(),
        "pack must parse clean: {diagnostics:?}"
    );
    Pack {
        name: "magic".to_owned(),
        documents: vec![document],
    }
}

/// Is `fo` a `practice` expert carriage?
fn is_practice_carriage(fo: &FactObject) -> bool {
    fo.expert_body
        .as_ref()
        .is_some_and(|b| b.kind == ExpertDeclKind::Practice)
}

/// Rewrite a v2 practice-carriage fact object into the schema-v1 JSON a pre-LS-9
/// store held: `schema_version` 1, and the embedded declaration's body reverted
/// from the typed `PracticeBody` to the old `Vec<Clause>` string body (exactly the
/// retained `clauses` array).
fn downgrade_practice_to_v1(fo: &FactObject) -> Value {
    let mut value = serde_json::to_value(fo).expect("fact object serializes");
    value["schema_version"] = Value::from(1);

    let embedded = value["expert_body"]["body"][0]
        .as_str()
        .expect("carriage body is a JSON string")
        .to_owned();
    let decl_v2: Value = serde_json::from_str(&embedded).expect("embedded decl is JSON");
    let practice = decl_v2
        .get("Practice")
        .expect("carriage is a practice decl");

    // The v1 body is the retained normalized clause texts — the pre-LS-9
    // `Vec<Clause>`, which is exactly `PracticeBody::clauses`.
    let v1_body = practice
        .get("body")
        .and_then(|b| b.get("clauses"))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));

    let mut practice_v1 = practice.clone();
    practice_v1["body"] = v1_body;
    let decl_v1 = serde_json::json!({ "Practice": practice_v1 });
    value["expert_body"]["body"][0] =
        Value::from(serde_json::to_string(&decl_v1).expect("v1 decl serializes"));
    value
}

/// The practice analogue of `v1_rule_carriage_upcasts_to_the_exact_v2_object`: a
/// v1 string-bodied `practice` carriage must, on upcast, be byte-identical (by
/// content hash) to the v2 carriage the front end now produces, and its embedded
/// declaration must deepen to the typed `PracticeBody`. Fails-before-fix: pre-LS-9
/// the practice body was `Vec<Clause>` with no typed interior, so nothing upcasts
/// and `body.def` does not exist.
#[test]
fn v1_practice_carriage_upcasts_to_the_exact_v2_object() {
    let pack = practice_pack();
    let (objects, _names) = lower_pack(&pack);
    let v2_carriage = objects
        .iter()
        .find(|fo| is_practice_carriage(fo))
        .expect("the pack carries a practice");

    let v1_value = downgrade_practice_to_v1(v2_carriage);

    // The v1 embedded declaration must NOT decode as a current `Decl` — the
    // string body (a JSON array) is not the typed `PracticeBody` (a struct).
    let v1_embedded = v1_value["expert_body"]["body"][0].as_str().unwrap();
    assert!(
        serde_json::from_str::<Decl>(v1_embedded).is_err(),
        "a v1 string-bodied practice decl must not decode as a typed v2 Decl without upcast"
    );

    // Upcast v1 → current, then decode: byte-identical to the v2 carriage, and the
    // interior deepened to the typed `PracticeBody`.
    let lifted = upcast_value(1, v1_value).expect("v1 carriage upcasts");
    let lifted_fo: FactObject = serde_json::from_value(lifted).expect("lifted carriage decodes");

    assert_eq!(
        lifted_fo.content_hash().expect("hash"),
        v2_carriage.content_hash().expect("hash"),
        "the upcast v1 practice carriage must be byte-identical to the v2 carriage"
    );
    assert_eq!(lifted_fo.schema_version, dli_lorestore::SCHEMA_VERSION);

    let embedded = &lifted_fo.expert_body.as_ref().unwrap().body[0];
    let Decl::Practice(p) = serde_json::from_str(embedded).expect("lifted decl decodes") else {
        panic!("carriage is a practice");
    };
    // The typed interior is present and correctly structured after upcast.
    let def = &p.body.def;
    assert_eq!(def.sorts.len(), 1);
    assert_eq!(def.sorts[0].name.text, "ground");
    assert_eq!(def.resources.len(), 1);
    assert_eq!(
        def.resources[0].kind,
        Some(dli_ast::practice::ResourceKind::Linear { conserved: true })
    );
    assert_eq!(def.errors.len(), 1);
    assert_eq!(
        def.errors[0].class,
        Some(dli_ast::practice::StructuralClass::L2)
    );
    assert_eq!(def.costs.len(), 2);
    // The raw clauses are retained so the surface still round-trips.
    assert_eq!(p.body.clauses.len(), 4);
}

#[test]
fn a_v1_stored_pack_compiles_identically_after_upcast() {
    let pack = wards_pack();

    // reference A: direct compile.
    let compiled_a = compile(&pack).expect("direct compile");
    let model_a = compiled_a.model();

    // Build a store on disk whose rule carriage is stored in the schema-v1
    // string-bodied shape, everything else v2 — a genuine mixed old/new store.
    let (_tmp, _head, store) = build_v1_store(&pack);

    // reference B: compile through the store, which upcasts the v1 carriage.
    let compiled_b = store.compile().expect("store compile (with upcast)");
    let model_b = compiled_b.model();

    assert_eq!(
        render_facts(&model_a.world),
        render_facts(&model_b.world),
        "extensional facts must match after v1 upcast"
    );
    assert_eq!(
        render_derived(&model_a.world),
        render_derived(&model_b.world),
        "fixpoint (the rule's derivation) must match after v1 upcast"
    );
    assert_eq!(
        render_findings(model_a.findings()),
        render_findings(model_b.findings()),
        "findings must match after v1 upcast"
    );
    // The rule really derived something — the oracle is not vacuous.
    assert!(
        !render_derived(&model_a.world).is_empty(),
        "the world rule must derive a fact for the oracle to be meaningful"
    );
}

/// Build an on-disk store whose commit chain holds the pack's `rule` carriages in
/// the schema-v1 string-bodied shape (everything else v2) — a store persisted
/// before LS-9. Returns the temp dir (kept alive so the store's files survive),
/// the head commit hash, and a `Lorestore` over it. Asserts the carriage really
/// is v1 on disk.
fn build_v1_store(pack: &Pack) -> (TempDir, dli_lorestore::ContentHash, Lorestore) {
    let tmp = TempDir::new().expect("tempdir");
    let storage = FilesystemStorage::new(tmp.path());
    let (objects, interner_names) = lower_pack(pack);

    let mut fact_hashes = Vec::with_capacity(objects.len());
    for fo in &objects {
        let hash = if is_rule_carriage(fo) {
            let v1_bytes = serde_json::to_vec(&downgrade_to_v1(fo)).expect("v1 bytes");
            storage.write_object(&v1_bytes).expect("write v1 carriage")
        } else {
            write_typed(&storage, fo).expect("write v2 object")
        };
        fact_hashes.push(hash);
    }

    let decisions: Vec<String> = declared_decisions(pack).into_iter().collect();
    let commit = Commit::root(
        interner_names,
        fact_hashes,
        Vec::new(),
        Vec::new(),
        decisions,
        pack.name.clone(),
    );
    let commit_hash = write_typed(&storage, &commit).expect("write commit");
    let main = dli_lorestore::store::Ref::new(MAIN_REF, commit_hash.clone());
    assert!(
        storage.cas_ref(MAIN_REF, None, &main).expect("cas ref"),
        "fresh ref creation must succeed"
    );
    assert!(
        commit_carriage_is_v1(&storage, &commit_hash),
        "the rule carriage must be stored as schema v1"
    );

    let store = Lorestore::from_storage(Box::new(storage));
    (tmp, commit_hash, store)
}

/// Read every fact object named by the commit at `commit_hash` (raw, no upcast)
/// and report whether the rule carriage is stored at `schema_version` 1.
fn commit_carriage_is_v1(
    storage: &FilesystemStorage,
    commit_hash: &dli_lorestore::ContentHash,
) -> bool {
    let bytes = storage
        .read_object(commit_hash)
        .expect("read commit")
        .unwrap();
    let commit: Commit = serde_json::from_slice(&bytes).expect("commit decodes");
    for hash in &commit.facts {
        let bytes = storage.read_object(hash).expect("read object").unwrap();
        let value: Value = serde_json::from_slice(&bytes).expect("object json");
        let is_rule = value
            .get("expert_body")
            .and_then(|e| e.get("kind"))
            .and_then(Value::as_str)
            == Some("rule");
        if is_rule {
            return value.get("schema_version").and_then(Value::as_u64) == Some(1);
        }
    }
    false
}

// The `read_typed` upcast path is exercised end-to-end by
// `a_v1_stored_pack_compiles_identically_after_upcast` (Lorestore::compile reads
// every fact object through `read_typed`). This direct check pins the contract.
#[test]
fn read_typed_upcasts_a_v1_object_on_read() {
    let pack = wards_pack();
    let (objects, _names) = lower_pack(&pack);
    let v2_carriage = objects.iter().find(|fo| is_rule_carriage(fo)).unwrap();

    let tmp = TempDir::new().expect("tempdir");
    let storage = FilesystemStorage::new(tmp.path());
    let v1_bytes = serde_json::to_vec(&downgrade_to_v1(v2_carriage)).unwrap();
    let hash = storage.write_object(&v1_bytes).expect("write v1");

    let read: FactObject = read_typed(&storage, &hash).expect("read").expect("present");
    assert_eq!(read.schema_version, dli_lorestore::SCHEMA_VERSION);
    assert_eq!(
        read.content_hash().expect("hash"),
        v2_carriage.content_hash().expect("hash"),
        "read_typed must lift the v1 carriage to the v2 object"
    );
}

/// LS-9 v1-store idempotency (the coordinator's Fix 2): a store persisted under
/// schema v1 must be recognized by `ensure_pack_imported` as the SAME lineage as
/// the freshly-lowered v2 pack — never refused as a `ForeignBase` at serve start.
///
/// The 1→2 schema bump changes every object's full-object STORAGE address
/// (`schema_version` is inside the JCS bytes, `object.rs`), so the pre-fix
/// comparison of freshly-lowered v2 storage addresses against the store's v1
/// addresses mismatches and raises `ForeignBase`. Fails-before-fix: with the old
/// storage-address comparison the `expect` below panics on `ForeignBase`. The fix
/// compares the version-independent identity projection (`FactObject::content_hash`,
/// which excludes `schema_version`), reading the stored side through the upcasting
/// `read_object` so a v1 carriage lifts to its v2 identity before comparison.
#[test]
fn ensure_pack_imported_recognizes_a_v1_store_as_the_same_lineage() {
    let pack = wards_pack();
    let (_tmp, head, store) = build_v1_store(&pack);

    // Re-attaching the same pack, freshly lowered to v2, must be idempotent.
    let result = store
        .ensure_pack_imported(&pack)
        .expect("a v1 store is the same lineage, not a ForeignBase");
    assert_eq!(
        result, head,
        "idempotent re-attach of a v1 store must return the existing head — no redundant commit"
    );
}
