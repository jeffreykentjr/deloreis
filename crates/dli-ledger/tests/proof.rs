#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The PRAC-2 acceptance oracle (`docs/spec/04-practices.md` §3, §5): the
//! lantern-ward worked scene checks clean; wrong-phase is an L3 timing error;
//! fizzle is L2-absent; overload is the L6 affine bound; a flawed argument that
//! accidentally validates is a caught bug; and a premise's grade rolls up
//! weakest-link over the procedure chain.

use dli_ast::decl::Decl;
use dli_core::grade::Grade;
use dli_ledger::ProofReport;
use dli_ledger::ledger_check::{
    Expectation, ProofFinding, ProofVerdict, check_proof, check_proof_with_locations,
};
use dli_ledger::practice::{StructuralClass, compile_practices};
use dli_ledger::premise::SceneProjection;
use dli_syntax::parse_document;

/// A small original practice used by the proof and failure cases below.
const PRACTICE: &str = "scope lighthouse\n\nuse chronology\ncycle night(4) = dusk | still | storm | dark proposed\npractice lantern-ward proposed {\n  kind ground = door | field | ore\n  resource debt : linear conserved\n  resource strain : affine bounded body-limit\n  rule ward-form(ground, fibre)\n  rule give-and-take\n  error fizzle refines L2 premise-not-available where absent\n  error rebound refines L2 premise-not-available where contradicted signals bodily-hit\n  error wrong-hour refines L3 side-condition [timing] over cycle night\n  error overload refines L6 resource-violation on strain signals collapse\n  cost bodily | material\n}\n";

/// Check a proof against the lighthouse practice and a scene.
fn check(proof_block: &str, scene: &SceneProjection) -> ProofReport {
    check_impl(proof_block, scene, false)
}

fn check_impl(proof_block: &str, scene: &SceneProjection, allow_parse_errors: bool) -> ProofReport {
    let practice_src = PRACTICE;
    let src = format!("{practice_src}\n{proof_block}\n");
    let (doc, diags) = parse_document(&src);
    if !allow_parse_errors {
        assert!(diags.is_empty(), "parsed with diagnostics: {diags:?}");
    }
    let practices = compile_practices(&doc);
    let practice = practices
        .iter()
        .find(|p| p.name.text == "lantern-ward")
        .expect("the lantern-ward practice compiles");
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
    scene.seat("holds(keeper, consent(household, door))")
}

/// The worked scene of `docs/spec/04` §5 checks clean: seated premises, a
/// conserved-debt spend that balances, and a close that follows.
#[test]
fn worked_lantern_ward_checks_clean() {
    let proof = "\
proof e01-s04 speaker protagonist in lantern-ward {
  premise p1 holds(keeper, consent(household, door)) cited D-1 cue \"line 112\"
  premise p2 phase(night, still) from-scene cue \"line 118\"
  premise p3 debt(1) drawn-against p1
  step s1 instantiate ward-form with ground = door, fibre = withy
  step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
  spend p3
  close bound(ward, door) expect holds
}";
    let scene = seat_consent(SceneProjection::new()).at_phase("night", "still");
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

/// Wrong-phase: the same ward, spoken in a storm-bound scene, fails at L3 timing —
/// the series-opening beat. The declared `fails-at wrong-hour` (the practice's own
/// diegetic error name) is confirmed against the `proof.wrong-phase` kernel code.
#[test]
fn wrong_phase_is_an_l3_timing_error() {
    let proof = "\
proof e01-storm speaker protagonist in lantern-ward {
  premise p1 holds(keeper, consent(household, door)) cited D-1
  premise p2 phase(night, still) from-scene
  step s1 instantiate ward-form with ground = door, fibre = withy
  close bound(ward, door) expect fails-at wrong-hour
}";
    // The scene is at storm night; the ward wants still.
    let scene = seat_consent(SceneProjection::new()).at_phase("night", "storm");
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
            ProofFinding::WrongPhase {
                cycle,
                required,
                actual,
                ..
            } => Some((cycle.clone(), required.clone(), actual.clone())),
            _ => None,
        })
        .expect("a wrong-hour finding");
    assert_eq!(wrong.0, "night");
    assert_eq!(wrong.1, "still");
    assert_eq!(wrong.2, Some("storm".to_owned()));
    assert_eq!(
        report
            .findings
            .iter()
            .find(|f| f.class() == Some(StructuralClass::L3))
            .map(ProofFinding::code),
        Some("proof.wrong-phase")
    );
}

