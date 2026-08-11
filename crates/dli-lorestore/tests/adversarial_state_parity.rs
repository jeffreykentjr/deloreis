#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Adversarial: does a state-bearing pack keep loader/lorestore interner and
//! bundle-counter in lock-step (byte-identity oracle) across tricky state forms?

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    Decl, Document, EntityDecl, EntityKind, FactDecl, RatifyDecl, ScopeHeader, StateDecl,
    TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_lorestore::Lorestore;
use dli_lorestore::lower::{lower_atoms, lower_pack};
use dli_world::compile;
use dli_world::pack::Pack;

use tempfile::TempDir;

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

fn state(
    name: &str,
    value: Option<Term>,
    time: Option<TimeClause>,
    prov: Option<Annotation>,
) -> Decl {
    Decl::State(StateDecl {
        name: id(name),
        value,
        time,
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

/// A pack mixing cited states (dotted valued, dotted valueless, undotted valued,
/// Str value, multi-dot) with an inert state (undotted valueless) that has
/// provenance, plus entities and a fact — the full lock-step gauntlet.
fn gauntlet() -> Pack {
    pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            entity(EntityKind::Place, "gwern", "D-1"),
            entity(EntityKind::Event, "enclosure", "D-1"),
            // dotted valued, with time
            state(
                "gwern.water-status",
                Some(Term::Sym(id("commons"))),
                Some(TimeClause::Since(id("enclosure"))),
                Some(ratified("D-1")),
            ),
            // dotted valueless -> unary holding p(s)
            state("gwern.alive", None, None, Some(ratified("D-1"))),
            // undotted valued -> n(v)
            state(
                "foo",
                Some(Term::Sym(id("bar"))),
                None,
                Some(ratified("D-1")),
            ),
            // Str value
            state(
                "gwern.title",
                Some(Term::Str("The Weir".to_owned())),
                None,
                Some(ratified("D-1")),
            ),
            // Int value
            state(
                "gwern.depth",
                Some(Term::Int(7)),
                None,
                Some(ratified("D-1")),
            ),
            // multi-dot name a.b.c
            state(
                "a.b.c",
                Some(Term::Sym(id("v"))),
                None,
                Some(ratified("D-1")),
            ),
            // INERT: undotted valueless WITH provenance (the lock-step trap)
            state("waterlogged", None, None, Some(ratified("D-1"))),
            fact("linked", &["gwern", "enclosure"], Some(ratified("D-1"))),
        ],
    )
}

#[test]
fn state_pack_round_trips_through_lorestore_identically() {
    let pack = gauntlet();
    let compiled_a = compile(&pack).expect("A compiles");
    let model_a = compiled_a.model();

    let tmp = TempDir::new().expect("tempdir");
    let store = Lorestore::open(tmp.path().to_path_buf()).expect("open");
    // If interner/bundle desync, import_pack refuses with LoweringDiverged.
    store.import_pack(&pack).expect("import must not diverge");
    let compiled_b = store.compile().expect("B compiles");
    let model_b = compiled_b.model();

    assert_eq!(
        model_a.interner().names().to_vec(),
        model_b.interner().names().to_vec(),
        "interner names must match after round-trip"
    );
    assert_eq!(
        model_a.atoms().len(),
        model_b.atoms().len(),
        "atom count must match after round-trip"
    );
    // Compare rendered atom surfaces.
    let mut a: Vec<String> = model_a
        .atoms()
        .iter()
        .map(|at| at.atom.pred.text.clone() + "/" + &at.atom.args.len().to_string())
        .collect();
    let mut b: Vec<String> = model_b
        .atoms()
        .iter()
        .map(|at| at.atom.pred.text.clone() + "/" + &at.atom.args.len().to_string())
        .collect();
    a.sort();
    b.sort();
    assert_eq!(a, b, "atom surfaces must match after round-trip");
}

#[test]
fn lower_pack_atoms_are_byte_identical_to_lower_atoms() {
    // The oracle in the lower.rs doc comment: lower_decl atom objects must be
    // byte-identical to lower_atoms(compiled.atoms()) given the same interner.
    let pack = gauntlet();
    let compiled = compile(&pack).expect("compiles");
    let atom_objs = lower_atoms(compiled.model().atoms());
    let (decl_objs, _names) = lower_pack(&pack);

    let atom_digests: std::collections::BTreeSet<String> = atom_objs
        .iter()
        .map(|o| o.content_hash().expect("hash").to_string())
        .collect();
    // lower_pack includes carriage objects too; every atom-object digest from
    // lower_atoms must appear among lower_pack's digests.
    let decl_digests: std::collections::BTreeSet<String> = decl_objs
        .iter()
        .map(|o| o.content_hash().expect("hash").to_string())
        .collect();
    let missing: Vec<&String> = atom_digests.difference(&decl_digests).collect();
    assert!(
        missing.is_empty(),
        "every lower_atoms digest must be reproduced by lower_pack; missing: {missing:?}"
    );
}
