#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The PRAC-2 acceptance oracle (`docs/spec/04-practices.md` §3, §5): the
//! spoken-proof worked scene checks clean; wrong-hour is an L3 timing error;
//! fizzle is L2-absent; overload is the L6 affine bound; a flawed argument that
//! accidentally validates is a caught bug; and a premise's grade rolls up
//! weakest-link over the procedure chain.

use std::path::PathBuf;

use dli_ast::decl::Decl;
use dli_core::grade::Grade;
use dli_ledger::ProofReport;
use dli_ledger::ledger_check::{Expectation, ProofFinding, ProofVerdict, check_proof};
use dli_ledger::practice::{StructuralClass, compile_practices};
use dli_ledger::premise::SceneProjection;
use dli_syntax::parse_document;

/// Load a committed `TPtH` practice pack from `worlds/tpth/practices/`, mirroring
/// the acceptance suite's `fixture` helper. The spoken-proof practice (the
/// Concordance) is DATA, not an inline const: every scene below is checked
/// against the very `.dli` pack the gate ships.
fn fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../worlds/tpth/practices");
    path.push(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Compile the spoken-proof practice with `proof_block` appended and check the
/// proof against `scene`. The practice — its `scope`, the `tide` cycle its
/// timing seats on, and the practice block — is loaded from the committed
/// `worlds/tpth/practices/spoken-proof.dli` pack, then the scene's proof is
/// parsed in the same document so it resolves against it.
fn check(proof_block: &str, scene: &SceneProjection) -> ProofReport {
    let practice_src = fixture("spoken-proof.dli");
    let src = format!("{practice_src}\n{proof_block}\n");
    let (doc, diags) = parse_document(&src);
    assert!(diags.is_empty(), "parsed with diagnostics: {diags:?}");
    let practices = compile_practices(&doc);
    let practice = practices
        .iter()
        .find(|p| p.name.text == "spoken-proof")
        .expect("the spoken-proof practice compiles");
    let proof = doc
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Proof(p) => Some(p),
            _ => None,
        })
        .expect("the document declares one proof");
    check_proof(practice, proof, scene)
}

/// The seated consent premise, shared by the scenes that rest on it.
fn seat_consent(scene: SceneProjection) -> SceneProjection {
    scene.seat("holds(enfys, consent(household, door))")
}

/// The worked scene of `docs/spec/04` §5 checks clean: seated premises, a
/// conserved-debt spend that balances, and a close that follows.
#[test]
fn worked_spoken_proof_checks_clean() {
    let proof = "\
proof e01-s04 speaker protagonist in spoken-proof {
  premise p1 holds(enfys, consent(household, door)) cited D-0113 cue \"line 112\"
  premise p2 phase(tide, slack) from-scene cue \"line 118\"
  premise p3 debt(1) drawn-against p1
  step s1 instantiate ward-form with ground = door, fibre = withy
  step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
  spend p3
  close bound(ward, door) expect holds
}";
    let scene = seat_consent(SceneProjection::new()).at_phase("tide", "slack");
    let report = check(proof, &scene);
    assert!(
        report.checks,
        "worked scene should check, got: {:?}",
        messages(&report)
    );
    assert_eq!(report.verdict, ProofVerdict::Holds);
    assert!(report.findings.is_empty(), "{:?}", messages(&report));
    // D-cited and scene-read premises re-derive: the whole casting checks.
    assert_eq!(report.grade, Grade::Checked);
    assert_eq!(report.speaker.as_deref(), Some("protagonist"));
}

/// Wrong-hour: the same ward, spoken in a flood-bound scene, fails at L3 timing —
/// the series-opening beat. The declared `fails-at wrong-hour` is confirmed.
#[test]
fn wrong_hour_is_an_l3_timing_error() {
    let proof = "\
proof e01-flood speaker protagonist in spoken-proof {
  premise p1 holds(enfys, consent(household, door)) cited D-0113
  premise p2 phase(tide, slack) from-scene
  step s1 instantiate ward-form with ground = door, fibre = withy
  close bound(ward, door) expect fails-at wrong-hour
}";
    // The scene is at flood tide; the ward wants slack.
    let scene = seat_consent(SceneProjection::new()).at_phase("tide", "flood");
    let report = check(proof, &scene);
    assert!(
        report.checks,
        "the drama is confirmed: {:?}",
        messages(&report)
    );
    assert_eq!(
        report.verdict,
        ProofVerdict::FailedAt {
            class: StructuralClass::L3,
            error: Some("wrong-hour".to_owned()),
            at: "p2".to_owned(),
        }
    );
    let wrong = report
        .findings
        .iter()
        .find_map(|f| match f {
            ProofFinding::WrongHour {
                cycle,
                required,
                actual,
                ..
            } => Some((cycle.clone(), required.clone(), actual.clone())),
            _ => None,
        })
        .expect("a wrong-hour finding");
    assert_eq!(wrong.0, "tide");
    assert_eq!(wrong.1, "slack");
    assert_eq!(wrong.2, Some("flood".to_owned()));
    assert_eq!(
        report
            .findings
            .iter()
            .find(|f| f.class() == Some(StructuralClass::L3))
            .map(ProofFinding::code),
        Some("proof.wrong-hour")
    );
}

