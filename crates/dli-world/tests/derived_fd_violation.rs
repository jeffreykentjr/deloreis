#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `continuity.derived-fd-violation` (WS-2, amending DL-0075): a functional-
//! dependency break the author's own check/materialize loop would otherwise
//! never see, because `continuity.fd-violation` reads authored atoms
//! pre-saturation (DL-0075) while a wire-authored rule's own derivations
//! exist only in the saturated layer.
//!
//! Four probes: a rule-derived second value is flagged only once saturated
//! (never at the source layer, which cannot see it); an authored-only break
//! is flagged exactly once — by `continuity.fd-violation`, never doubled by
//! the derived check too; and the identity-interaction case WS-2 calls for
//! by name — a derived break reachable only through an identity-closure copy
//! clears the moment the identity assertion is not certain, the same
//! algebraic un-merge `dli-world/tests/identity_merge.rs` already pins for
//! the source layer.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::clause::Clause;
use dli_ast::decl::{
    Decl, Document, FactDecl, FunctionalDecl, RatifyDecl, RuleDecl, SameAsDecl, ScopeHeader,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_world::check::{self, CompiledWorld};
use dli_world::compile;
use dli_world::pack::Pack;

// --- builders (mirrors fd_violation.rs / identity_merge.rs) -----------------

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn marker(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        bundle: None,
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

fn ratify_decl(decision: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(decision),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn atom(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| Term::Sym(id(a))).collect(),
    }
}

fn certain_fact(pred: &str, subject: &str, value: &str, cite: &str) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, &[subject, value]),
        triple: false,
        provenance: Some(ratified(cite)),
    })
}

fn functional(pred: &str, cite: &str) -> Decl {
    Decl::Functional(FunctionalDecl {
        pred: id(pred),
        provenance: Some(ratified(cite)),
    })
}

fn same_as(left: &str, right: &str, prov: Option<Annotation>) -> Decl {
    Decl::SameAs(SameAsDecl {
        left: id(left),
        right: id(right),
        provenance: prov,
    })
}

/// Structure a one-clause world rule body from its `head :- body` text — the
/// same convenience `dli-world/tests/loader.rs` uses; `dli-syntax` is an
/// unconditional dev-dependency of this crate (LS-7 gates only the shipped
/// artifacts' dependency graph, never test tooling).
fn rule_body(text: &str) -> dli_ast::rule::RuleBody {
    let clauses = vec![Clause {
        text: text.to_owned(),
        span: Span::synthetic(),
    }];
    dli_syntax::structure_rule_body(&clauses, &id("rule"), &[])
}

