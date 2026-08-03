//! LS-9 upcast-fidelity oracle for the language-layer carriages.
//!
//! `dialect`, `name`, and `ratify` bodies were string-bodied (`Vec<Clause>`)
//! before LS-9 structured their interiors. A schema-v1 string-bodied carriage of
//! any of the three must, on read, upcast to its v2 typed carriage and compile to
//! the identical world. Two levels of proof, mirroring `upcast_fidelity.rs` for
//! `rule`:
//!
//! 1. *Object fidelity* — a v1 carriage (its interior body reverted to the old
//!    `Vec<Clause>`) upcasts to a fact object byte-identical (by content hash) to
//!    the v2 carriage the front end now produces. The naive read (no upcast) is
//!    shown to fail: the v1 string body does not decode as a typed v2 `Decl`, so
//!    the upcast is load-bearing.
//! 2. *Compiled fidelity* — a whole pack stored with every `dialect`/`name`/
//!    `ratify` carriage downgraded to v1 on disk compiles, through
//!    `Lorestore::compile`, to the same facts / fixpoint / findings as a direct
//!    compile.
//!
//! Both fail against the pre-LS-9 state: there is no v2 schema and no typed
//! interior, so a string body is the only shape and nothing upcasts.

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

/// The vendored `cymric-fringe` pack: a `dialect` with the full interior grammar
/// (basis, inventories, `cluster-max`, `stress`, `name-kind`, `name-part`),
/// several `name` derivations (composition + `bearer …` provenance + a standalone
/// provenance line), and a `ratify` carrying a `hyp := true` valuation — every
/// language-layer body the LS-9 structuring touched, in one pack.
fn cymric_pack() -> Pack {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../worlds/tpth/packs/cymric-fringe.dli");
    let src = std::fs::read_to_string(&path).expect("read cymric-fringe.dli");
    let (document, diagnostics) = dli_syntax::parse_document(&src);
    assert!(
        diagnostics.is_empty(),
        "pack must parse clean: {diagnostics:?}"
    );
    Pack {
        name: "cymric-fringe".to_owned(),
        documents: vec![document],
    }
}

/// The externally-tagged `Decl` variant key for a language carriage kind.
fn variant_key(kind: ExpertDeclKind) -> &'static str {
    match kind {
        ExpertDeclKind::Dialect => "Dialect",
        ExpertDeclKind::Name => "Name",
        ExpertDeclKind::Ratify => "Ratify",
        _ => unreachable!("only the LS-9 language carriages are downgraded here"),
    }
}

/// Is `fo` one of the three LS-9 language carriages?
fn is_language_carriage(fo: &FactObject) -> bool {
    fo.expert_body.as_ref().is_some_and(|b| {
        matches!(
            b.kind,
            ExpertDeclKind::Dialect | ExpertDeclKind::Name | ExpertDeclKind::Ratify
        )
    })
}

/// Rewrite a v2 language-carriage fact object into the schema-v1 JSON a pre-LS-9
/// store held: `schema_version` 1, and the embedded declaration's typed body
/// reverted to the old `Vec<Clause>` string body — exactly the retained `clause`
/// value of each structured clause, in order.
fn downgrade_to_v1(fo: &FactObject) -> Value {
    let kind = fo.expert_body.as_ref().expect("carriage").kind;
    let variant = variant_key(kind);

    let mut value = serde_json::to_value(fo).expect("fact object serializes");
    value["schema_version"] = Value::from(1);

    let embedded = value["expert_body"]["body"][0]
        .as_str()
        .expect("carriage body is a JSON string")
        .to_owned();
    let mut decl_v2: Value = serde_json::from_str(&embedded).expect("embedded decl is JSON");
    let inner = decl_v2
        .get_mut(variant)
        .expect("carriage is the expected decl variant");

    // The v1 body is the ordered normalized clause texts (the `clause` field of
    // each typed clause), with the typed payload dropped — the pre-LS-9 shape.
    let v1_body: Vec<Value> = inner
        .get("body")
        .and_then(|b| b.get("clauses"))
        .and_then(Value::as_array)
        .map(|clauses| clauses.iter().map(|c| c["clause"].clone()).collect())
        .unwrap_or_default();
    inner["body"] = Value::Array(v1_body);

    value["expert_body"]["body"][0] =
        Value::from(serde_json::to_string(&decl_v2).expect("v1 decl serializes"));
    value
}