/// Fizzle: a premise absent from the pool is L2-absent — the casting draws on a
/// premise merely unestablished, and fizzles.
#[test]
fn absent_premise_fizzles_at_l2() {
    let proof = "\
proof e01-fizzle speaker protagonist in spoken-proof {
  premise p1 holds(unwritten-consent) cited D-0999
  close ward(door) expect fails-at fizzle
}";
    // The scene establishes nothing — p1 is not in the pool.
    let scene = SceneProjection::new();
    let report = check(proof, &scene);
    assert!(report.checks, "{:?}", messages(&report));
    assert!(matches!(
        report.verdict,
        ProofVerdict::FailedAt {
            class: StructuralClass::L2,
            ..
        }
    ));
    let absent = report
        .findings
        .iter()
        .any(|f| matches!(f, ProofFinding::PremiseAbsent { label, .. } if label == "p1"));
    assert!(
        absent,
        "expected a fizzle at p1, got: {:?}",
        messages(&report)
    );
}

/// The expected verdict cuts both ways: a scene that declares `fails-at
/// wrong-hour` but is spoken at the *right* hour validates — a flawed argument
/// that accidentally succeeds is a caught bug, not a silent pass (xfail-for-drama).
#[test]
fn accidental_validation_is_a_caught_bug() {
    let proof = "\
proof e01-drama speaker protagonist in spoken-proof {
  premise p1 holds(enfys, consent(household, door)) cited D-0113
  premise p2 phase(tide, slack) from-scene
  step s1 instantiate ward-form with ground = door, fibre = withy
  step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
  close bound(ward, door) expect fails-at wrong-hour
}";
    // Spoken at slack — the hour the ward wants — so it does NOT fail.
    let scene = seat_consent(SceneProjection::new()).at_phase("tide", "slack");
    let report = check(proof, &scene);
    assert!(
        !report.checks,
        "an accidental validation must not check: {:?}",
        report.verdict
    );
    assert_eq!(report.verdict, ProofVerdict::Holds);
    assert!(
        report
            .findings
            .iter()
            .any(|f| matches!(f, ProofFinding::ExpectationUnmet { .. })),
        "the silent-success bug must surface as a finding: {:?}",
        messages(&report)
    );
}

/// Overload: drawing more strain than the affine bound allows is L6 — the affine
/// discipline on the practice's `strain` resource, with the cap resolved from the
/// scene.
#[test]
fn overload_breaks_the_affine_bound_at_l6() {
    let proof = "\
proof e01-overreach speaker protagonist in spoken-proof {
  premise p1 holds(enfys, consent(household, door)) from-scene
  premise p2 strain(3) drawn-against p1
  close warded(door) expect fails-at overload
}";
    // The caster's body-limit resolves to two this scene; the casting draws three.
    let scene = seat_consent(SceneProjection::new()).cap("strain", 2);
    let report = check(proof, &scene);
    assert!(report.checks, "{:?}", messages(&report));
    assert_eq!(
        report.verdict,
        ProofVerdict::FailedAt {
            class: StructuralClass::L6,
            error: Some("overload".to_owned()),
            at: "p2".to_owned(),
        }
    );
    let overloaded = report.findings.iter().any(|f| {
        matches!(
            f,
            ProofFinding::ResourceViolation { resource, .. } if resource == "strain"
        )
    });
    assert!(
        overloaded,
        "expected an L6 on strain: {:?}",
        messages(&report)
    );
}

