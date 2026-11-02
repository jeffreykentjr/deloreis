#![allow(clippy::unwrap_used, clippy::expect_used)]
//! WS-1: the identity kernel's world-check honesty suite.
//!
//! The check-layer projection of identity is [`dli_core::alias::canon_map`]: a
//! union-find over the compiled world's certain `same-as` links, applied at
//! FD/intra-episode/fluent grouping before value comparison (canon BEFORE
//! `fd_value_key`, and to the value side too). These probes pin the load-bearing
//! interactions: a post-merge FD conflict names the identity assertion, a
//! merely-proposed or never-ratified identity does not merge anything, value-side
//! unification is not itself a violation, fluent timelines combine across an
//! identity-merged class, the v1 temporal boundary refuses eras/events loudly,
//! a killed token surfaces even inside a `same-as` argument, and every
//! `Decl::SameAs` reaches the world or raises a graded finding — the koan-4
//! permanent regression pin (missing/malformed provenance fails closed, never
//! silently drops).

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    Decl, Document, EntityDecl, EntityKind, FactDecl, FunctionalDecl, RatifyDecl, SameAsDecl,
    ScopeHeader, StateDecl, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_world::check::factcheck::{KilledToken, KilledTokens, MatchMode};
use dli_world::check::{self, CompiledWorld};
use dli_world::compile;
use dli_world::model::LoadFinding;
use dli_world::pack::Pack;

// --- builders ----------------------------------------------------------------

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn marker(marker: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
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

fn entity(kind: EntityKind, name: &str, cite: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(ratified(cite)),
    })
}