#[test]
fn phase_comparison_surface_uses_timing_semantics() {
    let proof = "\
proof e01-comparison speaker protagonist in lantern-ward {
  premise p1 phase(night) = still from-scene
  close x expect fails-at wrong-hour
}";
    let scene = SceneProjection::new().at_phase("night", "storm");
    let report = check(proof, &scene);
    assert!(has(&report, "proof.wrong-phase"), "{:?}", messages(&report));
    assert!(
        !has(&report, "proof.premise-absent"),
        "phase comparison was misclassified as a world premise: {:?}",
        messages(&report)
    );
}

/// Fizzle: a premise absent from the pool is L2-absent — the casting draws on a
/// premise merely unestablished, and fizzles.
#[test]
fn absent_premise_fizzles_at_l2() {
    let proof = "\
proof e01-fizzle speaker protagonist in lantern-ward {
  premise p1 holds(unwritten-consent) cited D-9
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
proof e01-drama speaker protagonist in lantern-ward {
  premise p1 holds(keeper, consent(household, door)) cited D-1
  premise p2 phase(night, still) from-scene
  step s1 instantiate ward-form with ground = door, fibre = withy
  step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
  close bound(ward, door) expect fails-at wrong-hour
}";
    // Spoken at still — the hour the ward wants — so it does NOT fail.
    let scene = seat_consent(SceneProjection::new()).at_phase("night", "still");
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
proof e01-overreach speaker protagonist in lantern-ward {
  premise p1 holds(keeper, consent(household, door)) from-scene
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
proof e01-graded speaker protagonist in lantern-ward {
  premise p1 holds(keeper, consent(household, door)) attested cue \"a witness recalled it\"
  premise p2 phase(night, still) from-scene
  premise p3 debt(1) drawn-against p1
  step s1 instantiate ward-form with ground = door, fibre = withy
  step s2 chain from p1, s1 by give-and-take conclusion bound(ward, door)
  spend p3
  close bound(ward, door) expect holds
}";
    let scene = seat_consent(SceneProjection::new()).at_phase("night", "still");
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

/// The report is a frozen wire seam (`SERVE-1` / PAY-2): it serializes, round-trips
/// through serde exactly, and yields a JSON schema. This exercises every tagged
/// report enum — `ProofFinding` (internally tagged on `finding`), `ProofVerdict`
/// and `Expectation` (externally tagged), and `ProofForm` — on the wrong-phase
/// scene, whose report carries an L3 failure, a `WrongPhase` finding, and a
/// `fails-at` expectation.
#[test]
fn the_proof_report_is_a_serde_and_schema_wire_seam() {
    let proof = "\
proof e01-storm speaker protagonist in lantern-ward {
  premise p1 holds(keeper, consent(household, door)) cited D-1
  premise p2 phase(night, still) from-scene
  step s1 instantiate ward-form with ground = door, fibre = withy
  close bound(ward, door) expect fails-at wrong-hour
}";
    let scene = seat_consent(SceneProjection::new()).at_phase("night", "storm");
    let report = check(proof, &scene);

    // Serialize, then round-trip back to an equal value.
    let json = serde_json::to_string(&report).expect("ProofReport serializes");
    let back: ProofReport = serde_json::from_str(&json).expect("ProofReport round-trips");
    assert_eq!(report, back, "the report must survive a serde round-trip");

    // The kernel finding code rides the `finding` tag as kebab-case `wrong-phase`;
    // the practice's diegetic error name `wrong-hour` rides the `fails-at`
    // expectation — the two channels stay distinct on the wire.
    assert!(
        json.contains("\"finding\":\"wrong-phase\""),
        "ProofFinding is internally tagged on `finding`: {json}"
    );
    assert!(
        json.contains("\"fails-at\":\"wrong-hour\""),
        "Expectation is externally tagged with kebab-case variants: {json}"
    );

    // The JSON schema derives for the whole report.
    let schema = serde_json::to_value(schemars::schema_for!(ProofReport))
        .expect("ProofReport has a JSON schema");
    assert!(schema.is_object(), "schema_for!(ProofReport) is an object");
}

// --- R3 regression coverage -------------------------------------------------

/// H6: a breach of `debt` must not borrow `strain`'s `overload` error just because
/// they share class L6. A debt-conservation breach with `expect fails-at overload`
/// must NOT check — the refining error is `None`, not the wrong sibling.
#[test]
fn a_breach_binds_only_its_own_declared_error() {
    let proof = "\
proof e01-h6 speaker protagonist in lantern-ward {
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
proof e01-spend speaker protagonist in lantern-ward {
  premise p1 debt(2) from-scene
  spend p1
  close done(debt) expect holds
}";
    let released = "\
proof e01-release speaker protagonist in lantern-ward {
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
proof e01-badspend speaker protagonist in lantern-ward {
  premise p1 holds(a-fact) from-scene
  spend p1
  close x expect blocked
}";
    let undefined = "\
proof e01-nospend speaker protagonist in lantern-ward {
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

#[test]
fn custom_steps_must_name_a_declared_practice_rule() {
    let declared = "\
proof custom-step speaker protagonist in lantern-ward {
  step s1 ward-form door, withy
  close x expect holds
}";
    let typo = "\
proof custom-step-typo speaker protagonist in lantern-ward {
  step s1 wrad-form door
  close x expect holds
}";

    let declared_report = check(declared, &SceneProjection::new());
    assert!(
        !has(&declared_report, "proof.unknown-form"),
        "{:?}",
        messages(&declared_report)
    );
    assert!(
        !has(&declared_report, "proof.bad-instantiation"),
        "{:?}",
        messages(&declared_report)
    );
    let typo_report = check(typo, &SceneProjection::new());
    assert!(
        has(&typo_report, "proof.unknown-form"),
        "{:?}",
        messages(&typo_report)
    );
    assert!(!typo_report.checks);
}

