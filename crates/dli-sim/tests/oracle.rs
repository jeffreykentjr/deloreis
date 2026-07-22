//! The SIM-1 oracle, end to end.
//!
//! Each test pins one clause of the brief's oracle: determinism, resume
//! equivalence, an invariant violation caught over a breakable conservation, a
//! metric's counts accurate on a known distribution, and the grade-honesty
//! distinction (metrics `tested`, invariants `checked` — a metric never
//! `checked`). Fixtures are built the honest way: parse a `.dli` practice and
//! compile it through the frozen PRAC-1 surface.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use dli_core::Grade;
use dli_ledger::practice::{PracticeDef, check_practice, compile_practices};
use dli_sim::battery::{Battery, GenBounds, Invariant, InvariantSpec, Metric, Observable};
use dli_sim::checkpoint::resume;
use dli_syntax::parse_document;

/// A fixture practice exercising all four members of the resource algebra.
fn all_algebra_practice() -> PracticeDef {
    let (doc, diags) = parse_document(
        "scope magic\n\
         practice spoken-proof proposed {\n\
           resource debt : linear conserved\n\
           resource pool : linear\n\
           resource strain : affine bounded 5\n\
           resource acquaintance : graded monotone whole > worn > husk\n\
           cost epistemic | bodily\n\
         }\n",
    );
    assert!(diags.is_empty(), "fixture must parse: {diags:?}");
    let mut practices = compile_practices(&doc);
    let practice = practices.remove(0);
    assert!(
        check_practice(&practice).is_empty(),
        "fixture must be well-formed: {:?}",
        check_practice(&practice)
    );
    practice
}

/// A battery over the fixture with both a metric and both a holding and a
/// breaking invariant.
fn battery(seed: u64, trials: u64, bounds: GenBounds) -> Battery {
    Battery {
        practice: all_algebra_practice(),
        bounds,
        seed,
        trials,
        metrics: vec![
            Metric {
                name: "debt-breach-rate".to_owned(),
                observable: Observable::Violated {
                    resource: "debt".to_owned(),
                },
            },
            Metric {
                name: "debt-closing-balance".to_owned(),
                observable: Observable::ClosingBalance {
                    resource: "debt".to_owned(),
                },
            },
            Metric {
                name: "violations-per-trial".to_owned(),
                observable: Observable::ViolationCount,
            },
        ],
        invariants: vec![
            // Breakable: an unconstrained generator unbalances a conserved multiset.
            InvariantSpec {
                name: "debt-always-conserved".to_owned(),
                predicate: Invariant::NeverViolated {
                    resource: "debt".to_owned(),
                },
            },
            // Always holds: the closing balance cannot exceed the drawn extent.
            InvariantSpec {
                name: "debt-balance-bounded".to_owned(),
                predicate: Invariant::BalanceAtMost {
                    resource: "debt".to_owned(),
                    bound: i64::from(bounds.max_steps) * i64::from(bounds.max_magnitude),
                },
            },
        ],
        affine_caps: BTreeMap::new(),
    }
}

#[test]
fn determinism_same_seed_same_manifest() {
    let b = battery(
        20_260_720,
        200,
        GenBounds {
            max_steps: 4,
            max_magnitude: 3,
        },
    );
    let a = b.run();
    let c = b.run();
    assert_eq!(a.canonical_bytes(), c.canonical_bytes());
    assert_eq!(a.digest(), c.digest());

    // A different seed produces a different run (the seed is a real input).
    let d = battery(
        1,
        200,
        GenBounds {
            max_steps: 4,
            max_magnitude: 3,
        },
    )
    .run();
    assert_ne!(a.canonical_bytes(), d.canonical_bytes());
}

#[test]
fn resume_equivalence_byte_identical() {
    let b = battery(
        7,
        250,
        GenBounds {
            max_steps: 4,
            max_magnitude: 3,
        },
    );

    // Straight-through, checkpointing every 10 trials.
    let (straight, snapshots) = b.run_checkpointing(10);

    // Interrupt at the first incomplete checkpoint, then resume to the end.
    let mid = snapshots
        .iter()
        .find(|c| !c.is_complete())
        .expect("a mid-run checkpoint");
    assert!(mid.trials_done > 0 && mid.trials_done < b.trials);
    let resumed = resume(&b, mid);

    // The resumed manifest is byte-identical to straight-through — the whole
    // point of the checkpoint contract.
    assert_eq!(straight.canonical_bytes(), resumed.canonical_bytes());

    // A plain run and a single-boundary checkpointed run also agree.
    assert_eq!(
        b.run().canonical_bytes(),
        b.run_checkpointing(b.trials).0.canonical_bytes()
    );

    // Checkpointing never changes the analytical result, only the lineage.
    let plain = b.run();
    assert_eq!(plain.metrics, straight.metrics);
    assert_eq!(plain.invariants, straight.invariants);
}