/// A graded chain: an *attested* premise (a stored judgment, not re-derivable)
/// rolls the whole casting's grade down to `attested` by weakest-link — even
/// though the proof otherwise checks clean.
#[test]
fn an_attested_premise_grades_the_chain_attested() {
    let proof = "\
proof e01-graded speaker protagonist in spoken-proof {
  premise p1 holds(enfys, consent(household, door)) attested cue \"a witness recalled it\"
  premise p2 phase(tide, slack) from-scene
  premise p3 debt(1) drawn-against p1
  step s1 instantiate ward-form with ground = door, fibre = withy
  step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
  spend p3
  close bound(ward, door) expect holds
}";
    let scene = seat_consent(SceneProjection::new()).at_phase("tide", "slack");
    let report = check(proof, &scene);
    assert!(
        report.checks,
        "the casting still checks: {:?}",
        messages(&report)
    );
    assert_eq!(report.verdict, ProofVerdict::Holds);
    // The weakest link is the attested premise, so the whole casting is attested.
    assert_eq!(report.grade, Grade::Attested);
    assert_eq!(report.expected, Expectation::Holds);
}

fn messages(report: &ProofReport) -> Vec<String> {
    report.findings.iter().map(ProofFinding::message).collect()
}

fn has(report: &ProofReport, code: &str) -> bool {
    report.findings.iter().any(|f| f.code() == code)
}

// --- R3 regression coverage -------------------------------------------------

/// H6: a breach of `debt` must not borrow `strain`'s `overload` error just because
/// they share class L6. A debt-conservation breach with `expect fails-at overload`
/// must NOT check — the refining error is `None`, not the wrong sibling.
#[test]
fn a_breach_binds_only_its_own_declared_error() {
    let proof = "\
proof e01-h6 speaker protagonist in spoken-proof {
  premise p1 debt(1) from-scene
  close discharged(debt) expect fails-at overload
}";
    // debt is revealed (+1) and never balanced → conservation breach on debt.
    let scene = SceneProjection::new();
    let report = check(proof, &scene);
    assert!(
        !report.checks,
        "a debt breach must not satisfy `fails-at overload`: {:?}",
        report.verdict
    );
    match &report.verdict {
        ProofVerdict::FailedAt { class, error, .. } => {
            assert_eq!(*class, StructuralClass::L6);
            assert_ne!(
                error.as_deref(),
                Some("overload"),
                "must not borrow strain's error"
            );
            assert_eq!(*error, None, "no declared L6 error binds `debt`");
        }
        ProofVerdict::Holds => panic!("expected an L6 failure, got a clean hold"),
    }
    assert!(has(&report, "proof.resource-violation"));
}

/// M3: `release` RETURNS a unit (positive delta), the opposite of `spend`. The
/// same revealed debt balances under `spend` but is minted under `release`.
#[test]
fn release_returns_a_unit_spend_draws_one() {
    let spent = "\
proof e01-spend speaker protagonist in spoken-proof {
  premise p1 debt(2) from-scene
  spend p1
  close done(debt) expect holds
}";
    let released = "\
proof e01-release speaker protagonist in spoken-proof {
  premise p1 debt(2) from-scene
  release p1
  close done(debt) expect holds
}";
    let scene = SceneProjection::new().seat("done(debt)");
    // reveal(+2) then spend(-2) → net 0 → conserved.
    assert!(
        !has(&check(spent, &scene), "proof.resource-violation"),
        "spend discharges the debt to zero"
    );
    // reveal(+2) then release(+2) → net +4 → minted, not conserved.
    assert!(
        has(&check(released, &scene), "proof.resource-violation"),
        "release adds a unit, so the conserved multiset no longer balances"
    );
}

/// M3: a `spend`/`release` naming a non-resource or undefined label is refused,
/// not silently accepted.
#[test]
fn spend_of_a_non_resource_is_refused() {
    let non_resource = "\
proof e01-badspend speaker protagonist in spoken-proof {
  premise p1 holds(a-fact) from-scene
  spend p1
  close x expect blocked
}";
    let undefined = "\
proof e01-nospend speaker protagonist in spoken-proof {
  spend p99
  close x expect blocked
}";
    let scene = SceneProjection::new().seat("holds(a-fact)");
    assert!(has(&check(non_resource, &scene), "proof.invalid-spend"));
    assert!(has(
        &check(undefined, &SceneProjection::new()),
        "proof.invalid-spend"
    ));
}

