#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Regression probes for the issue 47/48 fluent wiring, born from an adversarial
//! review. The load-bearing cases are the interval overlaps witnessed *away from*
//! a span's own `since`/`until` anchor — the common past of two `until` spans and
//! the shared downstream event of two unordered `since` spans — which a
//! naive anchor-only sweep silently misses. The check must sweep every declared
//! time point to catch them, without false-positiving on merely-unordered spans.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    AttrDecl, BeforeDecl, Decl, Document, EntityDecl, EntityKind, InertialDecl, RatifyDecl,
    SameAsDecl, ScopeHeader, StateDecl, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_world::check::factcheck::{Verdict, factcheck};
use dli_world::check::{self, CompiledWorld, WorldFinding};
use dli_world::compile;
use dli_world::pack::Pack;

// --- builders ---------------------------------------------------------------

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

fn ratify_decl(decision: &str) -> Decl {
    Decl::Ratify(RatifyDecl {
        decision: id(decision),
        body: dli_ast::language::RatifyBody::default(),
    })
}

fn event(name: &str, cite: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind: EntityKind::Event,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(ratified(cite)),
    })
}

fn before(earlier: &str, later: &str, cite: &str) -> Decl {
    Decl::Before(BeforeDecl {
        earlier: id(earlier),
        later: id(later),
        provenance: Some(ratified(cite)),
    })
}

fn state(
    name: &str,
    value: Option<&str>,
    time: Option<TimeClause>,
    prov: Option<Annotation>,
) -> Decl {
    Decl::State(StateDecl {
        name: id(name),
        value: value.map(|v| Term::Sym(id(v))),
        time,
        provenance: prov,
    })
}

fn attr(subject: &str, slot: &str, value: &str, prov: Option<Annotation>) -> Decl {
    Decl::Attr(AttrDecl {
        subject: id(subject),
        attr: id(slot),
        value: Term::Sym(id(value)),
        provenance: prov,
    })
}

fn inertial(pred: &str, prov: Option<Annotation>) -> Decl {
    Decl::Inertial(InertialDecl {
        pred: id(pred),
        provenance: prov,
    })
}