#[test]
fn assumptions_are_always_asserted() {
    let proof = "\
proof assumed speaker protagonist in lantern-ward {
  assume p1 holds(a)
  close holds(a) expect holds
}";
    let report = check(proof, &SceneProjection::new());
    assert!(report.checks, "{:?}", messages(&report));
    assert_eq!(report.grade, Grade::Asserted);
    assert_eq!(report.steps[0].grade, Grade::Asserted);
}

#[test]
fn nested_claims_share_one_premise_chain_and_close_grammar() {
    let proof = "\
proof nested speaker protagonist in lantern-ward {
  assume p1 holds(keeper, consent(household, door))
  step s1 chain from p1 by give-and-take conclusion holds(keeper, consent(household, door))
  close holds(keeper, consent(household, door)) expect holds
}";
    let report = check(proof, &SceneProjection::new());
    assert!(report.checks, "{:?}", messages(&report));
    assert_eq!(report.verdict, ProofVerdict::Holds);
    assert_eq!(report.grade, Grade::Asserted);
}

#[test]
fn close_expectation_separator_ignores_quoted_atom_text() {
    let proof = "\
proof quoted-close speaker protagonist in lantern-ward {
  assume p1 says(witness, \"we expect victory\")
  close says(witness, \"we expect victory\") expect holds
}";
    let report = check(proof, &SceneProjection::new());
    assert!(report.checks, "{:?}", messages(&report));
    assert_eq!(report.verdict, ProofVerdict::Holds);
}

#[test]
fn instantiation_requires_each_parameter_exactly_once() {
    let cases = [
        (
            "missing",
            "step s1 instantiate ward-form with ground = door",
        ),
        (
            "duplicate",
            "step s1 instantiate ward-form with ground = door, ground = field",
        ),
        (
            "unknown",
            "step s1 instantiate ward-form with ground = door, fibre = withy, extra = no",
        ),
        (
            "outside-kind",
            "step s1 instantiate ward-form with ground = no-such-ground, fibre = withy",
        ),
    ];
    for (name, step) in cases {
        let proof = format!(
            "proof bad-{name} speaker protagonist in lantern-ward {{\n  {step}\n  close x expect holds\n}}"
        );
        let report = check(&proof, &SceneProjection::new());
        assert!(
            has(&report, "proof.bad-instantiation"),
            "{name}: {:?}",
            messages(&report)
        );
        assert!(!report.checks, "{name} certified successfully");
    }

    let valid = "\
proof valid-bindings speaker protagonist in lantern-ward {
  step s1 instantiate ward-form with ground = field, fibre = caller-defined
  close x expect blocked
}";
    let report = check(valid, &SceneProjection::new());
    assert!(
        !has(&report, "proof.bad-instantiation"),
        "{:?}",
        messages(&report)
    );

    let quoted_comma = "\
proof quoted-binding speaker protagonist in lantern-ward {
  step s1 instantiate ward-form with ground = field, fibre = \"withy, twisted\"
  close x expect blocked
}";
    let report = check(quoted_comma, &SceneProjection::new());
    assert!(
        !has(&report, "proof.bad-instantiation"),
        "{:?}",
        messages(&report)
    );
}

