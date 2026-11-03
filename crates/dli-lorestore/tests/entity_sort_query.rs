//! Entity and era sort queries agree before and after store replay.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_ast::{
    ident::Ident,
    term::{AtomExpr, Term},
};
use dli_eval::query::Query;
use dli_world::{compile, pack::Pack};

#[test]
fn sort_queries_and_era_chain_members_survive_store_reopen() {
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope story\nratify d-1\nperson wren \"Wren\" ratified d-1\nera dawn < noon < dusk ratified d-1\n",
    );
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let pack = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let original = compile(&pack).unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let head = store.import_pack(&pack).unwrap();
    drop(store);
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let replay = store.compile_at(&head).unwrap();
    for (predicate, expected) in [
        ("person", vec!["wren"]),
        ("era", vec!["dawn", "dusk", "noon"]),
    ] {
        let query = Query {
            goal: AtomExpr {
                pred: Ident::synthetic(predicate),
                args: vec![Term::Var(Ident::synthetic("x"))],
            },
        };
        let values = |world: &dli_world::CompiledWorld| {
            world
                .model
                .world
                .query(&query)
                .unwrap()
                .rows
                .into_iter()
                .map(|row| row.bindings["x"].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(values(&original), expected);
        assert_eq!(values(&replay), expected);
    }
}

#[test]
fn proposed_era_members_remain_possible_after_reopen() {
    let (document, diagnostics) =
        dli_syntax::parse_document("scope story\nera dawn < noon < dusk proposed\n");
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let pack = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let head = store.import_pack(&pack).unwrap();
    drop(store);
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let replay = store.compile_at(&head).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("era"),
            args: vec![Term::Var(Ident::synthetic("x"))],
        },
    };
    let answer = replay.model.world.query(&query).unwrap();
    assert_eq!(answer.rows.len(), 3);
    assert!(
        answer
            .rows
            .iter()
            .all(|row| row.status == dli_core::ann::Status::Possible)
    );
}

#[test]
fn independent_era_supports_survive_chain_reconstruction() {
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope story\nera dawn < noon proposed first\nera noon proposed second\n",
    );
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let pack = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let original = compile(&pack).unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let head = store.import_pack(&pack).unwrap();
    drop(store);
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let replay = store.compile_at(&head).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("era"),
            args: vec![Term::Sym(Ident::synthetic("noon"))],
        },
    };
    let expected = original.model.world.query(&query).unwrap();
    let actual = replay.model.world.query(&query).unwrap();
    assert_eq!(expected, actual);
    assert_eq!(expected.rows.len(), 1);
    assert_eq!(expected.rows[0].verdict.support.monomials().count(), 2);
}
