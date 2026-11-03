//! Option-implied facts replay by exact support rather than atom spelling.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_core::Interner;
use dli_lorestore::lower::{lower_pack, reconstruct_pack, render_polynomial};
use dli_world::{compile, pack::Pack};
use std::collections::BTreeSet;

fn setup(keyword: &str) -> (Vec<dli_lorestore::FactObject>, Interner) {
    let source = format!(
        "scope story\n{keyword} mystery\n  option choice implies seed(wren)\nseed(wren) proposed independent\n"
    );
    let (document, findings) = dli_syntax::parse_document(&source);
    assert!(findings.is_empty(), "{findings:?}");
    let (objects, names) = lower_pack(&Pack {
        name: "story".into(),
        documents: vec![document],
    });
    (objects, Interner::from_names(names))
}

#[test]
fn an_independent_same_atom_support_is_not_consumed_by_an_option() {
    for keyword in ["question", "fork"] {
        let (mut objects, interner) = setup(keyword);
        let option = objects
            .iter()
            .position(|object| {
                object.expert_body.is_none()
                    && render_polynomial(&object.support, &interner) == "{+choice}"
            })
            .unwrap();
        let independent = objects
            .iter()
            .position(|object| {
                object.expert_body.is_none()
                    && render_polynomial(&object.support, &interner) == "{+h.independent}"
            })
            .unwrap();
        objects.swap(option, independent);
        let restored = reconstruct_pack(&objects, &interner, &BTreeSet::new(), "story").unwrap();
        let world = compile(&restored).unwrap();
        let supports: BTreeSet<_> = world
            .model
            .atoms()
            .iter()
            .filter(|atom| atom.atom.pred.text == "seed")
            .map(|atom| render_polynomial(&atom.ann.support, world.model.interner()))
            .collect();
        assert_eq!(
            supports,
            ["{+choice}".to_owned(), "{+h.independent}".to_owned()]
                .into_iter()
                .collect(),
            "{keyword}"
        );
    }
}

#[test]
fn an_option_cannot_regenerate_its_missing_fact_from_an_independent_support() {
    for keyword in ["question", "fork"] {
        let (mut objects, interner) = setup(keyword);
        objects.retain(|object| {
            object.expert_body.is_some()
                || render_polynomial(&object.support, &interner) != "{+choice}"
        });
        let error = reconstruct_pack(&objects, &interner, &BTreeSet::new(), "story").unwrap_err();
        assert!(error.to_string().contains("absent"), "{keyword}: {error}");
    }
}
