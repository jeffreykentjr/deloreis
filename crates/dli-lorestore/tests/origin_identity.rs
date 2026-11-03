//! Typed provenance origins must retain their exact identities after reopening.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_ast::decl::Decl;
use dli_lorestore::Lorestore;
use dli_world::pack::Pack;

#[test]
fn ambiguous_origins_remain_tags_after_a_store_restart() {
    for (provenance, origins) in [
        ("proposed origin", vec!["group#42"]),
        ("proposed origin", vec!["group+scene"]),
        ("proposed origin", vec!["group", "scene"]),
        ("proposed origin assumes clue", vec!["group"]),
        ("ratified d-1 origin", vec!["group"]),
    ] {
        let source = format!("scope story\nseed(wren) {provenance}\n");
        let (mut document, diagnostics) = dli_syntax::parse_document(&source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
        let Decl::Fact(fact) = &mut document.decls[0] else {
            panic!("fact")
        };
        fact.provenance.as_mut().unwrap().tags = origins.iter().map(ToString::to_string).collect();
        let pack = Pack {
            name: "story".into(),
            documents: vec![document],
        };
        let root = tempfile::tempdir().unwrap();
        let store = Lorestore::open(root.path().to_path_buf()).unwrap();
        let head = store.import_pack(&pack).unwrap();
        drop(store);
        let reopened = Lorestore::open(root.path().to_path_buf()).unwrap();
        let restored = reopened.reconstruct_at_uncached(&head).unwrap();
        let annotation = restored.documents[0]
            .decls
            .iter()
            .find_map(|decl| match decl {
                Decl::Fact(fact) => fact.provenance.as_ref(),
                _ => None,
            })
            .expect("fact remains");
        assert_eq!(annotation.tags, origins, "{source}");
        assert_eq!(annotation.bundle, None, "{source}");
    }
}

#[test]
fn ambiguous_origins_on_attributes_and_bare_entities_are_carried() {
    for source in [
        "scope story\nwren.home = nest proposed origin\n",
        "scope story\nperson wren proposed origin\n",
    ] {
        let (mut document, findings) = dli_syntax::parse_document(source);
        assert!(findings.is_empty(), "{findings:?}");
        let annotation = match &mut document.decls[0] {
            Decl::Attr(attr) => attr.provenance.as_mut().unwrap(),
            Decl::Entity(entity) => entity.provenance.as_mut().unwrap(),
            _ => panic!("expected attribute or entity"),
        };
        annotation.tags = vec!["group#42".into()];
        let pack = Pack {
            name: "story".into(),
            documents: vec![document],
        };
        let root = tempfile::tempdir().unwrap();
        let store = Lorestore::open(root.path().to_path_buf()).unwrap();
        let head = store.import_pack(&pack).unwrap();
        let restored = store.reconstruct_at_uncached(&head).unwrap();
        let annotation = restored.documents[0]
            .decls
            .iter()
            .find_map(|decl| match decl {
                Decl::Attr(attr) => attr.provenance.as_ref(),
                Decl::Entity(entity) => entity.provenance.as_ref(),
                _ => None,
            })
            .expect("declaration remains");
        assert_eq!(annotation.tags, ["group#42"], "{source}");
        assert_eq!(annotation.bundle, None, "{source}");
    }
}

#[test]
fn distinct_tag_shapes_survive_a_shared_support_and_deduplication() {
    let (mut first, _) = dli_syntax::parse_document("scope story\nseed(wren) proposed origin\n");
    let (mut second, _) = dli_syntax::parse_document("scope story\nseed(wren) proposed origin\n");
    let Decl::Fact(fact) = &mut first.decls[0] else {
        unreachable!()
    };
    fact.provenance.as_mut().unwrap().tags = vec!["group+scene".into()];
    let Decl::Fact(fact) = &mut second.decls[0] else {
        unreachable!()
    };
    fact.provenance.as_mut().unwrap().tags = vec!["group".into(), "scene".into()];
    let pack = Pack {
        name: "story".into(),
        documents: vec![first, second],
    };
    let root = tempfile::tempdir().unwrap();
    let store = Lorestore::open(root.path().to_path_buf()).unwrap();
    let head = store.import_pack(&pack).unwrap();
    let restored = store.reconstruct_at_uncached(&head).unwrap();
    let origins: std::collections::BTreeSet<Vec<String>> = restored.documents[0]
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fact(fact) => fact
                .provenance
                .as_ref()
                .map(|annotation| annotation.tags.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        origins,
        [
            vec!["group+scene".into()],
            vec!["group".into(), "scene".into()]
        ]
        .into_iter()
        .collect()
    );
}
