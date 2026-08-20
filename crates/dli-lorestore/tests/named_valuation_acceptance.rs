//! GH #103 acceptance over real persisted declarations and saturated rules.
//!
//! Numeric ref atoms are deliberately skewed from the independently compiled
//! receiver. Every verdict below comes from actual `ratify`, fact, and rule
//! declarations after lorestore lowering/reconstruction, never from a
//! hand-built annotation.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, FactDecl, RatifyDecl, RuleDecl};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Status;
use dli_core::atom::{Atom, Interner};
use dli_core::valuation::NamedValuation;
use dli_lorestore::{Lorestore, Ref};
use dli_world::CompiledWorld;

const DEAD: &str = "D-dead";
const LIVE: &str = "D-live";
const INDEPENDENT: &str = "D-independent";
const ORDERS: [[&str; 3]; 6] = [
    [DEAD, LIVE, INDEPENDENT],
    [DEAD, INDEPENDENT, LIVE],
    [LIVE, DEAD, INDEPENDENT],
    [LIVE, INDEPENDENT, DEAD],
    [INDEPENDENT, DEAD, LIVE],
    [INDEPENDENT, LIVE, DEAD],
];

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn annotation(marker: ProvenanceMarker, decision: Option<&str>) -> Annotation {
    Annotation {
        marker,
        citations: decision
            .into_iter()
            .map(|decision| CitationRef { id: id(decision) })
            .collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn fact(pred: &str, decision: Option<&str>) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(pred),
            args: vec![Term::Sym(id("subject"))],
        },
        triple: false,
        provenance: Some(annotation(
            if decision.is_some() {
                ProvenanceMarker::Ratified
            } else {
                ProvenanceMarker::Proposed
            },
            decision,
        )),
    })
}

fn rule(name: &str, head: &str, body: &str) -> Decl {
    let name = id(name);
    let clauses = vec![Clause {
        text: format!("{head}($x) :- {body}($x)"),
        span: Span::synthetic(),
    }];
    Decl::Rule(RuleDecl {
        body: dli_syntax::structure_rule_body(&clauses, &name, &[]),
        name,
        domain: None,
        head: Vec::new(),
        provenance: Some(annotation(ProvenanceMarker::Ratified, Some(LIVE))),
    })
}

fn declarations(order: [&str; 3], depth: usize, shared: usize) -> Vec<Decl> {
    let mut declarations: Vec<Decl> = order
        .into_iter()
        .map(|decision| {
            Decl::Ratify(RatifyDecl {
                decision: id(decision),
                body: dli_ast::language::RatifyBody::default(),
            })
        })
        .collect();
    declarations.extend([
        fact("seed", Some(DEAD)),
        fact("redundant", Some(DEAD)),
        fact("redundant", Some(LIVE)),
        fact("independent", Some(INDEPENDENT)),
        fact("open", None),
    ]);
    for level in 0..depth {
        let body = if level == 0 {
            "seed".to_owned()
        } else {
            format!("stage-{}", level - 1)
        };
        declarations.push(rule(
            &format!("chain-{level}"),
            &format!("stage-{level}"),
            &body,
        ));
    }
    let terminal = format!("stage-{}", depth - 1);
    for consequence in 0..shared {
        declarations.push(rule(
            &format!("share-{consequence}"),
            &format!("conclusion-{consequence}"),
            &terminal,
        ));
    }
    declarations
}

fn atom_named(names: &[String], name: &str) -> Atom {
    names
        .iter()
        .position(|candidate| candidate == name)
        .and_then(|position| u32::try_from(position).ok())
        .map(Atom)
        .expect("fixture name is interned")
}

fn standing_snapshot(world: &CompiledWorld) -> BTreeMap<String, Status> {
    world
        .model
        .world
        .derived
        .iter()
        .filter(|row| {
            let pred = row.atom.pred.as_str();
            matches!(pred, "seed" | "redundant" | "independent" | "open")
                || pred.starts_with("stage-")
                || pred.starts_with("conclusion-")
        })
        .map(|row| {
            (
                row.atom.render(),
                row.ann()
                    .status(&world.model.world.valuation, &world.model.world.theory),
            )
        })
        .collect()
}