#[test]
fn zero_trial_runs_are_byte_identical() {
    // The trials=0 boundary is where the three commit paths diverged: run()
    // committed lineage [0] while run_checkpointing(0) and a resume of the
    // initial checkpoint committed []. A zero-trial battery crosses no boundary,
    // so all three must agree byte-for-byte.
    let b = battery(
        3,
        0,
        GenBounds {
            max_steps: 4,
            max_magnitude: 3,
        },
    );

    let plain = b.run();
    assert!(
        plain.checkpoint_lineage.is_empty(),
        "a zero-trial run crosses no boundary: {:?}",
        plain.checkpoint_lineage
    );

    let (checkpointed, snapshots) = b.run_checkpointing(0);
    assert!(snapshots.is_empty(), "no boundary, no snapshot");
    assert_eq!(plain.canonical_bytes(), checkpointed.canonical_bytes());

    // Resuming the initial (already-complete) checkpoint lands on the same bytes.
    let initial = dli_sim::checkpoint::Checkpoint::initial(&b, Some(1));
    let resumed = resume(&b, &initial);
    assert_eq!(plain.canonical_bytes(), resumed.canonical_bytes());

    // And the empty manifest is still honest.
    assert!(plain.grades_are_honest());
}

#[test]
fn resume_rejects_a_foreign_checkpoint() {
    let a = battery(
        1,
        40,
        GenBounds {
            max_steps: 3,
            max_magnitude: 2,
        },
    );
    let (_, snaps) = a.run_checkpointing(10);
    let mid = snaps.iter().find(|c| !c.is_complete()).unwrap().clone();

    // A battery with a different seed has a different digest; resuming a's state
    // into it must fail loudly, never silently corrupt the run.
    let other = battery(
        2,
        40,
        GenBounds {
            max_steps: 3,
            max_magnitude: 2,
        },
    );
    let result = std::panic::catch_unwind(|| resume(&other, &mid));
    assert!(result.is_err(), "a foreign checkpoint must be rejected");
}

#[test]
fn invariant_violation_caught_with_a_real_witness() {
    let b = battery(
        20_260_720,
        200,
        GenBounds {
            max_steps: 4,
            max_magnitude: 3,
        },
    );
    let manifest = b.run();

    let conserved = &manifest.invariants[0];
    assert_eq!(conserved.invariant, "debt-always-conserved");
    assert_eq!(conserved.grade(), Grade::Checked);

    // The conserved multiset breaks under some drawn sequence — caught, with a
    // witnessing trial.
    let dli_sim::InvariantOutcome::Violated { trial, .. } = &conserved.outcome else {
        panic!(
            "expected a caught conservation breach, got {:?}",
            conserved.outcome
        );
    };
    let witness = *trial;

    // The witness is real: re-derive that exact trial and confirm debt violates.
    let outcome = b.account_trial(&b.generate_trial(witness));
    assert!(outcome.resource("debt").unwrap().violated());

    // And it is the *earliest* breach: every earlier trial upheld conservation.
    for earlier in 0..witness {
        let o = b.account_trial(&b.generate_trial(earlier));
        assert!(
            !o.resource("debt").unwrap().violated(),
            "trial {earlier} broke conservation before the reported witness {witness}"
        );
    }

    // The paired bound invariant holds (checked-true) at the same grade.
    let bounded = &manifest.invariants[1];
    assert_eq!(bounded.invariant, "debt-balance-bounded");
    assert_eq!(bounded.grade(), Grade::Checked);
    assert!(bounded.held());
}