#[test]
fn custom_steps_validate_declared_arity_and_finite_kinds() {
    for step in ["ward-form door", "ward-form no-such-ground, withy"] {
        let proof = format!(
            "proof bad-custom speaker protagonist in lantern-ward {{\n  step s1 {step}\n  close x expect holds\n}}"
        );
        let report = check(&proof, &SceneProjection::new());
        assert!(
            has(&report, "proof.bad-instantiation"),
            "{step}: {:?}",
            messages(&report)
        );
        assert!(!report.checks, "{step} certified successfully");
    }

    let quoted_comma = "\
proof quoted-custom speaker protagonist in lantern-ward {
  step s1 ward-form door, \"withy, twisted\"
  close x expect blocked
}";
    let report = check(quoted_comma, &SceneProjection::new());
    assert!(
        !has(&report, "proof.bad-instantiation"),
        "{:?}",
        messages(&report)
    );
}

#[test]
fn neutral_resource_operations_require_a_resource_premise() {
    for operation in ["reveal", "redistribute"] {
        for label in ["no-such-resource", "p1"] {
            let proof = format!(
                "proof bad-{operation}-{label} speaker protagonist in lantern-ward {{\n  \
                 premise p1 holds(a) from-scene\n  {operation} {label}\n  close x expect \
                 blocked\n}}"
            );
            let report = check(&proof, &SceneProjection::new().seat("holds(a)"));
            assert!(
                has(&report, "proof.invalid-spend"),
                "{operation} {label}: {:?}",
                messages(&report)
            );
        }
    }

    let valid = "\
proof valid-reveal speaker protagonist in lantern-ward {
  premise p1 debt(0) from-scene
  reveal p1
  redistribute p1
  close x expect blocked
}";
    let report = check(valid, &SceneProjection::new());
    assert!(
        !has(&report, "proof.invalid-spend"),
        "{:?}",
        messages(&report)
    );
}