#[test]
fn v1_language_carriages_upcast_to_the_exact_v2_object() {
    let pack = cymric_pack();
    let (objects, _names) = lower_pack(&pack);

    let mut checked = 0;
    for kind in [
        ExpertDeclKind::Dialect,
        ExpertDeclKind::Name,
        ExpertDeclKind::Ratify,
    ] {
        let v2_carriage = objects
            .iter()
            .find(|fo| fo.expert_body.as_ref().is_some_and(|b| b.kind == kind))
            .unwrap_or_else(|| panic!("the pack carries a {kind:?}"));

        let v1_value = downgrade_to_v1(v2_carriage);

        // The v1 embedded declaration must NOT decode as a current `Decl` — the
        // string body (a JSON array) is not the typed interior (a struct). This
        // is what makes the upcast load-bearing rather than cosmetic.
        let v1_embedded = v1_value["expert_body"]["body"][0].as_str().unwrap();
        assert!(
            serde_json::from_str::<Decl>(v1_embedded).is_err(),
            "a v1 string-bodied {kind:?} decl must not decode as a typed v2 Decl without upcast"
        );

        // Upcast v1 → current, decode, and demand byte-identity to the v2 object.
        let lifted = upcast_value(1, v1_value).expect("v1 carriage upcasts");
        let lifted_fo: FactObject =
            serde_json::from_value(lifted).expect("lifted carriage decodes");
        assert_eq!(
            lifted_fo.content_hash().expect("hash"),
            v2_carriage.content_hash().expect("hash"),
            "the upcast v1 {kind:?} carriage must be byte-identical to the v2 carriage"
        );
        assert_eq!(lifted_fo.schema_version, dli_lorestore::SCHEMA_VERSION);
        checked += 1;
    }
    assert_eq!(checked, 3, "all three language carriages were exercised");
}

#[test]
fn a_v1_stored_language_pack_compiles_identically_after_upcast() {
    let pack = cymric_pack();

    // reference A: direct compile.
    let compiled_a = compile(&pack).expect("direct compile");
    let model_a = compiled_a.model();

    // Build a store on disk whose dialect/name/ratify carriages are stored in the
    // schema-v1 string-bodied shape, everything else v2 — a genuine mixed store.
    let (_tmp, _head, store) = build_v1_store(&pack);

    // reference B: compile through the store, which upcasts the v1 carriages.
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
        "fixpoint must match after v1 upcast"
    );
    assert_eq!(
        render_findings(model_a.findings()),
        render_findings(model_b.findings()),
        "findings must match after v1 upcast"
    );
    // The language model really compiled something — the oracle is not vacuous.
    assert!(
        !model_a.language.registers.is_empty(),
        "the dialect must compile for the oracle to be meaningful"
    );
    assert!(
        !model_a.language.names.is_empty(),
        "the names must compile for the oracle to be meaningful"
    );
}

/// Build an on-disk store whose commit holds every `dialect`/`name`/`ratify`
/// carriage in the schema-v1 string-bodied shape (everything else v2). Returns the
/// temp dir (kept alive), the head commit hash, and a `Lorestore` over it.
fn build_v1_store(pack: &Pack) -> (TempDir, dli_lorestore::ContentHash, Lorestore) {
    let tmp = TempDir::new().expect("tempdir");
    let storage = FilesystemStorage::new(tmp.path());
    let (objects, interner_names) = lower_pack(pack);

    let mut fact_hashes = Vec::with_capacity(objects.len());
    for fo in &objects {
        let hash = if is_language_carriage(fo) {
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
        commit_language_carriages_are_v1(&storage, &commit_hash),
        "the language carriages must be stored as schema v1"
    );

    let store = Lorestore::from_storage(Box::new(storage));
    (tmp, commit_hash, store)
}

/// Every language carriage named by the commit is stored at `schema_version` 1.
fn commit_language_carriages_are_v1(
    storage: &FilesystemStorage,
    commit_hash: &dli_lorestore::ContentHash,
) -> bool {
    let bytes = storage
        .read_object(commit_hash)
        .expect("read commit")
        .unwrap();
    let commit: Commit = serde_json::from_slice(&bytes).expect("commit decodes");
    let mut saw_carriage = false;
    for hash in &commit.facts {
        let bytes = storage.read_object(hash).expect("read object").unwrap();
        let value: Value = serde_json::from_slice(&bytes).expect("object json");
        let kind = value
            .get("expert_body")
            .and_then(|e| e.get("kind"))
            .and_then(Value::as_str);
        if matches!(kind, Some("dialect" | "name" | "ratify")) {
            saw_carriage = true;
            if value.get("schema_version").and_then(Value::as_u64) != Some(1) {
                return false;
            }
        }
    }
    saw_carriage
}

/// Pin the `read_typed` upcast path directly for a language carriage.
#[test]
fn read_typed_upcasts_a_v1_language_object_on_read() {
    let pack = cymric_pack();
    let (objects, _names) = lower_pack(&pack);
    let v2_carriage = objects
        .iter()
        .find(|fo| {
            fo.expert_body
                .as_ref()
                .is_some_and(|b| b.kind == ExpertDeclKind::Name)
        })
        .unwrap();

    let tmp = TempDir::new().expect("tempdir");
    let storage = FilesystemStorage::new(tmp.path());
    let v1_bytes = serde_json::to_vec(&downgrade_to_v1(v2_carriage)).unwrap();
    let hash = storage.write_object(&v1_bytes).expect("write v1");

    let read: FactObject = read_typed(&storage, &hash).expect("read").expect("present");
    assert_eq!(read.schema_version, dli_lorestore::SCHEMA_VERSION);
    assert_eq!(
        read.content_hash().expect("hash"),
        v2_carriage.content_hash().expect("hash"),
        "read_typed must lift the v1 name carriage to the v2 object"
    );
}
