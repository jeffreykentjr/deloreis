#![allow(clippy::expect_used, clippy::unwrap_used)]
//! The predicate-identity declaration must survive every durable lowering
//! path.  It is carriage-only, just like the existing entity identity
//! declaration: the typed declaration is stored, no synthetic ground atom is
//! emitted, and reconstruction feeds the same identity state back to the
//! compiled world.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, PredicateSameAsDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_core::atom::Interner;
use dli_lorestore::lower::{declared_decisions, lower_decl, lower_pack, reconstruct_pack};
use dli_lorestore::object::ExpertDeclKind;
use dli_world::compile;
use dli_world::pack::Pack;

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn ratified(decision: &str) -> Annotation {
    Annotation {
        marker: ProvenanceMarker::Ratified,
        citations: vec![CitationRef { id: id(decision) }],
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn pack() -> Pack {
    Pack {
        name: "predicate-parity".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("predicate-parity"),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Ratify(RatifyDecl {
                    decision: id("D-predicate"),
                    body: dli_ast::language::RatifyBody::default(),
                }),
                Decl::PredicateSameAs(PredicateSameAsDecl {
                    left: id("current-work"),
                    right: id("current-job"),
                    provenance: Some(ratified("D-predicate")),
                }),
            ],
        }],
    }
}

#[test]
fn predicate_identity_carriage_is_typed_and_atom_free() {
    let pack = pack();
    let (objects, _) = lower_pack(&pack);
    let carriage = objects
        .iter()
        .find(|object| {
            object
                .expert_body
                .as_ref()
                .is_some_and(|body| body.kind == ExpertDeclKind::PredicateSameAs)
        })
        .expect("predicate identity carriage");

    assert_eq!(carriage.atom.pred, "__decl");
    assert_eq!(
        carriage.atom.args,
        vec!["PredicateSameAs", "current-work=current-job"]
    );
    assert_eq!(carriage.expert_body.as_ref().expect("body").body.len(), 1);
    assert!(
        objects.iter().all(|object| {
            object.expert_body.is_some() || object.atom.pred != "predicate-same-as"
        })
    );
}

#[test]
fn lower_decl_and_pack_and_reconstruction_agree_on_predicate_identity() {
    let pack = pack();
    let compiled = compile(&pack).expect("typed pack compiles");
    let (pack_objects, _) = lower_pack(&pack);

    let mut interner = Interner::new();
    let mut bundle_counter = 0;
    let decl_objects: Vec<_> = pack
        .documents
        .iter()
        .flat_map(|document| document.decls.iter())
        .flat_map(|decl| lower_decl(decl, &pack.name, &mut interner, &mut bundle_counter))
        .collect();
    let pack_carriage = pack_objects
        .iter()
        .find(|object| {
            object
                .expert_body
                .as_ref()
                .is_some_and(|body| body.kind == ExpertDeclKind::PredicateSameAs)
        })
        .expect("pack carriage");
    let decl_carriage = decl_objects
        .iter()
        .find(|object| {
            object
                .expert_body
                .as_ref()
                .is_some_and(|body| body.kind == ExpertDeclKind::PredicateSameAs)
        })
        .expect("per-declaration carriage");
    assert_eq!(pack_carriage, decl_carriage);

    let reconstructed = reconstruct_pack(
        &pack_objects,
        compiled.model().interner(),
        &declared_decisions(&pack),
        &pack.name,
    )
    .expect("carriage reconstructs");
    let recompiled = compile(&reconstructed).expect("reconstructed pack compiles");

    assert_eq!(
        compiled.model().predicate_identity.links,
        recompiled.model().predicate_identity.links,
        "the loader and durable reconstruction retain the same typed identity claim"
    );
}
