#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Fork-manager coverage (`ENG-2`): O(1) creation, suppose/suppress/add/restrict,
//! the derived-consequence diff, the ratification pre-flight, and generative
//! promotion. The base world is a hand-built saltmarket ledger — no parser is
//! involved.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, OptionClause, QuestionDecl, RatifyDecl};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Ann;
use dli_core::atom::Atom;
use dli_core::grade::Grade;

use dli_eval::Query;
use dli_eval::fork::{Addition, ConflictKind, ForkError, ForkId, ForkStore, KeepStamp, Visibility};
use dli_eval::world::{GroundAtom, World};

fn sym(name: &str) -> Term {
    Term::Sym(Ident::synthetic(name))
}

fn atom_expr(pred: &str, args: Vec<Term>) -> AtomExpr {
    AtomExpr {
        pred: Ident::synthetic(pred),
        args,
    }
}

fn ground(pred: &str, args: &[&str]) -> GroundAtom {
    GroundAtom {
        pred: pred.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
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

fn decided_fact(pred: &str, args: Vec<Term>, decision: &str) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom_expr(pred, args),
        triple: false,
        provenance: Some(provenance(ProvenanceMarker::Decided, &[decision])),
    })
}

fn option(name: &str, pred: &str, args: Vec<Term>) -> OptionClause {
    OptionClause {
        name: Ident::synthetic(name),
        implies: Some(atom_expr(pred, args)),
    }
}

/// A base world: one certain fact (`weathered(gate)`, decided, `Asserted` grade)
/// and an open question whose two mutually-exclusive options each imply a keeper
/// of the salt-ledger.
fn base_world() -> World {
    let document = Document {
        scope: None,
        decls: vec![
            // A citation carries authority only if the corpus declares the
            // decision, so d-weather must be ratified for weathered(gate) to be
            // certain (defense-in-depth against forged citations, B1).
            Decl::Ratify(RatifyDecl {
                decision: Ident::synthetic("d-weather"),
                body: Vec::new(),
            }),
            decided_fact("weathered", vec![sym("gate")], "d-weather"),
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
                provenance: Some(provenance(ProvenanceMarker::Proposed, &[])),
            }),
        ],
    };
    World::lower(&document)
        .expect("lowerable")
        .fixpoint()
        .expect("stratifiable")
}

fn atom_of(world: &World, name: &str) -> Atom {
    let position = world
        .interner
        .names()
        .iter()
        .position(|n| n == name)
        .expect("interned name");
    Atom(u32::try_from(position).expect("fits in u32"))
}

#[test]
fn fork_creation_is_o1_and_leaves_the_base_untouched() {
    let world = base_world();
    let mut store = ForkStore::new(world.clone());

    // A thousand suppose-only forks, each diffed. No fixpoint re-runs (additions
    // are empty, so every diff borrows the base saturation), so this is a
    // time-bound sanity check, not an O(forks × fixpoint) blow-up.
    let start = Instant::now();
    for _ in 0..1000 {
        let fork = store.fork();
        store.suppose(fork, "old-fen", true).unwrap();
        let delta = store.diff(fork).unwrap();
        assert_eq!(delta.newly_certain.len(), 1);
    }
    let elapsed = start.elapsed();

    assert_eq!(store.len(), 1000);
    assert!(
        elapsed < Duration::from_secs(5),
        "1000 suppose-only forks took {elapsed:?} — a re-fixpoint likely leaked in"
    );
    // The base world is a lens, never a copy: forking never mutated it.
    assert_eq!(store.base(), &world);
}

#[test]
fn suppose_rules_exclusivity_siblings_dead() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();
    store.suppose(fork, "old-fen", true).unwrap();

    let delta = store.diff(fork).unwrap();
    // The supposed option becomes certain; its exclusivity sibling goes dead.
    assert_eq!(delta.newly_certain.len(), 1);
    assert_eq!(
        delta.newly_certain[0].atom.render(),
        "keeps(old-fen, the-salt-ledger)"
    );
    assert_eq!(delta.newly_dead.len(), 1);
    assert_eq!(
        delta.newly_dead[0].atom.render(),
        "keeps(mara, the-salt-ledger)"
    );
}

