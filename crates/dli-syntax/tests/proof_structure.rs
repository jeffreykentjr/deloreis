//! LS-9: the front end is the single producer of the typed proof interior.
//!
//! `structure_proof_body` turns each captured proof line into a typed
//! [`ProofOp`], computing the parse-dependent pieces the ledger checker used to
//! recompute at check time. A recognized keyword whose arguments fail the strict
//! per-line grammar becomes [`ProofOp::Malformed`] (kept, never dropped), and a
//! line opening with no keyword gets no op — the two the structural validator
//! refuses, replacing the retired `validate_proof_decl` format-and-reparse.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_ast::clause::Clause;
use dli_ast::proof::{ProofOp, ProofStep, ResourceKeyword};
use dli_ast::span::Span;
use dli_syntax::structure_proof_body;

fn clauses(lines: &[&str]) -> Vec<Clause> {
    lines
        .iter()
        .map(|line| Clause {
            text: (*line).to_owned(),
            span: Span::synthetic(),
        })
        .collect()
}

#[test]
fn a_valid_proof_body_lowers_to_typed_ops() {
    let body = structure_proof_body(&clauses(&[
        "premise p1 holds(linet, consent(household, door)) cited D-0113",
        "premise p2 phase(tide, slack) from-scene",
        "premise p3 debt(1) drawn-against p1",
        "step s1 instantiate ward-form with ground = door, fibre = withy",
        "step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)",
        "step s3 read north-gate",
        "spend p3",
        "close bound(ward, door) expect holds",
    ]));

    // Every line is a recognized, well-formed operation.
    assert!(body.items.iter().all(|item| item.op.is_some()));

    let Some(ProofOp::Premise(p1)) = &body.items[0].op else {
        panic!("p1 is a premise")
    };
    assert_eq!(p1.label, "p1");
    assert_eq!(p1.atom, "holds(linet, consent(household, door))");
    assert_eq!(p1.mods, vec!["cited", "D-0113"]);

    // The phase premise pre-parses its atom so the timing classifier reads the
    // predicate without re-entering the parser.
    let Some(ProofOp::Premise(p2)) = &body.items[1].op else {
        panic!("p2 is a premise")
    };
    assert!(
        p2.phase_eq.is_none(),
        "phase(tide, slack) is the two-arg form, not the `phase(c) = s` comparison"
    );
    assert_eq!(p2.atom_expr.as_ref().unwrap().pred.text, "phase");

    // A resource draw pre-parses to its predicate and integer argument.
    let Some(ProofOp::Premise(p3)) = &body.items[2].op else {
        panic!("p3 is a premise")
    };
    assert_eq!(p3.atom_expr.as_ref().unwrap().pred.text, "debt");

    let Some(ProofOp::Step(ProofStep::Instantiate { form, bindings, .. })) = &body.items[3].op
    else {
        panic!("s1 is an instantiate step")
    };
    assert_eq!(form, "ward-form");
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].name, "ground");
    assert_eq!(bindings[0].value, "door");

    let Some(ProofOp::Step(ProofStep::Chain {
        joins,
        law,
        conclusion,
        ..
    })) = &body.items[4].op
    else {
        panic!("s2 is a chain step")
    };
    assert_eq!(joins, &["p1", "s1"]);
    assert_eq!(law.as_deref(), Some("give-and-take"));
    assert_eq!(conclusion.as_deref(), Some("bound(ward, door)"));

    assert!(matches!(
        &body.items[5].op,
        Some(ProofOp::Step(ProofStep::Custom { operation, .. })) if operation == "read"
    ));
    assert!(matches!(
        &body.items[6].op,
        Some(ProofOp::Resource(r)) if r.keyword == ResourceKeyword::Spend && r.label == "p3"
    ));
    assert!(matches!(
        &body.items[7].op,
        Some(ProofOp::Close(c)) if c.atom == "bound(ward, door)" && c.verdict.as_deref() == Some("holds")
    ));
}

#[test]
fn a_malformed_operation_is_captured_not_coerced() {
    // Each is a recognized keyword whose arguments fail the strict grammar.
    let body = structure_proof_body(&clauses(&[
        "step s1 instantiate ward-form with ground = door, junk, fibre = withy",
        "spend p1 trailing",
        "assume p1 holds(a) checked",
        "not a proof keyword",
    ]));

    assert!(
        matches!(&body.items[0].op, Some(ProofOp::Malformed(m)) if m.keyword == "step"),
        "a malformed instantiate is kept as Malformed, not coerced to a valid step"
    );
    assert!(
        matches!(&body.items[1].op, Some(ProofOp::Malformed(m)) if m.keyword == "spend"),
        "a spend with a trailing token is Malformed"
    );
    assert!(
        matches!(&body.items[2].op, Some(ProofOp::Malformed(m)) if m.keyword == "assume"),
        "an assumption claiming a promoted grade is Malformed"
    );
    assert!(
        body.items[3].op.is_none(),
        "a line opening with no proof keyword carries no op"
    );
}
