#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the resource ledger.

use dli_ast::ident::Ident;
use dli_core::grade::Grade;
use dli_ledger::practice::compile_practices;
use dli_ledger::resource::ResourceKind;
use dli_ledger::{Delta, LedgerEntry, check_ledger};
use dli_syntax::parse_document;

#[test]
fn resource_kind_round_trips() {
    for kind in [
        ResourceKind::Linear { conserved: true },
        ResourceKind::Linear { conserved: false },
        ResourceKind::AffineBounded,
        ResourceKind::GradedMonotone,
    ] {
        let json = serde_json::to_string(&kind).unwrap();
        let back: ResourceKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, back);
    }
}

fn delta(resource: &str, amount: i64) -> Delta {
    Delta {
        resource: Ident::synthetic(resource),
        amount,
    }
}

/// A symbolic affine cap cannot be checked until a scene supplies its value.
#[test]
fn symbolic_cap_is_deferred_at_the_ledger_layer() {
    let source = "scope lighthouse\npractice beacon proposed {\n  resource light : affine bounded lamp-cap\n  cost material\n}\n";
    let (doc, diags) = parse_document(source);
    assert!(diags.is_empty(), "{diags:?}");
    let practices = compile_practices(&doc);
    let entry = LedgerEntry {
        practice: Ident::synthetic("beacon"),
        deltas: vec![delta("light", 10)],
    };
    let report = check_ledger(&practices, &[entry]);
    assert!(report.violations.is_empty(), "{:?}", report.violations);
}

/// Over a practice
/// whose laws are all concrete, `check_ledger` flags every algebra breach and
/// passes every lawful ledger — the object-level replay SIM-1's battery consumes.
#[test]
fn check_ledger_flags_each_concrete_law_breach() {
    let src = "\
scope magic
practice concrete proposed {
  resource debt : linear conserved
  resource strain : affine bounded 2
  resource acquaintance : graded monotone whole > worn > husk
  cost material
}
";
    let (doc, diags) = parse_document(src);
    assert!(diags.is_empty(), "{diags:?}");
    let practices = compile_practices(&doc);

    // A lawful ledger: debt revealed and released (net zero), strain within the
    // bound of two, acquaintance only wearing down. No violation.
    let lawful = LedgerEntry {
        practice: Ident::synthetic("concrete"),
        deltas: vec![
            delta("debt", 1),
            delta("debt", -1),
            delta("strain", 2),
            delta("acquaintance", -1),
        ],
    };
    assert!(check_ledger(&practices, &[lawful]).violations.is_empty());

    // Conservation broken (debt minted), the affine cap overspread, and a graded
    // grade moving against its decay: three breaches, three entries.
    let broken = [
        LedgerEntry {
            practice: Ident::synthetic("concrete"),
            deltas: vec![delta("debt", 1)], // minted, never balanced
        },
        LedgerEntry {
            practice: Ident::synthetic("concrete"),
            deltas: vec![delta("strain", 3)], // over the cap of two
        },
        LedgerEntry {
            practice: Ident::synthetic("concrete"),
            deltas: vec![delta("acquaintance", 1)], // wears the wrong way
        },
    ];
    let report = check_ledger(&practices, &broken);
    assert_eq!(report.violations.len(), 3, "got: {:?}", report.violations);
    // Violations are tagged with their entry index and graded `checked`.
    assert_eq!(report.violations[0].entry, 0);
    assert_eq!(report.violations[1].entry, 1);
    assert_eq!(report.violations[2].entry, 2);
    assert!(report.violations.iter().all(|v| v.grade == Grade::Checked));
}
