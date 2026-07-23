//! Independent adversarial re-verification of the per-support-monomial defeater
//! conditioning fix (b8243cc). Written fresh; does not reuse the committed
//! `redteam_negation.rs`. The two demonstrated defects (`f6_robust`,
//! `edge_poison_valuation_neutral_reblocks_under_fork`) now assert the fixed
//! behavior.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::{Ann, DefeaterConditions, Status, Theory};
use dli_core::atom::{Atom, Lit};
use dli_core::posbool::{Monomial, PosBool};
use dli_core::valuation::Valuation;
use dli_eval::why::why_not;
use dli_eval::world::{GroundAtom, World};

fn id(s: &str) -> Ident {
    Ident::synthetic(s)
}
fn var(s: &str) -> Term {
    Term::Var(id(s))
}
fn patom(pred: &str, v: &str) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: vec![var(v)],
    }
}
/// A ground (constant-arg) goal atom, for `why_not`/query goals.
fn catom(pred: &str, arg: &str) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: vec![Term::Sym(id(arg))],
    }
}
fn ga(pred: &str, arg: &str) -> GroundAtom {
    GroundAtom {
        pred: pred.to_owned(),
        args: vec![arg.to_owned()],
    }
}
fn pos(a: AtomExpr) -> BodyLiteral {
    BodyLiteral::pos(a)
}
fn not(a: AtomExpr) -> BodyLiteral {
    BodyLiteral::not(a)
}

fn var_atom(world: &World, name: &str) -> Atom {
    let i = world
        .interner
        .names()
        .iter()
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("var {name} not interned"));
    Atom(u32::try_from(i).unwrap())
}

fn row<'a>(world: &'a World, pred: &str, arg: &str) -> Option<&'a dli_eval::world::Derived> {
    let g = ga(pred, arg);
    world.derived.iter().find(|d| d.atom == g)
}

fn status_at(world: &World, pred: &str, arg: &str, val: &Valuation) -> Status {
    match row(world, pred, arg) {
        Some(d) => d.ann().status(val, &world.theory),
        None => Status::Dead,
    }
}
fn at_risk_at(world: &World, pred: &str, arg: &str, val: &Valuation) -> bool {
    match row(world, pred, arg) {
        Some(d) => d.ann().at_risk(val, &world.theory),
        None => false,
    }
}

/// Ratified fact: pred(a) resting on a decision variable ruled true.
fn ratified(world: &mut World, pred: &str, arg: &str, decision: &str) {
    let d = world.interner.intern(decision);
    world.valuation.set(d, true);
    world.assert_fact(ga(pred, arg), Ann::supported(PosBool::pos(d)));
}
/// Proposed fact: pred(a) resting on a fresh open variable (never ruled).
fn proposed(world: &mut World, pred: &str, arg: &str, evar: &str) {
    let v = world.interner.intern(evar);
    world.assert_fact(ga(pred, arg), Ann::supported(PosBool::pos(v)));
}

// ---------------------------------------------------------------------------
// FINDING 1 — independent clean support must stay Certain, not at-risk.
// ---------------------------------------------------------------------------

