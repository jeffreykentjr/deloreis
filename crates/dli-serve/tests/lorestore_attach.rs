//! Audit A1 oracle at the serve layer: attaching a lorestore imports each
//! base pack with **no manual pre-import step**, so the store's folded state
//! contains the world the findings are graded against — `Lorestore::compile()`
//! reproduces the base — and re-attachment over the same directory writes
//! nothing new.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_ast::annot::{Annotation, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_serve::author::store::Store;
use dli_world::pack::Pack;

fn id(text: &str) -> Ident {
    Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

fn base_pack() -> Pack {
    let proposed = Annotation {
        marker: ProvenanceMarker::Proposed,
        citations: Vec::new(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    };
    Pack {
        name: "harbour".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("harbour"),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Entity(EntityDecl {
                    kind: EntityKind::Place,
                    name: id("quay"),
                    label: None,
                    time: None,
                    slots: Vec::new(),
                    provenance: Some(proposed.clone()),
                }),
                Decl::Fact(FactDecl {
                    atom: AtomExpr {
                        pred: id("sheltered"),
                        args: vec![Term::Sym(id("quay"))],
                    },
                    triple: true,
                    provenance: Some(proposed),
                }),
            ],
        }],
    }
}

#[test]
fn attach_imports_the_base_and_is_idempotent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join(".dli-lorestore");

    let mut store = Store::with_pack(base_pack()).expect("base compiles");
    store.attach_lorestore(root.clone()).expect("attach");

    // The base world is in the store with no manual import: compile() folds
    // to the same world the base was graded against.
    let ls = store.lorestore_for("harbour").expect("per-base store");
    let head_first = ls.root_hash().expect("root ok").expect("head");
    let from_store = ls.compile().expect("store compiles the base");
    let direct = dli_world::compile(&base_pack()).expect("direct compile");
    assert_eq!(
        from_store.model().interner().names(),
        direct.model().interner().names(),
        "store-compiled interner matches the direct base compile"
    );
    assert_eq!(
        from_store.model().atoms().len(),
        direct.model().atoms().len(),
        "store-compiled atom count matches the direct base compile"
    );

    // A fresh Store over the same directory re-attaches without writing a new
    // commit — serve restarts do not stack imports.
    let mut store2 = Store::with_pack(base_pack()).expect("base compiles");
    store2.attach_lorestore(root).expect("re-attach");
    let ls2 = store2.lorestore_for("harbour").expect("per-base store");
    let head_second = ls2.root_hash().expect("root ok").expect("head");
    assert_eq!(
        head_second, head_first,
        "re-attachment must not move the head"
    );
}
