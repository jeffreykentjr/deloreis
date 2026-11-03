//! Preserve distinct support and later standing revisions of carried entities.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use dli_ast::annot::ProvenanceMarker;
use dli_ast::decl::Decl;
use dli_core::atom::Interner;
use dli_core::posbool::PosBool;
use dli_lorestore::lower::{lower_pack, reconstruct_pack, render_polynomial};
use dli_world::{compile, pack::Pack};

fn fixture() -> (Vec<dli_lorestore::FactObject>, Interner, BTreeSet<String>) {
    let (document, findings) = dli_syntax::parse_document(
        "scope story\nratify d-1\nratify d-2\nperson wren \"Wren\" decided d-1\n",
    );
    assert!(findings.is_empty(), "{findings:?}");
    let (objects, names) = lower_pack(&Pack {
        name: "story".into(),
        documents: vec![document],
    });
    (
        objects,
        Interner::from_names(names),
        ["d-1".to_owned(), "d-2".to_owned()].into_iter().collect(),
    )
}

#[test]
fn a_metadata_revision_updates_the_carried_entity_standing() {
    let (mut objects, interner, decisions) = fixture();
    let fact = objects
        .iter_mut()
        .find(|object| object.expert_body.is_none())
        .unwrap();
    fact.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    let restored = reconstruct_pack(&objects, &interner, &decisions, "story").unwrap();
    let marker = restored
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .find_map(|decl| match decl {
            Decl::Entity(entity) => entity.provenance.as_ref().map(|ann| ann.marker),
            _ => None,
        });
    assert_eq!(marker, Some(ProvenanceMarker::Provisional));
}

#[test]
fn a_distinct_adjacent_support_is_not_mistaken_for_the_carried_atom() {
    let (mut objects, mut interner, decisions) = fixture();
    let d2 = interner.intern("d-2");
    let index = objects
        .iter()
        .position(|object| object.expert_body.is_none())
        .unwrap();
    let mut alternative = objects.remove(index);
    alternative.support = PosBool::pos(d2);
    alternative.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Ratified);
    objects.insert(index, alternative);
    let restored = reconstruct_pack(&objects, &interner, &decisions, "story").unwrap();
    let world = compile(&restored).unwrap();
    let supports: BTreeSet<_> = world
        .model
        .atoms()
        .iter()
        .map(|atom| render_polynomial(&atom.ann.support, world.model.interner()))
        .collect();
    assert_eq!(
        supports,
        ["{+d-1}".to_owned(), "{+d-2}".to_owned()]
            .into_iter()
            .collect()
    );
}

#[test]
fn a_folded_fact_before_its_carriage_still_updates_standing() {
    let (mut objects, interner, decisions) = fixture();
    let fact_index = objects
        .iter()
        .position(|object| object.expert_body.is_none())
        .unwrap();
    let mut fact = objects.remove(fact_index);
    fact.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    let carriage_index = objects
        .iter()
        .position(|object| {
            object
                .expert_body
                .as_ref()
                .is_some_and(|body| body.kind == dli_lorestore::object::ExpertDeclKind::Entity)
        })
        .unwrap();
    objects.insert(carriage_index, fact);
    let restored = reconstruct_pack(&objects, &interner, &decisions, "story").unwrap();
    let marker = restored
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .find_map(|decl| match decl {
            Decl::Entity(entity) => entity.provenance.as_ref().map(|ann| ann.marker),
            _ => None,
        });
    assert_eq!(marker, Some(ProvenanceMarker::Provisional));
}

#[test]
fn conflicting_unfolded_standing_metadata_is_refused() {
    let (mut objects, interner, decisions) = fixture();
    let index = objects
        .iter()
        .position(|object| object.expert_body.is_none())
        .unwrap();
    let mut other = objects[index].clone();
    other.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    objects.push(other);
    let error = reconstruct_pack(&objects, &interner, &decisions, "story").unwrap_err();
    assert!(
        error.to_string().contains("conflicting standing metadata"),
        "{error}"
    );
}

#[test]
fn a_standing_revision_survives_reopen_without_rewriting_history() {
    let directory = tempfile::tempdir().unwrap();
    let (document, findings) =
        dli_syntax::parse_document("scope story\nratify d-1\nperson wren \"Wren\" decided d-1\n");
    assert!(findings.is_empty(), "{findings:?}");
    let pack = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let store = dli_lorestore::Lorestore::open(directory.path().to_path_buf()).unwrap();
    let old_head = store.import_pack(&pack).unwrap();
    let old_hashes = store.fact_hashes_from(&old_head).unwrap();
    let original = old_hashes
        .iter()
        .map(|hash| {
            store
                .read_object::<dli_lorestore::FactObject>(hash)
                .unwrap()
                .unwrap()
        })
        .find(|object| object.expert_body.is_none() && object.atom.pred == "person")
        .unwrap();
    let mut revised = original.clone();
    revised.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    assert_eq!(
        original.content_hash().unwrap(),
        revised.content_hash().unwrap()
    );
    let revised_hash = store.write_object(&revised).unwrap();
    let old_hash = old_hashes
        .iter()
        .find(|hash| {
            store
                .read_object::<dli_lorestore::FactObject>(hash)
                .unwrap()
                .unwrap()
                == original
        })
        .unwrap();
    assert_ne!(old_hash, &revised_hash);
    let head = store
        .commit(
            store.head_interner().unwrap(),
            vec![revised_hash.clone()],
            vec![],
            vec![],
            vec!["d-1".into()],
            "story".into(),
        )
        .unwrap();
    drop(store);

    let reopened = dli_lorestore::Lorestore::open(directory.path().to_path_buf()).unwrap();
    let folded = reopened.fact_hashes_from(&head).unwrap();
    assert!(folded.contains(&revised_hash));
    assert!(!folded.contains(old_hash));
    let marker_at = |commit| {
        reopened
            .reconstruct_at_uncached(commit)
            .unwrap()
            .documents
            .into_iter()
            .flat_map(|doc| doc.decls)
            .find_map(|decl| match decl {
                Decl::Entity(entity) => entity.provenance.map(|ann| ann.marker),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(marker_at(&head), ProvenanceMarker::Provisional);
    assert_eq!(marker_at(&old_head), ProvenanceMarker::Decided);
    let compiled = reopened.compile().unwrap();
    let atom = compiled
        .model
        .atoms()
        .iter()
        .find(|atom| atom.atom.pred.text == "person" && atom.mentions("wren"))
        .unwrap();
    assert_eq!(atom.kind, dli_world::model::Provenance::Provisional);
    assert_eq!(atom.ann.support, original.support);
}