#[test]
fn f1a_independent_clean_support_stays_certain() {
    // p(a),q(a) ratified; b(a) proposed. r1: h:-p,not b   r2: h:-q
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    ratified(&mut w, "q", "a", "d-q");
    proposed(&mut w, "b", "a", "b-var");
    w.define_rule(
        "r1",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    w.define_rule("r2", patom("h", "x"), vec![pos(patom("q", "x"))])
        .unwrap();
    let w = w.fixpoint().unwrap();
    let b = var_atom(&w, "b-var");

    // Ambient (b open): h certain via clean q, NOT at risk.
    assert_eq!(
        status_at(&w, "h", "a", &w.valuation),
        Status::Certain,
        "ambient status"
    );
    assert!(
        !at_risk_at(&w, "h", "a", &w.valuation),
        "ambient at_risk must be false (clean q path)"
    );

    // Rule b TRUE: still certain via q, still NOT at risk.
    let mut val = w.valuation.clone();
    val.set(b, true);
    assert_eq!(
        status_at(&w, "h", "a", &val),
        Status::Certain,
        "ruling b true must leave h Certain (q path is clean)"
    );
    assert!(
        !at_risk_at(&w, "h", "a", &val),
        "ruling b true must NOT make h at-risk (q path is clean)"
    );
}

#[test]
fn f1b_two_poisoned_paths_plus_one_clean() {
    // p1,p2,q ratified; b1,b2 proposed.
    // r1: h:-p1,not b1  r2: h:-p2,not b2  r3: h:-q
    let mut w = World::new();
    ratified(&mut w, "p1", "a", "d-p1");
    ratified(&mut w, "p2", "a", "d-p2");
    ratified(&mut w, "q", "a", "d-q");
    proposed(&mut w, "b1", "a", "b1-var");
    proposed(&mut w, "b2", "a", "b2-var");
    w.define_rule(
        "r1",
        patom("h", "x"),
        vec![pos(patom("p1", "x")), not(patom("b1", "x"))],
    )
    .unwrap();
    w.define_rule(
        "r2",
        patom("h", "x"),
        vec![pos(patom("p2", "x")), not(patom("b2", "x"))],
    )
    .unwrap();
    w.define_rule("r3", patom("h", "x"), vec![pos(patom("q", "x"))])
        .unwrap();
    let w = w.fixpoint().unwrap();
    let b1 = var_atom(&w, "b1-var");
    let b2 = var_atom(&w, "b2-var");

    let mut val = w.valuation.clone();
    val.set(b1, true);
    val.set(b2, true);
    assert_eq!(
        status_at(&w, "h", "a", &val),
        Status::Certain,
        "both b1,b2 true: h must stay Certain via clean q path"
    );
    assert!(
        !at_risk_at(&w, "h", "a", &val),
        "both b1,b2 true: h must NOT be at-risk (clean q)"
    );
}

#[test]
fn f1c_all_paths_poisoned_by_different_premises() {
    // p1,p2 ratified; b1,b2 proposed. r1: h:-p1,not b1  r2: h:-p2,not b2  (NO clean path)
    let mut w = World::new();
    ratified(&mut w, "p1", "a", "d-p1");
    ratified(&mut w, "p2", "a", "d-p2");
    proposed(&mut w, "b1", "a", "b1-var");
    proposed(&mut w, "b2", "a", "b2-var");
    w.define_rule(
        "r1",
        patom("h", "x"),
        vec![pos(patom("p1", "x")), not(patom("b1", "x"))],
    )
    .unwrap();
    w.define_rule(
        "r2",
        patom("h", "x"),
        vec![pos(patom("p2", "x")), not(patom("b2", "x"))],
    )
    .unwrap();
    let w = w.fixpoint().unwrap();
    let b1 = var_atom(&w, "b1-var");
    let b2 = var_atom(&w, "b2-var");

    // Ambient (both open): certain but AT RISK (every certain path poisoned).
    assert_eq!(
        status_at(&w, "h", "a", &w.valuation),
        Status::Certain,
        "ambient: certain"
    );
    assert!(
        at_risk_at(&w, "h", "a", &w.valuation),
        "ambient: at risk (both paths poisoned)"
    );

    // b1 true, b2 open: p1 path defeated, p2 path still certain-but-poisoned.
    let mut v1 = w.valuation.clone();
    v1.set(b1, true);
    assert_eq!(
        status_at(&w, "h", "a", &v1),
        Status::Certain,
        "b1 true: still certain via p2"
    );
    assert!(
        at_risk_at(&w, "h", "a", &v1),
        "b1 true: p2 path still poisoned -> at risk"
    );

    // b1 AND b2 true: ALL paths defeated -> Possible.
    let mut v2 = w.valuation.clone();
    v2.set(b1, true);
    v2.set(b2, true);
    assert_eq!(
        status_at(&w, "h", "a", &v2),
        Status::Possible,
        "both poisons fired: h must fall to Possible"
    );
    assert!(
        !at_risk_at(&w, "h", "a", &v2),
        "at_risk is false unless Certain"
    );
}

// ---------------------------------------------------------------------------
// FINDING 2 — reversible block via the valuation seam (no re-fixpoint).
// ---------------------------------------------------------------------------

fn build_f2() -> World {
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    proposed(&mut w, "b", "a", "b-var");
    w.define_rule(
        "r",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    w
}

#[test]
fn f2_reversible_block_matches_fresh_fixpoint() {
    let base = build_f2().fixpoint().unwrap();
    let b = var_atom(&base, "b-var");

    // eval_under (no re-fixpoint):
    let mut block = base.valuation.clone();
    block.set(b, true);
    let mut unblock = base.valuation.clone();
    unblock.set(b, false);
    let s_block = status_at(&base, "h", "a", &block);
    let s_unblock = status_at(&base, "h", "a", &unblock);
    let r_unblock = at_risk_at(&base, "h", "a", &unblock);

    // fresh fixpoint b=true
    let mut wt = build_f2();
    let bt = wt.interner.intern("b-var");
    wt.valuation.set(bt, true);
    let ft = wt.fixpoint().unwrap();
    let s_fresh_block = status_at(&ft, "h", "a", &ft.valuation);

    // fresh fixpoint b=false
    let mut wf = build_f2();
    let bf = wf.interner.intern("b-var");
    wf.valuation.set(bf, false);
    let ff = wf.fixpoint().unwrap();
    let s_fresh_unblock = status_at(&ff, "h", "a", &ff.valuation);
    let r_fresh_unblock = at_risk_at(&ff, "h", "a", &ff.valuation);

    assert_eq!(
        s_block, s_fresh_block,
        "block: eval_under vs fresh fixpoint (b=true)"
    );
    assert_eq!(
        s_unblock, s_fresh_unblock,
        "unblock: eval_under vs fresh fixpoint (b=false)"
    );
    assert_eq!(s_block, Status::Possible, "b true blocks -> h Possible");
    assert_eq!(s_unblock, Status::Certain, "b false unblocks -> h Certain");
    assert!(
        !r_unblock,
        "b false: h robustly certain, not at risk (eval_under)"
    );
    assert_eq!(
        r_unblock, r_fresh_unblock,
        "unblock at_risk: eval_under vs fresh"
    );

    // Ambient (b open): certain AT RISK.
    assert_eq!(status_at(&base, "h", "a", &base.valuation), Status::Certain);
    assert!(
        at_risk_at(&base, "h", "a", &base.valuation),
        "b open: at risk"
    );
}

#[test]
fn f2_chain_negated_dependency() {
    // bb is itself negation-derived: bb:-s,not tt ; h:-p,not bb
    // s,p ratified; tt proposed.
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    ratified(&mut w, "s", "a", "d-s");
    proposed(&mut w, "tt", "a", "tt-var");
    w.define_rule(
        "rbb",
        patom("bb", "x"),
        vec![pos(patom("s", "x")), not(patom("tt", "x"))],
    )
    .unwrap();
    w.define_rule(
        "rh",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("bb", "x"))],
    )
    .unwrap();
    let w = w.fixpoint().unwrap();
    let tt = var_atom(&w, "tt-var");

    // Ambient (tt open): bb is certain-at-risk (s certain, poisoned by tt).
    //   => not bb: bb is certain, so h's p-path is poisoned/blocked -> h Possible.
    let s_bb = status_at(&w, "bb", "a", &w.valuation);
    let s_h = status_at(&w, "h", "a", &w.valuation);
    println!("chain ambient: bb={s_bb:?} h={s_h:?}");
    assert_eq!(
        s_bb,
        Status::Certain,
        "bb certain (s ratified), poisoned by tt"
    );

    // Rule tt TRUE -> bb defeated (Possible) -> not bb succeeds -> h Certain.
    let mut vt = w.valuation.clone();
    vt.set(tt, true);
    let s_bb_t = status_at(&w, "bb", "a", &vt);
    let s_h_t = status_at(&w, "h", "a", &vt);
    println!("chain tt=true: bb={s_bb_t:?} h={s_h_t:?}");
    assert_eq!(s_bb_t, Status::Possible, "tt true defeats bb");

    // Compare to a fresh fixpoint under tt=true.
    let mut w2 = World::new();
    ratified(&mut w2, "p", "a", "d-p");
    ratified(&mut w2, "s", "a", "d-s");
    proposed(&mut w2, "tt", "a", "tt-var");
    let tt2 = var_atom_after(&mut w2, "tt-var");
    w2.valuation.set(tt2, true);
    w2.define_rule(
        "rbb",
        patom("bb", "x"),
        vec![pos(patom("s", "x")), not(patom("tt", "x"))],
    )
    .unwrap();
    w2.define_rule(
        "rh",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("bb", "x"))],
    )
    .unwrap();
    let w2 = w2.fixpoint().unwrap();
    let s_h_fresh = status_at(&w2, "h", "a", &w2.valuation);
    println!("chain fresh tt=true: h={s_h_fresh:?}");
    assert_eq!(
        s_h_t, s_h_fresh,
        "chain: eval_under(tt=true) must match fresh fixpoint(tt=true)"
    );
}

