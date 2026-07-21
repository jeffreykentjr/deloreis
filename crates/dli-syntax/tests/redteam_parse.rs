#![allow(clippy::unwrap_used, clippy::expect_used)]
//! ADVERSARIAL red-team probes of the rule-clause parser (DL-0020).
//!
//! `not` handling, separators, nullary atoms, and the koan-4 "nothing silently
//! dropped" contract (trailing junk / checker bodies must yield None, never a
//! quietly truncated rule).

use dli_ast::clause::Clause;
use dli_ast::span::Span;
use dli_syntax::parse_rule_clause;

fn clause(text: &str) -> Clause {
    Clause {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

#[test]
fn extra_whitespace_around_not_is_irrelevant() {
    // The clause is re-lexed, so spacing does not matter.
    let (_, body) = parse_rule_clause(&clause("h($x) :-   p($x)  ,   not     b($x)")).unwrap();
    assert_eq!(body.len(), 2);
    assert!(!body[0].negated);
    assert!(body[1].negated);
    assert_eq!(body[1].atom.pred.text, "b");
}

#[test]
fn double_negation_is_rejected_not_folded() {
    // `not not b`: the first `not` is the polarity keyword; the second `not` is
    // then read as a predicate atom, leaving `b` as trailing junk → None.
    // (Double negation is NOT silently collapsed to a positive literal.)
    assert!(parse_rule_clause(&clause("p :- not not b")).is_none());
}

#[test]
fn negated_nullary_atom_parses() {
    // `not flooded` — a propositional negated literal.
    let (head, body) = parse_rule_clause(&clause("dry :- not flooded")).unwrap();
    assert_eq!(head.pred.text, "dry");
    assert!(head.args.is_empty());
    assert_eq!(body.len(), 1);
    assert!(body[0].negated);
    assert_eq!(body[0].atom.pred.text, "flooded");
    assert!(body[0].atom.args.is_empty());
}

#[test]
fn both_separators_accept_negation() {
    assert!(parse_rule_clause(&clause("p :- not q")).is_some());
    assert!(parse_rule_clause(&clause("p : not q")).is_some());
}

#[test]
fn several_negated_literals_all_flip() {
    let (_, body) = parse_rule_clause(&clause("h($x) :- p($x), not b($x), not c($x)")).unwrap();
    assert_eq!(body.len(), 3);
    assert!(!body[0].negated);
    assert!(body[1].negated && body[1].atom.pred.text == "b");
    assert!(body[2].negated && body[2].atom.pred.text == "c");
}

#[test]
fn dangling_not_is_rejected() {
    // `not` with no atom after it is not a clean rule.
    assert!(parse_rule_clause(&clause("p :- not")).is_none());
    assert!(parse_rule_clause(&clause("p :- not , q")).is_none());
}

#[test]
fn trailing_junk_after_a_negated_body_is_rejected() {
    // Koan 4: an unconsumed tail means this is not a plain datalog rule.
    assert!(parse_rule_clause(&clause("h($x) :- p($x), not b($x) leftover")).is_none());
    assert!(parse_rule_clause(&clause("h($x) :- p($x) requires slot >= 1")).is_none());
}

#[test]
fn not_written_as_a_function_call_does_not_parse() {
    // `not(x)` is read as the keyword `not` + a bare `(` which is not an atom.
    // Documented boundary: a predicate literally named `not` is not expressible.
    assert!(parse_rule_clause(&clause("p :- not(x)")).is_none());
}

#[test]
fn a_plain_atom_is_not_a_rule() {
    assert!(parse_rule_clause(&clause("adjacent(a, b)")).is_none());
}
