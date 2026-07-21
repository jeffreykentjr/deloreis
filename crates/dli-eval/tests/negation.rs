#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Rule-body negation semantics (`docs/spec/02-semantics.md §4`): a negated body
//! literal `not b` tests against the **certain layer only** and **poisons** the
//! conclusion's defeater polynomial. A merely-possible `b` never blocks the
//! derivation; it records the risk. A certain `b` blocks it. A variable that
//! appears only under `not` is unsafe (range restriction). A program with no
//! negated literals is byte-identical to plain positive datalog.
//!
//! Atoms are generic (`base`/`b`/`risky`, p/q) — never world tokens.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, RatifyDecl};
use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::{Ann, Status};

use dli_eval::EvalError;
use dli_eval::world::{Derived, World};

// --- AST helpers ------------------------------------------------------------

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

fn fact(pred: &str, args: Vec<Term>, provenance: Annotation) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance: Some(provenance),
    })
}

fn ratify(decision: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: Ident::synthetic(decision),
        body: Vec::new(),
    })
}

/// Find the single derived row for a nullary/unary predicate `pred`.
fn derived_row<'a>(world: &'a World, pred: &str) -> Option<&'a Derived> {
    world.derived.iter().find(|d| d.atom.pred == pred)
}

// --- (a) a possible b poisons: certain-at-risk ------------------------------