#[test]
fn metric_counts_accurate_by_independent_tally() {
    let b = battery(
        99,
        300,
        GenBounds {
            max_steps: 5,
            max_magnitude: 4,
        },
    );
    let manifest = b.run();

    // Independently replay the same deterministic trials and tally each metric,
    // then assert the manifest's stored counts match exactly.
    for metric in &b.metrics {
        let (mut trials, mut sum, mut sum_sq) = (0u64, 0i64, 0i64);
        for i in 0..b.trials {
            let outcome = b.account_trial(&b.generate_trial(i));
            if let Some(v) = metric.observable.observe(&outcome) {
                trials += 1;
                sum += v;
                sum_sq += v * v;
            }
        }
        let est = manifest
            .metrics
            .iter()
            .find(|m| m.metric == metric.name)
            .unwrap();
        assert_eq!(est.trials, trials, "{} trials", metric.name);
        assert_eq!(est.sum, sum, "{} sum", metric.name);
        assert_eq!(est.sum_of_squares, sum_sq, "{} sum_of_squares", metric.name);
    }
}

#[test]
fn metric_exact_on_a_known_degenerate_distribution() {
    // max_steps = 0 → every trial draws an empty scene: no deltas, no spend, no
    // balance, no violation. Every count is then exactly known.
    let b = battery(
        5,
        50,
        GenBounds {
            max_steps: 0,
            max_magnitude: 9,
        },
    );
    let manifest = b.run();

    let breach_rate = &manifest.metrics[0]; // Violated{debt}
    assert_eq!(breach_rate.trials, 50);
    assert_eq!(breach_rate.sum, 0);
    assert!(breach_rate.mean.abs() < 1e-12);

    let per_trial = &manifest.metrics[2]; // ViolationCount
    assert_eq!(per_trial.sum, 0);

    // With no deltas, a conserved multiset trivially balances: the invariant
    // holds (checked-true).
    assert!(manifest.invariants[0].held());

    // A second known-exact case: all-zero deltas (magnitude 0) also uphold every
    // law, so AllLawsHold holds.
    let (doc, _) = parse_document(
        "scope magic\n\
         practice p proposed {\n\
           resource debt : linear conserved\n\
           resource pool : linear\n\
           cost epistemic\n\
         }\n",
    );
    let zero_battery = Battery {
        practice: compile_practices(&doc).remove(0),
        bounds: GenBounds {
            max_steps: 6,
            max_magnitude: 0,
        },
        seed: 3,
        trials: 30,
        metrics: vec![Metric {
            name: "any".to_owned(),
            observable: Observable::AnyViolation,
        }],
        invariants: vec![InvariantSpec {
            name: "all-laws-hold".to_owned(),
            predicate: Invariant::AllLawsHold,
        }],
        affine_caps: BTreeMap::new(),
    };
    let zm = zero_battery.run();
    assert_eq!(zm.metrics[0].sum, 0);
    assert!(zm.invariants[0].held());
}

#[test]
fn grades_are_honest_metrics_tested_invariants_checked() {
    let b = battery(
        11,
        120,
        GenBounds {
            max_steps: 4,
            max_magnitude: 3,
        },
    );
    let manifest = b.run();

    assert!(manifest.grades_are_honest());

    // Every metric is tested — and, load-bearing, never checked.
    for m in &manifest.metrics {
        assert_eq!(m.grade(), Grade::Tested);
        assert_ne!(
            m.grade(),
            Grade::Checked,
            "a metric must NEVER report checked"
        );
    }
    // Every invariant is checked — and never tested.
    for i in &manifest.invariants {
        assert_eq!(i.grade(), Grade::Checked);
        assert_ne!(i.grade(), Grade::Tested);
    }

    // The type-level constants are the single source of the grades.
    assert_eq!(dli_sim::MetricEstimate::GRADE, Grade::Tested);
    assert_eq!(dli_sim::InvariantVerdict::GRADE, Grade::Checked);
}

#[test]
fn battery_validate_flags_undeclared_resources() {
    let mut b = battery(
        1,
        10,
        GenBounds {
            max_steps: 2,
            max_magnitude: 2,
        },
    );
    assert!(b.validate().is_empty(), "the base battery is well-formed");

    b.metrics.push(Metric {
        name: "ghost".to_owned(),
        observable: Observable::Violated {
            resource: "nonexistent".to_owned(),
        },
    });
    let findings = b.validate();
    assert_eq!(findings.len(), 1);
    assert!(findings[0].contains("nonexistent"));
}

