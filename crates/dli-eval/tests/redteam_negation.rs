#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]
//! ADVERSARIAL red-team probes of rule-body negation (DL-0020).
//!
//! These are attack tests, not the feature's own suite. They check EXACT values
//! against `docs/spec/02-semantics.md §4` and `08-reasoning.md §1`, and they are
//! written to *fail loudly* where behaviour diverges from the spec. Where a test
//! documents an accepted design boundary it says so in a comment; where it pins a
//! genuine defect the assertion is written against the OBSERVED (buggy) value and
//! flagged `BUG:` so the report can cite a green run.

use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::{Ann, Status};
use dli_core::atom::{Atom, Lit};
use dli_core::posbool::{Monomial, PosBool};
use dli_eval::why::why_not;
use dli_eval::world::{Derived, GroundAtom, World};
use dli_eval::{EvalError, Query};

// --- helpers ---------------------------------------------------------------

fn var(name: &str) -> Term {
    Term::Var(Ident::synthetic(name))
}

fn atom(pred: &str, args: Vec<Term>) -> AtomExpr {
    AtomExpr {
        pred: Ident::synthetic(pred),
        args,
    }
}

fn ground(pred: &str, args: &[&str]) -> GroundAtom {
    GroundAtom {
        pred: pred.to_owned(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// A support monomial of positive event-variable literals, for reading a
/// per-monomial defeater condition back out.
fn monomial(ids: &[u32]) -> Monomial {
    Monomial::from_lits(ids.iter().map(|&i| Lit::pos(Atom(i))))
}

fn row<'a>(world: &'a World, pred: &str, args: &[&str]) -> Option<&'a Derived> {
    let want = ground(pred, args);
    world.derived.iter().find(|d| d.atom == want)
}

fn query_goal(pred: &str, args: &[&str]) -> Query {
    Query {
        goal: atom(
            pred,
            args.iter()
                .map(|s| Term::Sym(Ident::synthetic(*s)))
                .collect(),
        ),
    }
}

// ===========================================================================
// ITEM 1c — an ABSENT negated predicate is NOT negation-as-failure: h derives
// cleanly, with an EMPTY defeater polynomial (nothing to poison).
// ===========================================================================

#[test]
fn absent_negated_predicate_derives_cleanly_no_poison() {
    // p(a) certain; predicate b never asserted at all.
    //     h($x) :- p($x), not b($x)
    // Spec 02 §4: "A b the relation never derives has support FALSE: not certain,
    // nothing to poison." So h is derived, CERTAIN, and NOT at-risk.
    let mut world = World::new();
    world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
    world.valuation.set(Atom(0), true); // d-p ruled
    world
        .define_rule(
            "h-rule",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    let h = row(&world, "h", &["a"]).expect("absent b ⇒ h derived (not NAF)");
    let ann = Ann::new(h.support.clone(), h.defeaters.clone());
    assert_eq!(ann.status(&world.valuation, &world.theory), Status::Certain);
    assert!(
        h.defeaters.is_false(),
        "absent b poisons nothing; P⁻ must be empty, got {:?}",
        h.defeaters
    );
    assert!(!ann.at_risk(&world.valuation, &world.theory));
}

// ===========================================================================
// ITEM 1a — a POSSIBLE b poisons: assert the EXACT defeater monomial is b's
// open support monomial, not merely "non-empty".
// ===========================================================================

#[test]
fn possible_b_poison_is_exactly_bs_open_monomial() {
    let mut world = World::new();
    world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
    world.assert_fact(ground("b", &["a"]), Ann::supported(PosBool::pos(Atom(1))));
    world.valuation.set(Atom(0), true); // p certain
    // Atom(1) (b's support var) stays OPEN → b is possible.
    world
        .define_rule(
            "h-rule",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    let h = row(&world, "h", &["a"]).expect("possible b ⇒ h derived");
    assert_eq!(h.support, PosBool::pos(Atom(0)), "support is exactly p's");
    assert_eq!(
        h.defeaters,
        PosBool::pos(Atom(1)),
        "P⁻ must be exactly b's open monomial {{+1}}, got {:?}",
        h.defeaters
    );
    let ann = Ann::new(h.support.clone(), h.defeaters.clone());
    assert_eq!(ann.status(&world.valuation, &world.theory), Status::Certain);
    assert!(ann.at_risk(&world.valuation, &world.theory));
}

// ===========================================================================
// ITEM 1b + ITEM 7 (FIXED) — a CERTAIN b DEFEATS rather than drops: the
// conclusion is present-but-Possible, and why_not names the blocking premise b
// (Finding 3, the blame gap, closed by the same conditioning fix).
// ===========================================================================

#[test]
fn certain_b_defeats_and_whynot_explains_the_block() {
    let mut world = World::new();
    world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
    world.assert_fact(ground("b", &["a"]), Ann::supported(PosBool::pos(Atom(1))));
    world.valuation.set(Atom(0), true);
    world.valuation.set(Atom(1), true); // b CERTAIN → defeats (reversibly)
    world
        .define_rule(
            "h-rule",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    // The block is a defeater condition on h's support monomial, not absence:
    // h is derived, present, and Possible (defeated), not dropped.
    let h = row(&world, "h", &["a"]).expect("certain b defeats but does not drop h");
    assert_eq!(
        h.ann().status(&world.valuation, &world.theory),
        Status::Possible
    );
    let ans = world.query(&query_goal("h", &["a"])).unwrap();
    assert_eq!(ans.rows.len(), 1, "h is present (defeated), not absent");
    assert_eq!(ans.rows[0].status, Status::Possible);

    // ITEM 7 (Finding 3): blame for the defeated conclusion now names the
    // negated premise b — the block leaves a trail through the defeater.
    let wn = why_not(&world, &atom("h", vec![Term::Sym(Ident::synthetic("a"))]));
    assert!(wn.known, "defeated h is known (present), not unknown");
    assert_eq!(wn.status, Status::Possible);
    assert_eq!(
        wn.defeater.map(|c| c.0),
        Some(1),
        "b (Atom 1) is blamed for the defeat"
    );
    assert!(
        wn.rendered.iter().any(|line| line.contains("defeated by")),
        "the block is explained: {:?}",
        wn.rendered
    );
}

// ===========================================================================
// ITEM 7 (positive) — the certain-AT-RISK conclusion's why_not names the
// negated premise's variable as blame, and flips wording once defeated.
// ===========================================================================

#[test]
fn whynot_names_the_poison_premise_and_flips_when_defeated() {
    let mut world = World::new();
    world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
    world.assert_fact(ground("b", &["a"]), Ann::supported(PosBool::pos(Atom(1))));
    world.valuation.set(Atom(0), true);
    world
        .define_rule(
            "h-rule",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    let wn = why_not(&world, &atom("h", vec![Term::Sym(Ident::synthetic("a"))]));
    assert!(wn.known && wn.status == Status::Certain);
    assert_eq!(
        wn.defeater.map(|c| c.0),
        Some(1),
        "the defeater blame is b's support variable (Atom 1)"
    );
    assert!(
        wn.rendered.iter().any(|l| l.contains("at risk from")),
        "at-risk wording: {:?}",
        wn.rendered
    );

    // Rule b true (ambient) and re-read: wording flips to "defeated by".
    let mut defeated = world.clone();
    defeated.valuation.set(Atom(1), true);
    let wn2 = why_not(
        &defeated,
        &atom("h", vec![Term::Sym(Ident::synthetic("a"))]),
    );
    assert_eq!(
        wn2.status,
        Status::Possible,
        "defeated ⇒ possible, not dead"
    );
    assert!(
        wn2.rendered.iter().any(|l| l.contains("defeated by")),
        "defeated wording: {:?}",
        wn2.rendered
    );
}

// ===========================================================================
// HEADLINE SOUNDNESS FIX — an atom with an INDEPENDENT, undefeated, certain
// support stays Certain when an UNRELATED default's exception is ruled, because
// P⁻ is conditioned per support monomial: the poison rode in on the p-path only
// and never touches the clean q-path.
// ===========================================================================

#[test]
fn independent_clean_support_is_not_defeated_by_unrelated_poison() {
    // p(a), q(a) both certain (Atom0, Atom1 ruled). b(a) possible (Atom2 open).
    //   r1: h($x) :- p($x), not b($x)     (defeasible path, poisons with b)
    //   r2: h($x) :- q($x)                (CLEAN path, no negation)
    // h has an independent ratified derivation via q that does NOT rest on
    // `not b`. Ruling b true must NOT touch it: h stays Certain.
    let mut world = World::new();
    world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
    world.assert_fact(ground("q", &["a"]), Ann::supported(PosBool::pos(Atom(1))));
    world.assert_fact(ground("b", &["a"]), Ann::supported(PosBool::pos(Atom(2))));
    world.valuation.set(Atom(0), true);
    world.valuation.set(Atom(1), true);
    // Atom(2) open → b possible.
    world
        .define_rule(
            "r1",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    world
        .define_rule(
            "r2",
            atom("h", vec![var("x")]),
            vec![atom("q", vec![var("x")])],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    let h = row(&world, "h", &["a"]).expect("h derived by both rules");
    // Support carries BOTH certain monomials {+0} and {+1}.
    assert_eq!(h.support, PosBool::pos(Atom(0)).or_(&PosBool::pos(Atom(1))));
    // The atom-global P⁻ still carries b's poison from r1 (for blame/citation)…
    assert_eq!(h.defeaters, PosBool::pos(Atom(2)));
    // …but it is conditioned on the p-path only: {+0} carries it, {+1} does not.
    let ann = h.ann();
    assert_eq!(
        *ann.conditions.defeater_of(&monomial(&[0])),
        PosBool::pos(Atom(2)),
        "the p-path monomial {{+0}} carries b's poison"
    );
    assert!(
        ann.conditions.defeater_of(&monomial(&[1])).is_false(),
        "the clean q-path monomial {{+1}} carries no defeater"
    );

    // Base valuation: h is certain, and NOT at-risk — the clean q-path makes it
    // robustly certain no matter how b is ruled.
    assert_eq!(ann.status(&world.valuation, &world.theory), Status::Certain);
    assert!(
        !ann.at_risk(&world.valuation, &world.theory),
        "an independent clean certain support is not at-risk"
    );

    // Rule b true. The clean q-path (Atom1) is untouched and certain, so h stays
    // Certain — the poison belongs to the p-path only.
    let mut nu = world.valuation.clone();
    nu.set(Atom(2), true);
    assert_eq!(
        ann.status(&nu, &world.theory),
        Status::Certain,
        "FIXED: the independent q-path (no defeater) keeps h Certain when the \
         unrelated b is ruled true"
    );
}

// ===========================================================================
// ITEM 2 — valuation-phase ruling and the BAKED-BLOCK PERSISTENCE:
// ruling the blocker TRUE via valuation (post-fixpoint) downgrades the
// blocked atom to Possible but does NOT delete it — the block is baked into
// the saturation, so post-hoc confirmation degrades but preserves the row.
// ===========================================================================

#[test]
fn ruling_poison_true_downgrades_but_does_not_delete_the_row() {
    // b possible at fixpoint ⇒ h derived at-risk. Ruling b true via VALUATION
    // (no re-fixpoint) leaves h PRESENT with status Possible — contrast the
    // load-time-certain case, where h is ABSENT. Same logical situation, two
    // different query shapes depending on WHEN b became certain.
    let mut world = World::new();
    world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
    world.assert_fact(ground("b", &["a"]), Ann::supported(PosBool::pos(Atom(1))));
    world.valuation.set(Atom(0), true);
    world
        .define_rule(
            "h-rule",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    // rule b true via ambient valuation and re-query (no re-fixpoint).
    let mut ruled = world.clone();
    ruled.valuation.set(Atom(1), true);
    let ans = ruled.query(&query_goal("h", &["a"])).unwrap();
    assert_eq!(ans.rows.len(), 1, "h is still a row (present)…");
    assert_eq!(ans.rows[0].status, Status::Possible, "…but only Possible");
}

#[test]
fn baked_block_is_unblocked_by_later_supersession() {
    // b CERTAIN at fixpoint (Atom1 ruled true) ⇒ h defeated, but PRESENT in the
    // saturation with the block held as a defeater condition on its support
    // monomial. Superseding b (revaluing Atom1 false) via the ambient ledger
    // WITHOUT re-fixpoint now un-blocks h — it becomes Certain — matching a fresh
    // fixpoint under b=false. The block is a support condition the valuation
    // moves both ways, so "compute once, valuate after" (spec 02 §1 / §5) holds
    // across the negation boundary in the un-blocking direction.
    let build = |b_true: bool| {
        let mut world = World::new();
        world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
        world.assert_fact(ground("b", &["a"]), Ann::supported(PosBool::pos(Atom(1))));
        world.valuation.set(Atom(0), true);
        world.valuation.set(Atom(1), b_true);
        world
            .define_rule(
                "h-rule",
                atom("h", vec![var("x")]),
                vec![
                    BodyLiteral::pos(atom("p", vec![var("x")])),
                    BodyLiteral::not(atom("b", vec![var("x")])),
                ],
            )
            .unwrap();
        world
    };

    // Baked with b certain: h defeated (Possible) but present, not dropped.
    let blocked = build(true).fixpoint().unwrap();
    let h = row(&blocked, "h", &["a"]).expect("defeated h is present, not absent");
    assert_eq!(
        h.ann().status(&blocked.valuation, &blocked.theory),
        Status::Possible
    );

    // Supersede b (Atom1 -> false) WITHOUT re-fixpoint: h un-blocks to Certain.
    let mut superseded = blocked.clone();
    superseded.valuation.set(Atom(1), false);
    let ans = superseded.query(&query_goal("h", &["a"])).unwrap();
    assert_eq!(ans.rows.len(), 1, "valuation-only supersession derives h");
    assert_eq!(
        ans.rows[0].status,
        Status::Certain,
        "FIXED: valuation-only supersession un-blocks h to Certain"
    );

    // A fresh fixpoint under b=false ALSO derives h Certain — the answers agree.
    let refixed = build(false).fixpoint().unwrap();
    let h_ref = row(&refixed, "h", &["a"]).expect("re-fixpoint under b=false derives h");
    assert_eq!(
        h_ref.ann().status(&refixed.valuation, &refixed.theory),
        Status::Certain,
        "valuation-only and re-fixpoint agree — no divergence"
    );
}

// ===========================================================================
// ITEM 3 — stratification.
// ===========================================================================

#[test]
fn three_cycle_negative_dependency_is_unstratifiable() {
    // p :- not q, q :- not r, r :- not p — a 3-cycle through negation.
    let mut world = World::new();
    for (h, b) in [("p", "q"), ("q", "r"), ("r", "p")] {
        world
            .define_rule(
                format!("{h}-rule"),
                atom(h, Vec::new()),
                vec![BodyLiteral::not(atom(b, Vec::new()))],
            )
            .unwrap();
    }
    assert!(matches!(
        world.stratify(),
        Err(EvalError::Unstratifiable { .. })
    ));
    assert!(matches!(
        world.fixpoint(),
        Err(EvalError::Unstratifiable { .. })
    ));
}

#[test]
fn direct_self_negation_is_unstratifiable() {
    // p :- not p — a negative self-loop (SCC of size 1).
    let mut world = World::new();
    world
        .define_rule(
            "p-rule",
            atom("p", Vec::new()),
            vec![BodyLiteral::not(atom("p", Vec::new()))],
        )
        .unwrap();
    assert!(
        matches!(world.stratify(), Err(EvalError::Unstratifiable { .. })),
        "a negative self-loop must be caught"
    );
}

#[test]
fn stratifiable_negated_program_loads_and_evaluates() {
    // q :- base ; p :- not q. No cycle. base certain ⇒ q certain ⇒ p blocked.
    let mut world = World::new();
    world.assert_fact(ground("base", &[]), Ann::supported(PosBool::pos(Atom(0))));
    world.valuation.set(Atom(0), true); // base certain
    world
        .define_rule(
            "q-rule",
            atom("q", Vec::new()),
            vec![atom("base", Vec::new())],
        )
        .unwrap();
    world
        .define_rule(
            "p-rule",
            atom("p", Vec::new()),
            vec![BodyLiteral::not(atom("q", Vec::new()))],
        )
        .unwrap();
    let strata = world.stratify().expect("stratifiable");
    // q's stratum must be strictly earlier than p's.
    let level_of = |pred: &str| {
        strata
            .levels
            .iter()
            .position(|l| l.iter().any(|s| s == pred))
    };
    assert!(
        level_of("q") < level_of("p"),
        "q evaluated before p: {strata:?}"
    );

    let world = world.fixpoint().unwrap();
    let q = row(&world, "q", &[]).expect("q derived from base");
    assert_eq!(
        q.ann().status(&world.valuation, &world.theory),
        Status::Certain
    );
    // p is present but defeated (the block is a reversible defeater condition,
    // not absence): q is certain, so `not q` poisons p to Possible.
    let p = row(&world, "p", &[]).expect("p present, defeated by certain q");
    assert_eq!(
        p.ann().status(&world.valuation, &world.theory),
        Status::Possible,
        "p defeated: q is certain"
    );
}

#[test]
fn mixed_positive_recursion_plus_negation_stratifies_and_evaluates() {
    // Transitive closure `reach` (positive recursion) + `orphan` depending
    // NEGATIVELY on reach. Stratifiable (reach and orphan are separate SCCs).
    let mut world = World::new();
    let cert = |a: u32| Ann::supported(PosBool::pos(Atom(a)));
    world.assert_fact(ground("edge", &["a", "b"]), cert(0));
    world.assert_fact(ground("edge", &["b", "c"]), cert(0));
    for n in ["a", "b", "c", "z"] {
        world.assert_fact(ground("node", &[n]), cert(0));
    }
    world.valuation.set(Atom(0), true);
    world
        .define_rule(
            "reach-base",
            atom("reach", vec![var("X"), var("Y")]),
            vec![atom("edge", vec![var("X"), var("Y")])],
        )
        .unwrap();
    world
        .define_rule(
            "reach-step",
            atom("reach", vec![var("X"), var("Y")]),
            vec![
                atom("edge", vec![var("X"), var("Z")]),
                atom("reach", vec![var("Z"), var("Y")]),
            ],
        )
        .unwrap();
    // orphan(Y) :- node(Y), not reach(a, Y)
    world
        .define_rule(
            "orphan-rule",
            atom("orphan", vec![var("Y")]),
            vec![
                BodyLiteral::pos(atom("node", vec![var("Y")])),
                BodyLiteral::not(atom(
                    "reach",
                    vec![Term::Sym(Ident::synthetic("a")), var("Y")],
                )),
            ],
        )
        .unwrap();
    world.stratify().expect("mixed program is stratifiable");
    let world = world.fixpoint().unwrap();

    assert!(row(&world, "reach", &["a", "c"]).is_some(), "a reaches c");
    // reach(a,b) and reach(a,c) certain ⇒ orphan(b), orphan(c) present but
    // DEFEATED (Possible) — the block is a reversible defeater condition, not
    // absence.
    let orphan_status = |node: &str| {
        row(&world, "orphan", &[node])
            .unwrap_or_else(|| panic!("orphan({node}) present"))
            .ann()
            .status(&world.valuation, &world.theory)
    };
    assert_eq!(
        orphan_status("b"),
        Status::Possible,
        "b is reached ⇒ not an orphan"
    );
    assert_eq!(
        orphan_status("c"),
        Status::Possible,
        "c is reached ⇒ not an orphan"
    );
    // a does not reach a; z is isolated ⇒ orphan(a), orphan(z) certain.
    assert_eq!(
        orphan_status("a"),
        Status::Certain,
        "a is not reached from a"
    );
    assert_eq!(orphan_status("z"), Status::Certain, "z is unreached");
}

// ===========================================================================
// ITEM 4 — range restriction / safety.
// ===========================================================================

#[test]
fn head_variable_absent_from_body_is_rejected() {
    // h($x) :- p($y) — $x appears in the head but no body atom binds it.
    let mut world = World::new();
    let result = world.define_rule(
        "bad",
        atom("h", vec![var("x")]),
        vec![atom("p", vec![var("y")])],
    );
    assert!(matches!(result, Err(EvalError::Unstratifiable { .. })));
    assert!(world.rules.is_empty());
}

#[test]
fn safe_rule_with_negation_loads_clean() {
    // Every variable positively bound: loads and the rule is retained.
    let mut world = World::new();
    world
        .define_rule(
            "safe",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .expect("a safe negated rule loads");
    assert_eq!(world.rules.len(), 1);
}

// ===========================================================================
// ITEM 5 — positive-datalog regression: transitive closure is byte-identical
// with and without the negation machinery present in the rule set.
// ===========================================================================

#[test]
fn positive_closure_carries_zero_defeaters() {
    let mut world = World::new();
    let cert = Ann::supported(PosBool::pos(Atom(0)));
    world.assert_fact(ground("edge", &["a", "b"]), cert.clone());
    world.assert_fact(ground("edge", &["b", "c"]), cert.clone());
    world.assert_fact(ground("edge", &["c", "d"]), cert);
    world.valuation.set(Atom(0), true);
    world
        .define_rule(
            "reach-base",
            atom("reach", vec![var("X"), var("Y")]),
            vec![atom("edge", vec![var("X"), var("Y")])],
        )
        .unwrap();
    world
        .define_rule(
            "reach-step",
            atom("reach", vec![var("X"), var("Y")]),
            vec![
                atom("edge", vec![var("X"), var("Z")]),
                atom("reach", vec![var("Z"), var("Y")]),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();

    // Full closure a→d and every derived atom has an empty defeater polynomial.
    assert!(row(&world, "reach", &["a", "d"]).is_some());
    assert!(row(&world, "reach", &["a", "c"]).is_some());
    assert!(world.derived.iter().all(|d| d.defeaters.is_false()));
    // Exactly the 6 reachable pairs (ab bc cd ac bd ad) + 3 edges present.
    let reach_pairs = world
        .derived
        .iter()
        .filter(|d| d.atom.pred == "reach")
        .count();
    assert_eq!(reach_pairs, 6, "transitive closure of a path of length 3");
}

// ===========================================================================
// EDGE — an EMPTY typed body (only reachable via the raw define_rule API; the
// parser rejects it) exercises the vacuous `body.iter().all(negated)` branch:
// the head is seeded once and derived unconditionally. No panic, sane result.
// ===========================================================================

#[test]
fn empty_typed_body_derives_the_head_unconditionally() {
    let mut world = World::new();
    world
        .define_rule("always", atom("t", Vec::new()), Vec::<BodyLiteral>::new())
        .unwrap();
    let world = world.fixpoint().unwrap();
    let t = row(&world, "t", &[]).expect("empty-body rule seeds its head");
    assert!(t.support.is_true(), "unconditional support");
    assert!(t.defeaters.is_false(), "no poison");
    assert_eq!(
        Ann::new(t.support.clone(), t.defeaters.clone()).status(&world.valuation, &world.theory),
        Status::Certain
    );
}

// ===========================================================================
// EDGE — a negated premise named twice poisons idempotently (one monomial).
// ===========================================================================

#[test]
fn duplicate_negated_literal_poisons_once() {
    let mut world = World::new();
    world.assert_fact(ground("p", &["a"]), Ann::supported(PosBool::pos(Atom(0))));
    world.assert_fact(ground("b", &["a"]), Ann::supported(PosBool::pos(Atom(1))));
    world.valuation.set(Atom(0), true);
    world
        .define_rule(
            "dup",
            atom("h", vec![var("x")]),
            vec![
                BodyLiteral::pos(atom("p", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
                BodyLiteral::not(atom("b", vec![var("x")])),
            ],
        )
        .unwrap();
    let world = world.fixpoint().unwrap();
    let h = row(&world, "h", &["a"]).expect("h derived");
    assert_eq!(
        h.defeaters,
        PosBool::pos(Atom(1)),
        "duplicate `not b` folds to one poison monomial (absorption)"
    );
}
