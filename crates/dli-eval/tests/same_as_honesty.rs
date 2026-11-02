#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]
//! WS-1: the identity kernel's evaluator-side honesty suite.
//!
//! `same-as` closes symmetric-transitive over synthesized Datalog rules
//! (`dli_eval::world::synthesize_alias_rules`) — no rewriting of any authored
//! or stored layer, so everything a merged copy needs (support conjunction,
//! defeat, grade rollup, negation) falls out of machinery this crate already
//! tests elsewhere. These probes pin that composition specifically for
//! identity: closure both directions, chains, cycles, self-merge, the
//! why-names-the-assertion property, defeat algebra, propose/suppose, a rule
//! joining across aliases, grade rollup, the zero-links frozen-contract guard,
//! conditioned-fact threading, and negation honesty under a merge.

use dli_ast::annot::{Annotation, CitationRef, CondExpr, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, RatifyDecl, SameAsDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::literal::BodyLiteral;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::ann::Status;
use dli_core::grade::Grade;
use dli_eval::query::Query;
use dli_eval::why::Why;
use dli_eval::world::World;

// --- helpers -----------------------------------------------------------------

fn id(t: &str) -> Ident {
    Ident::synthetic(t)
}

fn var(name: &str) -> Term {
    Term::Var(id(name))
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

fn proposed() -> Annotation {
    marker(ProvenanceMarker::Proposed, &[])
}

fn proposed_assumes(hyp: &str) -> Annotation {
    Annotation {
        marker: ProvenanceMarker::Proposed,
        citations: Vec::new(),
        condition: Some(CondExpr::Hyp(id(hyp))),
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratify_decl(d: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(d),
        body: dli_ast::language::RatifyBody::default(),
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

fn same_as(left: &str, right: &str, prov: Option<Annotation>) -> Decl {
    Decl::SameAs(SameAsDecl {
        left: id(left),
        right: id(right),
        provenance: prov,
    })
}

fn document(decls: Vec<Decl>) -> Document {
    Document {
        scope: Some(ScopeHeader {
            name: id("weir"),
            span: Span::synthetic(),
        }),
        decls,
    }
}

fn goal(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| Term::Sym(id(a))).collect(),
    }
}

fn status_of(world: &World, pred: &str, args: &[&str]) -> Status {
    let answer = world
        .query(&Query {
            goal: goal(pred, args),
        })
        .unwrap_or_else(|e| panic!("query {pred}({args:?}) failed: {e:?}"));
    answer
        .rows
        .first()
        .map_or(dli_core::ann::Status::Dead, |row| row.status)
}

// ===========================================================================
// 1. Closure runs BOTH directions — no goal rewriting needed.
// ===========================================================================
#[test]
fn closure_merges_both_directions() {
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        same_as("avalon", "the-isle", Some(ratified("D-1"))),
        fact("mist-bound", &["avalon"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert_eq!(
        status_of(&world, "mist-bound", &["avalon"]),
        Status::Certain
    );
    assert_eq!(
        status_of(&world, "mist-bound", &["the-isle"]),
        Status::Certain,
        "the rewrite rule projects the fact onto the alias"
    );
    assert_eq!(
        status_of(&world, "same-as", &["the-isle", "avalon"]),
        Status::Certain,
        "symmetry answers the reverse pair too"
    );
}

// ===========================================================================
// 2. Chained identity a=b=c closes transitively.
// ===========================================================================
#[test]
fn chained_identity_closes_transitively() {
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        same_as("charlie", "bob", Some(ratified("D-1"))),
        same_as("bob", "alice", Some(ratified("D-1"))),
        fact("rank", &["charlie", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    for name in ["charlie", "bob", "alice"] {
        assert_eq!(
            status_of(&world, "rank", &[name, "gold"]),
            Status::Certain,
            "{name} must inherit the chained identity's rank"
        );
    }
    assert_eq!(
        status_of(&world, "same-as", &["alice", "charlie"]),
        Status::Certain
    );
}

// ===========================================================================
// 3. A cycle a=b, b=a terminates (does not hang the fixpoint) and is a no-op
//    beyond the direct pair.
// ===========================================================================
#[test]
fn cycle_terminates_cleanly() {
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        same_as("a", "b", Some(ratified("D-1"))),
        same_as("b", "a", Some(ratified("D-1"))),
        fact("rank", &["a", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates — a cycle must not diverge the seminaive fixpoint");

    assert_eq!(status_of(&world, "rank", &["b", "gold"]), Status::Certain);
    assert_eq!(status_of(&world, "same-as", &["a", "b"]), Status::Certain);
    assert_eq!(status_of(&world, "same-as", &["b", "a"]), Status::Certain);
}

// ===========================================================================
// 4. Self-merge (a=a) is an idempotent no-op: derives nothing new.
// ===========================================================================
#[test]
fn self_merge_derives_nothing_new() {
    let with_self = World::lower(&document(vec![
        ratify_decl("D-1"),
        same_as("avalon", "avalon", Some(ratified("D-1"))),
        fact("mist-bound", &["avalon"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    let without = World::lower(&document(vec![
        ratify_decl("D-1"),
        fact("mist-bound", &["avalon"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    // The self-loop mints its own same-as(avalon, avalon) and internal
    // same-as:link(avalon, avalon) rows (harmless), but it must derive no
    // OTHER predicate's row the identity-free world lacks.
    let other_rows = |w: &World, skip_same_as: bool| -> Vec<String> {
        w.derived
            .iter()
            .filter(|d| {
                !skip_same_as || (d.atom.pred != "same-as" && d.atom.pred != "same-as:link")
            })
            .map(|d| d.atom.render())
            .collect()
    };
    assert_eq!(
        other_rows(&with_self, true),
        other_rows(&without, true),
        "a self-merge must not derive any new non-identity row"
    );
}

// ===========================================================================
// 5. `why` names the identity assertion: a merged copy's support includes the
//    same-as link's own citation, not only the fact it rode in on.
// ===========================================================================
#[test]
fn why_names_the_identity_assertion() {
    let world = World::lower(&document(vec![
        ratify_decl("D-2"),
        ratify_decl("D-9"),
        fact("rank", &["nara", "gold"], Some(ratified("D-2"))),
        same_as("nara", "moira", Some(ratified("D-9"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    let row = world
        .derived
        .iter()
        .find(|d| {
            d.atom.pred == "rank" && d.atom.args == vec!["moira".to_owned(), "gold".to_owned()]
        })
        .expect("the alias inherits the rank");
    let why = Why::of_support(&row.ann().support, &world.interner);
    let names: Vec<&str> = why.rendered.iter().map(String::as_str).collect();
    assert!(
        names.iter().any(|n| n.contains("D-2")),
        "the fact's own citation must still appear: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("D-9")),
        "the identity ruling's citation must be named too: {names:?}"
    );
}

// ===========================================================================
// 6. Defeat algebra: ruling the identity decision false kills the merged copy
//    algebraically (one valuation flip, no recompile) while the surface fact
//    is untouched.
// ===========================================================================
#[test]
fn defeat_kills_merged_copy_leaves_surface_fact() {
    let world = World::lower(&document(vec![
        ratify_decl("D-2"),
        ratify_decl("D-9"),
        fact("rank", &["nara", "gold"], Some(ratified("D-2"))),
        same_as("nara", "moira", Some(ratified("D-9"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert_eq!(
        status_of(&world, "rank", &["nara", "gold"]),
        Status::Certain
    );
    assert_eq!(
        status_of(&world, "rank", &["moira", "gold"]),
        Status::Certain
    );

    // Defeat: rule D-9 (the identity ruling) false. No re-fixpoint — the
    // ALREADY-SATURATED world is re-read under a mutated valuation.
    // `intern` is idempotent — since D-9 was interned during lowering, this
    // looks it up rather than minting anything new.
    let mut defeated = world.clone();
    let d9 = defeated.interner.intern("D-9");
    defeated.valuation.set(d9, false);

    assert_eq!(
        status_of(&defeated, "rank", &["nara", "gold"]),
        Status::Certain,
        "the surface fact never depended on the identity ruling"
    );
    assert_eq!(
        status_of(&defeated, "rank", &["moira", "gold"]),
        Status::Dead,
        "the merged copy's sole support monomial required D-9; the world un-merges"
    );
}

// ===========================================================================
// 7. A proposed identity's copies are Possible, never Certain — and a
//    fork-suppose (valuating the proposal variable true) flips them, again
//    with no re-fixpoint.
// ===========================================================================
#[test]
fn proposed_identity_never_certain_suppose_flips_it() {
    let world = World::lower(&document(vec![
        ratify_decl("D-2"),
        fact("rank", &["nara", "gold"], Some(ratified("D-2"))),
        same_as("nara", "moira", Some(proposed())),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert_eq!(
        status_of(&world, "rank", &["nara", "gold"]),
        Status::Certain
    );
    assert_eq!(
        status_of(&world, "rank", &["moira", "gold"]),
        Status::Possible,
        "a proposed identity's copy is possible-never-certain"
    );

    // The proposal variable is the only bare-proposal bundle minted here.
    let proposal_var = world
        .interner
        .names()
        .iter()
        .find(|n| n.starts_with("proposal#"))
        .cloned()
        .expect("one proposal bundle was minted for the bare `proposed` same-as");
    let mut supposed = world.clone();
    let proposal_atom = supposed.interner.intern(&proposal_var);
    supposed.valuation.set(proposal_atom, true);
    assert_eq!(
        status_of(&supposed, "rank", &["moira", "gold"]),
        Status::Certain,
        "supposing the proposal true flips the merged copy to certain — no recompile"
    );
}

// ===========================================================================
// 8. A rule joins correctly across an alias: both operands of a conjunction
//    become available under either name.
// ===========================================================================
#[test]
fn rule_join_across_aliases() {
    let mut world = World::lower(&document(vec![
        ratify_decl("D-1"),
        fact("sworn", &["nara"], Some(ratified("D-1"))),
        fact("armed", &["moira"], Some(ratified("D-1"))),
        same_as("nara", "moira", Some(ratified("D-1"))),
    ]))
    .expect("lowers");
    world
        .define_rule(
            "ready-rule",
            AtomExpr {
                pred: id("ready"),
                args: vec![var("x")],
            },
            vec![
                BodyLiteral::pos(AtomExpr {
                    pred: id("sworn"),
                    args: vec![var("x")],
                }),
                BodyLiteral::pos(AtomExpr {
                    pred: id("armed"),
                    args: vec![var("x")],
                }),
            ],
        )
        .expect("range-restricted");
    let world = world.fixpoint().expect("saturates");

    // Neither `sworn` nor `armed` alone names both bearers; only the identity
    // merge makes both hold under both names, so the conjunction fires for
    // both — this is the join-across-aliases case, not mere rewriting of one
    // predicate.
    assert_eq!(status_of(&world, "ready", &["nara"]), Status::Certain);
    assert_eq!(status_of(&world, "ready", &["moira"]), Status::Certain);
}

// ===========================================================================
// 9. Grade rollup is weakest-of-chain: a synthesized rule's own grade
//    (`Grade::Asserted`, hardcoded by `define_rule`) caps every copy it
//    produces, regardless of how strongly graded the underlying fact was.
// ===========================================================================
#[test]
fn grade_rollup_is_weakest_of_chain() {
    // Built directly (not via `World::lower`+`Decl::Fact`, which always grades
    // `Asserted`): both facts must exist BEFORE `synthesize_alias_rules` runs
    // its census, so the rewrite rule for `rank/2` is actually synthesized.
    // `assert_same_as` (not a raw `assert_fact`) is required — it is the ONLY
    // intake that registers the pair in the declaration channel; a raw fact
    // push would leave `same_as_links` empty and synthesize nothing.
    let mut world = World::new();
    world.assert_same_as("nara", "moira", dli_core::ann::Ann::unconditional());
    // A Checked-grade fact — the strongest grade on the lattice.
    world.assert_fact_graded(
        dli_eval::world::GroundAtom {
            pred: "rank".to_owned(),
            args: vec!["nara".to_owned(), "gold".to_owned()],
        },
        dli_core::ann::Ann::unconditional(),
        Grade::Checked,
    );
    dli_eval::world::synthesize_alias_rules(&mut world);
    let world = world.fixpoint().expect("saturates");

    let base = world
        .derived
        .iter()
        .find(|d| d.atom.pred == "rank" && d.atom.args[0] == "nara")
        .expect("base row");
    assert_eq!(
        base.grade,
        Grade::Checked,
        "the base fact keeps its own grade"
    );

    let copy = world
        .derived
        .iter()
        .find(|d| d.atom.pred == "rank" && d.atom.args[0] == "moira")
        .expect("merged copy");
    assert_eq!(
        copy.grade,
        Grade::Asserted,
        "the synthesized rewrite rule's own Asserted grade caps the merged copy"
    );
}

// ===========================================================================
// 10. Behavior neutrality (the frozen-contract guard): zero `same-as`
//     declarations synthesizes zero rules and is byte-identical to a world
//     that never called `synthesize_alias_rules` at all.
// ===========================================================================
#[test]
fn zero_links_synthesizes_nothing_frozen_guard() {
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        fact("rank", &["nara", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers");
    assert!(
        world.rules.is_empty(),
        "a world with zero same-as facts must synthesize zero rules: {:?}",
        world.rules
    );

    // Calling the synthesis entry point again explicitly, over the identical
    // world, must still be a no-op — the guard is on the FACT set, not on
    // whether the function was invoked.
    let mut same = world.clone();
    dli_eval::world::synthesize_alias_rules(&mut same);
    assert_eq!(
        serde_json::to_string(&same).expect("serializes"),
        serde_json::to_string(&world).expect("serializes"),
        "re-synthesizing over an identity-free world changes nothing"
    );
}

// ===========================================================================
// 11. Conditioned-fact threading: a hypothesis-conditioned proposal combines
//     correctly with a ledger-cited identity — the merged copy's support is
//     the conjunction, so it settles to certain only once BOTH the hypothesis
//     and the citation are ruled.
// ===========================================================================
#[test]
fn conditioned_fact_threads_through_the_merge() {
    let world = World::lower(&document(vec![
        ratify_decl("D-9"),
        fact(
            "rank",
            &["nara", "gold"],
            Some(proposed_assumes("h-nara-ranked")),
        ),
        same_as("nara", "moira", Some(ratified("D-9"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert_eq!(
        status_of(&world, "rank", &["moira", "gold"]),
        Status::Possible,
        "the hypothesis is not yet ruled, so the merged copy is only possible"
    );

    let mut ruled = world.clone();
    let h = ruled.interner.intern("h-nara-ranked");
    ruled.valuation.set(h, true);
    assert_eq!(
        status_of(&ruled, "rank", &["moira", "gold"]),
        Status::Certain,
        "once the hypothesis is ruled, the identity-supported copy settles certain"
    );
}

// ===========================================================================
// 12. Negation honesty: a rule negating a predicate that is absent for BOTH
//     the bearer and its alias derives cleanly under the merge, exactly as
//     the absent-negated-predicate case works with no identity involved —
//     the synthesized copies do not corrupt defeater-condition poisoning.
// ===========================================================================
#[test]
fn negation_honesty_under_a_merge() {
    let mut world = World::lower(&document(vec![
        ratify_decl("D-1"),
        fact("sworn", &["nara"], Some(ratified("D-1"))),
        same_as("nara", "moira", Some(ratified("D-1"))),
    ]))
    .expect("lowers");
    world
        .define_rule(
            "safe-rule",
            AtomExpr {
                pred: id("safe"),
                args: vec![var("x")],
            },
            vec![
                BodyLiteral::pos(AtomExpr {
                    pred: id("sworn"),
                    args: vec![var("x")],
                }),
                BodyLiteral::not(AtomExpr {
                    pred: id("danger"),
                    args: vec![var("x")],
                }),
            ],
        )
        .expect("range-restricted");
    let world = world.fixpoint().expect("saturates");

    // `danger` is never asserted for either name: an absent negated predicate
    // has support FALSE — nothing to poison (spec 02 §4) — for both aliases.
    assert_eq!(status_of(&world, "safe", &["nara"]), Status::Certain);
    assert_eq!(status_of(&world, "safe", &["moira"]), Status::Certain);

    let row = world
        .derived
        .iter()
        .find(|d| d.atom.pred == "safe" && d.atom.args[0] == "moira")
        .expect("merged copy of safe");
    assert!(
        row.defeaters.is_false(),
        "an absent negated predicate must not poison the merged copy either: {:?}",
        row.defeaters
    );
}

// ===========================================================================
// rt-1a FINDING 1 (permanent regression): a PLAIN `fact same-as(x, y)` must
// NOT drive the merge — DL-0078's ruling that identity is a declaration-only
// claim (koan 3), the exact asymmetry `Decl::Before`'s chronology edge
// already has against a plain `fact before(a, b)`. Grouped together so the
// negative and positive cases sit side by side: only the declaration form
// merges; the fact form, alone or in a world that also declares an unrelated
// identity, never does.
// ===========================================================================

#[test]
fn plain_same_as_fact_does_not_merge() {
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        fact("same-as", &["bob", "robert"], Some(ratified("D-1"))),
        fact("rank", &["bob", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert_eq!(status_of(&world, "rank", &["bob", "gold"]), Status::Certain);
    assert_eq!(
        status_of(&world, "rank", &["robert", "gold"]),
        Status::Dead,
        "a plain same-as FACT must not merge robert into bob's rank"
    );
    assert!(
        world.rules.is_empty(),
        "no declaration exists anywhere in this world, so no rule is synthesized: {:?}",
        world.rules
    );
}

#[test]
fn declaration_same_as_still_merges_the_positive_guard() {
    // The direct contrast to the case above: swap the plain fact for the
    // declaration form and the SAME rank fact now DOES reach the alias.
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        same_as("bob", "robert", Some(ratified("D-1"))),
        fact("rank", &["bob", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert_eq!(status_of(&world, "rank", &["bob", "gold"]), Status::Certain);
    assert_eq!(
        status_of(&world, "rank", &["robert", "gold"]),
        Status::Certain,
        "the `same-as` DECLARATION does merge — this is the positive guard against over-correcting FINDING 1"
    );
}

#[test]
fn plain_same_as_fact_does_not_ride_along_with_an_unrelated_declaration() {
    // The deeper version of FINDING 1: a plain fact for one pair must stay
    // inert even when a GENUINE declaration for a DIFFERENT pair exists
    // elsewhere and DOES trigger rule synthesis. If the closure rules joined
    // against the public `same-as` predicate directly (rather than the
    // reserved, declaration-only `same-as:link`), the mere presence of ANY
    // synthesized rule would sweep this unrelated plain fact in too.
    let world = World::lower(&document(vec![
        ratify_decl("D-1"),
        same_as("charlie", "chuck", Some(ratified("D-1"))), // triggers synthesis
        fact("same-as", &["bob", "robert"], Some(ratified("D-1"))), // unrelated plain fact
        fact("rank", &["bob", "gold"], Some(ratified("D-1"))),
    ]))
    .expect("lowers")
    .fixpoint()
    .expect("saturates");

    assert!(
        !world.rules.is_empty(),
        "the genuine declaration DOES synthesize rules"
    );
    assert_eq!(status_of(&world, "rank", &["bob", "gold"]), Status::Certain);
    assert_eq!(
        status_of(&world, "rank", &["robert", "gold"]),
        Status::Dead,
        "an unrelated plain same-as fact must stay inert even while real synthesis is active"
    );
}
