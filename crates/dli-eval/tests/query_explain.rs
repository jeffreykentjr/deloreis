#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Coverage for the query engine's explanation layer (`ENG-1`): the `--why`
//! antichain, `why_not` blame, the `--layer` standing floor, the `--fork`
//! valuation overlay, and the `--as-of` time filter. No parser is involved —
//! the AST is built directly, the same idiom as `smoke.rs`.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{BeforeDecl, Decl, Document, FactDecl, OptionClause, QuestionDecl};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term, TimeRef};

use dli_core::ann::{Ann, Status};
use dli_core::atom::Atom;
use dli_core::derivation::ClaimId;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;

use dli_eval::query::{Layer, Query, QueryOpts};
use dli_eval::why::why_not;
use dli_eval::world::{GroundAtom, World};

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

fn ga(pred: &str, args: &[&str]) -> GroundAtom {
    GroundAtom {
        pred: pred.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
    }
}

fn query(goal: AtomExpr) -> Query {
    Query { goal }
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

fn fact(pred: &str, args: Vec<Term>, provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance,
    })
}

fn decided_fact(pred: &str, args: Vec<Term>, decision: &str) -> Decl {
    fact(
        pred,
        args,
        Some(provenance(ProvenanceMarker::Decided, &[decision])),
    )
}

fn option(name: &str, pred: &str, args: Vec<Term>) -> OptionClause {
    OptionClause {
        name: Ident::synthetic(name),
        label: None,
        implies: Some(atom(pred, args)),
    }
}

fn atom_index(world: &World, name: &str) -> Atom {
    let position = world
        .interner
        .names()
        .iter()
        .position(|n| n == name)
        .expect("name interned");
    Atom(u32::try_from(position).unwrap())
}

// --- Golden worlds ----------------------------------------------------------

/// The storybook golden: two decided adjacency facts on one map decision, plus
/// an open question whose options each imply who keeps the ledger.
fn storybook_world() -> World {
    let document = Document {
        scope: None,
        decls: vec![
            decided_fact(
                "adjacent",
                vec![sym("storybook"), sym("riverside")],
                "d-storybook-map",
            ),
            decided_fact(
                "adjacent",
                vec![sym("riverside"), sym("the-weir")],
                "d-storybook-map",
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
                    option("nara", "keeps", vec![sym("nara"), sym("the-salt-ledger")]),
                ],
                provenance: Some(provenance(ProvenanceMarker::Proposed, &[])),
            }),
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
    world.fixpoint().unwrap()
}

/// A world with one unconditional (certain) keeper and one open-variable
/// (possible) keeper under the same predicate — a mixed-standing relation.
fn mixed_keeper_world() -> World {
    let mut world = World::new();
    world.assert_fact(ga("keeps", &["fen"]), Ann::unconditional());
    let option = world.interner.intern("nara-option");
    world.assert_fact(ga("keeps", &["nara"]), Ann::supported(PosBool::pos(option)));
    world.fixpoint().unwrap()
}

// --- --why: the support antichain -------------------------------------------

#[test]
fn why_renders_the_support_antichain_over_the_golden() {
    let world = storybook_world();
    let goal = query(atom("reachable", vec![sym("storybook"), sym("the-weir")]));
    let opts = QueryOpts {
        why: true,
        ..Default::default()
    };
    let answer = world.query_with(&goal, &opts).unwrap();
    assert_eq!(answer.rows.len(), 1);

    let map = atom_index(&world, "d-storybook-map");
    let why = answer.rows[0].why.as_ref().expect("why attached");
    // The two-hop closure rests on one witness: the map decision (both hops
    // cite it; absorption collapses the conjunction to a single claim).
    assert_eq!(why.antichain, vec![vec![ClaimId(map.0)]]);
    assert_eq!(why.rendered, vec!["d-storybook-map".to_owned()]);
    assert_eq!(why.min_why(), Some(&vec![ClaimId(map.0)]));
}