/// M4: a timing premise naming a spoke the cycle does not carry is a
/// bad-instantiation (L4), not merely the wrong hour — caught when the scene pins
/// the cycle's declared spokes.
#[test]
fn an_undeclared_spoke_is_a_bad_instantiation() {
    let proof = "\
proof e01-spoke speaker protagonist in spoken-proof {
  premise p1 phase(tide, purple) from-scene
  close x expect fails-at wrong-hour
}";
    // The scene knows tide's four spokes; `purple` is not among them.
    let scene = SceneProjection::new()
        .spokes("tide", &["ebb", "slack", "flood", "high"])
        .at_phase("tide", "flood");
    let report = check(proof, &scene);
    assert!(
        has(&report, "proof.unknown-spoke"),
        "{:?}",
        messages(&report)
    );
    assert!(
        !has(&report, "proof.wrong-hour"),
        "an invalid spoke is not a wrong hour"
    );
    assert!(matches!(
        report.verdict,
        ProofVerdict::FailedAt {
            class: StructuralClass::L4,
            ..
        }
    ));
    // Without pinned spokes, validation is deferred (a valid spoke still checks).
    let valid = "\
proof e01-spoke-ok speaker protagonist in spoken-proof {
  premise p1 phase(tide, slack) from-scene
  close x expect fails-at wrong-hour
}";
    let ok_scene = SceneProjection::new()
        .spokes("tide", &["ebb", "slack", "flood", "high"])
        .at_phase("tide", "flood");
    assert!(!has(&check(valid, &ok_scene), "proof.unknown-spoke"));
}

/// M5: an `expect` clause naming a non-verdict token is refused, never coerced to
/// `holds` (which would gut the expected-verdict discipline).
#[test]
fn an_unknown_expectation_token_is_refused() {
    let proof = "\
proof e01-typo speaker protagonist in spoken-proof {
  premise p1 holds(a-fact) from-scene
  close holds(a-fact) expect banana
}";
    let scene = SceneProjection::new().seat("holds(a-fact)");
    let report = check(proof, &scene);
    assert_eq!(report.expected, Expectation::Unknown("banana".to_owned()));
    assert!(has(&report, "proof.unknown-expectation"));
    assert!(!report.checks, "a typo'd expectation must never check");
}

/// M6: a proof with no procedure grades `asserted` (nothing to re-run), and a
/// proof with no close is refused — neither silently checks at `checked`.
#[test]
fn an_empty_or_closeless_proof_does_not_silently_check() {
    let empty = "proof e01-empty speaker protagonist in spoken-proof {\n}";
    let report = check(empty, &SceneProjection::new());
    assert_eq!(report.grade, Grade::Asserted, "no procedure = asserted");
    assert!(has(&report, "proof.missing-close"));
    assert!(!report.checks);

    let no_close = "\
proof e01-noclose speaker protagonist in spoken-proof {
  premise p1 holds(a-fact) from-scene
}";
    let report = check(no_close, &SceneProjection::new().seat("holds(a-fact)"));
    assert!(has(&report, "proof.missing-close"));
    assert!(!report.checks);
    // It has a premise, so its grade is that premise's, not the empty-chain default.
    assert_eq!(report.grade, Grade::Checked);
}

/// L2: an unmet expectation raises the finding in EVERY direction — a held
/// `expect blocked` and a blocked `expect holds`, not only a validated `fails-at`.
#[test]
fn every_unmet_expectation_raises_a_finding() {
    let held_but_blocked = "\
proof e01-blocked speaker protagonist in spoken-proof {
  premise p1 holds(a-fact) from-scene
  close holds(a-fact) expect blocked
}";
    let report = check(
        held_but_blocked,
        &SceneProjection::new().seat("holds(a-fact)"),
    );
    assert_eq!(report.verdict, ProofVerdict::Holds);
    assert!(!report.checks);
    assert!(
        has(&report, "proof.expectation-unmet"),
        "expect blocked that held must surface"
    );

    let failed_but_holds = "\
proof e01-heldfail speaker protagonist in spoken-proof {
  premise p1 holds(unwritten) cited D-0999
  close x expect holds
}";
    let report = check(failed_but_holds, &SceneProjection::new());
    assert!(!report.checks);
    assert!(has(&report, "proof.premise-absent"));
    assert!(
        has(&report, "proof.expectation-unmet"),
        "expect holds that failed must surface"
    );
}

/// L3: a proof whose header `in <practice>` names a different practice than the
/// def it is checked against is refused — the verdict would be about the wrong
/// law-set.
#[test]
fn a_practice_name_mismatch_is_refused() {
    let proof = "\
proof e01-mismatch speaker protagonist in the-other-practice {
  premise p1 holds(a-fact) from-scene
  close holds(a-fact) expect holds
}";
    let scene = SceneProjection::new().seat("holds(a-fact)");
    let report = check(proof, &scene);
    assert!(
        has(&report, "proof.practice-mismatch"),
        "{:?}",
        messages(&report)
    );
    assert!(
        !report.checks,
        "a proof naming the wrong practice must not check"
    );
}
