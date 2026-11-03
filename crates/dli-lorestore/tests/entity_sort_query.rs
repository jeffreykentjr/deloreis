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

#[test]
fn old_era_chain_store_without_tail_objects_remains_queryable() {
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
    let (objects, names) = dli_lorestore::lower::lower_pack(&pack);
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    // Older stores carried the declaration and head but not its chain tails.
    let hashes = objects
        .iter()
        .filter(|o| o.expert_body.is_some() || o.atom.args.first().is_some_and(|arg| arg == "dawn"))
        .map(|object| store.write_object(object).unwrap())
        .collect();
    let head = store
        .commit_on_branch(
            "main",
            names,
            hashes,
            vec![],
            vec![],
            vec![],
            "story".into(),
        )
        .unwrap();
    drop(store);
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let world = store.compile_at(&head).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("era"),
            args: vec![Term::Var(Ident::synthetic("x"))],
        },
    };
    assert_eq!(world.model.world.query(&query).unwrap().rows.len(), 3);
}

#[test]
fn old_era_chain_keeps_independent_same_member_support() {
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
    let (mut objects, names) = dli_lorestore::lower::lower_pack(&pack);
    let chain_member = objects
        .iter()
        .position(|object| {
            object.atom.pred == "era"
                && object.atom.args == ["noon"]
                && dli_lorestore::lower::render_polynomial(
                    &object.support,
                    &dli_core::Interner::from_names(names.clone()),
                )
                .contains("h.first")
        })
        .unwrap();
    objects.remove(chain_member);
    let restored = dli_lorestore::lower::reconstruct_pack(
        &objects,
        &dli_core::Interner::from_names(names),
        &Default::default(),
        "story",
    )
    .unwrap();
    let replay = compile(&restored).unwrap();
    let query = Query {
        goal: AtomExpr {
            pred: Ident::synthetic("era"),
            args: vec![Term::Sym(Ident::synthetic("noon"))],
        },
    };
    assert_eq!(
        original.model.world.query(&query).unwrap(),
        replay.model.world.query(&query).unwrap()
    );
}

#[test]
fn era_chain_source_atoms_match_per_declaration_store_lowering() {
    let (document, diagnostics) = dli_syntax::parse_document(
        "scope story\nratify d-1\nera dawn < noon < dusk ratified d-1\nera noon ratified d-1\n",
    );
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let pack = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let world = compile(&pack).unwrap();
    let (objects, _) = dli_lorestore::lower::lower_pack(&pack);
    let by_decl: Vec<_> = objects
        .iter()
        .filter(|object| object.expert_body.is_none())
        .map(|object| object.content_hash().unwrap())
        .collect();
    let by_atom: Vec<_> = dli_lorestore::lower::lower_atoms(world.model.atoms())
        .iter()
        .map(|object| object.content_hash().unwrap())
        .collect();
    assert_eq!(by_decl, by_atom);
    assert_eq!(by_atom.len(), 4);
}

#[test]
fn differently_revised_era_member_refuses_instead_of_losing_standing() {
    let (document, diagnostics) =
        dli_syntax::parse_document("scope story\nratify d-1\nera dawn < noon ratified d-1\n");
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{diagnostics:?}"
    );
    let pack = Pack {
        name: "story".into(),
        documents: vec![document],
    };
    let (mut objects, names) = dli_lorestore::lower::lower_pack(&pack);
    let member = objects
        .iter_mut()
        .find(|object| object.atom.pred == "era" && object.atom.args == ["noon"])
        .unwrap();
    member.provenance.as_mut().unwrap().marker =
        Some(dli_ast::annot::ProvenanceMarker::Provisional);
    let error = dli_lorestore::lower::reconstruct_pack(
        &objects,
        &dli_core::Interner::from_names(names),
        &["d-1".to_owned()].into_iter().collect(),
        "story",
    )
    .unwrap_err();
    assert!(error.to_string().contains("standing differs"), "{error}");
}
