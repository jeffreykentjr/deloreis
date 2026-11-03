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
fn a_distinct_adjacent_support_does_not_replace_the_missing_carried_atom() {
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
    let error = reconstruct_pack(&objects, &interner, &decisions, "story").unwrap_err();
    assert!(error.to_string().contains("absent"), "{error}");
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

#[test]
fn carried_entity_and_slot_revisions_remain_independent() {
    let (document, findings) = dli_syntax::parse_document(
        "scope story\nratify d-1\nperson wren \"Wren\" decided d-1\n  wren.home = hollow\n  wren.trade = baker decided d-1\n",
    );
    assert!(
        findings.iter().all(|finding| !finding.class.is_error()),
        "{findings:?}"
    );
    let (mut objects, names) = lower_pack(&Pack {
        name: "story".into(),
        documents: vec![document],
    });
    let home = objects
        .iter_mut()
        .find(|object| object.atom.pred == "home")
        .unwrap();
    home.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &["d-1".to_owned()].into_iter().collect(),
        "story",
    )
    .unwrap();
    let entity = restored
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .find_map(|decl| match decl {
            Decl::Entity(entity) => Some(entity),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        entity.provenance.as_ref().unwrap().marker,
        ProvenanceMarker::Decided
    );
    assert_eq!(
        entity.slots[0].provenance.as_ref().unwrap().marker,
        ProvenanceMarker::Provisional
    );
    assert_eq!(
        entity.slots[1].provenance.as_ref().unwrap().marker,
        ProvenanceMarker::Decided
    );
}

#[test]
fn a_slot_standing_revision_survives_reopen_and_preserves_old_revision() {
    let directory = tempfile::tempdir().unwrap();
    let (document, findings) = dli_syntax::parse_document(
        "scope story\nratify d-1\nperson wren \"Wren\" decided d-1\n  wren.home = hollow\n",
    );
    assert!(
        findings.iter().all(|finding| !finding.class.is_error()),
        "{findings:?}"
    );
    let pack = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let store = dli_lorestore::Lorestore::open(directory.path().to_path_buf()).unwrap();
    let old = store.import_pack(&pack).unwrap();
    let original = store
        .fact_hashes_from(&old)
        .unwrap()
        .iter()
        .map(|hash| {
            store
                .read_object::<dli_lorestore::FactObject>(hash)
                .unwrap()
                .unwrap()
        })
        .find(|object| object.atom.pred == "home")
        .unwrap();
    let mut revised = original.clone();
    revised.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    let revised_hash = store.write_object(&revised).unwrap();
    let head = store
        .commit(
            store.head_interner().unwrap(),
            vec![revised_hash],
            vec![],
            vec![],
            vec!["d-1".into()],
            "story".into(),
        )
        .unwrap();
    drop(store);
    let reopened = dli_lorestore::Lorestore::open(directory.path().to_path_buf()).unwrap();
    let get = |hash| {
        reopened
            .reconstruct_at_uncached(hash)
            .unwrap()
            .documents
            .into_iter()
            .flat_map(|doc| doc.decls)
            .find_map(|decl| match decl {
                Decl::Entity(entity) => Some((
                    entity.provenance.unwrap().marker,
                    entity.slots[0].provenance.as_ref().map(|ann| ann.marker),
                )),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(get(&old), (ProvenanceMarker::Decided, None));
    assert_eq!(
        get(&head),
        (
            ProvenanceMarker::Decided,
            Some(ProvenanceMarker::Provisional)
        )
    );
    let compiled = reopened.compile().unwrap();
    let home = compiled
        .model
        .atoms()
        .iter()
        .find(|atom| atom.atom.pred.text == "home")
        .unwrap();
    assert_eq!(home.kind, dli_world::model::Provenance::Provisional);
    assert_eq!(home.ann.support, original.support);
}

#[test]
fn an_independent_adjacent_slot_support_is_not_consumed() {
    let (document, findings) = dli_syntax::parse_document(
        "scope story\nratify d-1\nratify d-2\nperson wren \"Wren\" decided d-1\n  wren.home = hollow decided d-1\n",
    );
    assert!(
        findings.iter().all(|finding| !finding.class.is_error()),
        "{findings:?}"
    );
    let (mut objects, names) = lower_pack(&Pack {
        name: "story".into(),
        documents: vec![document],
    });
    let mut interner = Interner::from_names(names);
    let alternative_index = objects
        .iter()
        .position(|object| object.atom.pred == "home")
        .unwrap();
    let mut alternative = objects[alternative_index].clone();
    alternative.support = PosBool::pos(interner.intern("d-2"));
    alternative.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Ratified);
    objects.insert(alternative_index, alternative);
    let restored = reconstruct_pack(
        &objects,
        &interner,
        &["d-1".to_owned(), "d-2".to_owned()].into_iter().collect(),
        "story",
    )
    .unwrap();
    let world = compile(&restored).unwrap();
    let supports: BTreeSet<_> = world
        .model
        .atoms()
        .iter()
        .filter(|atom| atom.atom.pred.text == "home")
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
fn mixed_cited_and_proposed_slots_keep_independent_support_and_standing() {
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope story\nratify d-1\nratify d-2\nperson wren \"Wren\" decided d-1\n  wren.home = hollow decided d-1\n  wren.trade = baker proposed\n",
    );
    assert!(
        diagnostics.iter().all(|item| !item.class.is_error()),
        "{diagnostics:?}"
    );
    let (mut objects, names) = lower_pack(&Pack {
        name: "story".into(),
        documents: vec![document],
    });
    let mut interner = Interner::from_names(names);
    for object in objects.iter_mut().filter(|object| {
        object.expert_body.is_none() && matches!(object.atom.pred.as_str(), "person" | "home")
    }) {
        object.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    }
    let index = objects
        .iter()
        .position(|object| object.atom.pred == "home")
        .unwrap();
    let mut alternative = objects[index].clone();
    alternative.support = PosBool::pos(interner.intern("d-2"));
    alternative.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Ratified);
    objects.insert(index, alternative);
    let restored = reconstruct_pack(
        &objects,
        &interner,
        &["d-1".to_owned(), "d-2".to_owned()].into_iter().collect(),
        "story",
    )
    .unwrap();
    let entity = restored
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .find_map(|decl| match decl {
            Decl::Entity(entity) => Some(entity),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        entity.provenance.as_ref().unwrap().marker,
        ProvenanceMarker::Provisional
    );
    assert_eq!(
        entity.slots[0].provenance.as_ref().unwrap().marker,
        ProvenanceMarker::Provisional
    );
    assert_eq!(
        entity.slots[1].provenance.as_ref().unwrap().marker,
        ProvenanceMarker::Proposed
    );
    let world = compile(&restored).unwrap();
    let supports: BTreeSet<_> = world
        .model
        .atoms()
        .iter()
        .filter(|atom| atom.atom.pred.text == "home")
        .map(|atom| render_polynomial(&atom.ann.support, world.model.interner()))
        .collect();
    assert_eq!(
        supports,
        ["{+d-1}".to_owned(), "{+d-2}".to_owned()]
            .into_iter()
            .collect()
    );
    let trade = world
        .model
        .atoms()
        .iter()
        .find(|atom| atom.atom.pred.text == "trade")
        .unwrap();
    assert_eq!(
        render_polynomial(&trade.ann.support, world.model.interner()),
        "{+h.story#0}"
    );
}

#[test]
fn mixed_slot_preserves_a_proposal_after_an_earlier_proposal() {
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope story\nratify d-1\nperson reed \"Reed\" proposed\nperson wren \"Wren\" decided d-1\n  wren.trade = baker proposed\n",
    );
    assert!(
        diagnostics.iter().all(|item| !item.class.is_error()),
        "{diagnostics:?}"
    );
    let original = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let (objects, names) = lower_pack(&original);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &["d-1".to_owned()].into_iter().collect(),
        "story",
    )
    .unwrap();
    let world = compile(&restored).unwrap();
    let supports: BTreeSet<_> = world
        .model
        .atoms()
        .iter()
        .filter(|atom| atom.atom.pred.text == "trade")
        .map(|atom| render_polynomial(&atom.ann.support, world.model.interner()))
        .collect();
    assert_eq!(supports, ["{+h.story#1}".to_owned()].into_iter().collect());
}

#[test]
fn mixed_slots_and_separate_support_survive_a_store_reopen() {
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope story\nratify d-1\nratify d-2\nperson wren \"Wren\" decided d-1\n  wren.home = hollow decided d-1\n  wren.trade = baker proposed\n",
    );
    assert!(
        diagnostics.iter().all(|item| !item.class.is_error()),
        "{diagnostics:?}"
    );
    let (mut objects, names) = lower_pack(&Pack {
        name: "story".into(),
        documents: vec![document],
    });
    let mut interner = Interner::from_names(names);
    for object in objects.iter_mut().filter(|object| {
        object.expert_body.is_none() && matches!(object.atom.pred.as_str(), "person" | "home")
    }) {
        object.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    }
    let index = objects
        .iter()
        .position(|object| object.atom.pred == "home")
        .unwrap();
    let mut independent = objects[index].clone();
    independent.support = PosBool::pos(interner.intern("d-2"));
    independent.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Ratified);
    objects.insert(index, independent);
    let hashes = objects
        .iter()
        .map(|object| store.write_object(object).unwrap())
        .collect();
    let head = store
        .commit_on_branch(
            "main",
            interner.names().to_vec(),
            hashes,
            vec![],
            vec![],
            vec!["d-1".into(), "d-2".into()],
            "story".into(),
        )
        .unwrap();
    drop(store);
    let reopened = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let restored = reopened.reconstruct_at_uncached(&head).unwrap();
    let world = compile(&restored).unwrap();
    let home: BTreeSet<_> = world
        .model
        .atoms()
        .iter()
        .filter(|atom| atom.atom.pred.text == "home")
        .map(|atom| render_polynomial(&atom.ann.support, world.model.interner()))
        .collect();
    assert_eq!(
        home,
        ["{+d-1}".to_owned(), "{+d-2}".to_owned()]
            .into_iter()
            .collect()
    );
    let trade = world
        .model
        .atoms()
        .iter()
        .find(|atom| atom.atom.pred.text == "trade")
        .unwrap();
    assert_eq!(
        render_polynomial(&trade.ann.support, world.model.interner()),
        "{+h.story#0}"
    );
    let text = dli_syntax::format_document(&restored.documents[0]);
    let (reparsed, diagnostics) = dli_syntax::parse_document(&text);
    assert!(
        diagnostics.iter().all(|item| !item.class.is_error()),
        "{diagnostics:?}"
    );
    let roundtrip = compile(&Pack {
        name: "story".into(),
        documents: vec![reparsed],
    })
    .unwrap();
    let roundtrip_home: BTreeSet<_> = roundtrip
        .model
        .atoms()
        .iter()
        .filter(|atom| atom.atom.pred.text == "home")
        .map(|atom| render_polynomial(&atom.ann.support, roundtrip.model.interner()))
        .collect();
    assert_eq!(home, roundtrip_home);
}
