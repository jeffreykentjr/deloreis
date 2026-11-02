#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Issue 49: `functional <pred>` is a **carriage-only** declaration.
//!
//! It lowers to a carriage object and no atom object, and — like `ratify` — it
//! interns no provenance: its annotation rides the serialized carriage body, so
//! the loader's bundle counter and the store walk's stay in lock-step (DL-0071).
//! The probes here are differential: the same pack with and without the
//! `functional` declarations must produce the *same* interner table and the
//! *same* atom-object digests, so a lowering that quietly interned the
//! declaration's provenance would shift a proposal-bundle variable and fail
//! here rather than at some later, unrelated import.

use std::collections::BTreeSet;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    AttrDecl, Decl, Document, EntityDecl, EntityKind, FactDecl, FunctionalDecl, RatifyDecl,
    ScopeHeader,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Status;
use dli_eval::query::Query;
use dli_eval::world::World;

use dli_lorestore::Lorestore;
use dli_lorestore::lower::{declared_decisions, lower_atoms, lower_pack, reconstruct_pack};
use dli_lorestore::object::{ExpertDeclKind, FactObject};

use dli_world::compile;
use dli_world::pack::Pack;

use tempfile::TempDir;

// ── builders ──────────────────────────────────────────────────────────

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn marker(m: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker: m,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratified(cite: &str) -> Annotation {
    marker(ProvenanceMarker::Ratified, &[cite])
}

/// A bare proposal — no citation, no tags, so it mints a fresh
/// `h.<pack>#N` bundle variable. That variable is the desync tripwire.
fn proposed() -> Annotation {
    marker(ProvenanceMarker::Proposed, &[])
}

fn ratify_decl(d: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(d),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn entity(kind: EntityKind, name: &str, cite: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(ratified(cite)),
    })
}

fn attr(subject: &str, slot: &str, value: &str, prov: Option<Annotation>) -> Decl {
    Decl::Attr(AttrDecl {
        subject: id(subject),
        attr: id(slot),
        value: Term::Sym(id(value)),
        provenance: prov,
    })
}

fn fact(pred: &str, args: &[&str], prov: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(pred),
            args: args.iter().map(|a| Term::Sym(id(a))).collect(),
        },
        triple: false,
        provenance: prov,
    })
}

fn functional(pred: &str, prov: Option<Annotation>) -> Decl {
    Decl::Functional(FunctionalDecl {
        pred: id(pred),
        provenance: prov,
    })
}

