#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The PRAC-1 acceptance oracle (`docs/spec/04-practices.md` §4).
//!
//! The arbitrariness proof: two magic systems disjoint on every declared axis
//! host on the one generic checker with zero per-system kernel code. Both
//! fixtures compile and check well-formed; the negatives prove the checker
//! actually catches each malformation rather than passing everything.

use std::path::PathBuf;

use dli_ledger::practice::{PracticeDef, PracticeFinding, check_practice, compile_practices};
use dli_ledger::resource::ResourceKind;
use dli_syntax::parse_document;

/// A coarse tag over the algebra members, for the disjointness assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceKindTag {
    Linear,
    Conserved,
    Affine,
    Graded,
}

fn fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../worlds/tpth/packs");
    path.push(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn compile_one(name: &str, practice: &str) -> PracticeDef {
    let (doc, diags) = parse_document(&fixture(name));
    assert!(
        diags.is_empty(),
        "{name} parsed with diagnostics: {diags:?}"
    );
    let practices = compile_practices(&doc);
    practices
        .into_iter()
        .find(|p| p.name.text == practice)
        .unwrap_or_else(|| panic!("{name} declares no practice '{practice}'"))
}

/// The acceptance test: the true-naming fixture hosts with zero findings.
#[test]
fn true_naming_hosts_with_zero_findings() {
    let def = compile_one("true-naming.dli", "true-naming");
    let findings = check_practice(&def);
    assert!(
        findings.is_empty(),
        "true-naming should host cleanly, got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::message)
            .collect::<Vec<_>>()
    );
    // Its one honest cost: a graded-monotone resource (the algebra member the
    // fixture added). Nothing conserved, nothing affine.
    assert_eq!(def.resources.len(), 1);
    assert_eq!(
        def.resources[0].kind,
        Some(ResourceKind::GradedMonotone),
        "true-naming's resource is graded-monotone decay"
    );
    // Its failures refine L2/L3/L4 — no resource-violation, no chain error.
    assert!(!def.errors.is_empty());
    assert!(def.errors.iter().all(|e| e.class.is_some()));
}

/// The second arbitrariness proof: the-turning hosts, disjoint on every axis,
/// with zero kernel-code change.
#[test]
fn the_turning_hosts_with_zero_findings() {
    let def = compile_one("second-practice.dli", "the-turning");
    let findings = check_practice(&def);
    assert!(
        findings.is_empty(),
        "the-turning should host cleanly, got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::message)
            .collect::<Vec<_>>()
    );
    // Its resource is affine-bounded — neither the conserved debt of proof-magic
    // nor the graded decay of true-naming.
    assert_eq!(
        def.resources[0].kind,
        Some(ResourceKind::AffineBounded),
        "the-turning's resource is affine-bounded"
    );
    // It seats a timing failure on the document-declared cycle, which resolves.
    assert!(def.cycles_in_scope.contains("turning-leaf"));
    // A declared finite sort with five figures.
    assert_eq!(def.sorts.len(), 1);
    assert_eq!(def.sorts[0].variants.len(), 5);
}

/// The arbitrariness proof stated as a property: the two fixtures share no
/// resource-algebra member, yet both host generically.
#[test]
fn the_two_fixtures_are_structurally_disjoint() {
    let naming = compile_one("true-naming.dli", "true-naming");
    let turning = compile_one("second-practice.dli", "the-turning");
    let naming_kinds: Vec<ResourceKindTag> = naming
        .resources
        .iter()
        .filter_map(|r| r.kind.as_ref().map(tag))
        .collect();
    let turning_kinds: Vec<ResourceKindTag> = turning
        .resources
        .iter()
        .filter_map(|r| r.kind.as_ref().map(tag))
        .collect();
    assert!(
        naming_kinds.iter().all(|k| !turning_kinds.contains(k)),
        "the fixtures must share no resource-algebra member: {naming_kinds:?} vs {turning_kinds:?}"
    );
    // Both nevertheless host on the one generic checker.
    assert!(check_practice(&naming).is_empty());
    assert!(check_practice(&turning).is_empty());
}

/// A coarse tag distinguishing the algebra members for the disjointness test.
fn tag(kind: &ResourceKind) -> ResourceKindTag {
    match kind {
        ResourceKind::Linear { conserved: false } => ResourceKindTag::Linear,
        ResourceKind::Linear { conserved: true } => ResourceKindTag::Conserved,
        ResourceKind::AffineBounded => ResourceKindTag::Affine,
        ResourceKind::GradedMonotone => ResourceKindTag::Graded,
    }
}

// --- negatives: the checker catches each malformation -----------------------

fn findings_of(src: &str) -> Vec<PracticeFinding> {
    let (doc, _) = parse_document(src);
    let practices = compile_practices(&doc);
    assert_eq!(
        practices.len(),
        1,
        "expected exactly one practice in fixture"
    );
    check_practice(&practices[0])
}

#[test]
fn negative_error_refining_nothing_is_caught() {
    let findings = findings_of(
        "\
pack magic
practice bad-error proposed {
  error void refines L9 nowhere
  cost epistemic
}
",
    );
    assert!(
        findings
            .iter()
            .any(|f| f.code() == "practice.error-refines-nothing"),
        "got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn negative_resource_outside_algebra_is_caught() {
    let findings = findings_of(
        "\
pack magic
practice bad-resource proposed {
  resource mana : quadratic wobble
  cost epistemic
}
",
    );
    assert!(
        findings
            .iter()
            .any(|f| f.code() == "practice.resource-outside-algebra"),
        "got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn negative_unbound_sort_is_caught() {
    let findings = findings_of(
        "\
pack magic
practice bad-sort proposed {
  role seer : nonesuch requires standing(seer)
  cost epistemic
}
",
    );
    assert!(
        findings.iter().any(|f| f.code() == "practice.unbound-sort"),
        "got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn negative_unresolved_cycle_is_caught() {
    // A timing failure seated on a cycle the document never declares.
    let findings = findings_of(
        "\
scope magic
practice bad-cycle proposed {
  error wrong-phase refines L3 side-condition [timing] over cycle ghost-cycle
  cost temporal
}
",
    );
    assert!(
        findings
            .iter()
            .any(|f| f.code() == "practice.unresolved-cycle"),
        "got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn negative_retired_over_wheel_is_refused_with_its_successor() {
    // DL-0016: a practice interior that seats a timing failure with the retired
    // `over wheel` is refused with `over cycle`, and the reference still resolves
    // (the document-declared cycle `tide` is in scope) so nothing is silently
    // dropped.
    let findings = findings_of(
        "\
scope magic
cycle tide(4) = ebb | slack | flood | high  proposed
practice legacy proposed {
  error wrong-hour refines L3 side-condition [timing] over wheel tide
  cost temporal
}
",
    );
    assert!(
        findings
            .iter()
            .any(|f| f.code() == "practice.renamed-interior-form"),
        "got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::code)
            .collect::<Vec<_>>()
    );
    // The retired spelling is diagnosed, not misread as an unresolved cycle:
    // `tide` is a document-declared cycle and resolves.
    assert!(
        !findings
            .iter()
            .any(|f| f.code() == "practice.unresolved-cycle"),
        "the reference must still resolve leniently"
    );
}

#[test]
fn negative_l6_naming_an_undeclared_resource_is_caught() {
    let findings = findings_of(
        "\
pack magic
practice bad-spend proposed {
  error blowout refines L6 resource-violation on phantom signals collapse
  cost bodily
}
",
    );
    assert!(
        findings
            .iter()
            .any(|f| f.code() == "practice.error-resource-unknown"),
        "got: {:?}",
        findings
            .iter()
            .map(PracticeFinding::code)
            .collect::<Vec<_>>()
    );
}