fn same_as(left: &str, right: &str, prov: Option<Annotation>) -> Decl {
    Decl::SameAs(SameAsDecl {
        left: id(left),
        right: id(right),
        provenance: prov,
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

fn fluent_findings(world: &CompiledWorld) -> Vec<String> {
    check::run(world)
        .into_iter()
        .filter(|f| f.code == "continuity.fluent-contradiction")
        .map(|f| f.message)
        .collect()
}

/// The `continuity.fluent-order-undeclared` findings (WS-4) — the full
/// [`WorldFinding`], not just the message, so a test can also inspect
/// `data`'s repair candidates.
fn order_gap_findings(world: &CompiledWorld) -> Vec<WorldFinding> {
    check::run(world)
        .into_iter()
        .filter(|f| f.code == "continuity.fluent-order-undeclared")
        .collect()
}

// --- overlaps witnessed away from span anchors (must sweep all points) ------

/// Two `since` spans whose anchors are UNORDERED but both precede a common
/// downstream event e3. At e3 both spans hold certainly => genuine contradiction,
/// even though e3 is neither span's own anchor. Must be flagged exactly once.
#[test]
fn two_since_converging_at_downstream_event_clashes() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("e1", "D-1"),
            event("e2", "D-1"),
            event("e3", "D-1"),
            before("e1", "e3", "D-1"),
            before("e2", "e3", "D-1"),
            // e1 and e2 are deliberately unordered w.r.t. each other.
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("e1"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("e2"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    let findings = fluent_findings(&world);
    assert_eq!(
        findings.len(),
        1,
        "a genuine certain overlap at the convergence event e3 must be flagged, got: {findings:?}"
    );
}

/// Two `until flood` spans with distinct values (both open-start). They both
/// hold over ALL pre-flood time; at `dawn` (before flood) both cover certainly
/// => contradiction. The witness is not either span's own anchor (`flood`, where
/// `until` is exclusive so neither covers). Must be flagged exactly once.
#[test]
fn two_until_spans_over_common_past_clash() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("dawn", "D-1"),
            event("flood", "D-1"),
            before("dawn", "flood", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Until(id("flood"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Until(id("flood"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    let findings = fluent_findings(&world);
    assert_eq!(
        findings.len(),
        1,
        "two distinct values both holding over the common past (before flood) must clash, got: {findings:?}"
    );
}

// --- true positives / negatives we must NOT break (front 2) -----------------

#[test]
fn disjoint_until_since_zero() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("enclosure", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Until(id("enclosure"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "disjoint spans are one fluent"
    );
}

#[test]
fn until_e2_and_since_e1_overlap_on_the_interval() {
    // commons until e2, enclosed since e1, with before e1 -> e2. commons holds
    // (-inf, e2) and enclosed holds [e1, inf), so they overlap on [e1, e2) — a
    // genuine contradiction, witnessed at e1.
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("e1", "D-1"),
            event("e2", "D-1"),
            before("e1", "e2", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Until(id("e2"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("e1"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    let findings = fluent_findings(&world);
    assert_eq!(
        findings.len(),
        1,
        "commons until e2 and enclosed since e1 overlap on [e1,e2): {findings:?}"
    );
}

#[test]
fn fully_open_two_values_one() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            state("gwern.status", Some("commons"), None, Some(ratified("D-1"))),
            state(
                "gwern.status",
                Some("enclosed"),
                None,
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        1,
        "two open distinct values clash"
    );
}

#[test]
fn fully_open_same_value_zero() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            state("gwern.status", Some("commons"), None, Some(ratified("D-1"))),
            state("gwern.status", Some("commons"), None, Some(ratified("D-1"))),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "same value twice is no clash"
    );
}

#[test]
fn overlap_same_value_since_zero() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("enclosure", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "same value overlapping is no clash"
    );
}

#[test]
fn proposed_overlap_zero() {
    // Both spans proposed (Possible, not Certain) => not a certain overlap.
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("enclosure", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(marker(ProvenanceMarker::Proposed, &[])),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(marker(ProvenanceMarker::Proposed, &[])),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "proposed spans are not certain"
    );
}

#[test]
fn mixed_certain_proposed_overlap_zero() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("enclosure", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(marker(ProvenanceMarker::Proposed, &[])),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(fluent_findings(&world).len(), 0, "only one span is certain");
}

#[test]
fn partial_order_unordered_since_no_false_positive() {
    // Two `since` spans with UNORDERED anchors and NO common successor.
    // No point where both certainly cover => must be 0 (OrderIndeterminate).
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("e1", "D-1"),
            event("e2", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("e1"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("e2"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "unordered anchors are OrderIndeterminate, not a certain clash"
    );
}

/// WS-4: the exact scenario above — two certain, distinct-valued `since`
/// spans with unordered anchors — is silent on `continuity.fluent-
/// contradiction` (soundness unchanged) but WARNS on `continuity.fluent-
/// order-undeclared`, naming `e1`/`e2` as the settleable pair.
#[test]
fn order_gap_warns_on_settleable_indeterminacy() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("e1", "D-1"),
            event("e2", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("e1"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("e2"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(fluent_findings(&world).len(), 0, "soundness unchanged");
    let warnings = order_gap_findings(&world);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].message.contains("commons") && warnings[0].message.contains("enclosed"),
        "{}",
        warnings[0].message
    );
    let data = warnings[0].data.as_ref().expect("data payload");
    let candidates = data["repair"]["candidates"]
        .as_array()
        .expect("candidates array");
    assert!(
        candidates.iter().any(|pair| {
            let pair = pair.as_array().expect("a pair");
            let names: Vec<&str> = pair.iter().map(|p| p.as_str().unwrap()).collect();
            names.contains(&"e1") && names.contains(&"e2")
        }),
        "candidates must name the settleable pair e1/e2: {candidates:?}"
    );
    assert_eq!(data["repair"]["kind"], serde_json::json!("declare-order"));
}

/// Disjoint spans (the same anchor closing one and opening the other) are not
/// merely unordered — the anchor comparison is trivially decided (equal), so
/// no repair candidate exists and the warn stays silent, exactly as
/// `disjoint_until_since_zero` pins for the contradiction check.
#[test]
fn disjoint_spans_do_not_warn() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("enclosure", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Until(id("enclosure"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("enclosure"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(fluent_findings(&world).len(), 0);
    assert_eq!(
        order_gap_findings(&world).len(),
        0,
        "a shared anchor is trivially ordered against itself, not indeterminate"
    );
}

/// Two order-indeterminate spans over the SAME value are not a genuine
/// ambiguity — only a distinct-value pair signals a settleable gap worth
/// repairing.
#[test]
fn same_value_indeterminate_spans_do_not_warn() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("e1", "D-1"),
            event("e2", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("e1"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("e2"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(fluent_findings(&world).len(), 0);
    assert_eq!(
        order_gap_findings(&world).len(),
        0,
        "one value backed twice is not an ambiguity to repair"
    );
}

/// rt-features SEV-B repro: the clean single-anchor transition (`commons
/// until x`, `enclosed since x`) is silent — with an UNRELATED third event
/// present and unordered against `x`. An earlier point-sweep design tested
/// each span's cover against `other` (a probe point neither span is
/// anchored to) rather than the two spans against EACH OTHER, and
/// false-positived here: at `other`, both spans read `OrderIndeterminate`
/// (their shared anchor `x` is unordered against `other`), which the buggy
/// check misread as two distinct order-indeterminate values needing a
/// repair. There is no repair to make: `x` is a SHARED anchor (`a.until ==
/// b.since`), so the two spans can never overlap under any order at all,
/// regardless of where `other` falls. This is the exact shape
/// `examples/camelot.dli`'s `excalibur.keeper` and
/// `examples/norse/nine-realms.dli`'s `sol.course` both use.
#[test]
fn sev_b_shared_anchor_transition_is_silent_even_with_an_unrelated_unordered_point() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("x", "D-1"),
            event("other", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Until(id("x"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("x"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "a clean disjoint transition, never a contradiction"
    );
    assert_eq!(
        order_gap_findings(&world).len(),
        0,
        "a shared anchor is provably disjoint under every order; `other` is irrelevant"
    );
}

/// The real-corpus counterpart to the synthetic repro above: rt-features
/// named `examples/norse/nine-realms.dli`'s `sol.course` as a second live
/// false positive under the old point-sweep design, alongside camelot's
/// `excalibur.keeper` (both pinned in `dispatch.rs`'s
/// `round_table_check_stays_clean`, which only covers camelot). This
/// compiles the actual committed source — not a synthetic rebuild — so it
/// stays honest if the corpus changes shape.
///
/// `sol.course` is `crossing until the-last-battle` / `devoured since
/// the-last-battle`: the identical shared-anchor pattern, so it must stay
/// silent under the corrected pairwise check.
#[test]
fn sev_b_nine_realms_sol_course_is_silent() {
    let source = read("examples/norse/nine-realms.dli");
    let (document, diagnostics) = dli_syntax::parse_document(&source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "nine-realms.dli must parse clean, got {diagnostics:?}"
    );
    let world = compile(&Pack {
        name: "nine-realms".to_owned(),
        documents: vec![document],
    })
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "sol.course's until/since transition at the-last-battle is a clean disjoint span, never a contradiction"
    );
    assert_eq!(
        order_gap_findings(&world).len(),
        0,
        "sol.course's shared anchor is provably disjoint under every order"
    );
}

fn repo(rel: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn era(name: &str, cite: &str) -> Decl {
    Decl::Entity(EntityDecl {
        kind: EntityKind::Era,
        name: id(name),
        label: None,
        time: None,
        slots: Vec::new(),
        provenance: Some(ratified(cite)),
    })
}

#[test]
fn during_era_overlap_one() {
    // Two distinct values both `during oldtime` -> both open at the era point,
    // so they overlap and clash. Confirms the `during`/era clause threads through.
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            era("oldtime", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::During(id("oldtime"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::During(id("oldtime"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        1,
        "two distinct `during` values overlap"
    );
}

#[test]
fn open_and_until_over_common_past_clash() {
    // An all-open value (since None until None) and an `until flood` value, both
    // distinct. They overlap over all pre-flood time; witnessed at dawn (before
    // flood), which is not a span anchor (anchors = {flood}, where until is
    // exclusive). Another overlap the sweep must catch by sampling all points.
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("dawn", "D-1"),
            event("flood", "D-1"),
            before("dawn", "flood", "D-1"),
            state("gwern.status", Some("commons"), None, Some(ratified("D-1"))),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Until(id("flood"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        1,
        "open value and until-flood value overlap over the common past: {:?}",
        fluent_findings(&world)
    );
}

// --- documented boundaries of the fluent feature ----------------------------

#[test]
fn attr_and_state_on_one_slot_is_a_documented_boundary() {
    // A slot carrying BOTH a timeless attr (`color = blue`) and a fluent state
    // (`color = red`) is an authoring smell the fluent check does not adjudicate:
    // the attr value never enters the slot's timeline, and there is no
    // canon-internal single-valued check to compare them (there never was — it is
    // a claim-relative class). This is out of scope for issues 47/48, not a
    // regression; the test pins the boundary so a future canon-internal check is
    // a deliberate choice, not an accident.
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            attr("gwern", "color", "blue", Some(ratified("D-1"))),
            state("gwern.color", Some("red"), None, Some(ratified("D-1"))),
        ],
    ))
    .expect("compiles");
    let fluent: Vec<_> = check::run(&world)
        .into_iter()
        .filter(|f| f.code == "continuity.fluent-contradiction")
        .collect();
    // The timeline knows only the state's value.
    assert!(fluent.iter().all(|f| f.message.contains("red")));
    // And the fluent slot is never fact-check-contradicted (interval semantics).
    let reports = factcheck(
        &world,
        &[AtomExpr {
            pred: id("color"),
            args: vec![Term::Sym(id("gwern")), Term::Sym(id("green"))],
        }],
    );
    assert_ne!(
        reports[0].verdict,
        Verdict::Contradicted,
        "fluent slot never contradicts"
    );
}

#[test]
fn timeless_double_assertion_on_a_fluent_slot_is_not_a_contradiction() {
    // Canon has a single ratified state, so slot (status, gwern) is a fluent. A
    // draft asserting TWO distinct values with NO time context is *not* treated
    // as a contradiction: on a time-varying slot two bare values can be a
    // legitimate transition (commons THEN enclosed), and there is no span to
    // prove they overlap. This is the deliberate inverse of the disjoint-span
    // false positive the fluent skip exists to avoid; genuine overlaps are
    // adjudicated by `fluent_contradictions` over the compiled world's spans,
    // where staged states DO carry their intervals.
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            state("gwern.status", Some("commons"), None, Some(ratified("D-1"))),
        ],
    ))
    .expect("compiles");
    let findings = dli_world::check::continuity::check_claims(
        &world,
        &[
            AtomExpr {
                pred: id("status"),
                args: vec![Term::Sym(id("gwern")), Term::Sym(id("a"))],
            },
            AtomExpr {
                pred: id("status"),
                args: vec![Term::Sym(id("gwern")), Term::Sym(id("b"))],
            },
        ],
    );
    let intra = findings
        .iter()
        .filter(|f| f.code == "continuity.intra-episode-contradiction")
        .count();
    assert_eq!(
        intra, 0,
        "intra is suppressed on fluent slots: {findings:?}"
    );
}

// --- inertia (DL-0081): derived fluent persistence --------------------------

/// Every `WorldFinding` of a given code from the world-internal battery.
fn findings_of(world: &CompiledWorld, code: &str) -> Vec<WorldFinding> {
    check::run(world)
        .into_iter()
        .filter(|f| f.code == code)
        .collect()
}

/// The declaration set for an inertial world: `ratify D-1` and
/// `inertial <pred> ratified D-1`, so capping is active (the decl reads
/// certain).
fn inertial_status_decls() -> Vec<Decl> {
    vec![
        ratify_decl("D-1"),
        inertial("status", Some(ratified("D-1"))),
    ]
}

/// Transition (silent): `commons since dawn`, `enclosed since noon`, with
/// `dawn before noon`. On an inertial slot the earlier value is capped at the
/// successor's anchor, so nothing clashes and nothing is contested.
#[test]
fn inertial_ordered_transition_is_silent() {
    let mut decls = inertial_status_decls();
    decls.extend([
        event("dawn", "D-1"),
        event("noon", "D-1"),
        before("dawn", "noon", "D-1"),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Since(id("dawn"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("noon"))),
            Some(ratified("D-1")),
        ),
    ]);
    let world = compile(&pack("weir", decls)).expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "an ordered inertial transition is silent"
    );
    assert_eq!(
        order_gap_findings(&world).len(),
        0,
        "declared order, no contest"
    );
}

/// The SAME two open values on a predicate NOT declared inertial clash — the
/// declaration-only contrast that proves inertia changes behavior and a
/// non-inertial slot is provably untouched.
#[test]
fn non_inertial_open_transition_still_clashes() {
    let world = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            event("dawn", "D-1"),
            event("noon", "D-1"),
            before("dawn", "noon", "D-1"),
            state(
                "gwern.status",
                Some("commons"),
                Some(TimeClause::Since(id("dawn"))),
                Some(ratified("D-1")),
            ),
            state(
                "gwern.status",
                Some("enclosed"),
                Some(TimeClause::Since(id("noon"))),
                Some(ratified("D-1")),
            ),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        1,
        "without an `inertial` declaration the two open values clash forward, as ever"
    );
}

/// Contested (warn): `commons since e1`, `enclosed since e2`, anchors
/// unordered. Neither is certainly current downstream, so no clash — but a
/// `continuity.fluent-order-undeclared` warn names the `e1`/`e2` pair, and the
/// data carries `inertial: true` with unbounded (null) derived `until`s.
#[test]
fn inertial_unordered_transition_is_contested() {
    let mut decls = inertial_status_decls();
    decls.extend([
        event("e1", "D-1"),
        event("e2", "D-1"),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Since(id("e1"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("e2"))),
            Some(ratified("D-1")),
        ),
    ]);
    let world = compile(&pack("weir", decls)).expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "unordered anchors never clash"
    );
    let warns = order_gap_findings(&world);
    assert_eq!(warns.len(), 1, "{warns:?}");
    let data = warns[0].data.as_ref().expect("data");
    assert_eq!(data["inertial"], serde_json::json!(true));
    let candidates = data["repair"]["candidates"].as_array().expect("candidates");
    assert!(
        candidates.iter().any(|pair| {
            let names: Vec<&str> = pair
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap())
                .collect();
            names.contains(&"e1") && names.contains(&"e2")
        }),
        "the repair names the since/since pair: {candidates:?}"
    );
    // Two open, unordered values persist unbounded — no certain Before-defeater.
    for span in data["spans"].as_array().expect("spans") {
        assert!(
            span["until"].is_null(),
            "unbounded until for an uncapped value: {span:?}"
        );
    }
}

/// Clash (error): two distinct values `since e` (the SAME point). They
/// certainly co-hold and inertia cannot linearize them, so
/// `continuity.fluent-contradiction` fires — and the order-undeclared warn
/// never manufactures a `before(e, e)` candidate (B.1 pin).
#[test]
fn inertial_same_point_distinct_values_clash() {
    let mut decls = inertial_status_decls();
    decls.extend([
        event("e", "D-1"),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Since(id("e"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("e"))),
            Some(ratified("D-1")),
        ),
    ]);
    let world = compile(&pack("weir", decls)).expect("compiles");
    let clash = fluent_findings(&world);
    assert_eq!(
        clash.len(),
        1,
        "same-point distinct values are a clash: {clash:?}"
    );
    assert_eq!(
        order_gap_findings(&world).len(),
        0,
        "a clash is never a settleable order gap"
    );
    // The clash finding carries the inertial marker in its data.
    let finding = findings_of(&world, "continuity.fluent-contradiction");
    assert_eq!(
        finding[0].data.as_ref().expect("data")["inertial"],
        serde_json::json!(true)
    );
}

/// The returning-value chain (A.1 pin): `commons@dawn`, `enclosed@noon`,
/// `commons@dusk` (dawn<noon<dusk) is SILENT — each point has exactly one
/// certain value and the first `commons` never re-extends. The precise
/// boundaries are pinned at the projection level in `dli-time`; here we hold
/// the check-level silence.
#[test]
fn inertial_returning_value_chain_is_silent() {
    let mut decls = inertial_status_decls();
    decls.extend([
        event("dawn", "D-1"),
        event("noon", "D-1"),
        event("dusk", "D-1"),
        before("dawn", "noon", "D-1"),
        before("noon", "dusk", "D-1"),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Since(id("dawn"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("noon"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Since(id("dusk"))),
            Some(ratified("D-1")),
        ),
    ]);
    let world = compile(&pack("weir", decls)).expect("compiles");
    assert_eq!(fluent_findings(&world).len(), 0, "a clean return is silent");
    assert_eq!(
        order_gap_findings(&world).len(),
        0,
        "every anchor order is declared"
    );
}

/// A same-point clash with a later certain defeater reports the DERIVED `until`
/// (the defeater's `since`), not `null`: `commons@dawn` / `enclosed@dawn` clash,
/// and both are capped at `mist@noon` (dawn before noon).
#[test]
fn inertial_finding_data_reports_derived_until() {
    let mut decls = inertial_status_decls();
    decls.extend([
        event("dawn", "D-1"),
        event("noon", "D-1"),
        before("dawn", "noon", "D-1"),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Since(id("dawn"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("dawn"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("mist"),
            Some(TimeClause::Since(id("noon"))),
            Some(ratified("D-1")),
        ),
    ]);
    let world = compile(&pack("weir", decls)).expect("compiles");
    let finding = findings_of(&world, "continuity.fluent-contradiction");
    assert_eq!(
        finding.len(),
        1,
        "the same-point pair still clashes: {finding:?}"
    );
    let data = finding[0].data.as_ref().expect("data");
    assert_eq!(data["inertial"], serde_json::json!(true));
    for span in data["spans"].as_array().expect("spans") {
        // Only the two clashing values (commons, enclosed) are reported; both
        // are capped at noon by the later certain `mist`.
        assert_eq!(
            span["until"],
            serde_json::json!("noon"),
            "derived cap at the defeater: {span:?}"
        );
    }
}

/// An authored `until` overrides derivation: `commons until enclosure` /
/// `enclosed since enclosure` is a clean disjoint transition even on an
/// inertial slot — byte-unaffected from the non-inertial reading.
#[test]
fn inertial_authored_until_overrides() {
    let mut decls = inertial_status_decls();
    decls.extend([
        event("enclosure", "D-1"),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Until(id("enclosure"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("enclosure"))),
            Some(ratified("D-1")),
        ),
    ]);
    let world = compile(&pack("weir", decls)).expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "authored disjoint spans never clash"
    );
    assert_eq!(order_gap_findings(&world).len(), 0);
}

/// Koan 4: an inertial value with no `since` anchor cannot produce a
/// persistence interval, so `continuity.inertia-unanchored` names the slot
/// (warn). A predicate never declared inertial is untouched.
#[test]
fn inertial_unanchored_value_warns() {
    let mut decls = inertial_status_decls();
    decls.push(state(
        "gwern.status",
        Some("commons"),
        None,
        Some(ratified("D-1")),
    ));
    let world = compile(&pack("weir", decls)).expect("compiles");
    let warns = findings_of(&world, "continuity.inertia-unanchored");
    assert_eq!(
        warns.len(),
        1,
        "an unanchored inertial value warns: {warns:?}"
    );
    assert!(warns[0].message.contains("commons"), "{}", warns[0].message);

    // The same open value on a NON-inertial predicate is ordinary open lore.
    let plain = compile(&pack(
        "weir",
        vec![
            ratify_decl("D-1"),
            state("gwern.status", Some("commons"), None, Some(ratified("D-1"))),
        ],
    ))
    .expect("compiles");
    assert_eq!(
        findings_of(&plain, "continuity.inertia-unanchored").len(),
        0,
        "declaration-only: a non-inertial predicate is untouched"
    );
}

/// Defeat without recompile at the check level: a silent inertial transition,
/// then flip the declaration's citing decision false over the SAME compiled
/// world — capping reverts to plain projection and the open spans clash again
/// (the value never vanished). Mirrors the fork-suppose "defeat is a valuation
/// flip" pattern.
#[test]
fn defeating_the_inertia_declaration_reverts_to_plain_projection() {
    let mut decls = vec![
        ratify_decl("D-inertia"),
        inertial("status", Some(ratified("D-inertia"))),
    ];
    decls.extend([
        ratify_decl("D-1"),
        event("dawn", "D-1"),
        event("noon", "D-1"),
        before("dawn", "noon", "D-1"),
        state(
            "gwern.status",
            Some("commons"),
            Some(TimeClause::Since(id("dawn"))),
            Some(ratified("D-1")),
        ),
        state(
            "gwern.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("noon"))),
            Some(ratified("D-1")),
        ),
    ]);
    let mut world = compile(&pack("weir", decls)).expect("compiles");
    assert_eq!(
        fluent_findings(&world).len(),
        0,
        "capping active: transition is silent"
    );

    // Defeat the declaration — a valuation flip, no recompile.
    let var = world
        .model
        .var_atom("D-inertia")
        .expect("the decl's decision is interned");
    world.model.world.valuation.set(var, false);
    assert_eq!(
        fluent_findings(&world).len(),
        1,
        "a defeated declaration reverts to plain open-interval projection: the spans clash again"
    );
}

/// Identity interaction: two inertial values on different subjects, merged by a
/// certain `same-as`, that certainly co-hold (`since e` each) clash — and the
/// finding's citations name the identity assertion. Defeating the merge
/// un-persists (un-merges), so the clash disappears.
#[test]
fn inertial_identity_merge_clashes_and_defeat_un_persists() {
    let decls = vec![
        ratify_decl("D-inertia"),
        ratify_decl("D-a"),
        ratify_decl("D-b"),
        ratify_decl("D-merge"),
        inertial("status", Some(ratified("D-inertia"))),
        event("e", "D-a"),
        state(
            "a.status",
            Some("commons"),
            Some(TimeClause::Since(id("e"))),
            Some(ratified("D-a")),
        ),
        state(
            "b.status",
            Some("enclosed"),
            Some(TimeClause::Since(id("e"))),
            Some(ratified("D-b")),
        ),
        same_as("a", "b", Some(ratified("D-merge"))),
    ];
    let mut world = compile(&pack("weir", decls)).expect("compiles");
    let clash = findings_of(&world, "continuity.fluent-contradiction");
    assert_eq!(
        clash.len(),
        1,
        "merged inertial values co-holding at e clash: {clash:?}"
    );
    assert!(
        clash[0].citations.contains(&"D-merge".to_owned()),
        "the clash names the identity assertion: {:?}",
        clash[0].citations
    );

    // Defeat the merge: a and b un-merge, so there is no shared slot to clash.
    let var = world
        .model
        .var_atom("D-merge")
        .expect("merge decision interned");
    world.model.world.valuation.set(var, false);
    assert_eq!(
        findings_of(&world, "continuity.fluent-contradiction").len(),
        0,
        "defeating the merge un-persists the shared slot"
    );
}
