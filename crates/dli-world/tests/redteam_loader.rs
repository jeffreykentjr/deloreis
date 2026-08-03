#![allow(clippy::unwrap_used, clippy::expect_used)]
//! ADVERSARIAL red-team probes at the loader/compile surface (DL-0020): the
//! headline soundness defect reproduced end-to-end through real `.dli`
//! declarations, plus the "nothing silently lowered" boundary for checker and
//! requires-style bodies.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, Document, FactDecl, RatifyDecl, RuleDecl, RuleDomain, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Status;
use dli_core::atom::Atom;
use dli_eval::Query;
use dli_world::{Pack, compile};

fn id(name: &str) -> Ident {
    Ident::synthetic(name)
}

fn ann(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn ratify(d: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(d),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn fact(pred: &str, arg: &str, provenance: Annotation) -> Decl {
    Decl::Fact(FactDecl {
        atom: AtomExpr {
            pred: id(pred),
            args: vec![Term::Sym(id(arg))],
        },
        triple: false,
        provenance: Some(provenance),
    })
}

fn rule(name: &str, domain: Option<RuleDomain>, body_text: &str) -> Decl {
    let clause = Clause {
        text: body_text.to_owned(),
        span: Span::synthetic(),
    };
    Decl::Rule(RuleDecl {
        name: id(name),
        domain,
        head: Vec::new(),
        // Structure the captured clause exactly as the front end does (LS-9).
        body: dli_syntax::structure_rule_body(std::slice::from_ref(&clause), &id(name), &[]),
        provenance: Some(ann(ProvenanceMarker::Ratified, &["D-1"])),
    })
}

fn pack(decls: Vec<Decl>) -> Pack {
    Pack {
        name: "redteam".to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id("redteam"),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

// ---------------------------------------------------------------------------
// HEADLINE SOUNDNESS FIX, END TO END: a conclusion with an independent,
// undefeated, ratified support is NOT demoted by an unrelated poison —
// reproduced through parse + lower + compile from real declarations. The
// defeater is conditioned per support monomial (`docs/spec/02-semantics.md §4`),
// so the clean q-path keeps h Certain even after b is ruled true.
// ---------------------------------------------------------------------------

#[test]
fn independent_support_is_not_defeated_by_unrelated_poison_through_compile() {
    let compiled = compile(&pack(vec![
        ratify("D-1"),
        ratify("D-2"),
        fact("p", "a", ann(ProvenanceMarker::Ratified, &["D-1"])),
        fact("q", "a", ann(ProvenanceMarker::Ratified, &["D-2"])),
        fact("b", "a", ann(ProvenanceMarker::Proposed, &[])),
        // r1 is defeasible (poisons with b); r2 is a clean independent path.
        rule("r1", None, "h($x) :- p($x), not b($x)"),
        rule("r2", None, "h($x) :- q($x)"),
    ]))
    .expect("compiles + stratifies");

    let world = &compiled.model().world;
    // h(a) is present and Certain under the ledger (b open).
    let h_ans = world
        .query(&Query {
            goal: AtomExpr {
                pred: id("h"),
                args: vec![Term::Sym(id("a"))],
            },
        })
        .unwrap();
    assert_eq!(h_ans.rows.len(), 1);
    assert_eq!(h_ans.rows[0].status, Status::Certain);

    // Find b(a)'s support variable (its single proposal event var).
    let b_row = world
        .derived
        .iter()
        .find(|d| d.atom.pred == "b" && d.atom.args == ["a"])
        .expect("b(a) present");
    let b_var: Atom = b_row
        .support
        .monomials()
        .next()
        .unwrap()
        .lits()
        .next()
        .unwrap()
        .atom;

    // Rule b true (a would-be ratification) and re-evaluate h(a) WITHOUT any
    // re-fixpoint. The clean q-path (ratified D-2) is untouched, so h(a) is
    // STILL certainly true — the per-monomial defeater conditioning keeps it.
    let h = world
        .derived
        .iter()
        .find(|d| d.atom.pred == "h" && d.atom.args == ["a"])
        .expect("h(a) derived");
    let ann_h = h.ann();
    let mut nu = world.valuation.clone();
    nu.set(b_var, true);
    assert_eq!(
        ann_h.status(&nu, &world.theory),
        Status::Certain,
        "FIXED (end-to-end): the independent ratified q-path (D-2) has no defeater, \
         so ruling the unrelated b true does not demote h"
    );
    // And it is not merely at-risk: the clean q-path makes it robustly certain.
    assert!(
        !ann_h.at_risk(&nu, &world.theory),
        "an independent clean certain support is not at-risk"
    );
}

// ---------------------------------------------------------------------------
// A well-formed negated world rule lowers and evaluates through compile.
// ---------------------------------------------------------------------------

#[test]
fn negated_world_rule_evaluates_through_compile() {
    let compiled = compile(&pack(vec![
        ratify("D-1"),
        fact("seated", "oak", ann(ProvenanceMarker::Ratified, &["D-1"])),
        fact("breached", "oak", ann(ProvenanceMarker::Proposed, &[])),
        rule(
            "ward",
            None,
            "ward-holds($g) :- seated($g), not breached($g)",
        ),
    ]))
    .expect("compiles");
    let world = &compiled.model().world;
    let ans = world
        .query(&Query {
            goal: AtomExpr {
                pred: id("ward-holds"),
                args: vec![Term::Sym(id("oak"))],
            },
        })
        .unwrap();
    assert_eq!(
        ans.rows.len(),
        1,
        "ward-holds(oak) is derived (possible breach)"
    );
    assert_eq!(ans.rows[0].status, Status::Certain, "certain-at-risk");
}

// ---------------------------------------------------------------------------
// Boundary: checker `over season`/`over order` bodies lower no world rule, while
// a bare top-level `requires` body is an explicit load finding.
// ---------------------------------------------------------------------------

#[test]
fn over_domain_rules_are_preserved_and_bare_requires_is_refused() {
    let compiled = compile(&pack(vec![
        ratify("D-1"),
        rule(
            "season-rule",
            Some(RuleDomain::Season),
            "held($e) :- listener-has($e)",
        ),
        rule(
            "order-rule",
            Some(RuleDomain::Order),
            "held($e) :- listener-has($e)",
        ),
        // A bare rule whose body is a checker `requires` clause: not datalog.
        rule("checker", None, "beat $b requires slot >= 10"),
    ]))
    .expect("load findings are reported without crashing compilation");
    assert_eq!(
        compiled.model().world.rules.len(),
        0,
        "no world rule lowers from checker/over-domain bodies: {:?}",
        compiled.model().world.rules
    );
    assert!(
        compiled
            .model()
            .findings
            .iter()
            .any(|finding| finding.code() == "load.malformed-world-rule"),
        "the bare requires-style body must not disappear"
    );
}