fn pack(name: &str, decls: Vec<Decl>) -> Pack {
    Pack {
        name: name.to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id(name),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

/// The differential pack: ordinary facts, entities, and a slot, with two
/// `functional` declarations threaded *between* two bare proposals when
/// `with_functional` is set. Both a ledger-cited (`ratified D-2`) and a bare
/// `proposed` functional appear, because they would desync differently — the
/// first by interning a decision variable, the second by burning a bundle index.
fn gauntlet(with_functional: bool) -> Pack {
    let mut decls = vec![
        ratify_decl("D-1"),
        // Declared so the citation resolves, and cited by *nothing else* — if
        // the functional decl below interned its provenance, `D-2` would appear
        // in the interner table and the walk would diverge from the loader.
        ratify_decl("D-2"),
        entity(EntityKind::Place, "joren", "D-1"),
        entity(EntityKind::Event, "enclosure", "D-1"),
        attr("joren", "rank", "high", Some(ratified("D-1"))),
    ];
    if with_functional {
        decls.push(functional("rank", Some(ratified("D-2"))));
    }
    // h.weir#0
    decls.push(fact("linked", &["joren", "enclosure"], Some(proposed())));
    if with_functional {
        decls.push(functional("mood", Some(proposed())));
    }
    // h.weir#1 — #2 if the functional above burned an index
    decls.push(fact("omen", &["joren", "storm"], Some(proposed())));
    decls.push(fact("secret", &["joren", "hidden"], None));
    pack("weir", decls)
}

fn carriage_objects(objects: &[FactObject], kind: ExpertDeclKind) -> Vec<&FactObject> {
    objects
        .iter()
        .filter(|o| o.expert_body.as_ref().is_some_and(|b| b.kind == kind))
        .collect()
}

fn digests(objects: &[FactObject]) -> BTreeSet<String> {
    objects
        .iter()
        .map(|o| o.content_hash().expect("hash").to_string())
        .collect()
}

// ── the probes ────────────────────────────────────────────────────────

/// The durable kind name is wire-visible in every stored object — pin the
/// const so a rename lands as a test failure, not as an unreadable store.
#[test]
fn functional_carriage_kind_serializes_as_the_durable_const() {
    assert_eq!(
        serde_json::to_string(&ExpertDeclKind::Functional).expect("serializes"),
        "\"functional\""
    );
}

/// A `functional` declaration lowers to exactly one carriage object of kind
/// `Functional`, under the synthetic `__decl` atom, and to no atom object: it
/// asserts no ground fact.
#[test]
fn functional_decl_lowers_to_a_carriage_object_and_no_atom() {
    let (objects, _names) = lower_pack(&gauntlet(true));

    let carriage = carriage_objects(&objects, ExpertDeclKind::Functional);
    let names: Vec<&str> = carriage
        .iter()
        .map(|o| o.expert_body.as_ref().expect("body").name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["rank", "mood"],
        "each functional decl carries one object, named by its predicate"
    );
    for object in &carriage {
        assert_eq!(
            object.atom.pred, "__decl",
            "a carriage object rides the synthetic decl atom, not a lore fact"
        );
        assert!(
            object
                .expert_body
                .as_ref()
                .is_some_and(|b| b.body.len() == 1 && b.body[0].contains("Functional")),
            "the carriage body is the serialized Decl::Functional"
        );
    }

    // No atom object anywhere mentions the declared predicates as their own
    // fact: `functional mood` must not conjure a `mood(...)` atom, and the
    // `rank` atom that exists comes from the slot, not from the declaration.
    let (bare, _) = lower_pack(&gauntlet(false));
    let atoms_with: Vec<String> = objects
        .iter()
        .filter(|o| o.expert_body.is_none())
        .map(|o| o.atom.render())
        .collect();
    let atoms_without: Vec<String> = bare
        .iter()
        .filter(|o| o.expert_body.is_none())
        .map(|o| o.atom.render())
        .collect();
    assert_eq!(
        atoms_with, atoms_without,
        "a carriage-only declaration adds no atom object"
    );
    assert_eq!(
        objects.len(),
        bare.len() + 2,
        "and adds exactly its two carriage objects"
    );
}

/// The lock-step probe (DL-0071): interning the functional's provenance would
/// shift the interner table — a cited decision appearing early, or a burned
/// `h.weir#N` index — and every atom object minted after it would change bytes.
#[test]
fn functional_decl_interns_no_provenance_and_keeps_the_bundle_counter() {
    let (with_objects, with_names) = lower_pack(&gauntlet(true));
    let (without_objects, without_names) = lower_pack(&gauntlet(false));

    assert_eq!(
        with_names, without_names,
        "a carriage-only decl must not touch the interner"
    );
    let bundles: Vec<&String> = with_names.iter().filter(|n| n.starts_with("h.")).collect();
    assert_eq!(
        bundles,
        vec!["h.weir#0", "h.weir#1"],
        "one bundle variable per bare proposal — the functional decls mint none"
    );
    assert!(
        !with_names.iter().any(|n| n == "D-2"),
        "the decision only the functional decl cites must never be interned: {with_names:?}"
    );

    let with_atoms: Vec<&FactObject> = with_objects
        .iter()
        .filter(|o| o.expert_body.is_none())
        .collect();
    let without_atoms: Vec<&FactObject> = without_objects
        .iter()
        .filter(|o| o.expert_body.is_none())
        .collect();
    let with_digests: Vec<String> = with_atoms
        .iter()
        .map(|o| o.content_hash().expect("hash").to_string())
        .collect();
    let without_digests: Vec<String> = without_atoms
        .iter()
        .map(|o| o.content_hash().expect("hash").to_string())
        .collect();
    assert_eq!(
        with_digests, without_digests,
        "atom objects must be byte-identical with and without the functional decls"
    );
}

/// The walk must track the loader exactly — the check `import_pack` gates on.
#[test]
fn functional_pack_walk_interner_matches_the_compiled_model() {
    let pack = gauntlet(true);
    let compiled = compile(&pack).expect("compiles");
    let (_objects, walk_names) = lower_pack(&pack);
    assert_eq!(
        walk_names,
        compiled.model().interner().names().to_vec(),
        "walk and loader must agree name-for-name"
    );
}

/// The byte-identity oracle of `lower.rs`: every digest `lower_atoms` mints from
/// the compiled model must be reproduced by the per-declaration walk, with the
/// functional decls mixed in.
#[test]
fn lower_pack_atoms_stay_byte_identical_to_lower_atoms() {
    let pack = gauntlet(true);
    let compiled = compile(&pack).expect("compiles");
    let atom_digests = digests(&lower_atoms(compiled.model().atoms()));
    let (decl_objects, _names) = lower_pack(&pack);
    let decl_digests = digests(&decl_objects);
    let missing: Vec<&String> = atom_digests.difference(&decl_digests).collect();
    assert!(
        missing.is_empty(),
        "every lower_atoms digest must be reproduced by lower_pack; missing: {missing:?}"
    );
}

/// Reverse lowering recovers the declaration itself — predicate and provenance —
/// because the carriage body is the whole `Decl`.
#[test]
fn functional_decl_round_trips_through_reconstruct_pack() {
    let pack = gauntlet(true);
    let compiled = compile(&pack).expect("compiles");
    let (objects, _names) = lower_pack(&pack);
    let decisions = declared_decisions(&pack);
    let back = reconstruct_pack(
        &objects,
        compiled.model().interner(),
        &decisions,
        &pack.name,
    )
    .expect("the lowered pack reconstructs");

    let recovered: Vec<&FunctionalDecl> = back
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter_map(|decl| match decl {
            Decl::Functional(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(
        recovered.len(),
        2,
        "both functional decls survive the store"
    );
    assert_eq!(recovered[0].pred.text, "rank");
    assert_eq!(
        recovered[0].provenance,
        Some(ratified("D-2")),
        "the cited provenance rides the carriage body, not the polynomial"
    );
    assert_eq!(recovered[1].pred.text, "mood");
    assert_eq!(recovered[1].provenance, Some(proposed()));

    // And the surrounding declarations are unharmed: the reconstruction walk
    // skipped no atom it should have kept.
    let facts: BTreeSet<String> = back
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter_map(|decl| match decl {
            Decl::Fact(f) => Some(f.atom.pred.text.clone()),
            _ => None,
        })
        .collect();
    for pred in ["rank", "linked", "omen", "secret"] {
        assert!(
            facts.contains(pred),
            "reconstruction lost {pred}: {facts:?}"
        );
    }
}

/// The full store round trip: import (which refuses a diverged lowering), then
/// recompile out of the store and find the declared predicate still declared.
#[test]
fn functional_pack_round_trips_through_the_lorestore() {
    let pack = gauntlet(true);
    let compiled_a = compile(&pack).expect("A compiles");
    let model_a = compiled_a.model();

    let tmp = TempDir::new().expect("tempdir");
    let store = Lorestore::open(tmp.path().to_path_buf()).expect("open");
    // A desynced interner or bundle counter surfaces here as LoweringDiverged.
    store.import_pack(&pack).expect("import must not diverge");
    let compiled_b = store.compile().expect("B compiles");
    let model_b = compiled_b.model();

    assert_eq!(
        model_a.interner().names().to_vec(),
        model_b.interner().names().to_vec(),
        "interner names must match after round-trip"
    );
    assert_eq!(
        model_a.functional_predicates, model_b.functional_predicates,
        "the declared predicates must survive the store"
    );
    assert_eq!(
        model_a.functional_predicates,
        ["mood".to_owned(), "rank".to_owned()].into_iter().collect(),
        "both declarations reach the checker's set"
    );

    let mut a: Vec<String> = model_a
        .atoms()
        .iter()
        .map(|at| at.atom.pred.text.clone())
        .collect();
    let mut b: Vec<String> = model_b
        .atoms()
        .iter()
        .map(|at| at.atom.pred.text.clone())
        .collect();
    a.sort();
    b.sort();
    assert_eq!(a, b, "atom surfaces must match after round-trip");
}

/// The two-lowering convergence oracle, with a `functional` decl in the
/// document: the loader and the evaluator must still agree fact-for-fact. A
/// `functional` decl is trivially convergent — neither side emits an atom — but
/// it must not perturb the proposal indices either side mints, which is what
/// would move a standing.
#[test]
fn functional_decl_keeps_the_two_lowerings_at_zero_divergence() {
    let pack = gauntlet(true);
    let doc = &pack.documents[0];
    let compiled = compile(&pack).expect("compiles");
    let evaluated = World::lower(doc)
        .expect("lowers")
        .fixpoint()
        .expect("saturates");

    for (pred, args, expected) in [
        ("rank", ["joren", "high"], Status::Certain),
        ("linked", ["joren", "enclosure"], Status::Possible),
        ("omen", ["joren", "storm"], Status::Possible),
        ("secret", ["joren", "hidden"], Status::Dead),
    ] {
        let rendered: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let loaded = compiled
            .model()
            .atoms()
            .iter()
            .find(|a| {
                a.atom.pred.text == pred
                    && a.atom
                        .args
                        .iter()
                        .filter_map(|t| match t {
                            Term::Sym(i) | Term::Var(i) => Some(i.text.clone()),
                            Term::Str(s) => Some(s.clone()),
                            Term::Int(n) => Some(n.to_string()),
                            Term::Time(_) => None,
                        })
                        .eq(rendered.iter().cloned())
            })
            .expect("loaded atom present");
        let loaded_status = loaded.ann.status(
            &compiled.model().world.valuation,
            &compiled.model().world.theory,
        );
        let answer = evaluated
            .query(&Query {
                goal: AtomExpr {
                    pred: id(pred),
                    args: args.iter().map(|a| Term::Sym(id(a))).collect(),
                },
            })
            .expect("known pred");
        let eval_status = answer.rows.first().map_or(Status::Dead, |row| row.status);
        assert_eq!(loaded_status, eval_status, "{pred} divergence");
        assert_eq!(loaded_status, expected, "{pred} standing");
    }
}