fn rule(name: &str, datalog_text: &str, cite: &str) -> Decl {
    Decl::Rule(RuleDecl {
        name: id(name),
        domain: None,
        head: Vec::new(),
        body: rule_body(datalog_text),
        provenance: Some(ratified(cite)),
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

/// Compile a world whose ledger already declares `D-1`, so every `ratified`
/// annotation above reads certain.
fn world(decls: Vec<Decl>) -> CompiledWorld {
    let mut all = vec![ratify_decl("D-1")];
    all.extend(decls);
    compile(&pack("fleet", all)).expect("compiles")
}

fn messages(world: &CompiledWorld, code: &str) -> Vec<String> {
    check::run(world)
        .into_iter()
        .filter(|f| f.code == code)
        .map(|f| f.message)
        .collect()
}

// --- a rule-only break is invisible at the source layer ----------------------

#[test]
fn a_rule_derived_second_value_is_flagged_only_at_the_derived_layer() {
    let w = world(vec![
        functional("rank", "D-1"),
        certain_fact("rank", "nara", "gold", "D-1"),
        rule(
            "gold-implies-silver",
            "rank($x, silver) :- rank($x, gold)",
            "D-1",
        ),
    ]);
    assert!(
        messages(&w, "continuity.fd-violation").is_empty(),
        "the source (pre-saturation) layer holds exactly one authored rank for nara"
    );
    let derived = messages(&w, "continuity.derived-fd-violation");
    assert_eq!(derived.len(), 1, "{derived:?}");
    assert!(
        derived[0].contains("rank")
            && derived[0].contains("nara")
            && derived[0].contains("gold")
            && derived[0].contains("silver"),
        "names the slot and both values: {}",
        derived[0]
    );
}

#[test]
fn a_rule_deriving_no_new_value_raises_nothing() {
    // The rule fires (nara's own gold restates itself as a "gold" derivation
    // via a trivial self-echo) but produces no SECOND distinct value.
    let w = world(vec![
        functional("rank", "D-1"),
        certain_fact("rank", "nara", "gold", "D-1"),
        rule("echo", "echoed-rank($x, $y) :- rank($x, $y)", "D-1"),
    ]);
    assert!(messages(&w, "continuity.fd-violation").is_empty());
    assert!(
        messages(&w, "continuity.derived-fd-violation").is_empty(),
        "no second value exists on the functional predicate itself"
    );
}

// --- no double report ---------------------------------------------------------

#[test]
fn a_source_only_break_is_reported_once_never_doubled() {
    let w = world(vec![
        functional("rank", "D-1"),
        certain_fact("rank", "nara", "captain", "D-1"),
        certain_fact("rank", "nara", "admiral", "D-1"),
    ]);
    let source = messages(&w, "continuity.fd-violation");
    assert_eq!(source.len(), 1, "{source:?}");
    assert!(
        messages(&w, "continuity.derived-fd-violation").is_empty(),
        "the same (pred, subject) break is excluded from the derived lens — \
         the two checks partition, they never double-report the same break"
    );
}

// --- the identity-interaction case (WS-2, named explicitly) ------------------

/// A derived break reachable ONLY through an identity-closure copy: `moira`
/// alone authors the functional predicate's one certain value (`gold`);
/// `nara` alone carries an unrelated marker fact a rule turns into a
/// SEPARATE, second value (`silver`) — on `nara`, never on `moira`. Neither
/// subject holds two values by itself; the break exists only once `same-as`
/// merges them into one canonical subject, so it appears if and only if the
/// identity link is certain.
fn closure_world(same_as_provenance: Option<Annotation>) -> CompiledWorld {
    world(vec![
        functional("rank", "D-1"),
        same_as("nara", "moira", same_as_provenance),
        certain_fact("rank", "moira", "gold", "D-1"),
        certain_fact("special", "nara", "yes", "D-1"),
        rule(
            "special-implies-silver",
            "rank($x, silver) :- special($x, yes)",
            "D-1",
        ),
    ])
}

#[test]
fn a_closure_copy_derived_break_appears_under_a_certain_identity() {
    let w = closure_world(Some(ratified("D-1")));
    assert!(
        messages(&w, "continuity.fd-violation").is_empty(),
        "the only authored rank value is moira's single 'gold' fact"
    );
    let derived = messages(&w, "continuity.derived-fd-violation");
    assert_eq!(derived.len(), 1, "{derived:?}");
    assert!(
        derived[0].contains("rank") && derived[0].contains("gold") && derived[0].contains("silver"),
        "{}",
        derived[0]
    );
}

#[test]
fn the_closure_copy_break_clears_when_identity_is_not_certain() {
    // A merely-proposed same-as never merges (identity_merge.rs pins this for
    // the source layer directly); the derived lens must agree — the closure
    // rules are structurally synthesized regardless, but every copy they
    // produce carries the link's own (here non-certain) annotation, so none
    // of them reads Certain and the derived break never appears. This is the
    // same algebraic mechanism a real `author.defeat` on the citing decision
    // exercises: flip the link's standing false and every merged consequence
    // — including a derived-fd-violation like this one — goes dead with no
    // re-fixpoint.
    let w = closure_world(Some(proposed()));
    assert!(messages(&w, "continuity.fd-violation").is_empty());
    assert!(
        messages(&w, "continuity.derived-fd-violation").is_empty(),
        "the closure copy that manufactured the break is not certain, so the break is not certain either"
    );
}
