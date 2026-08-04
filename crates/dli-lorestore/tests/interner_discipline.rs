//! Audit A6 oracles: the commit interner is append-only across the chain
//! (`compile()` resolves every commit's facts against the head interner, so a
//! child that drops or reorders a parent name silently misresolves earlier
//! facts' polynomials), and base import is idempotent + single-lineage
//! (audit A1: attaching at every serve start must not stack commits, and one
//! single-branch store carries exactly one pack lineage).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_ast::annot::{Annotation, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, EntityDecl, EntityKind, FactDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_lorestore::{Lorestore, StoreError};
use dli_world::pack::Pack;

fn id(text: &str) -> Ident {
    Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

fn tiny_pack(name: &str, subject: &str, object: &str) -> Pack {
    let proposed = Annotation {
        marker: ProvenanceMarker::Proposed,
        citations: Vec::new(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    };
    Pack {
        name: name.to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id(name),
                span: Span::synthetic(),
            }),
            decls: vec![
                Decl::Entity(EntityDecl {
                    kind: EntityKind::Place,
                    name: id(subject),
                    label: None,
                    time: None,
                    slots: Vec::new(),
                    provenance: Some(proposed.clone()),
                }),
                Decl::Entity(EntityDecl {
                    kind: EntityKind::Place,
                    name: id(object),
                    label: None,
                    time: None,
                    slots: Vec::new(),
                    provenance: Some(proposed.clone()),
                }),
                Decl::Fact(FactDecl {
                    atom: AtomExpr {
                        pred: id("linked"),
                        args: vec![Term::Sym(id(subject)), Term::Sym(id(object))],
                    },
                    triple: true,
                    provenance: Some(proposed),
                }),
            ],
        }],
    }
}

#[test]
fn commit_refuses_interner_regression() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ls = Lorestore::open(temp.path().to_path_buf()).expect("open");

    let names = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let commit = |interner: Vec<String>| {
        ls.commit(
            interner,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            "t".to_owned(),
        )
    };

    // Extension of the (empty) genesis interner: fine.
    commit(names(&["a", "b"])).expect("extension over genesis");
    // Append-only extension: fine.
    commit(names(&["a", "b", "c"])).expect("append-only extension");
    // Reordering an earlier name: refused.
    match commit(names(&["a", "x", "c"])) {
        Err(StoreError::InternerRegression { diverges_at, .. }) => assert_eq!(diverges_at, 1),
        other => panic!("reorder must be refused, got {other:?}"),
    }
    // Dropping names: refused.
    match commit(names(&["a"])) {
        Err(StoreError::InternerRegression {
            parent_len,
            diverges_at,
        }) => {
            assert_eq!(parent_len, 3);
            assert_eq!(diverges_at, 1);
        }
        other => panic!("truncation must be refused, got {other:?}"),
    }
}

#[test]
fn ensure_pack_imported_is_idempotent_and_single_lineage() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ls = Lorestore::open(temp.path().to_path_buf()).expect("open");
    let pack = tiny_pack("harbour", "quay", "mole");

    // First import advances the head from genesis.
    let first = ls.ensure_pack_imported(&pack).expect("first import");
    let head_after_first = ls.root_hash().expect("root ok").expect("head");
    assert_eq!(first, head_after_first, "import returns the new head");

    // Second import is a no-op: same head, no stacked commit.
    let second = ls.ensure_pack_imported(&pack).expect("second import");
    assert_eq!(second, first, "re-import must not move the head");
    assert_eq!(
        ls.walk_dag(&first, None).expect("walk").len(),
        2,
        "exactly genesis + one import commit"
    );

    // A different pack lineage is refused, not silently mixed in.
    let foreign = tiny_pack("orchard", "bough", "root-stock");
    match ls.ensure_pack_imported(&foreign) {
        Err(StoreError::ForeignBase {
            store_pack,
            importing,
        }) => {
            assert_eq!(store_pack, "harbour");
            assert_eq!(importing, "orchard");
        }
        other => panic!("foreign base must be refused, got {other:?}"),
    }

    // The folded store still compiles to the imported world.
    let from_store = ls.compile().expect("store compiles");
    let direct = dli_world::compile(&pack).expect("direct compile");
    assert_eq!(
        from_store.model().interner().names(),
        direct.model().interner().names(),
        "store-compiled interner matches the direct compile"
    );
    assert_eq!(
        from_store.model().atoms().len(),
        direct.model().atoms().len(),
        "store-compiled atom count matches the direct compile"
    );
}

/// The decision set obeys the same append-only discipline as the interner, and
/// for the same reason: `reconstruct` reads standing against the head commit's
/// decisions, so a child that drops one demotes every fact resting on it from
/// `decided` to `proposed`. Held at the commit seam so every write path
/// inherits it — three separate callers grew the same hole while the check
/// lived above this layer (DL-0056).
///
/// Containment, not prefix: the merge path unions decisions as a set, so order
/// carries no meaning here.
#[test]
fn commit_refuses_decision_regression() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ls = Lorestore::open(temp.path().to_path_buf()).expect("open");

    let decisions = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let commit = |decisions: Vec<String>| {
        ls.commit(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            decisions,
            "t".to_owned(),
        )
    };

    commit(decisions(&["d-1"])).expect("a decision over genesis");
    commit(decisions(&["d-1", "d-2"])).expect("append-only extension");
    // Reordering is not a regression — the set is what matters.
    commit(decisions(&["d-2", "d-1"])).expect("reorder keeps every decision");
    // Dropping one the parent held: refused.
    match commit(decisions(&["d-1"])) {
        Err(StoreError::DecisionRegression {
            dropped,
            parent_len,
        }) => {
            assert_eq!(dropped, "d-2");
            assert_eq!(parent_len, 2);
        }
        other => panic!("dropping a known decision must be refused, got {other:?}"),
    }
    // And the empty set — what an omitted wire field decodes to.
    match commit(Vec::new()) {
        Err(StoreError::DecisionRegression { .. }) => {}
        other => panic!("an empty decision set must be refused, got {other:?}"),
    }
}