/// M4: a timing premise naming a spoke the cycle does not carry is a
/// bad-instantiation (L4), not merely the wrong hour — caught when the scene pins
/// the cycle's declared spokes.
#[test]
fn an_undeclared_spoke_is_a_bad_instantiation() {
    let proof = "\
proof e01-spoke speaker protagonist in lantern-ward {
  premise p1 phase(night, purple) from-scene
  close x expect fails-at wrong-hour
}";
    // The scene knows night's four spokes; `purple` is not among them.
    let scene = SceneProjection::new()
        .spokes("night", &["dusk", "still", "storm", "dark"])
        .at_phase("night", "storm");
    let report = check(proof, &scene);
    assert!(
        has(&report, "proof.unknown-spoke"),
        "{:?}",
        messages(&report)
    );
    assert!(
        !has(&report, "proof.wrong-phase"),
        "an invalid spoke is not a wrong phase"
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
proof e01-spoke-ok speaker protagonist in lantern-ward {
  premise p1 phase(night, still) from-scene
  close x expect fails-at wrong-hour
}";
    let ok_scene = SceneProjection::new()
        .spokes("night", &["dusk", "still", "storm", "dark"])
        .at_phase("night", "storm");
    assert!(!has(&check(valid, &ok_scene), "proof.unknown-spoke"));
}

/// M5: an `expect` clause naming a non-verdict token is refused, never coerced to
/// `holds` (which would gut the expected-verdict discipline).
#[test]
fn an_unknown_expectation_token_is_refused() {
    let proof = "\
proof e01-typo speaker protagonist in lantern-ward {
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
    let empty = "proof e01-empty speaker protagonist in lantern-ward {\n}";
    let report = check_impl(empty, &SceneProjection::new(), true);
    assert_eq!(report.grade, Grade::Asserted, "no procedure = asserted");
    assert!(has(&report, "proof.missing-close"));
    assert!(has(&report, "proof.invalid-close-layout"));
    assert!(!report.checks);

    let no_close = "\
proof e01-noclose speaker protagonist in lantern-ward {
  premise p1 holds(a-fact) from-scene
}";
    let report = check_impl(
        no_close,
        &SceneProjection::new().seat("holds(a-fact)"),
        true,
    );
    assert!(has(&report, "proof.missing-close"));
    assert!(has(&report, "proof.invalid-close-layout"));
    assert!(!report.checks);
    // It has a premise, so its grade is that premise's, not the empty-chain default.
    assert_eq!(report.grade, Grade::Checked);
}

#[test]
fn duplicate_or_nonfinal_closes_never_check_for_direct_ast_callers() {
    let duplicate = "\
proof duplicate speaker protagonist in lantern-ward {
  close first expect blocked
  close second expect blocked
}";
    let report = check_impl(duplicate, &SceneProjection::new(), true);
    assert!(has(&report, "proof.invalid-close-layout"));
    assert!(!report.checks);

    let nonfinal = "\
proof nonfinal speaker protagonist in lantern-ward {
  close first expect blocked
  assume p1 later
}";
    let report = check_impl(nonfinal, &SceneProjection::new(), true);
    assert!(has(&report, "proof.invalid-close-layout"));
    assert!(!report.checks);
}

#[test]
fn malformed_close_layout_does_not_invent_a_resource_location() {
    for body in [
        "  premise p1 debt(1) drawn-against p0\n  close first expect blocked\n  close second expect blocked\n",
        "  premise p1 debt(1) drawn-against p0\n  close first expect blocked\n  assume p2 later\n",
    ] {
        let source =
            format!("{PRACTICE}\nproof invalid speaker protagonist in lantern-ward {{\n{body}}}\n");
        let (document, _) = parse_document(&source);
        let practice = compile_practices(&document)
            .into_iter()
            .find(|p| p.name.text == "lantern-ward")
            .unwrap();
        let proof = document
            .decls
            .iter()
            .find_map(|decl| match decl {
                Decl::Proof(proof) => Some(proof),
                _ => None,
            })
            .unwrap();
        let (report, locations) =
            check_proof_with_locations(&practice, proof, &SceneProjection::new());
        let index = report
            .findings
            .iter()
            .position(|finding| matches!(finding, ProofFinding::ResourceViolation { .. }))
            .expect("unbalanced resource");
        assert!(locations[index].is_none(), "{locations:?}");
    }
}

