#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Order-rule coverage: the linear-scan checker, the `over order` compiler, and
//! the standing-aware precedence read (carry-item R2-G).

use dli_ast::decl::Decl;
use dli_core::ann::{Ann, Theory};
use dli_core::atom::Atom;
use dli_core::posbool::PosBool;
use dli_core::valuation::Valuation;
use dli_syntax::parse_document;
use dli_time::chronology::{Chronology, TimePoint};
use dli_time::order_rules::{
    OrderFindingKind, OrderRule, OrderRuleBody, SlotGuard, SlotRequirement, SlotView,
    StandingOrder, check, precedes_with_standing,
};

fn compile(source: &str) -> OrderRule {
    let (doc, diags) = parse_document(source);
    assert!(
        !diags.iter().any(|d| d.class.is_error()),
        "parse errors: {diags:?}"
    );
    let Decl::Rule(rule) = &doc.decls[0] else {
        panic!("expected a rule decl");
    };
    OrderRule::compile(rule).expect("order rule compiles")
}

// --- the linear-scan checker -------------------------------------------------

#[test]
fn full_team_floor_flags_a_team_assembled_too_early() {
    let slots = vec![
        SlotView::new("e5", 5),
        SlotView::new("e8", 8).with_configuration("full-team"),
        SlotView::new("e12", 12).with_configuration("full-team"),
    ];
    let rule = OrderRule::guarded(
        "full-team-floor",
        SlotGuard::ListenerHasConfiguration("full-team".to_owned()),
        SlotRequirement::SlotAtLeast(10),
    );
    let findings = check(&slots, &[rule]);
    // e8 is too early; e12 is fine.
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].episode, "e8");
    assert_eq!(findings[0].kind, OrderFindingKind::SlotTooEarly);
}

#[test]
fn a_slot_ceiling_flags_a_slot_placed_too_late() {
    let slots = vec![SlotView::new("late", 20).with_configuration("cold-open")];
    let rule = OrderRule::guarded(
        "cold-open-early",
        SlotGuard::ListenerHasConfiguration("cold-open".to_owned()),
        SlotRequirement::SlotAtMost(3),
    );
    let findings = check(&slots, &[rule]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, OrderFindingKind::SlotTooLate);
}

#[test]
fn no_deep_time_act_one_flags_by_act_and_reports_unknown_acts() {
    let slots = vec![
        SlotView::new("act1-deep", 2)
            .with_act(1)
            .learns_during("elder-cycles"),
        SlotView::new("act3-deep", 30)
            .with_act(3)
            .learns_during("elder-cycles"),
        SlotView::new("unknown-act", 40).learns_during("elder-cycles"),
    ];
    let rule = OrderRule::guarded(
        "no-deep-time-act-one",
        SlotGuard::ListenerLearnsDuringEra("elder-cycles".to_owned()),
        SlotRequirement::ActAtLeast(2),
    );
    let findings = check(&slots, &[rule]);
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].episode, "act1-deep");
    assert_eq!(findings[0].kind, OrderFindingKind::ActTooEarly);
    // An episode whose act is unknown is indeterminate, not a silent pass.
    assert_eq!(findings[1].episode, "unknown-act");
    assert_eq!(findings[1].kind, OrderFindingKind::ActIndeterminate);
}

#[test]
fn payoff_after_reveal_flags_a_payoff_with_no_prior_reveal() {
    let slots = vec![
        // The mask is paid off at slot 2, but only revealed at slot 5 — too late.
        SlotView::new("e1", 1).with_reveal("the-well"),
        SlotView::new("e2", 2)
            .with_payoff("the-mask")
            .with_payoff("the-well"),
        SlotView::new("e5", 5).with_reveal("the-mask"),
    ];
    let rule = OrderRule::payoff_after_reveal("payoff-after-reveal");
    let findings = check(&slots, &[rule]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].episode, "e2");
    assert_eq!(findings[0].kind, OrderFindingKind::MissingPriorReveal);
    assert!(findings[0].message.contains("the-mask"));
}

#[test]
fn payoff_after_reveal_allows_setup_and_payoff_in_one_episode() {
    // A promise set up and paid off inside the same episode is legal (<=).
    let slots = vec![
        SlotView::new("e3", 3)
            .with_reveal("the-knot")
            .with_payoff("the-knot"),
    ];
    let findings = check(&slots, &[OrderRule::payoff_after_reveal("par")]);
    assert!(findings.is_empty());
}

// --- the `over order` compiler -----------------------------------------------

#[test]
fn compiles_the_three_canonical_forms() {
    let floor = compile(
        "rule full-team-floor over order ratified d-0162\n\
         \x20 : listener-has(configuration full-team) requires slot >= 10\n",
    );
    assert_eq!(
        floor.body,
        OrderRuleBody::Guarded {
            guard: SlotGuard::ListenerHasConfiguration("full-team".to_owned()),
            requirement: SlotRequirement::SlotAtLeast(10),
        }
    );

    // `act > 1` normalizes to the inclusive `act >= 2`.
    let deep = compile(
        "rule no-deep-time-act-one over order ratified d-0168\n\
         \x20 : listener-learns(during elder-cycles _) requires act > 1\n",
    );
    assert_eq!(
        deep.body,
        OrderRuleBody::Guarded {
            guard: SlotGuard::ListenerLearnsDuringEra("elder-cycles".to_owned()),
            requirement: SlotRequirement::ActAtLeast(2),
        }
    );

    let par = compile(
        "rule payoff-after-reveal over order\n\
         \x20 : payoff($p) at slot $j requires reveal($p) at slot $i, $i < $j\n",
    );
    assert_eq!(par.body, OrderRuleBody::PayoffAfterReveal);
}