#[test]
fn diff_catches_newly_certain_newly_dead_and_grade_changed() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();
    // Suppose old-fen: keeps(old-fen) newly certain, keeps(mara) newly dead.
    store.suppose(fork, "old-fen", true).unwrap();
    // Add a checked re-derivation of the already-certain (asserted) weathered
    // fact: same status, stronger grade — a grade change.
    store
        .add(
            fork,
            [Addition::new(
                ground("weathered", &["gate"]),
                Ann::unconditional(),
                Grade::Checked,
            )],
        )
        .unwrap();

    let delta = store.diff(fork).unwrap();

    assert_eq!(delta.newly_certain.len(), 1);
    assert_eq!(
        delta.newly_certain[0].atom.render(),
        "keeps(old-fen, the-salt-ledger)"
    );

    assert_eq!(delta.newly_dead.len(), 1);
    assert_eq!(
        delta.newly_dead[0].atom.render(),
        "keeps(mara, the-salt-ledger)"
    );

    assert_eq!(delta.grade_changed.len(), 1);
    let graded = &delta.grade_changed[0];
    assert_eq!(graded.atom.render(), "weathered(gate)");
    assert_eq!(graded.before.grade, Grade::Asserted);
    assert_eq!(graded.after.grade, Grade::Checked);
}

#[test]
fn add_of_a_new_atom_is_newly_certain() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();
    store
        .add(
            fork,
            [Addition::asserted(
                ground("raised-by", &["tide-bell", "guild"]),
                Ann::unconditional(),
            )],
        )
        .unwrap();

    let delta = store.diff(fork).unwrap();
    assert!(
        delta
            .newly_certain
            .iter()
            .any(|c| c.atom.render() == "raised-by(tide-bell, guild)")
    );
}

#[test]
fn preflight_surfaces_an_option_contradicted_by_a_certain_fact() {
    // A certain fact has already closed the `mara` option (its variable is ruled
    // false in the base ledger).
    let mut world = base_world();
    let mara = atom_of(&world, "mara");
    world.valuation.set(mara, false);

    let mut store = ForkStore::new(world);
    let fork = store.fork();
    // Supposing the ruled-out option true contradicts the ledger.
    store.suppose(fork, "mara", true).unwrap();

    let pre = store.preflight(fork).unwrap();
    assert!(
        pre.conflicts
            .iter()
            .any(|c| c.kind == ConflictKind::Valuation),
        "preflight should surface the valuation conflict: {:?}",
        pre.conflicts
    );
    // The candidate valuation and affected atoms come back for the author.
    assert!(!pre.affected.is_empty());
}

#[test]
fn preflight_surfaces_two_supposed_siblings_as_exclusivity() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();
    store.suppose(fork, "old-fen", true).unwrap();
    store.suppose(fork, "mara", true).unwrap();

    let pre = store.preflight(fork).unwrap();
    assert!(
        pre.conflicts
            .iter()
            .any(|c| c.kind == ConflictKind::Exclusivity)
    );
}

#[test]
fn promote_affects_exactly_the_touched_atoms() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork_named("the-fen-keeps-the-ledger");
    store.suppose(fork, "old-fen", true).unwrap();

    let delta = store.diff(fork).unwrap();
    let touched: BTreeSet<String> = delta
        .newly_certain
        .iter()
        .chain(&delta.newly_possible)
        .chain(&delta.newly_dead)
        .chain(&delta.grade_changed)
        .map(|c| c.atom.render())
        .collect();

    let rq = store.promote(fork).unwrap();
    let affects: BTreeSet<String> = rq.affects.iter().cloned().collect();

    assert_eq!(affects, touched);
    assert_eq!(rq.slug, "the-fen-keeps-the-ledger");
    // The evidence records what the fork supposed.
    assert!(rq.evidence.iter().any(|e| e.contains("suppose old-fen")));
}