fn var_atom_after(world: &mut World, name: &str) -> Atom {
    world.interner.intern(name)
}

// ---------------------------------------------------------------------------
// ATTACK — defeater staleness: a defeater condition that changes WITHOUT the
// support monomial set changing is not re-propagated to a downstream consumer,
// because seminaive re-enqueue is gated on SUPPORT change only.
// ---------------------------------------------------------------------------

#[test]
fn attack_defeater_staleness_downstream() {
    // Single shared decision var d-p. p(a) ratified.
    // A poisoned early via r_a1 (A:-p,not b), then re-derived CLEAN & LATE via
    // the chain A:-w:-v:-p on the SAME support monomial {+d-p}.
    // C:-A consumes A while poisoned, before A is cleaned.
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    proposed(&mut w, "b", "a", "b-var");
    w.define_rule(
        "r_a1",
        patom("A", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    w.define_rule("r_v", patom("v", "x"), vec![pos(patom("p", "x"))])
        .unwrap();
    w.define_rule("r_w", patom("w", "x"), vec![pos(patom("v", "x"))])
        .unwrap();
    w.define_rule("r_a2", patom("A", "x"), vec![pos(patom("w", "x"))])
        .unwrap();
    w.define_rule("r_c", patom("C", "x"), vec![pos(patom("A", "x"))])
        .unwrap();
    let w = w.fixpoint().unwrap();

    let a_row = row(&w, "A", "a").expect("A derived");
    let c_row = row(&w, "C", "a").expect("C derived");
    println!(
        "A support={:?} defeaters={:?}",
        a_row.support, a_row.defeaters
    );
    println!(
        "C support={:?} defeaters={:?}",
        c_row.support, c_row.defeaters
    );

    // A has a fully clean derivation (w<-v<-p, all positive ratified), so both
    // A and C are ROBUSTLY certain: not at-risk, no live defeater.
    assert_eq!(
        status_at(&w, "A", "a", &w.valuation),
        Status::Certain,
        "A certain"
    );
    assert!(
        !at_risk_at(&w, "A", "a", &w.valuation),
        "A has a clean derivation -> must NOT be at risk"
    );
    assert_eq!(
        status_at(&w, "C", "a", &w.valuation),
        Status::Certain,
        "C certain"
    );
    assert!(
        !at_risk_at(&w, "C", "a", &w.valuation),
        "C rests on robustly-certain A -> must NOT be at risk (stale defeater bug if it is)"
    );
    assert!(
        c_row.defeaters.is_false(),
        "C must carry NO defeater; stale {{+b-var}} indicates the seminaive re-enqueue bug"
    );
}

// ---------------------------------------------------------------------------
// FINDING 4 — conditioned absorption soundness.
// ---------------------------------------------------------------------------

#[test]
fn f4_absorption_keeps_needed_path_different_defeaters() {
    // Two derivations of h whose supports subsume ({+d-p} vs {+d-p,+d-q}) but
    // carry DIFFERENT defeaters, so the specific one must NOT be dropped.
    //   r1: h:-p,      not b1         support {+d-p},        defeater {+b1}
    //   r2: h:-p,q,    not b2         support {+d-p,+d-q},   defeater {+b2}
    // When b1 fires but b2 does not, only the r2 path keeps h certain, so the
    // subsumed r2 monomial must survive absorption.
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    ratified(&mut w, "q", "a", "d-q");
    proposed(&mut w, "b1", "a", "b1-var");
    proposed(&mut w, "b2", "a", "b2-var");
    w.define_rule(
        "r1",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b1", "x"))],
    )
    .unwrap();
    w.define_rule(
        "r2",
        patom("h", "x"),
        vec![
            pos(patom("p", "x")),
            pos(patom("q", "x")),
            not(patom("b2", "x")),
        ],
    )
    .unwrap();
    let w = w.fixpoint().unwrap();
    let b1 = var_atom(&w, "b1-var");
    let b2 = var_atom(&w, "b2-var");

    let h = row(&w, "h", "a").expect("h derived");
    println!("f4 h support monomials = {}", h.support.monomials().count());

    // b1 true, b2 open: r1 path defeated; r2 path ({+d-p,+d-q}) still certain,
    // defeater {+b2} not yet fired -> h Certain (would be WRONGLY Possible if
    // absorption dropped the subsumed r2 monomial).
    let mut v = w.valuation.clone();
    v.set(b1, true);
    assert_eq!(
        status_at(&w, "h", "a", &v),
        Status::Certain,
        "b1 fired but b2 not: subsumed r2 path must keep h certain"
    );
    assert!(
        at_risk_at(&w, "h", "a", &v),
        "only path left is poisoned by b2 -> at risk"
    );

    // b1 AND b2 true: both defeated -> Possible.
    let mut v2 = w.valuation.clone();
    v2.set(b1, true);
    v2.set(b2, true);
    assert_eq!(
        status_at(&w, "h", "a", &v2),
        Status::Possible,
        "both fired -> Possible"
    );
}

#[test]
fn f4_posbool_implies_soundness() {
    let m = |lits: &[Lit]| Monomial::from_lits(lits.iter().copied());
    let pb = |ms: &[Monomial]| PosBool(ms.iter().cloned().collect());
    let a0 = Lit::pos(Atom(0));
    let a1 = Lit::pos(Atom(1));
    let a2 = Lit::pos(Atom(2));

    // {+0,+1} => {+0}
    assert!(pb(&[m(&[a0, a1])]).implies(&pb(&[m(&[a0])])));
    // {+0} does NOT => {+0,+1}
    assert!(!pb(&[m(&[a0])]).implies(&pb(&[m(&[a0, a1])])));
    // FALSE => anything
    assert!(PosBool::FALSE.implies(&pb(&[m(&[a0])])));
    // {+0} => {+0}∨{+2}
    assert!(pb(&[m(&[a0])]).implies(&pb(&[m(&[a0]), m(&[a2])])));
    // Disjoint atoms: {+2} does NOT => {+0}
    assert!(!pb(&[m(&[a2])]).implies(&pb(&[m(&[a0])])));
    // Multi-monomial: {+0}∨{+1} => {+0}∨{+1}∨{+2}
    assert!(pb(&[m(&[a0]), m(&[a1])]).implies(&pb(&[m(&[a0]), m(&[a1]), m(&[a2])])));
    // {+0}∨{+2} does NOT => {+0}  (the {+2} disjunct escapes)
    assert!(!pb(&[m(&[a0]), m(&[a2])]).implies(&pb(&[m(&[a0])])));
    // anything => TRUE
    assert!(pb(&[m(&[a0, a1])]).implies(&PosBool::truth()));
    // TRUE does NOT => {+0}
    assert!(!PosBool::truth().implies(&pb(&[m(&[a0])])));
}

// ---------------------------------------------------------------------------
// FINDING 5 — status/at_risk exactness + serialization round-trip.
// ---------------------------------------------------------------------------

#[test]
fn f5_certain_poisoned_alongside_possible_clean_is_not_certain_via_wrong_path() {
    // One certain-but-poisoned monomial {+d-p} (defeater {+b}, b certain) and one
    // clean-but-merely-possible monomial {+o} (o open). Neither yields Certain:
    //   - the certain path is defeated (b ruled)
    //   - the clean path is only possible (o open)
    // Expected status: Possible; at_risk: false.
    let dp = Atom(0);
    let b = Atom(1);
    let o = Atom(2);
    let mut cond = DefeaterConditions::new();
    cond.set(Monomial::from_lits([Lit::pos(dp)]), PosBool::pos(b));
    let support = PosBool::pos(dp).or_(&PosBool::pos(o));
    let ann = Ann::conditioned(support, cond);

    let mut val = Valuation::new();
    val.set(dp, true); // certain path's support ruled
    val.set(b, true); // ...but its defeater has fired
    // o left open
    let theory = Theory::new();
    assert_eq!(
        ann.status(&val, &theory),
        Status::Possible,
        "certain path defeated + clean path only possible => Possible"
    );
    assert!(!ann.at_risk(&val, &theory), "not Certain => not at risk");

    // Now rule o true: clean path becomes certain -> Certain, not at risk.
    let mut val2 = val.clone();
    val2.set(o, true);
    assert_eq!(
        ann.status(&val2, &theory),
        Status::Certain,
        "o true: clean path carries it"
    );
    assert!(
        !ann.at_risk(&val2, &theory),
        "clean certain path => not at risk"
    );
}

#[test]
fn f5_serialization_round_trip_with_conditions() {
    let dp = Atom(3);
    let dq = Atom(4);
    let b = Atom(7);
    let mut cond = DefeaterConditions::new();
    cond.set(Monomial::from_lits([Lit::pos(dp)]), PosBool::pos(b));
    // dq monomial clean (not in map)
    let support = PosBool::pos(dp).or_(&PosBool::pos(dq));
    let ann = Ann::conditioned(support, cond);
    assert!(!ann.conditions.is_empty());

    let json = serde_json::to_string(&ann).unwrap();
    println!("serialized Ann = {json}");
    let back: Ann = serde_json::from_str(&json).unwrap();
    assert_eq!(
        ann, back,
        "Ann with non-empty conditions must round-trip exactly"
    );

    // Status must be identical before/after round-trip under a probing valuation.
    let mut val = Valuation::new();
    val.set(dp, true);
    val.set(b, true);
    let theory = Theory::new();
    // dp path defeated (b true), dq open -> Possible
    assert_eq!(ann.status(&val, &theory), back.status(&val, &theory));
    assert_eq!(ann.status(&val, &theory), Status::Possible);
}

// ---------------------------------------------------------------------------
// FINDING 6 — why_not names the blocking premise; flips at-risk <-> defeated.
// ---------------------------------------------------------------------------

#[test]
fn f6_why_not_defeated_names_blocker() {
    // h:-p,not b ; p ratified, b proposed. Rule b true at fixpoint -> h defeated.
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    proposed(&mut w, "b", "a", "b-var");
    w.define_rule(
        "r",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    let b = w.interner.intern("b-var");
    w.valuation.set(b, true); // b ruled certain at fixpoint
    let w = w.fixpoint().unwrap();

    // h must be PRESENT (derived) and Possible (defeated), not absent.
    let s = status_at(&w, "h", "a", &w.valuation);
    println!("f6 h status (b ruled) = {s:?}");
    assert_eq!(
        s,
        Status::Possible,
        "defeated conclusion must be present as Possible"
    );

    let wn = why_not(&w, &catom("h", "a"));
    println!("f6 why_not = {wn:?}");
    assert!(wn.known, "defeated h must be known/present");
    assert_eq!(wn.status, Status::Possible);
    assert!(wn.defeater.is_some(), "why_not must name a defeater");
    // The named defeater must be b-var.
    let named = w
        .interner
        .resolve(Atom(wn.defeater.unwrap().0))
        .unwrap_or("?");
    assert_eq!(
        named, "b-var",
        "why_not must NAME the blocking premise b-var"
    );
    assert!(
        wn.rendered
            .iter()
            .any(|l| l.contains("defeated by") && l.contains("b-var")),
        "rendered must say 'defeated by ... b-var', got {:?}",
        wn.rendered
    );
}

#[test]
fn f6_why_not_at_risk_verb_flip() {
    // Certain-at-risk case: h:-p,not b ; b OPEN -> h Certain but at risk.
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    proposed(&mut w, "b", "a", "b-var");
    w.define_rule(
        "r",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    let w = w.fixpoint().unwrap();

    assert_eq!(status_at(&w, "h", "a", &w.valuation), Status::Certain);
    assert!(
        at_risk_at(&w, "h", "a", &w.valuation),
        "b open -> h at risk"
    );
    let wn = why_not(&w, &catom("h", "a"));
    println!("f6 at-risk why_not = {wn:?}");
    assert_eq!(wn.status, Status::Certain);
    assert!(
        wn.rendered
            .iter()
            .any(|l| l.contains("at risk from") && l.contains("b-var")),
        "certain-at-risk must render 'at risk from b-var', got {:?}",
        wn.rendered
    );
}

#[test]
fn f6_why_not_robustly_certain_should_not_claim_at_risk() {
    // FINDING-1 shape with a clean q path: h is robustly certain (NOT at risk),
    // yet the atom-global defeater pool still holds {+b-var} from the p path.
    // why_not reads the atom-global defeater and status==Certain, so it may
    // WRONGLY render "at risk from b-var" for an atom at_risk() calls false.
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    ratified(&mut w, "q", "a", "d-q");
    proposed(&mut w, "b", "a", "b-var");
    w.define_rule(
        "r1",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    w.define_rule("r2", patom("h", "x"), vec![pos(patom("q", "x"))])
        .unwrap();
    let w = w.fixpoint().unwrap();

    assert_eq!(status_at(&w, "h", "a", &w.valuation), Status::Certain);
    let robustly_not_at_risk = !at_risk_at(&w, "h", "a", &w.valuation);
    assert!(robustly_not_at_risk, "clean q path -> at_risk() is false");

    let wn = why_not(&w, &catom("h", "a"));
    println!("f6 robust why_not = {wn:?}");
    // FIXED (defect a): why_not now gates the "at risk from" line on the per-monomial
    // `at_risk()` and blames the *conditioned* defeater (the poison riding on certain
    // monomials), never the atom-global pool. A robustly-certain atom — Certain via
    // the clean q path, `at_risk()` false — surfaces NO risk line and no blame.
    let claims_at_risk = wn.rendered.iter().any(|l| l.contains("at risk"));
    assert!(
        robustly_not_at_risk && !claims_at_risk,
        "robustly-certain atom must NOT render an at-risk line (rendered={:?})",
        wn.rendered
    );
    assert_eq!(
        wn.status,
        Status::Certain,
        "still certain via the clean q path"
    );
    assert_eq!(
        wn.defeater, None,
        "no blame to surface for a robustly-certain atom"
    );
}

// ---------------------------------------------------------------------------
// FINDING 3 — positive datalog carries ZERO defeaters/conditions.
// ---------------------------------------------------------------------------

#[test]
fn f3_transitive_closure_zero_defeaters() {
    // reach(x,y):-edge(x,y); reach(x,z):-edge(x,y),reach(y,z). No negation.
    let mut w = World::new();
    let edge = |from: &str, to: &str| GroundAtom {
        pred: "edge".to_owned(),
        args: vec![from.to_owned(), to.to_owned()],
    };
    let d = w.interner.intern("d-graph");
    w.valuation.set(d, true);
    for (a, b) in [("a", "b"), ("b", "c"), ("c", "d")] {
        w.assert_fact(edge(a, b), Ann::supported(PosBool::pos(d)));
    }
    let e2 = |p: &str, x: &str, y: &str| AtomExpr {
        pred: id(p),
        args: vec![var(x), var(y)],
    };
    w.define_rule(
        "base",
        e2("reach", "x", "y"),
        vec![pos(e2("edge", "x", "y"))],
    )
    .unwrap();
    w.define_rule(
        "trans",
        e2("reach", "x", "z"),
        vec![pos(e2("edge", "x", "y")), pos(e2("reach", "y", "z"))],
    )
    .unwrap();
    let w = w.fixpoint().unwrap();

    // Every derived row: empty conditions and FALSE atom-global defeaters.
    for d in &w.derived {
        assert!(
            d.conditions.is_empty(),
            "{} must carry no conditions",
            d.atom.render()
        );
        assert!(
            d.defeaters.is_false(),
            "{} must carry no defeaters",
            d.atom.render()
        );
    }
    // reach(a,d) exists and is Certain.
    let rad = GroundAtom {
        pred: "reach".to_owned(),
        args: vec!["a".to_owned(), "d".to_owned()],
    };
    let row = w
        .derived
        .iter()
        .find(|r| r.atom == rad)
        .expect("reach(a,d)");
    assert_eq!(row.ann().status(&w.valuation, &w.theory), Status::Certain);
    // Wire form omits the conditions field entirely for a positive-only row.
    let json = serde_json::to_string(row).unwrap();
    assert!(
        !json.contains("conditions"),
        "positive-only row must omit conditions: {json}"
    );
}

// ---------------------------------------------------------------------------
// FINDING 7 — stratification and range restriction still hold.
// ---------------------------------------------------------------------------

#[test]
fn f7_self_negation_unstratifiable() {
    let mut w = World::new();
    proposed(&mut w, "base", "a", "o");
    w.define_rule(
        "r",
        patom("p", "x"),
        vec![pos(patom("base", "x")), not(patom("p", "x"))],
    )
    .unwrap();
    let err = w.fixpoint().unwrap_err();
    println!("self-negation err = {err:?}");
    assert!(
        format!("{err:?}").to_lowercase().contains("stratif"),
        "self-negation must be Unstratifiable, got {err:?}"
    );
}

#[test]
fn f7_two_cycle_negation_unstratifiable() {
    let mut w = World::new();
    proposed(&mut w, "base", "a", "o");
    w.define_rule(
        "ra",
        patom("aa", "x"),
        vec![pos(patom("base", "x")), not(patom("bb", "x"))],
    )
    .unwrap();
    w.define_rule(
        "rb",
        patom("bb", "x"),
        vec![pos(patom("base", "x")), not(patom("aa", "x"))],
    )
    .unwrap();
    let err = w.fixpoint().unwrap_err();
    println!("2-cycle err = {err:?}");
    assert!(
        format!("{err:?}").to_lowercase().contains("stratif"),
        "neg 2-cycle unstratifiable"
    );
}

#[test]
fn f7_three_cycle_negation_unstratifiable() {
    let mut w = World::new();
    proposed(&mut w, "base", "a", "o");
    w.define_rule(
        "ra",
        patom("aa", "x"),
        vec![pos(patom("base", "x")), pos(patom("bb", "x"))],
    )
    .unwrap();
    w.define_rule(
        "rb",
        patom("bb", "x"),
        vec![pos(patom("base", "x")), pos(patom("cc", "x"))],
    )
    .unwrap();
    w.define_rule(
        "rc",
        patom("cc", "x"),
        vec![pos(patom("base", "x")), not(patom("aa", "x"))],
    )
    .unwrap();
    let err = w.fixpoint().unwrap_err();
    println!("3-cycle err = {err:?}");
    assert!(
        format!("{err:?}").to_lowercase().contains("stratif"),
        "neg 3-cycle unstratifiable"
    );
}

#[test]
fn f7_range_restriction_rejects_negation_only_var() {
    // y appears only under `not`; must be rejected at define_rule time.
    let mut w = World::new();
    let err = w
        .define_rule(
            "r",
            patom("h", "x"),
            vec![
                pos(patom("base", "x")),
                not(AtomExpr {
                    pred: id("b"),
                    args: vec![var("x"), var("y")],
                }),
            ],
        )
        .unwrap_err();
    println!("range err = {err:?}");
    assert!(
        format!("{err:?}").to_lowercase().contains("range"),
        "negation-only variable must fail range restriction, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// EDGE — negation poison is VALUATION-NEUTRAL, so a fork that re-rules a
// base-false variable true DOES re-block, commuting with a fresh fixpoint (§1).
// ---------------------------------------------------------------------------

#[test]
fn edge_poison_valuation_neutral_reblocks_under_fork() {
    // b rests on b-var, ruled FALSE in the base BEFORE the fixpoint.
    let mut w = World::new();
    ratified(&mut w, "p", "a", "d-p");
    proposed(&mut w, "b", "a", "b-var");
    let b = w.interner.intern("b-var");
    w.valuation.set(b, false); // ledger rules b's support out
    w.define_rule(
        "r",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    let w = w.fixpoint().unwrap();

    // Base: h Certain, clean (b dead).
    assert_eq!(status_at(&w, "h", "a", &w.valuation), Status::Certain);
    assert!(!at_risk_at(&w, "h", "a", &w.valuation));
    let h = row(&w, "h", "a").unwrap();
    println!("edge h defeaters(base b=false) = {:?}", h.defeaters);

    // eval_under re-supposing b-var TRUE (a conflicting fork). A fresh fixpoint
    // under b=true DEFEATS h (Possible); eval_under must too, because the poison is
    // valuation-neutral and was NOT filtered out at fixpoint time.
    let mut reblock = w.valuation.clone();
    reblock.set(b, true);
    let s_eval = status_at(&w, "h", "a", &reblock);

    let mut w2 = World::new();
    ratified(&mut w2, "p", "a", "d-p");
    proposed(&mut w2, "b", "a", "b-var");
    let b2 = w2.interner.intern("b-var");
    w2.valuation.set(b2, true);
    w2.define_rule(
        "r",
        patom("h", "x"),
        vec![pos(patom("p", "x")), not(patom("b", "x"))],
    )
    .unwrap();
    let w2 = w2.fixpoint().unwrap();
    let s_fresh = status_at(&w2, "h", "a", &w2.valuation);

    println!("edge: eval_under(b:false->true) h={s_eval:?}  fresh(b=true) h={s_fresh:?}");
    assert_eq!(
        s_fresh,
        Status::Possible,
        "fresh fixpoint under b=true defeats h"
    );
    // FIXED (defect b): the poison is valuation-neutral, so re-supposing b true in a
    // fork re-blocks h even though b was base-false at fixpoint time. eval_under now
    // commutes with a fresh fixpoint (spec §1).
    assert_eq!(
        s_eval,
        Status::Possible,
        "eval_under(b:false->true) must re-block h, matching a fresh fixpoint"
    );
    assert_eq!(
        s_eval, s_fresh,
        "eval_under must commute with a fresh fixpoint under the conflicting fork"
    );
}