/// The simulator's own per-trial accounting agrees, resource for resource, with
/// PRAC-2's `check_ledger` on the domain the two share (linear, conserved,
/// graded, and integer-cap affine). Both consume the same frozen `check_*`
/// primitives, so this pins that the two consumers cannot silently drift. The
/// fixture's affine cap is the integer `5`, so no symbolic-cap deferral (which
/// `check_ledger` cannot resolve without a scene) splits the two paths.
#[test]
fn accounting_agrees_with_check_ledger() {
    use dli_ledger::{LedgerEntry, check_ledger};

    let b = battery(
        20_260_720,
        150,
        GenBounds {
            max_steps: 4,
            max_magnitude: 3,
        },
    );
    let practices = [b.practice.clone()];

    for i in 0..b.trials {
        let state = b.generate_trial(i);
        let outcome = b.account_trial(&state);

        // One ledger entry per resource, in declaration order, so an entry index
        // identifies its resource in the report.
        let entries: Vec<LedgerEntry> = state
            .resources
            .iter()
            .map(|draw| LedgerEntry {
                practice: b.practice.name.clone(),
                deltas: draw.deltas.clone(),
            })
            .collect();
        let report = check_ledger(&practices, &entries);
        let flagged: std::collections::BTreeSet<usize> =
            report.violations.iter().map(|v| v.entry).collect();

        for (idx, res) in outcome.resources.iter().enumerate() {
            assert_eq!(
                res.violated(),
                flagged.contains(&idx),
                "trial {i}, resource '{}': direct accounting and check_ledger disagree",
                res.resource,
            );
        }
    }
}

/// F3 honest-surfacing: an `affine bounded <symbolic-cap>` law whose cap never
/// resolves to a number is *deferred* — it was never enforced — so its
/// `never-violated` sweep must report deferred, NOT a held `[checked]` pass that
/// would forge a verified grade. A conserved (`debt`-style) law in the same
/// battery stays honestly graded, and binding the cap turns the same law
/// genuinely checkable. The manifest remains grade-honest throughout.
#[test]
fn an_unresolved_affine_cap_law_is_deferred_not_held() {
    let (doc, diags) = parse_document(
        "scope magic\n\
         practice oath-binding proposed {\n\
           resource boon : linear conserved\n\
           resource strain : affine bounded body-limit\n\
           cost bodily\n\
         }\n",
    );
    assert!(diags.is_empty(), "fixture parses: {diags:?}");
    let practice = compile_practices(&doc).remove(0);

    let invariants = vec![
        InvariantSpec {
            name: "boon-never-violated".to_owned(),
            predicate: Invariant::NeverViolated {
                resource: "boon".to_owned(),
            },
        },
        InvariantSpec {
            name: "strain-never-violated".to_owned(),
            predicate: Invariant::NeverViolated {
                resource: "strain".to_owned(),
            },
        },
    ];
    let base = Battery {
        practice,
        bounds: GenBounds {
            max_steps: 6,
            max_magnitude: 3,
        },
        seed: 7,
        trials: 200,
        metrics: Vec::new(),
        invariants,
        affine_caps: BTreeMap::new(),
    };

    // validate() names the unresolvable cap as a configuration finding.
    let findings = base.validate();
    assert!(
        findings
            .iter()
            .any(|f| f.contains("strain") && f.contains("cap")),
        "validate must flag the unresolvable affine cap: {findings:?}"
    );

    let manifest = base.run();
    // The two-kind honesty oracle still holds: a deferred outcome is not a forged
    // grade — it is an honest "nothing was swept".
    assert!(manifest.grades_are_honest(), "the manifest is still honest");

    let strain = manifest
        .invariants
        .iter()
        .find(|v| v.invariant == "strain-never-violated")
        .expect("the strain sweep is present");
    assert!(
        strain.deferred(),
        "strain's symbolic-cap law is deferred, got {:?}",
        strain.outcome
    );
    assert!(!strain.held(), "a deferred law never counts as held");
    let dli_sim::InvariantOutcome::Deferred { reason } = &strain.outcome else {
        panic!("expected Deferred, got {:?}", strain.outcome);
    };
    assert!(
        reason.contains("did not resolve"),
        "the reason explains the unresolved cap: {reason}"
    );

    // The conserved law in the same run is genuinely graded — never deferred.
    let boon = manifest
        .invariants
        .iter()
        .find(|v| v.invariant == "boon-never-violated")
        .expect("the boon sweep is present");
    assert!(
        !boon.deferred(),
        "the conserved boon law is genuinely checked, not deferred"
    );
    assert_eq!(boon.grade(), Grade::Checked);

    // Binding the cap resolves the law: no config finding, and the same sweep is
    // now genuinely checkable rather than deferred.
    let mut capped = base.clone();
    capped.affine_caps = BTreeMap::from([("strain".to_owned(), 3)]);
    assert!(
        capped.validate().iter().all(|f| !f.contains("strain")),
        "a bound cap resolves; no config finding remains"
    );
    let capped_strain = capped
        .run()
        .invariants
        .iter()
        .find(|v| v.invariant == "strain-never-violated")
        .expect("the strain sweep is present")
        .clone();
    assert!(
        !capped_strain.deferred(),
        "a resolved cap is genuinely checked, not deferred"
    );
    assert_eq!(capped_strain.grade(), Grade::Checked);
}