#[test]
fn suppress_restores_base_behavior() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();

    store.suppose(fork, "old-fen", true).unwrap();
    let changed = store.diff(fork).unwrap();
    assert!(!changed.newly_certain.is_empty());

    // Retract the supposition: the fork reverts to the base ledger exactly.
    store.suppress(fork, "old-fen").unwrap();
    let restored = store.diff(fork).unwrap();
    assert!(restored.newly_certain.is_empty());
    assert!(restored.newly_possible.is_empty());
    assert!(restored.newly_dead.is_empty());
    assert!(restored.grade_changed.is_empty());
}

#[test]
fn restrict_hides_from_the_diff_but_not_from_preflight() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();
    store.suppose(fork, "old-fen", true).unwrap();
    // Keep-stamp the keeps predicate out of the fork's presented consequences.
    store
        .restrict(
            fork,
            KeepStamp {
                scope: vec!["keeps".to_owned()],
                visibility: Visibility::Removed,
            },
        )
        .unwrap();

    // Visibility, not valuation: the diff hides the keeps atoms …
    let delta = store.diff(fork).unwrap();
    assert!(delta.newly_certain.is_empty());
    assert!(delta.newly_dead.is_empty());

    // … but they are still true and still surface in the ratification pre-flight.
    let pre = store.preflight(fork).unwrap();
    assert!(pre.affected.iter().any(|a| a.pred == "keeps"));
}

#[test]
fn merge_reports_exclusivity_and_valuation_conflicts() {
    let mut store = ForkStore::new(base_world());

    // Two forks each supposing a different exclusive option: they jointly break Θ.
    let picks_fen = store.fork();
    store.suppose(picks_fen, "old-fen", true).unwrap();
    let picks_mara = store.fork();
    store.suppose(picks_mara, "mara", true).unwrap();
    let exclusivity = store.merge(picks_fen, picks_mara).unwrap();
    assert!(
        exclusivity
            .iter()
            .any(|c| c.kind == ConflictKind::Exclusivity)
    );

    // Two forks valuating the same variable differently disagree.
    let fen_true = store.fork();
    store.suppose(fen_true, "old-fen", true).unwrap();
    let fen_false = store.fork();
    store.suppose(fen_false, "old-fen", false).unwrap();
    let valuation = store.merge(fen_true, fen_false).unwrap();
    assert!(valuation.iter().any(|x| x.kind == ConflictKind::Valuation));

    // Two compatible forks merge clean.
    let supposes = store.fork();
    store.suppose(supposes, "old-fen", true).unwrap();
    let adds = store.fork();
    store
        .add(
            adds,
            [Addition::asserted(
                ground("raised-by", &["tide-bell", "guild"]),
                Ann::unconditional(),
            )],
        )
        .unwrap();
    assert!(store.merge(supposes, adds).unwrap().is_empty());
}

#[test]
fn fork_from_inherits_overlay_and_records_lineage() {
    let mut store = ForkStore::new(base_world());
    let parent = store.fork();
    store.suppose(parent, "old-fen", true).unwrap();

    let child = store.fork_from(parent).unwrap();
    assert_eq!(store.get(child).unwrap().parent, Some(parent));
    // The child inherits the supposition, so its diff matches the parent's.
    assert_eq!(
        store.diff(child).unwrap().newly_certain.len(),
        store.diff(parent).unwrap().newly_certain.len()
    );
}