#[test]
fn proof_header_roles_must_be_generic_or_declared_by_the_practice() {
    let proof = "\
proof bad-role invented-role protagonist in lantern-ward {
  close x expect blocked
}";
    let report = check(proof, &SceneProjection::new());
    assert!(
        has(&report, "proof.unknown-role"),
        "{:?}",
        messages(&report)
    );
    assert_eq!(report.speaker.as_deref(), Some("protagonist"));
    assert!(!report.checks);
}

#[test]
fn malformed_direct_ast_headers_never_check() {
    for proof in [
        "proof empty {\n  close x expect blocked\n}",
        "proof surplus speaker protagonist in lantern-ward trailing {\n  close x expect blocked\n}",
        "proof reordered in lantern-ward speaker protagonist {\n  close x expect blocked\n}",
    ] {
        let report = check_impl(proof, &SceneProjection::new(), true);
        assert!(
            has(&report, "proof.invalid-header-layout"),
            "{:?}",
            messages(&report)
        );
        assert!(!report.checks, "malformed header certified: {proof}");
    }
}

#[test]
fn malformed_direct_ast_bodies_never_check() {
    for child in [
        "assume p1 holds(a) checked",
        "step s1 instantiate ward-form with ground = door, junk, fibre = withy",
        "step s1 instantiate ward-form with ground = door, fibre = withy,",
        "step s1 instantiate ward-form with ground = door, fibre = withy, dangling",
        "step s1 chain from p1, by give-and-take conclusion holds(a)",
        "step s1 chain from p1 by give-and-take conclusion holds(a",
        "step s1 ward-form door,,withy",
        "spend p1 trailing",
        "unknown proof child",
    ] {
        let proof = format!(
            "proof malformed-body speaker protagonist in lantern-ward {{\n  {child}\n  close x expect blocked\n}}"
        );
        let report = check_impl(&proof, &SceneProjection::new(), true);
        assert!(
            has(&report, "proof.malformed-body"),
            "{child}: {:?}",
            messages(&report)
        );
        assert!(!report.checks, "malformed body certified: {child}");
    }
}

#[test]
fn malformed_shape_does_not_rewrite_expectation_outcome() {
    let proof = "\
proof malformed-body speaker protagonist in lantern-ward {
  assume p1 holds(a)
  unknown proof child
  close holds(a) expect holds
}";
    let report = check_impl(proof, &SceneProjection::new(), true);
    assert_eq!(report.verdict, ProofVerdict::Holds);
    assert_eq!(report.expected, Expectation::Holds);
    assert!(report.expectation_met);
    assert!(!report.checks);
    assert!(has(&report, "proof.malformed-body"));
}

#[test]
fn quoted_claim_values_are_not_collapsed_or_split_as_modifiers() {
    let distinct = r#"
proof distinct speaker protagonist in lantern-ward {
  assume p1 says(the-seeress, "a b")
  close says(the-seeress, "ab") expect holds
}"#;
    let report = check(distinct, &SceneProjection::new());
    assert!(!report.checks, "distinct quoted values certified as equal");
    assert!(
        has(&report, "proof.close-mismatch"),
        "{:?}",
        messages(&report)
    );

    let balanced = r#"
proof balanced speaker protagonist in lantern-ward {
  assume p1 says(the-seeress, "a ) cited marker")
  close says(the-seeress, "a ) cited marker") expect holds
}"#;
    let report = check(balanced, &SceneProjection::new());
    assert!(report.checks, "{:?}", messages(&report));
}

/// L2: an unmet expectation raises the finding in EVERY direction — a held
/// `expect blocked` and a blocked `expect holds`, not only a validated `fails-at`.
#[test]
fn every_unmet_expectation_raises_a_finding() {
    let held_but_blocked = "\
proof e01-blocked speaker protagonist in lantern-ward {
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
proof e01-heldfail speaker protagonist in lantern-ward {
  premise p1 holds(unwritten) cited D-9
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