#[test]
fn possible_negated_literal_poisons_the_conclusion() {
    // base(x) is certain; b(x) is merely proposed (possible). The rule
    //     risky($x) :- base($x), not b($x)
    // derives risky(x) — because a possible b never blocks — but folds b's
    // support monomial into risky's defeaters, so risky is CERTAIN-AT-RISK.
    let document = Document {
        scope: None,
        decls: vec![
            ratify("d-base"),
            fact(
                "base",
                vec![sym("x")],
                provenance(ProvenanceMarker::Ratified, &["d-base"]),
            ),
            fact(
                "b",
                vec![sym("x")],
                provenance(ProvenanceMarker::Proposed, &[]),
            ),
        ],
    };
    let mut world = World::lower(&document).unwrap();
    world
        .define_rule(
            "risky-unless-b",
            atom("risky", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("base", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    let risky = derived_row(&world, "risky").expect("risky(x) is derived");
    let ann = Ann::new(risky.support.clone(), risky.defeaters.clone());
    // Derived and assertable: base(x) is certain.
    assert_eq!(ann.status(&world.valuation, &world.theory), Status::Certain);
    // But at risk: b(x)'s open monomial rode into the defeaters (the poison).
    assert!(
        !risky.defeaters.is_false(),
        "P⁻ must carry b's monomial: {:?}",
        risky.defeaters
    );
    assert!(
        ann.at_risk(&world.valuation, &world.theory),
        "risky must be certain-at-risk"
    );
    // The defeater literal is b(x)'s proposal event variable.
    let names: Vec<String> = risky
        .defeaters
        .monomials()
        .flat_map(dli_core::posbool::Monomial::lits)
        .filter_map(|lit| world.interner.resolve(lit.atom).map(ToOwned::to_owned))
        .collect();
    assert!(
        names.iter().any(|n| n.contains("proposal")),
        "the defeater is b's proposal variable, got {names:?}"
    );
}

// --- (b) a certain b defeats the derivation (a reversible block) -------------

#[test]
fn certain_negated_literal_defeats_the_conclusion_reversibly() {
    // base(x) certain, b(x) certain. `not b($x)` tests against the certain layer:
    // b(x) holds there, so risky(x) is NOT assertable. But the block is a
    // *defeater condition* on risky's support monomial, not the absence of a row
    // (`docs/spec/02-semantics.md §4/§5`): risky is derived, present, and merely
    // Possible — and revaluing b false un-blocks it with no re-fixpoint.
    let document = Document {
        scope: None,
        decls: vec![
            ratify("d-base"),
            ratify("d-b"),
            fact(
                "base",
                vec![sym("x")],
                provenance(ProvenanceMarker::Ratified, &["d-base"]),
            ),
            fact(
                "b",
                vec![sym("x")],
                provenance(ProvenanceMarker::Ratified, &["d-b"]),
            ),
        ],
    };
    let mut world = World::lower(&document).unwrap();
    world
        .define_rule(
            "risky-unless-b",
            atom("risky", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("base", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    let risky = derived_row(&world, "risky").expect("a certain b defeats but does not drop risky");
    assert_eq!(
        risky.ann().status(&world.valuation, &world.theory),
        Status::Possible,
        "a certain b defeats risky to merely-possible"
    );
    let goal = dli_eval::Query {
        goal: atom("risky", vec![sym("x")]),
    };
    let rows = world.query(&goal).unwrap().rows;
    assert_eq!(rows.len(), 1, "risky(x) is present (defeated, not absent)");
    assert_eq!(rows[0].status, Status::Possible);

    // Supersede b (revalue its decision variable false) WITHOUT a re-fixpoint:
    // the block lifts and risky becomes Certain — the valuation moves it both
    // ways across the fork/eval seam.
    let b_var = world
        .interner
        .names()
        .iter()
        .position(|n| n == "d-b")
        .expect("d-b is an interned decision variable");
    let mut superseded = world.clone();
    superseded
        .valuation
        .set(dli_core::atom::Atom(u32::try_from(b_var).unwrap()), false);
    assert_eq!(
        superseded.query(&goal).unwrap().rows[0].status,
        Status::Certain,
        "revaluing b false un-blocks risky with no re-fixpoint"
    );
}

// --- (b') supposing b makes a previously-open negation defeat the conclusion --

#[test]
fn ruling_the_poison_true_defeats_the_certain_conclusion() {
    // The same possible-b world as (a): risky is certain-at-risk. Ruling b's
    // proposal variable TRUE (a would-be ratification) flips risky from certain
    // to merely-possible — the defeater fires, no re-fixpoint needed.
    let document = Document {
        scope: None,
        decls: vec![
            ratify("d-base"),
            fact(
                "base",
                vec![sym("x")],
                provenance(ProvenanceMarker::Ratified, &["d-base"]),
            ),
            fact(
                "b",
                vec![sym("x")],
                provenance(ProvenanceMarker::Proposed, &[]),
            ),
        ],
    };
    let mut world = World::lower(&document).unwrap();
    world
        .define_rule(
            "risky-unless-b",
            atom("risky", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("base", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    let risky = derived_row(&world, "risky").expect("risky(x) is derived");
    let ann = Ann::new(risky.support.clone(), risky.defeaters.clone());
    // Rule the proposal variable true — the defeater becomes certain.
    let proposal = world
        .interner
        .names()
        .iter()
        .position(|n| n.contains("proposal"))
        .expect("b minted a proposal variable");
    let mut nu = world.valuation.clone();
    nu.set(dli_core::atom::Atom(u32::try_from(proposal).unwrap()), true);
    assert_eq!(
        ann.status(&nu, &world.theory),
        Status::Possible,
        "a ruled-true defeater downgrades the conclusion to possible"
    );
}

// --- (c) a negation-only variable is range-restriction-unsafe ---------------

#[test]
fn variable_only_in_a_negated_literal_is_unsafe() {
    // risky($x) :- not b($x) — nothing positive binds $x. define_rule rejects it.
    let mut world = World::new();
    let result = world.define_rule(
        "bad",
        atom("risky", vec![var("x")]),
        vec![BodyLiteral::not(atom("b", vec![var("x")]))],
    );
    assert!(
        matches!(result, Err(EvalError::Unstratifiable { .. })),
        "a variable appearing only under `not` violates range restriction: {result:?}"
    );
    assert!(world.rules.is_empty(), "the unsafe rule is not added");
}

// --- (d) a positive-only program is unchanged (no poison, no blocking) -------

#[test]
fn positive_only_program_carries_no_defeaters() {
    // reachable(X, Z) :- adjacent(X, Z) / adjacent(X, Y), reachable(Y, Z).
    // With no negated literal, every derived atom carries an empty defeater
    // polynomial — byte-identical to plain positive datalog.
    let document = Document {
        scope: None,
        decls: vec![
            ratify("d-map"),
            fact(
                "adjacent",
                vec![sym("a"), sym("b")],
                provenance(ProvenanceMarker::Ratified, &["d-map"]),
            ),
            fact(
                "adjacent",
                vec![sym("b"), sym("c")],
                provenance(ProvenanceMarker::Ratified, &["d-map"]),
            ),
        ],
    };
    let mut world = World::lower(&document).unwrap();
    world
        .define_rule(
            "reach-base",
            atom("reachable", vec![var("X"), var("Z")]),
            vec![atom("adjacent", vec![var("X"), var("Z")])],
        )
        .unwrap();
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
    let world = world.fixpoint().unwrap();

    // Transitive closure reaches c from a, and nothing carries a defeater.
    assert!(
        world
            .derived
            .iter()
            .any(|d| d.atom.pred == "reachable" && d.atom.args == vec!["a", "c"]),
        "reachable(a, c) is derived"
    );
    assert!(
        world.derived.iter().all(|d| d.defeaters.is_false()),
        "a positive-only program folds no defeaters"
    );
}

// --- a pure-negative-body rule fires (seed pass) ----------------------------

#[test]
fn pure_negative_rule_fires_when_its_negation_is_absent() {
    // p :- not q, with q never derived. A pure-negative body has no positive atom
    // to delta-trigger it, so the stratum seed fires it once: p is derived.
    let mut world = World::new();
    world
        .define_rule(
            "p-holds",
            atom("p", Vec::new()),
            vec![BodyLiteral::not(atom("q", Vec::new()))],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();
    assert!(
        derived_row(&world, "p").is_some(),
        "p is derived because q is absent (not certain)"
    );
}