#[test]
fn why_is_absent_unless_requested() {
    let world = storybook_world();
    let goal = query(atom("reachable", vec![sym("storybook"), sym("the-weir")]));

    let plain = world.query_with(&goal, &QueryOpts::default()).unwrap();
    assert!(plain.rows.iter().all(|row| row.why.is_none()));

    let asked = world
        .query_with(
            &goal,
            &QueryOpts {
                why: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(asked.rows.iter().all(|row| row.why.is_some()));
}

#[test]
fn min_why_picks_the_smallest_witness() {
    // support = {a} ∨ {b, c}: the singleton is the minimal witness.
    let mut world = World::new();
    let a = world.interner.intern("a");
    let b = world.interner.intern("b");
    let c = world.interner.intern("c");
    let support = PosBool::pos(a).or_(&PosBool::pos(b).and_(&PosBool::pos(c)));
    world.assert_fact(ga("q", &["x"]), Ann::supported(support));
    let world = world.fixpoint().unwrap();

    let answer = world
        .query_with(
            &query(atom("q", vec![sym("x")])),
            &QueryOpts {
                why: true,
                ..Default::default()
            },
        )
        .unwrap();
    let why = answer.rows[0].why.as_ref().unwrap();
    assert_eq!(why.antichain.len(), 2);
    assert_eq!(why.min_why(), Some(&vec![ClaimId(a.0)]));
}

// --- why_not: blame ---------------------------------------------------------

#[test]
fn why_not_on_a_proposed_fact_names_its_proposal_var() {
    // A `proposed` fact rests on a fresh open proposal variable.
    let document = Document {
        scope: None,
        decls: vec![fact(
            "keeps",
            vec![sym("nara"), sym("the-salt-ledger")],
            Some(provenance(ProvenanceMarker::Proposed, &[])),
        )],
    };
    let world = World::lower(&document).unwrap().fixpoint().unwrap();

    let goal = atom("keeps", vec![sym("nara"), sym("the-salt-ledger")]);
    let wn = why_not(&world, &goal);
    assert!(wn.known);
    // Possible, not certain: the proposal awaits a ruling.
    assert_eq!(wn.status, Status::Possible);
    // Exactly the proposal variable blocks certainty, and it is named.
    let proposal = atom_index(&world, "h.#0");
    assert_eq!(wn.blocking, vec![ClaimId(proposal.0)]);
    assert_eq!(wn.closest, vec![ClaimId(proposal.0)]);
    assert_eq!(wn.defeater, None);
    assert!(
        wn.rendered.iter().any(|line| line == "awaits ruling: h.#0"),
        "rendered was {:?}",
        wn.rendered
    );
}

#[test]
fn why_not_reads_the_defeater_as_blame() {
    // Certain support, but a fully-ruled defeater overrides it — the atom is
    // possible-not-certain, and the blame is the defeater.
    let mut world = World::new();
    let decision = world.interner.intern("d-0009");
    world.assert_fact(
        ga("holds", &["ward"]),
        Ann::new(PosBool::truth(), PosBool::pos(decision)),
    );
    world.valuation.set(decision, true); // the defeating ruling is in force
    let world = world.fixpoint().unwrap();

    let wn = why_not(&world, &atom("holds", vec![sym("ward")]));
    assert_eq!(wn.status, Status::Possible);
    // Nothing *open* blocks it; the obstacle is the ruled defeater.
    assert!(wn.blocking.is_empty());
    assert_eq!(wn.defeater, Some(ClaimId(decision.0)));
    assert!(
        wn.rendered.iter().any(|line| line == "defeated by: d-0009"),
        "rendered was {:?}",
        wn.rendered
    );
}

#[test]
fn why_not_on_an_unknown_atom_is_marked_unknown() {
    let world = storybook_world();
    let wn = why_not(&world, &atom("floats", vec![sym("storybook")]));
    assert!(!wn.known);
    assert!(wn.blocking.is_empty());
    assert_eq!(wn.defeater, None);
}

// --- --layer: the standing floor --------------------------------------------

#[test]
fn layer_certain_keeps_only_assertable_rows() {
    let world = mixed_keeper_world();
    let goal = query(atom("keeps", vec![var("who")]));

    let certain = world
        .query_with(
            &goal,
            &QueryOpts {
                layer: Layer::Certain,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(certain.rows.len(), 1);
    assert_eq!(certain.rows[0].bindings["who"], "fen");
    assert_eq!(certain.rows[0].status, Status::Certain);
}

#[test]
fn layer_possible_keeps_certain_and_shadow() {
    let world = mixed_keeper_world();
    let goal = query(atom("keeps", vec![var("who")]));

    // Possible does NOT exclude certain: it is the floor certain+possible.
    let possible = world
        .query_with(
            &goal,
            &QueryOpts {
                layer: Layer::Possible,
                ..Default::default()
            },
        )
        .unwrap();
    let mut who: Vec<&str> = possible
        .rows
        .iter()
        .map(|row| row.bindings["who"].as_str())
        .collect();
    who.sort_unstable();
    assert_eq!(who, vec!["fen", "nara"]);

    // All coincides with Possible on the query surface (dead atoms never
    // appear in any layer).
    let all = world
        .query_with(
            &goal,
            &QueryOpts {
                layer: Layer::All,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(all.rows, possible.rows);
}

// --- --fork: the valuation overlay ------------------------------------------

#[test]
fn fork_overlay_changes_answers_without_re_lowering() {
    let world = storybook_world();
    let goal = query(atom("keeps", vec![var("who"), sym("the-salt-ledger")]));

    // Base: both keepers are merely possible while the question is open.
    let base = world.query_with(&goal, &QueryOpts::default()).unwrap();
    assert_eq!(base.rows.len(), 2);
    assert!(base.rows.iter().all(|row| row.status == Status::Possible));

    // A fork overlay ruling for old-fen (and its exclusivity sibling false, as
    // the fork lane would). Applied over the ambient ledger.
    let old_fen = atom_index(&world, "old-fen");
    let nara = atom_index(&world, "nara");
    let mut overlay = Valuation::new();
    overlay.set(old_fen, true);
    overlay.set(nara, false);
    let forked = world
        .query_with(
            &goal,
            &QueryOpts {
                fork: Some(overlay),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(forked.rows.len(), 1);
    assert_eq!(forked.rows[0].bindings["who"], "old-fen");
    assert_eq!(forked.rows[0].status, Status::Certain);

    // The overlay is a read: the world is untouched, so the base query still
    // returns the two shadows — no re-lowering, no re-fixpoint.
    let again = world.query_with(&goal, &QueryOpts::default()).unwrap();
    assert_eq!(again.rows, base.rows);
}

// --- --as-of: the time filter -----------------------------------------------

/// A world with a two-era spine and one fact anchored in each era.
fn timeline_world() -> World {
    let document = Document {
        scope: None,
        decls: vec![
            Decl::Before(BeforeDecl {
                earlier: Ident::synthetic("first-age"),
                later: Ident::synthetic("second-age"),
                provenance: None,
            }),
            decided_fact(
                "at",
                vec![
                    sym("bell"),
                    Term::Time(TimeRef::Era(Ident::synthetic("first-age"))),
                ],
                "d-0001",
            ),
            decided_fact(
                "at",
                vec![
                    sym("weir"),
                    Term::Time(TimeRef::Era(Ident::synthetic("second-age"))),
                ],
                "d-0001",
            ),
        ],
    };
    World::lower(&document).unwrap().fixpoint().unwrap()
}

#[test]
fn as_of_drops_facts_anchored_after_the_cut() {
    let world = timeline_world();
    let goal = query(atom("at", vec![var("what"), var("when")]));

    // No cut: both eras' facts.
    let all = world.query_with(&goal, &QueryOpts::default()).unwrap();
    assert_eq!(all.rows.len(), 2);

    // As of the first age: the second-age fact is strictly after, so it drops.
    let cut = world
        .query_with(
            &goal,
            &QueryOpts {
                as_of: Some("first-age".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
    let what: Vec<&str> = cut
        .rows
        .iter()
        .map(|row| row.bindings["what"].as_str())
        .collect();
    assert_eq!(what, vec!["bell"]);

    // As of the second age: nothing is after it, so both survive.
    let later = world
        .query_with(
            &goal,
            &QueryOpts {
                as_of: Some("second-age".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(later.rows.len(), 2);
}

#[test]
fn as_of_an_unknown_point_filters_nothing() {
    let world = timeline_world();
    let goal = query(atom("at", vec![var("what"), var("when")]));
    let answer = world
        .query_with(
            &goal,
            &QueryOpts {
                as_of: Some("no-such-era".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(answer.rows.len(), 2);
}

// --- query == query_with(default) -------------------------------------------

#[test]
fn query_is_the_default_options_case() {
    let world = storybook_world();
    for goal in [
        atom("reachable", vec![sym("storybook"), var("Z")]),
        atom("keeps", vec![var("who"), sym("the-salt-ledger")]),
    ] {
        let q = query(goal);
        let base = world.query(&q).unwrap();
        let with = world.query_with(&q, &QueryOpts::default()).unwrap();
        assert_eq!(base.rows, with.rows);
    }
}
