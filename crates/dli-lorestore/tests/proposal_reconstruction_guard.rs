//! Refuse replay when positional proposal identities cannot be preserved.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_core::Interner;
use dli_lorestore::lower::{lower_decl, lower_pack, reconstruct_pack};
use dli_world::pack::Pack;
use std::collections::BTreeSet;

fn doc(source: &str) -> dli_ast::decl::Document {
    let (doc, diagnostics) = dli_syntax::parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    doc
}

#[test]
fn preserves_a_gap_in_anonymous_proposals() {
    let document = doc("scope story\nperson wren \"Wren\" proposed\n");
    let mut interner = Interner::from_names(vec!["h.story#0".into()]);
    let objects = lower_decl(&document.decls[0], "story", &mut interner, &mut 1);
    let pack = reconstruct_pack(&objects, &interner, &BTreeSet::new(), "story").unwrap();
    let rendered = dli_syntax::format_document(&pack.documents[0]);
    assert!(
        rendered.contains("proposed-bundle \"story#1\""),
        "{rendered}"
    );
}

#[test]
fn preserves_anonymous_proposals_from_multiple_scopes() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            doc("scope a\nperson wren \"Wren\" proposed\n"),
            doc("scope b\nperson robin \"Robin\" proposed\n"),
        ],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    assert!(rendered.contains("proposed-bundle \"a#0\""), "{rendered}");
    assert!(rendered.contains("proposed-bundle \"b#1\""), "{rendered}");
}

#[test]
fn keeps_sequential_anonymous_proposals_readable() {
    let pack = Pack {
        name: "story".into(),
        documents: vec![doc(
            "scope story\nperson wren \"Wren\" proposed\nperson robin \"Robin\" proposed\n",
        )],
    };
    let (objects, names) = lower_pack(&pack);
    reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "story",
    )
    .expect("proposal identities can be preserved");
}

#[test]
fn refuses_a_missing_proposal_fact_regenerated_from_carriage() {
    let document = doc("scope story\nperson wren \"Wren\" proposed\n");
    let mut interner = Interner::from_names(vec!["h.story#0".into()]);
    let mut objects = lower_decl(&document.decls[0], "story", &mut interner, &mut 1);
    objects.retain(|object| object.expert_body.is_some());
    let error = reconstruct_pack(&objects, &interner, &BTreeSet::new(), "story").unwrap_err();
    assert!(error.to_string().contains("absent"), "{error}");
}

#[test]
fn stored_carriage_without_its_proposal_fact_refuses_on_reopen() {
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let document = doc("scope story\nperson wren \"Wren\" proposed\n");
    let mut interner = Interner::from_names(vec!["h.story#0".into()]);
    let mut objects = lower_decl(&document.decls[0], "story", &mut interner, &mut 1);
    objects.retain(|object| object.expert_body.is_some());
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
            vec![],
            "story".into(),
        )
        .unwrap();
    drop(store);
    let reopened = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let error = reopened.reconstruct_at_uncached(&head).unwrap_err();
    assert!(error.to_string().contains("absent"), "{error}");
}
