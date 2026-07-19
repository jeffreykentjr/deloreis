#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the evaluator: a hand-built saltmarket world, lowered,
//! saturated, and queried. No parser is involved — the AST is built directly.

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::decl::{
    BeforeDecl, Decl, Document, EntityDecl, EntityKind, FactDecl, OptionClause, QuestionDecl,
    TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Status;
use dli_core::atom::Atom;
use dli_core::derivation::ClaimId;

use dli_eval::SupportIndex;
use dli_eval::world::World;

fn sym(name: &str) -> Term {
    Term::Sym(Ident::synthetic(name))
}

fn var(name: &str) -> Term {
    Term::Var(Ident::synthetic(name))
}

fn atom(pred: &str, args: Vec<Term>) -> AtomExpr {
    AtomExpr {
        pred: Ident::synthetic(pred),
        args,
    }
}

fn fact(pred: &str, args: Vec<Term>, provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance,
    })
}

/// Provenance carrying a marker and its (possibly empty) citations.
fn provenance(marker: ProvenanceMarker, citations: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: citations
            .iter()
            .map(|c| CitationRef {
                id: Ident::synthetic(*c),
            })
            .collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

/// A `decided <decision>` fact — certain by default.
fn decided_fact(pred: &str, args: Vec<Term>, decision: &str) -> Decl {
    fact(
        pred,
        args,
        Some(provenance(ProvenanceMarker::Decided, &[decision])),
    )
}

/// An option `name implies <pred>(<args>)`.
fn option(name: &str, pred: &str, args: Vec<Term>) -> OptionClause {
    OptionClause {
        name: Ident::synthetic(name),
        implies: Some(atom(pred, args)),
    }
}

/// The saltmarket document: two adjacency facts and one open question whose
/// options each imply who keeps the ledger.
fn saltmarket_document() -> Document {
    Document {
        pack: None,
        decls: vec![
            decided_fact(
                "adjacent",
                vec![sym("saltmarket"), sym("the-shallows")],
                "d-saltmarket-map",
            ),
            decided_fact(
                "adjacent",
                vec![sym("the-shallows"), sym("the-weir")],
                "d-saltmarket-map",
            ),
            Decl::Question(QuestionDecl {
                name: Ident::synthetic("who-keeps-the-ledger"),
                prompt: None,
                options: vec![
                    option(
                        "old-fen",
                        "keeps",
                        vec![sym("old-fen"), sym("the-salt-ledger")],
                    ),
                    option("mara", "keeps", vec![sym("mara"), sym("the-salt-ledger")]),
                ],
                provenance: Some(Annotation {
                    marker: ProvenanceMarker::Proposed,
                    citations: Vec::new(),
                    condition: None,
                    tags: Vec::new(),
                    span: Span::synthetic(),
                }),
            }),
        ],
    }
}

/// Lower the document, add the transitive-closure rules, and saturate.
fn saltmarket_world() -> World {
    let mut world = World::lower(&saltmarket_document()).unwrap();
    // reachable(X, Z) :- adjacent(X, Z)
    world
        .define_rule(
            "reach-base",
            atom("reachable", vec![var("X"), var("Z")]),
            vec![atom("adjacent", vec![var("X"), var("Z")])],
        )
        .unwrap();
    // reachable(X, Z) :- adjacent(X, Y), reachable(Y, Z)
    world
        .define_rule(
            "reach-step",
            atom("reachable", vec![var("X"), var("Z")]),
            vec![
                atom("adjacent", vec![var("X"), var("Y")]),
                atom("reachable", vec![var("Y"), var("Z")]),
            ],
        )
        .unwrap();
    world.fixpoint().unwrap()
}

fn query(goal: AtomExpr) -> dli_eval::Query {
    dli_eval::Query { goal }
}

/// The status of a unary `pred(arg)` in a world, or `None` when it is not
/// certain-or-possible (dead atoms are excluded from answers).
fn status_of(world: &World, pred: &str, arg: &str) -> Option<Status> {
    let answer = world.query(&query(atom(pred, vec![sym(arg)]))).unwrap();
    answer.rows.first().map(|row| row.status)
}

#[test]
fn empty_world_constructs() {
    let world = World::new();
    assert!(world.facts.is_empty());
    assert!(world.rules.is_empty());
    assert!(world.wheels.is_empty());
    assert_eq!(world, World::default());
}

#[test]
fn t2_transitive_closure_reaches_the_weir() {
    let world = saltmarket_world();
    // reachable(saltmarket, ?Z) closes over adjacency: the-shallows and the-weir.
    let answer = world
        .query(&query(atom("reachable", vec![sym("saltmarket"), var("Z")])))
        .unwrap();
    let mut reached: Vec<String> = answer
        .rows
        .iter()
        .map(|row| row.bindings["Z"].clone())
        .collect();
    reached.sort();
    assert_eq!(reached, vec!["the-shallows", "the-weir"]);
    // Every hop rests on the ruled map decision, so the closure is certain.
    assert!(answer.rows.iter().all(|row| row.status == Status::Certain));
}

#[test]
fn t2_support_rests_on_the_map_decision() {
    let world = saltmarket_world();
    let answer = world
        .query(&query(atom(
            "reachable",
            vec![sym("saltmarket"), sym("the-weir")],
        )))
        .unwrap();
    assert_eq!(answer.rows.len(), 1);
    // The two-hop path rests on the ratified map decision, and that decision is
    // ruled true in the default valuation, so the closure is certain.
    let support = &answer.rows[0].verdict.support;
    assert!(!support.is_false());
    assert!(support.is_certain_under(&world.valuation));
    assert_eq!(answer.rows[0].status, Status::Certain);
}

#[test]
fn t1_open_option_is_a_shadow_then_ruled_certain() {
    let world = saltmarket_world();
    let goal = atom("keeps", vec![var("who"), sym("the-salt-ledger")]);
    // Both keepers are merely possible while the question is open.
    let open = world.query(&query(goal.clone())).unwrap();
    assert_eq!(open.rows.len(), 2);
    assert!(open.rows.iter().all(|row| row.status == Status::Possible));

    // Suppose the question rules for old-fen: old-fen becomes certain, and the
    // excluded option (mara) goes dead — dropped from the answer.
    let ruled = world
        .fork_eval(&query(goal), &[Ident::synthetic("old-fen")])
        .unwrap();
    assert_eq!(ruled.rows.len(), 1);
    assert_eq!(ruled.rows[0].bindings["who"], "old-fen");
    assert_eq!(ruled.rows[0].status, Status::Certain);
}

#[test]
fn unknown_goal_predicate_is_an_error() {
    let world = saltmarket_world();
    let result = world.query(&query(atom("floats", vec![sym("saltmarket")])));
    assert!(result.is_err());
}

#[test]
fn support_index_inverts_the_option_variable() {
    let world = saltmarket_world();
    // The old-fen option variable indexes the keeps(old-fen, …) atom.
    let old_fen = world.interner.names().iter().position(|n| n == "old-fen");
    assert!(old_fen.is_some());
    let atom_id = dli_core::atom::Atom(u32::try_from(old_fen.unwrap()).unwrap());
    let dependents = world.support_index().get(&atom_id);
    assert!(dependents.is_some_and(|set| set.iter().any(|a| a.pred == "keeps")));
}

#[test]
fn support_index_answers_why_and_defeater() {
    let world = saltmarket_world();
    let index = SupportIndex::build(&world);
    let keeps = atom("keeps", vec![sym("old-fen"), sym("the-salt-ledger")]);

    // keeps(old-fen, …) rests on exactly one witness: the old-fen option claim.
    let old_fen = world
        .interner
        .names()
        .iter()
        .position(|n| n == "old-fen")
        .unwrap();
    let witnesses = index.why(&keeps);
    assert_eq!(
        witnesses,
        vec![vec![ClaimId(u32::try_from(old_fen).unwrap())]]
    );
    // No defeasible default, so no defeater.
    assert_eq!(index.defeater(&keeps), None);
    // The inverted read finds the same atom from the variable side.
    let dependents = index.dependents(Atom(u32::try_from(old_fen).unwrap()));
    assert!(dependents.is_some_and(|set| set.iter().any(|a| a.pred == "keeps")));

    // The two-hop reachable atom rests on one witness: the map decision claim
    // (both hops cite it, and absorption collapses the conjunction to one).
    let map = world
        .interner
        .names()
        .iter()
        .position(|n| n == "d-saltmarket-map")
        .unwrap();
    let reachable = atom("reachable", vec![sym("saltmarket"), sym("the-weir")]);
    assert_eq!(
        index.why(&reachable),
        vec![vec![ClaimId(u32::try_from(map).unwrap())]]
    );
}

#[test]
fn c1_provenance_gates_certainty() {
    let document = Document {
        pack: None,
        decls: vec![
            fact(
                "p",
                vec![sym("x")],
                Some(provenance(ProvenanceMarker::Proposed, &[])),
            ),
            fact(
                "d",
                vec![sym("x")],
                Some(provenance(ProvenanceMarker::Decided, &["d-0001"])),
            ),
            fact(
                "r",
                vec![sym("x")],
                Some(provenance(ProvenanceMarker::Ratified, &["d-0002"])),
            ),
            fact("n", vec![sym("x")], None),
        ],
    };
    let world = World::lower(&document).unwrap().fixpoint().unwrap();
    // proposed → possible: its fresh proposal variable is left open.
    assert_eq!(status_of(&world, "p", "x"), Some(Status::Possible));
    // decided/ratified → certain: their decision variables are ruled true in
    // the default valuation.
    assert_eq!(status_of(&world, "d", "x"), Some(Status::Certain));
    assert_eq!(status_of(&world, "r", "x"), Some(Status::Certain));
    // no provenance → dead (fail-closed): never appears in an answer.
    assert_eq!(status_of(&world, "n", "x"), None);
}

#[test]
fn c1_supersession_revalues_a_decided_fact() {
    let document = Document {
        pack: None,
        decls: vec![fact(
            "d",
            vec![sym("x")],
            Some(provenance(ProvenanceMarker::Decided, &["d-0001"])),
        )],
    };
    let world = World::lower(&document).unwrap().fixpoint().unwrap();
    assert_eq!(status_of(&world, "d", "x"), Some(Status::Certain));

    // Supersede: revalue the decision variable false. The fact loses its only
    // support and is no longer certain — here, dead — with no re-lowering.
    let decision = world
        .interner
        .names()
        .iter()
        .position(|n| n == "d-0001")
        .unwrap();
    let mut nu = world.valuation.clone();
    nu.set(Atom(u32::try_from(decision).unwrap()), false);
    let answer = world
        .eval_under(&nu, &query(atom("d", vec![sym("x")])))
        .unwrap();
    assert!(answer.rows.is_empty());
}

#[test]
fn a2_proposed_with_assumes_is_ratifiable() {
    // `proposed p(x) assumes h-x` — the condition IS the bundle gate; there is
    // no extra never-ruled proposal variable, so ruling h-x ratifies the claim.
    let document = Document {
        pack: None,
        decls: vec![Decl::Fact(FactDecl {
            atom: atom("p", vec![sym("x")]),
            triple: false,
            provenance: Some(Annotation {
                marker: ProvenanceMarker::Proposed,
                citations: Vec::new(),
                condition: Some(CondExpr::Hyp(Ident::synthetic("h-x"))),
                tags: Vec::new(),
                span: Span::synthetic(),
            }),
        })],
    };
    let world = World::lower(&document).unwrap().fixpoint().unwrap();
    // h-x open → a shadow, like any proposed claim.
    assert_eq!(status_of(&world, "p", "x"), Some(Status::Possible));

    let hyp = world
        .interner
        .names()
        .iter()
        .position(|n| n == "h-x")
        .unwrap();
    let hyp = Atom(u32::try_from(hyp).unwrap());
    let goal = query(atom("p", vec![sym("x")]));

    // Ruling the assumed hypothesis true ratifies the claim: Certain, not stuck.
    let mut ruled = world.valuation.clone();
    ruled.set(hyp, true);
    assert_eq!(
        world
            .eval_under(&ruled, &goal)
            .unwrap()
            .rows
            .first()
            .map(|row| row.status),
        Some(Status::Certain)
    );

    // Ruling it false kills the claim: Dead.
    let mut denied = world.valuation.clone();
    denied.set(hyp, false);
    assert!(world.eval_under(&denied, &goal).unwrap().rows.is_empty());
}

#[test]
fn lower_populates_the_chronology() {
    let document = Document {
        pack: None,
        decls: vec![
            Decl::Before(BeforeDecl {
                earlier: Ident::synthetic("the-long-drought"),
                later: Ident::synthetic("tideturn"),
                provenance: None,
            }),
            Decl::Entity(EntityDecl {
                kind: EntityKind::Era,
                name: Ident::synthetic("the-ages"),
                label: None,
                time: Some(TimeClause::Order(vec![
                    Ident::synthetic("first-age"),
                    Ident::synthetic("second-age"),
                    Ident::synthetic("third-age"),
                ])),
                slots: Vec::new(),
                provenance: None,
            }),
        ],
    };
    let world = World::lower(&document).unwrap();
    // One before-edge plus two consecutive order-chain edges.
    assert_eq!(world.chronology.edges.len(), 3);
    // Every era/event is interned as a resolvable time point.
    for name in [
        "the-long-drought",
        "tideturn",
        "first-age",
        "second-age",
        "third-age",
    ] {
        assert!(world.time_points.names().iter().any(|n| n == name));
    }
}