fn state(name: &str, value: &str, time: Option<TimeClause>, cite: &str) -> Decl {
    Decl::State(StateDecl {
        name: id(name),
        value: Some(Term::Sym(id(value))),
        time,
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

/// Compile a world whose ledger already declares D-1 and D-9, so `ratified`
/// citations to either read certain.
fn world(decls: Vec<Decl>) -> CompiledWorld {
    let mut all = vec![ratify_decl("D-1"), ratify_decl("D-9")];
    all.extend(decls);
    compile(&pack("weir", all)).expect("compiles")
}

fn findings_with_code<'a>(
    findings: &'a [check::WorldFinding],
    code: &str,
) -> Vec<&'a check::WorldFinding> {
    findings.iter().filter(|f| f.code == code).collect()
}

// ===========================================================================
// 1. Headline case: a declared-functional FD violation created via identity
//    closure names both values AND the identity assertion in its citations.
// ===========================================================================
#[test]
fn headline_fd_violation_names_both_values_and_the_identity_link() {
    let world = world(vec![
        functional("rank", "D-1"),
        certain_fact("rank", "nara", "gold", "D-1"),
        certain_fact("rank", "moira", "silver", "D-1"),
        same_as("nara", "moira", Some(ratified("D-9"))),
    ]);
    let findings = check::run(&world);
    let fd = findings_with_code(&findings, "continuity.fd-violation");
    assert_eq!(
        fd.len(),
        1,
        "exactly one violation for the merged slot: {fd:?}"
    );
    let f = fd[0];
    assert!(
        f.message.contains("gold") && f.message.contains("silver"),
        "{}",
        f.message
    );
    assert!(
        f.citations.contains(&"D-1".to_owned()) && f.citations.contains(&"D-9".to_owned()),
        "citations must include both facts' ruling AND the identity ruling: {:?}",
        f.citations
    );
}

// ===========================================================================
// 2. A merely-proposed identity never merges anything for FD purposes — the
//    canon map is built from CERTAIN same-as links only.
// ===========================================================================
#[test]
fn proposed_identity_does_not_trigger_an_fd_merge() {
    let world = world(vec![
        functional("rank", "D-1"),
        certain_fact("rank", "nara", "gold", "D-1"),
        certain_fact("rank", "moira", "silver", "D-1"),
        same_as("nara", "moira", Some(proposed())),
    ]);
    let findings = check::run(&world);
    assert!(
        findings_with_code(&findings, "continuity.fd-violation").is_empty(),
        "a proposed identity must not merge slots: {findings:?}"
    );
}

// ===========================================================================
// 3. An identity whose citation is never ratified (the ledger never declares
//    the decision) likewise never merges — the standing that would let
//    `author.defeat` un-merge on the wire is exactly this: not certain.
// ===========================================================================
#[test]
fn unratified_identity_citation_does_not_trigger_an_fd_merge() {
    let world = world(vec![
        functional("rank", "D-1"),
        certain_fact("rank", "nara", "gold", "D-1"),
        certain_fact("rank", "moira", "silver", "D-1"),
        same_as("nara", "moira", Some(ratified("D-404"))), // D-404 is never `ratify`d
    ]);
    let findings = check::run(&world);
    assert!(
        findings_with_code(&findings, "continuity.fd-violation").is_empty(),
        "an uncited-to-nowhere identity must not merge slots: {findings:?}"
    );
    // The forged-authority citation is still honestly reported.
    assert!(
        findings
            .iter()
            .any(|f| f.code == "citation.unknown-decision"),
        "the unknown citation on the same-as link is still flagged: {findings:?}"
    );
}

// ===========================================================================
// 4. Value-side unification is not itself a violation: two DIFFERENT
//    predicates' values that happen to be an identity-merged pair
//    (`spouse(x, a)` / `spouse(x, b)` under a certain `a = b`) collapse to
//    ONE value, not a functional-dependency break.
// ===========================================================================
#[test]
fn value_side_unification_is_not_a_violation() {
    let world = world(vec![
        functional("spouse", "D-1"),
        certain_fact("spouse", "tristan", "iseult", "D-1"),
        certain_fact("spouse", "tristan", "the-fair-one", "D-1"),
        same_as("iseult", "the-fair-one", Some(ratified("D-9"))),
    ]);
    let findings = check::run(&world);
    assert!(
        findings_with_code(&findings, "continuity.fd-violation").is_empty(),
        "one bearer under two value-side names is one value, not a break: {findings:?}"
    );
}

// ===========================================================================
// 5/6. Fluent combine: an identity-merged class's timelines concatenate.
//      Disjoint spans across the two aliased names raise nothing; genuinely
//      overlapping certain spans raise exactly one fluent-contradiction.
// ===========================================================================
#[test]
fn fluent_combine_disjoint_spans_across_aliases_raise_nothing() {
    let world = world(vec![
        entity(EntityKind::Event, "coronation", "D-1"),
        state(
            "nara.station",
            "commons",
            Some(TimeClause::Until(id("coronation"))),
            "D-1",
        ),
        state(
            "moira.station",
            "throne",
            Some(TimeClause::Since(id("coronation"))),
            "D-1",
        ),
        same_as("nara", "moira", Some(ratified("D-9"))),
    ]);
    let findings = check::run(&world);
    assert!(
        findings_with_code(&findings, "continuity.fluent-contradiction").is_empty(),
        "disjoint spans across a merged class are ordinary history: {findings:?}"
    );
}

#[test]
fn fluent_combine_overlapping_certain_spans_raise_one_contradiction() {
    let world = world(vec![
        state("nara.station", "commons", None, "D-1"),
        state("moira.station", "throne", None, "D-1"),
        same_as("nara", "moira", Some(ratified("D-9"))),
    ]);
    let findings = check::run(&world);
    let fluent = findings_with_code(&findings, "continuity.fluent-contradiction");
    assert_eq!(
        fluent.len(),
        1,
        "two certain, fully-open, distinct-valued spans on a merged class must clash exactly once: {fluent:?}"
    );
    assert!(
        fluent[0].message.contains("commons") && fluent[0].message.contains("throne"),
        "{}",
        fluent[0].message
    );
}

// ===========================================================================
// 7. Intra-episode fluent-skip consults every class member: a slot that is
//    itself timeless is still skipped by the flat intra-episode sweep once
//    its identity-merged partner carries a fluent timeline.
// ===========================================================================
#[test]
fn intra_episode_fluent_skip_consults_every_class_member() {
    let world = world(vec![
        // moira.station is a fluent (has a `state` declaration and timeline);
        // nara.station is not — but nara and moira are one bearer.
        state("moira.station", "throne", None, "D-1"),
        same_as("nara", "moira", Some(ratified("D-9"))),
    ]);
    // Two claims setting the (canon-merged) slot to two different values in one
    // draft would ordinarily be `continuity.intra-episode-contradiction` for a
    // single-valued slot — but the class holds a fluent member, so it must be
    // judged over intervals (fluent_contradictions), not flagged flatly here.
    let claims = vec![
        atom("station", &["nara", "commons"]),
        atom("station", &["nara", "throne"]),
    ];
    let findings = check::continuity::check_claims(&world, &claims);
    assert!(
        findings_with_code(&findings, "continuity.intra-episode-contradiction").is_empty(),
        "a class with a fluent member must be interval-judged, not flat-flagged: {findings:?}"
    );
}

// ===========================================================================
// 8. The v1 temporal boundary: a `same-as` naming a declared era/event is
//    refused loudly. Nothing half-merges.
// ===========================================================================
#[test]
fn temporal_same_as_over_two_events_is_refused() {
    let world = world(vec![
        entity(EntityKind::Event, "the-crowning", "D-1"),
        entity(EntityKind::Event, "the-anointing", "D-1"),
        same_as("the-crowning", "the-anointing", Some(ratified("D-9"))),
    ]);
    let findings = check::run(&world);
    let refusal = findings_with_code(&findings, "continuity.identity-temporal-unsupported");
    assert_eq!(
        refusal.len(),
        1,
        "the temporal same-as is refused once: {refusal:?}"
    );
    assert_eq!(refusal[0].severity, check::Severity::Error);

    // Nothing half-merges: the identity's own atom stays certain (it IS staged
    // as an ordinary fact), but no other predicate on either name is merged —
    // there is no functional/fluent/rewrite consequence, because the closure
    // mechanism reads the SAME atom stream and the refusal is orthogonal to it
    // (it does not remove the same-as fact — it is a loud, additional finding).
    // Regression: no derived `before` edge and no chronology corruption.
    assert!(
        world.model.world.chronology.edges.is_empty(),
        "a same-as link must never add a chronology edge: {:?}",
        world.model.world.chronology.edges
    );
}

// ===========================================================================
// 9. A killed token surfaces even inside a `same-as` argument — the identity
//    kernel gets no exemption from the negative-constraint store.
// ===========================================================================
#[test]
fn killed_token_inside_a_same_as_argument_is_rejected_lore() {
    let killed = KilledTokens::new(vec![KilledToken {
        token: "irondale".to_owned(),
        mode: MatchMode::Word,
        decision: "D-0101".to_owned(),
        scope: "region name".to_owned(),
    }]);
    let world =
        world(vec![same_as("nara", "irondale", Some(ratified("D-9")))]).with_killed_tokens(killed);
    let findings = check::run(&world);
    let rejected = findings_with_code(&findings, "continuity.rejected-lore");
    assert_eq!(
        rejected.len(),
        1,
        "a killed token inside a same-as argument is still caught: {rejected:?}"
    );
}

// ===========================================================================
// 10. Koan-4 permanent regression: every `Decl::SameAs` reaches the world (as
//     a fact + atom) or raises a graded finding — never a silent drop.
//     Missing provenance and malformed provenance both fail closed, exactly
//     as `Decl::Before` does.
// ===========================================================================
#[test]
fn missing_provenance_fails_closed_with_a_finding() {
    let world = world(vec![same_as("nara", "moira", None)]);
    assert!(
        world
            .model
            .findings()
            .iter()
            .any(|f| matches!(f, LoadFinding::MissingProvenance { decl, .. } if decl.contains("nara") || decl.contains("identity"))),
        "a same-as with no provenance is a claim from nowhere, reported not dropped: {:?}",
        world.model.findings()
    );
    // Fail-closed: the atom is still present (nothing is silently dropped —
    // koan 4) but it can never be certain.
    let has_same_as_atom = world
        .model
        .atoms()
        .iter()
        .any(|a| a.atom.pred.text == "same-as");
    assert!(has_same_as_atom, "the atom is recorded, just fail-closed");
    let findings = check::run(&world);
    assert!(
        findings_with_code(&findings, "continuity.fd-violation").is_empty(),
        "a fail-closed identity never merges anything"
    );
}

#[test]
fn malformed_provenance_fails_closed_with_a_finding() {
    // A `decided`/`ratified` marker with no citation at all is malformed.
    let malformed = Annotation {
        marker: ProvenanceMarker::Ratified,
        citations: Vec::new(),
        condition: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    };
    let world = world(vec![same_as("nara", "moira", Some(malformed))]);
    assert!(
        world
            .model
            .findings()
            .iter()
            .any(|f| matches!(f, LoadFinding::MalformedProvenance { .. })),
        "a citation-requiring marker with no citation is malformed, reported not dropped: {:?}",
        world.model.findings()
    );
}

// ===========================================================================
// rt-1a FINDING 1 (permanent regression, check-layer side): a PLAIN
// `fact same-as(bob, robert)` must not drive the FD merge either — the
// check-layer twin of `same_as_honesty.rs`'s eval-layer pin. `headline_fd_
// violation_names_both_values_and_the_identity_link` above is this test's
// positive guard: the real `same-as` DECLARATION for the identical shape does
// fire the violation.
// ===========================================================================
#[test]
fn plain_same_as_fact_does_not_trigger_an_fd_merge() {
    let world = world(vec![
        functional("rank", "D-1"),
        certain_fact("same-as", "bob", "robert", "D-1"),
        certain_fact("rank", "bob", "gold", "D-1"),
        certain_fact("rank", "robert", "silver", "D-1"),
    ]);
    let findings = check::run(&world);
    assert!(
        findings_with_code(&findings, "continuity.fd-violation").is_empty(),
        "a plain same-as fact is not a declaration and must not merge bob/robert: {findings:?}"
    );
}