#[test]
fn compile_rejects_a_non_order_rule_and_a_bodyless_rule() {
    // A `over season` rule is not an order rule.
    let (doc, _diags) = parse_document("rule r over season severity error\n  : beat $b\n");
    let Decl::Rule(season) = &doc.decls[0] else {
        panic!("expected a rule");
    };
    assert!(OrderRule::compile(season).is_err());

    // An order rule with no body clause cannot compile.
    let (doc, _diags) = parse_document("rule r over order ratified d-1\n");
    let Decl::Rule(bare) = &doc.decls[0] else {
        panic!("expected a rule");
    };
    assert!(OrderRule::compile(bare).is_err());
}

#[test]
fn compile_rejects_an_unknown_guard_and_a_non_numeric_bound() {
    let (doc, _diags) = parse_document(
        "rule r over order\n  : mystery-guard(configuration x) requires slot >= 3\n",
    );
    let Decl::Rule(bad_guard) = &doc.decls[0] else {
        panic!("expected a rule");
    };
    assert!(OrderRule::compile(bad_guard).is_err());

    let (doc, _diags) = parse_document(
        "rule r over order\n  : listener-has(configuration x) requires slot >= many\n",
    );
    let Decl::Rule(bad_bound) = &doc.decls[0] else {
        panic!("expected a rule");
    };
    assert!(OrderRule::compile(bad_bound).is_err());
}

// --- standing-aware precedence (carry-item R2-G) -----------------------------

/// A proposed annotation: support rests on an open event, so it reads `Possible`.
fn proposed(event: u32) -> Ann {
    Ann::supported(PosBool::pos(Atom(event)))
}

#[test]
fn a_proposed_before_fact_is_only_possibly_before_never_hard_before() {
    // breaking(0) -> tithe(1), declared by a merely-proposed before-fact.
    let chrono = Chronology {
        edges: vec![(TimePoint(0), TimePoint(1))],
    };
    let nu = Valuation::new();
    let theory = Theory::new();

    let verdict = precedes_with_standing(
        &chrono,
        &[proposed(9)],
        &nu,
        &theory,
        TimePoint(0),
        TimePoint(1),
    );
    assert_eq!(verdict, StandingOrder::PossiblyBefore);
    // The reverse query is PossiblyAfter, not hard After.
    let reverse = precedes_with_standing(
        &chrono,
        &[proposed(9)],
        &nu,
        &theory,
        TimePoint(1),
        TimePoint(0),
    );
    assert_eq!(reverse, StandingOrder::PossiblyAfter);
}

#[test]
fn a_ratified_before_fact_is_certainly_before() {
    let chrono = Chronology {
        edges: vec![(TimePoint(0), TimePoint(1))],
    };
    let verdict = precedes_with_standing(
        &chrono,
        &[Ann::unconditional()],
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(1),
    );
    assert_eq!(verdict, StandingOrder::CertainlyBefore);
}

#[test]
fn a_missing_annotation_reads_as_a_certain_edge() {
    // Passing `&[]` (no rulings to hand) treats every edge as declared/certain,
    // matching the standing-blind Chronology::precedes.
    let chrono = Chronology {
        edges: vec![(TimePoint(0), TimePoint(1))],
    };
    let verdict = precedes_with_standing(
        &chrono,
        &[],
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(1),
    );
    assert_eq!(verdict, StandingOrder::CertainlyBefore);
}

#[test]
fn a_certain_chain_through_a_proposed_link_is_only_possibly_before() {
    // 0 ->(certain) 1 ->(proposed) 2. The only path from 0 to 2 rests on the
    // proposed link, so the order is possible, never certain.
    let chrono = Chronology {
        edges: vec![(TimePoint(0), TimePoint(1)), (TimePoint(1), TimePoint(2))],
    };
    let anns = vec![Ann::unconditional(), proposed(9)];
    let verdict = precedes_with_standing(
        &chrono,
        &anns,
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(2),
    );
    assert_eq!(verdict, StandingOrder::PossiblyBefore);
    // But 0 -> 1 alone is fully certain.
    let near = precedes_with_standing(
        &chrono,
        &anns,
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(1),
    );
    assert_eq!(near, StandingOrder::CertainlyBefore);
}

#[test]
fn a_dead_before_fact_orders_nothing() {
    // A defeated before-fact (support identically false) is dropped from both
    // readings: the pair is unordered.
    let chrono = Chronology {
        edges: vec![(TimePoint(0), TimePoint(1))],
    };
    let dead = Ann::supported(PosBool::FALSE);
    let verdict = precedes_with_standing(
        &chrono,
        &[dead],
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(1),
    );
    assert_eq!(verdict, StandingOrder::Unordered);
}

#[test]
fn an_undeclared_pair_is_unordered_under_uncertainty() {
    let chrono = Chronology {
        edges: vec![(TimePoint(0), TimePoint(1))],
    };
    let verdict = precedes_with_standing(
        &chrono,
        &[Ann::unconditional()],
        &Valuation::new(),
        &Theory::new(),
        TimePoint(0),
        TimePoint(2),
    );
    assert_eq!(verdict, StandingOrder::Unordered);
}