fn assert_exact_standings(standings: &BTreeMap<String, Status>, depth: usize, shared: usize) {
    assert_eq!(standings["seed(subject)"], Status::Dead);
    assert_eq!(standings["redundant(subject)"], Status::Certain);
    assert_eq!(standings["independent(subject)"], Status::Certain);
    assert_eq!(standings["open(subject)"], Status::Possible);
    for level in 0..depth {
        assert_eq!(standings[&format!("stage-{level}(subject)")], Status::Dead);
    }
    for consequence in 0..shared {
        assert_eq!(
            standings[&format!("conclusion-{consequence}(subject)")],
            Status::Dead
        );
    }
    assert_eq!(standings.len(), depth + shared + 4);
}

fn persist_declarations(
    store: &Lorestore,
    order: [&str; 3],
    depth: usize,
    shared: usize,
) -> dli_lorestore::ContentHash {
    let mut interner = Interner::from_names(
        std::iter::once("padding".to_owned())
            .chain(order.map(str::to_owned))
            .collect(),
    );
    let mut bundle = 0;
    let mut hashes = Vec::new();
    for declaration in declarations(order, depth, shared) {
        for object in
            dli_lorestore::lower::lower_decl(&declaration, "matrix", &mut interner, &mut bundle)
        {
            hashes.push(store.write_object(&object).expect("write fact object"));
        }
    }
    store
        .commit_on_branch(
            "main",
            interner.names().to_vec(),
            hashes,
            Vec::new(),
            Vec::new(),
            order.map(str::to_owned).to_vec(),
            "matrix".to_owned(),
        )
        .expect("commit real declarations")
}

fn run_fixture(order: [&str; 3], depth: usize, shared: usize) -> Vec<u8> {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = Lorestore::open(directory.path().to_path_buf()).expect("open store");
    let head = persist_declarations(&store, order, depth, shared);
    let compiled = store
        .compile_at(&head)
        .expect("compile persisted declarations");
    let before = standing_snapshot(&compiled);
    assert!(
        before
            .iter()
            .all(|(atom, standing)| atom == "open(subject)" || *standing == Status::Certain),
        "all ruled declaration paths start certain: {before:?}"
    );

    let durable_names = store.interner_at(&head).expect("durable interner");
    let compiled_names = compiled.model.interner().names().to_vec();
    assert_ne!(
        atom_named(&durable_names, DEAD),
        atom_named(&compiled_names, DEAD),
        "fixture must carry a real durable/compiled skew"
    );
    let current = store.head_ref().expect("head ref").expect("main exists");
    let token = current.digest().expect("ref digest").to_string();
    let mut valuation = current.valuation;
    valuation.set(atom_named(&durable_names, DEAD), false);
    let updated = Ref::with_valuation("main", current.commit, valuation);
    assert!(
        store
            .cas_main_ref(Some(&token), &updated)
            .expect("CAS defeat")
    );

    let reopened = Lorestore::open(directory.path().to_path_buf()).expect("reopen store");
    let stored_ref = reopened.head_ref().expect("head ref").expect("main exists");
    let named = NamedValuation::from_indexed(&stored_ref.valuation, &durable_names)
        .expect("durable overlay resolves");
    let mut received = compiled.clone();
    let target = received.model.interner().clone();
    named
        .apply_to(&target, &mut received.model.world.valuation)
        .expect("receiving compile contains every persisted name");

    let mut reference = compiled;
    reference
        .model
        .world
        .valuation
        .set(atom_named(&compiled_names, DEAD), false);
    let received_standings = standing_snapshot(&received);
    assert_eq!(received_standings, standing_snapshot(&reference));
    assert_exact_standings(&received_standings, depth, shared);
    serde_json::to_vec(&received_standings).expect("serialize named standing snapshot")
}

#[test]
fn six_real_declaration_ingestion_orders_are_byte_equivalent_by_name() {
    let expected = run_fixture(ORDERS[0], 4, 5);
    for order in ORDERS.into_iter().skip(1) {
        assert_eq!(run_fixture(order, 4, 5), expected, "order={order:?}");
    }
}

#[test]
fn real_rule_depth_and_shared_consequence_grid_survives_post_compile_defeat() {
    for depth in [1, 2, 4, 8] {
        for shared in [1, 2, 5] {
            run_fixture(ORDERS[0], depth, shared);
        }
    }
}