/// Binding a symbolic affine cap doesn't merely stop the *deferred* verdict — it
/// makes the law genuinely ENFORCED: an overspend that breaks the bound is caught,
/// and the run-level `never-violated` sweep reports the exact witnessing trial.
/// Unbound, the same symbolic-cap law is never even accounted (honest deferral);
/// bound, it is accounted every trial and breaks when the balance overtops the cap.
/// This is the difference between an honest gap and a real check.
#[test]
fn a_bound_affine_cap_enforces_the_law_and_catches_overspends() {
    let (doc, diags) = parse_document(
        "scope magic\n\
         practice oath-binding proposed {\n\
           resource strain : affine bounded body-limit\n\
           cost bodily\n\
         }\n",
    );
    assert!(diags.is_empty(), "fixture parses: {diags:?}");
    let practice = compile_practices(&doc).remove(0);

    let base = Battery {
        practice,
        bounds: GenBounds {
            max_steps: 6,
            max_magnitude: 3,
        },
        seed: 7,
        trials: 200,
        metrics: Vec::new(),
        invariants: vec![InvariantSpec {
            name: "strain-never-violated".to_owned(),
            predicate: Invariant::NeverViolated {
                resource: "strain".to_owned(),
            },
        }],
        affine_caps: BTreeMap::new(),
    };

    // Bind the caster's body-limit low, so the generated draws overtop it.
    let mut bound = base.clone();
    bound.affine_caps = BTreeMap::from([("strain".to_owned(), 1)]);

    // Ground truth over the seeded trials: bound, strain's law is applied every
    // trial and catches genuine overspends; unbound, it is never accounted at all.
    let mut expected_first_breach: Option<u64> = None;
    for index in 0..base.trials {
        let trial = base.generate_trial(index);
        let bound_strain = bound
            .account_trial(&trial)
            .resource("strain")
            .expect("strain is declared")
            .clone();
        let unbound_strain = base
            .account_trial(&trial)
            .resource("strain")
            .expect("strain is declared")
            .clone();
        assert!(
            bound_strain.accounted,
            "a bound cap means strain's affine law is enforced (accounted) at trial {index}"
        );
        assert!(
            !unbound_strain.accounted,
            "an unbindable symbolic cap is never accounted — deferral is honest, not a pass"
        );
        assert!(
            !unbound_strain.violated(),
            "an unaccounted law is never reported violated"
        );
        if bound_strain.violated() && expected_first_breach.is_none() {
            expected_first_breach = Some(index);
        }
    }
    let breach_trial =
        expected_first_breach.expect("the low bound is genuinely breached by some draw");

    // The run-level sweep reflects that ground truth exactly: bound → Violated at
    // the witnessing trial; unbound → Deferred (the same law, never enforced).
    let bound_sweep = bound
        .run()
        .invariants
        .into_iter()
        .find(|v| v.invariant == "strain-never-violated")
        .expect("the strain sweep is present");
    assert!(
        !bound_sweep.deferred() && !bound_sweep.held(),
        "a bound, breached affine law is violated — not deferred, not held: {:?}",
        bound_sweep.outcome
    );
    let dli_sim::InvariantOutcome::Violated { trial, .. } = &bound_sweep.outcome else {
        panic!("expected Violated, got {:?}", bound_sweep.outcome);
    };
    assert_eq!(
        *trial, breach_trial,
        "the sweep names the earliest genuinely-overspending trial"
    );
    assert_eq!(bound_sweep.grade(), Grade::Checked);

    let unbound_sweep = base
        .run()
        .invariants
        .into_iter()
        .find(|v| v.invariant == "strain-never-violated")
        .expect("the strain sweep is present");
    assert!(
        unbound_sweep.deferred(),
        "the same law with no binding stays honestly deferred: {:?}",
        unbound_sweep.outcome
    );
}