#[test]
fn exposed_valuation_bakes_exclusivity_and_drives_eval_under() {
    // The seam ENG-1's `--fork` query path consumes: a plain Valuation with the
    // theory pass already baked in.
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();
    store.suppose(fork, "old-fen", true).unwrap();

    let old_fen = atom_of(store.base(), "old-fen");
    let mara = atom_of(store.base(), "mara");

    let nu = store.valuation(fork).unwrap();
    // Supposing old-fen true rules the sibling mara false inside the valuation.
    assert_eq!(nu.get(old_fen), Some(true));
    assert_eq!(nu.get(mara), Some(false));
    // It is exactly the pre-flight candidate.
    assert_eq!(nu, store.preflight(fork).unwrap().candidate);

    // Driving eval_under with it directly (no re-lowering, no theory re-pass)
    // reproduces the fork's certain layer: old-fen certain, mara gone dead.
    let goal = Query {
        goal: atom_expr("keeps", vec![sym("old-fen"), sym("the-salt-ledger")]),
    };
    let answer = store.base().eval_under(&nu, &goal).unwrap();
    assert_eq!(answer.rows.len(), 1);
    assert_eq!(answer.rows[0].status, dli_core::ann::Status::Certain);
}

#[test]
fn unknown_targets_and_handles_error() {
    let mut store = ForkStore::new(base_world());
    let fork = store.fork();

    assert!(matches!(
        store.suppose(fork, "no-such-option", true),
        Err(ForkError::UnknownTarget(_))
    ));
    assert!(matches!(
        store.suppress(fork, "no-such-option"),
        Err(ForkError::UnknownTarget(_))
    ));
    assert!(matches!(
        store.diff(ForkId(9999)),
        Err(ForkError::UnknownFork(_))
    ));
}

#[test]
fn fork_add_grade_vector_per_monomial() {
    use dli_core::posbool::PosBool;
    use dli_core::valuation::Valuation;

    // Base world: one fact on var_a (Checked) and one on var_b (Asserted).
    let mut world = World::new();
    let var_a = world.interner.intern("h.a");
    let var_b = world.interner.intern("h.b");

    world.assert_fact_graded(
        GroundAtom {
            pred: "p".to_owned(),
            args: vec![],
        },
        Ann::supported(PosBool::pos(var_a)),
        Grade::Checked,
    );
    world.assert_fact_graded(
        GroundAtom {
            pred: "p".to_owned(),
            args: vec![],
        },
        Ann::supported(PosBool::pos(var_b)),
        Grade::Asserted,
    );

    let world = world.fixpoint().expect("stratifiable");
    let mut store = ForkStore::new(world.clone());

    // Query under var_a true, var_b false → Checked.
    let mut nu_a = Valuation::new();
    nu_a.set(var_a, true);
    nu_a.set(var_b, false);
    let derived = world.derived.iter().find(|d| d.atom.pred == "p").unwrap();
    assert_eq!(world.grade_under(derived, &nu_a), Grade::Checked);

    // Query under var_b true, var_a false → Asserted.
    let mut nu_b = Valuation::new();
    nu_b.set(var_a, false);
    nu_b.set(var_b, true);
    assert_eq!(world.grade_under(derived, &nu_b), Grade::Asserted);

    // Fork with an addition: a new fact on var_a at grade Asserted. After
    // re-fixpoint, the var_a monomial's grade should be join_alt(Checked, Asserted) = Checked.
    let fork = store.fork();

    // Add a fork-local fact on var_a at Asserted grade.
    store
        .add(
            fork,
            vec![Addition::new(
                GroundAtom {
                    pred: "p".to_owned(),
                    args: vec![],
                },
                Ann::supported(PosBool::pos(var_a)),
                Grade::Asserted,
            )],
        )
        .expect("add to fork");

    // Exercise the fork standings seam (fork.rs:737): diff() calls standings()
    // which calls grade_under() on the layered world. The base has {a}@Checked
    // and {b}@Asserted; the fork adds {a}@Asserted which join_alt's to Checked.
    // Under the base valuation the grade should remain unchanged (both vars
    // unset → both monomials don't fire → Asserted fallback).
    let delta = store.diff(fork).expect("diff");

    // Verify the base standings via store.base() — grade_under on the base world.
    let base_derived = store
        .base()
        .derived
        .iter()
        .find(|d| d.atom.pred == "p")
        .unwrap();
    assert_eq!(
        store.base().grade_under(base_derived, &nu_a),
        Grade::Checked
    );
    assert_eq!(
        store.base().grade_under(base_derived, &nu_b),
        Grade::Asserted
    );

    // The fork's diff exercises grade_under through the standings function.
    // Since the base valuation has vars unset, the fork addition should not
    // produce a grade_changed entry (both before/after would be Asserted under
    // empty valuation). But we verify the diff ran without error and the grade
    // path was exercised.
    // (If vars are set in valuation, it would show up in grade_changed.)
    // Under the empty base valuation, both before/after grade_under == Asserted
    // (no monomial fires), so no grade_changed entry for `p`.
    assert!(
        delta.grade_changed.is_empty(),
        "under empty valuation, no grade change expected; got: {:?}",
        delta.grade_changed
    );

    // Also verify using a direct world with the fork's addition for correctness.
    let mut program = world.clone();
    program.derived.clear();
    program.support_index.clear();
    program.assert_fact_graded(
        GroundAtom {
            pred: "p".to_owned(),
            args: vec![],
        },
        Ann::supported(PosBool::pos(var_a)),
        Grade::Asserted,
    );
    let layered = program.fixpoint().expect("stratifiable");
    let derived2 = layered.derived.iter().find(|d| d.atom.pred == "p").unwrap();

    // Under var_a: join_alt(Checked, Asserted) = Checked (best derivation wins).
    assert_eq!(layered.grade_under(derived2, &nu_a), Grade::Checked);
    // Under var_b: still Asserted (only one derivation path).
    assert_eq!(layered.grade_under(derived2, &nu_b), Grade::Asserted);
}

#[test]
fn fork_add_does_not_contaminate_base() {
    // TQ-N4 (stronger form): fork additions never leak into the base world.
    // This is the trust-boundary invariant.

    // 1. Create a world with some facts, fixpoint it.
    let world = base_world();
    let pre_fork_world = world.clone();

    // 2. Clone the world into a ForkStore.
    let mut store = ForkStore::new(world);

    // 3. Fork, add an atom via fork.add.
    let fork = store.fork();
    store
        .add(
            fork,
            [Addition::asserted(
                ground("new-predicate", &["fresh-entity", "novel-data"]),
                Ann::unconditional(),
            )],
        )
        .unwrap();

    // Verify the addition is visible in the fork's diff.
    let delta = store.diff(fork).unwrap();
    assert!(
        delta
            .newly_certain
            .iter()
            .any(|c| c.atom.render() == "new-predicate(fresh-entity, novel-data)"),
        "the added atom must be newly-certain in the fork diff"
    );

    // 4. Promote the fork.
    let _rq = store.promote(fork).unwrap();

    // 5. Assert store.base() field-equals the pre-fork world (the base is immutable).
    assert_eq!(
        store.base(),
        &pre_fork_world,
        "the base world must be immutable — fork operations must never mutate it"
    );

    // 6. Query the base world for the added atom → must return zero rows.
    let query = Query {
        goal: atom_expr(
            "new-predicate",
            vec![sym("fresh-entity"), sym("novel-data")],
        ),
    };
    let answer = store.base().query(&query);
    match answer {
        Ok(ans) => {
            assert_eq!(
                ans.rows.len(),
                0,
                "the base world must not contain the fork-added atom — \
                 fork additions must never leak into the base"
            );
        }
        // UnknownPredicate is also acceptable: it means the base has never
        // heard of the predicate, which is the strongest form of non-leakage.
        Err(dli_eval::error::EvalError::UnknownPredicate { .. }) => {}
        Err(e) => panic!("unexpected eval error querying base for fork atom: {e}"),
    }
}
