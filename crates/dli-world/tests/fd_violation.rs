#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `continuity.fd-violation` (issue 49): canon that already breaks a functional
//! dependency — one subject carrying two distinct certain values for a
//! single-valued predicate — is a finding on the *load*, not only on a merge.
//!
//! The load-bearing cases are the two ways a predicate earns "functional".
//! Declared (`functional <pred>`) judges a predicate on its first sighting, which
//! canon precedent alone cannot. Undeclared, the check infers it from the
//! predicate's *other* subjects, and stays silent when there are none — a novel
//! two-valued key is conservatively a genuine relation, never a slip. Distinctness
//! is on the normalized key throughout, so `captain` and `Captain` are one value.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    Decl, Document, FactDecl, FunctionalDecl, RatifyDecl, ScopeHeader, StateDecl, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_world::check::{self, CompiledWorld};
use dli_world::compile;
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

/// `ratify <decision>` — the decision is declared, so a citation to it carries
/// authority and the citing fact reads certain.
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

/// A certain fact: `pred(subject, value)` citing the ratified decision `D-1`.
fn certain_fact(pred: &str, subject: &str, value: &str) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, &[subject, value]),
        triple: false,
        provenance: Some(ratified("D-1")),
    })
}

/// `functional <pred>` — the author's single-valuedness declaration.
fn functional(pred: &str) -> Decl {
    Decl::Functional(FunctionalDecl {
        pred: id(pred),
        provenance: Some(ratified("D-1")),
    })
}

fn state(name: &str, value: &str, time: Option<TimeClause>) -> Decl {
    Decl::State(StateDecl {
        name: id(name),
        value: Some(Term::Sym(id(value))),
        time,
        provenance: Some(ratified("D-1")),
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

/// Compile a world whose ledger already declares `D-1`, so every `certain_fact`
/// above reads certain.
fn world(decls: Vec<Decl>) -> CompiledWorld {
    let mut all = vec![ratify_decl("D-1")];
    all.extend(decls);
    compile(&pack("fleet", all)).expect("compiles")
}

/// The `continuity.fd-violation` findings `check::run` raises over a world.
fn fd_findings(world: &CompiledWorld) -> Vec<String> {
    check::run(world)
        .into_iter()
        .filter(|f| f.code == "continuity.fd-violation")
        .map(|f| f.message)
        .collect()
}

// --- the declared route ------------------------------------------------------

#[test]
fn declared_functional_predicate_flags_two_certain_values() {
    // Sub-claim 4: `functional rank` judges a predicate canon has no precedent
    // for. Two certain ranks for one officer is a functional-dependency break.
    let world = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "nara", "admiral"),
    ]);
    let findings = fd_findings(&world);
    assert_eq!(
        findings.len(),
        1,
        "one finding for the one broken slot: {findings:?}"
    );
    assert!(
        findings[0].contains("rank")
            && findings[0].contains("nara")
            && findings[0].contains("captain")
            && findings[0].contains("admiral"),
        "names the slot and both values: {}",
        findings[0]
    );
}

#[test]
fn a_first_seen_predicate_without_the_declaration_is_not_flagged() {
    // The same two facts, undeclared, with no other subject to witness the
    // predicate's shape: canon cannot tell a functional slip from a genuine
    // multi-valued relation, so the conservative default is silence. This is
    // exactly the gap the `functional` declaration exists to close.
    let world = world(vec![
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "nara", "admiral"),
    ]);
    assert!(
        fd_findings(&world).is_empty(),
        "a novel two-valued key is not a violation: {:?}",
        fd_findings(&world)
    );
}

// --- the inferred route ------------------------------------------------------

#[test]
fn an_undeclared_hub_and_chain_topology_is_not_flagged() {
    // A subject with several edges while others carry one each is a *hub in a
    // many-to-many relation* just as readily as a functional-dependency break —
    // canon's shape cannot tell them apart (R2/B). Without a `functional`
    // declaration the engine must not guess: this exact `adjacent`-style topology
    // (one hub, several one-edge subjects) reds two bundled corpora if inferred,
    // so it must raise nothing here.
    let world = world(vec![
        certain_fact("adjacent", "camelot", "the-lake"),
        certain_fact("adjacent", "camelot", "camlann"),
        certain_fact("adjacent", "the-lake", "camelot"),
        certain_fact("adjacent", "camlann", "camelot"),
    ]);
    assert!(
        fd_findings(&world).is_empty(),
        "an undeclared predicate is never flagged by canon's shape alone: {:?}",
        fd_findings(&world)
    );
}

#[test]
fn a_declared_predicate_flags_regardless_of_other_subjects() {
    // The declaration is the only thing that arms the class. Once `rank` is
    // declared functional, nara's two ranks are a break — and the presence of a
    // one-ranked officer neither adds nor removes the signal.
    let world = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "nara", "admiral"),
        certain_fact("rank", "bryn", "ensign"),
    ]);
    let findings = fd_findings(&world);
    assert_eq!(
        findings.len(),
        1,
        "one finding, blamed on nara's slot: {findings:?}"
    );
    assert!(
        findings[0].contains("nara") && !findings[0].contains("bryn"),
        "blames the two-valued subject only: {}",
        findings[0]
    );
}

#[test]
fn a_genuine_multivalued_relation_is_never_flagged() {
    // R2/B's shape, at load: an alliance is many-to-many. `ally-of` appears for
    // nara alone, so nothing witnesses single-valuedness and nothing fires.
    let world = world(vec![
        certain_fact("ally-of", "nara", "bryn"),
        certain_fact("ally-of", "nara", "gwen"),
    ]);
    assert!(
        fd_findings(&world).is_empty(),
        "a multi-valued relation must not raise an FD violation: {:?}",
        fd_findings(&world)
    );
}

#[test]
fn a_multivalued_relation_stays_clean_with_other_subjects_present() {
    // And the inference itself must not misfire: `ally-of` here is two-valued for
    // *two* subjects, so no subject witnesses it as functional.
    let world = world(vec![
        certain_fact("ally-of", "nara", "bryn"),
        certain_fact("ally-of", "nara", "gwen"),
        certain_fact("ally-of", "bryn", "nara"),
        certain_fact("ally-of", "bryn", "gwen"),
    ]);
    assert!(
        fd_findings(&world).is_empty(),
        "two two-valued subjects witness a relation, not a dependency: {:?}",
        fd_findings(&world)
    );
}

// --- normalization -----------------------------------------------------------

#[test]
fn cosmetic_drift_is_one_value_not_a_violation() {
    // Sub-claim 1: distinctness is on the normalized key, so a case drift is the
    // same value restated — not a second one. A declared-functional predicate is
    // the strictest setting available, and it still stays silent here.
    let world = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "nara", "Captain"),
    ]);
    assert!(
        fd_findings(&world).is_empty(),
        "`captain` and `Captain` are one value: {:?}",
        fd_findings(&world)
    );
}

#[test]
fn separator_drift_is_one_value_too() {
    // The same fold covers hyphen/space drift — `bell-song` and `bell song`.
    let world = world(vec![
        functional("post"),
        certain_fact("post", "nara", "bell-song"),
        certain_fact("post", "nara", "bell song"),
    ]);
    assert!(
        fd_findings(&world).is_empty(),
        "`bell-song` and `bell song` are one value: {:?}",
        fd_findings(&world)
    );
}

#[test]
fn normalization_does_not_stem_distinct_values() {
    // …and the fold is only case and separators. `gold` and `golden` are two
    // values, so a declared-functional slot holding both is a real violation.
    let world = world(vec![
        functional("hue"),
        certain_fact("hue", "banner", "gold"),
        certain_fact("hue", "banner", "golden"),
    ]);
    assert_eq!(
        fd_findings(&world).len(),
        1,
        "`gold` and `golden` stay distinct: {:?}",
        fd_findings(&world)
    );
}

#[test]
fn normalization_folds_only_separators_not_all_punctuation() {
    // The fold is case + `-_` + whitespace only — never other punctuation. A
    // decimal `2.5` and a range `2-5` are genuinely distinct values; folding
    // every non-alphanumeric would merge them and silently drop the violation
    // (issue 49, koan 4). Declared functional, both certain, so it must fire.
    let world = world(vec![
        functional("price"),
        certain_fact("price", "loaf", "2.5"),
        certain_fact("price", "loaf", "2-5"),
    ]);
    assert_eq!(
        fd_findings(&world).len(),
        1,
        "`2.5` and `2-5` are two values, not one: {:?}",
        fd_findings(&world)
    );
}

// --- what the check must not touch -------------------------------------------

#[test]
fn a_fluent_slot_is_left_to_the_interval_check() {
    // A `state` decl is interval-sensitive: two values over disjoint spans is
    // ordinary history, judged by `continuity.fluent-contradiction`, never by the
    // flat FD sweep. The declaration must not drag a fluent into it.
    let world = world(vec![
        Decl::Entity(dli_ast::decl::EntityDecl {
            kind: dli_ast::decl::EntityKind::Event,
            name: id("promotion"),
            label: None,
            time: None,
            slots: Vec::new(),
            provenance: Some(ratified("D-1")),
        }),
        functional("rank"),
        Decl::Fact(FactDecl {
            atom: atom("rank", &["nara", "ensign"]),
            triple: false,
            provenance: Some(marker(ProvenanceMarker::Proposed, &[])),
        }),
        state(
            "nara.rank",
            "captain",
            Some(TimeClause::Until(id("promotion"))),
        ),
        state(
            "nara.rank",
            "admiral",
            Some(TimeClause::Since(id("promotion"))),
        ),
    ]);
    assert!(
        world
            .model
            .fluents
            .contains_key(&("rank".to_owned(), "nara".to_owned())),
        "the slot really is a fluent — otherwise this asserts nothing"
    );
    assert!(
        !check::run(&world)
            .iter()
            .any(|f| f.code == "continuity.pending-conflict")
    );
    assert!(
        fd_findings(&world).is_empty(),
        "disjoint fluent spans are not an FD violation: {:?}",
        fd_findings(&world)
    );
}

#[test]
fn a_shadow_value_is_not_a_second_certain_value() {
    // The sweep reads the certain layer only: a proposed alternative sits in the
    // shadow layer and is not a contradiction of the ratified value.
    let world = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        Decl::Fact(FactDecl {
            atom: atom("rank", &["nara", "admiral"]),
            triple: false,
            provenance: Some(marker(ProvenanceMarker::Proposed, &[])),
        }),
    ]);
    assert!(
        fd_findings(&world).is_empty(),
        "a proposed alternative is not a certain second value: {:?}",
        fd_findings(&world)
    );
}

#[test]
fn one_finding_per_slot_not_per_pair() {
    // Three certain values for one slot is still one finding — the finding is per
    // broken slot, and it names every value it found.
    let world = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "nara", "admiral"),
        certain_fact("rank", "nara", "ensign"),
    ]);
    let findings = fd_findings(&world);
    assert_eq!(findings.len(), 1, "one finding for the slot: {findings:?}");
    assert!(
        findings[0].contains("3 distinct certain values"),
        "counts all three: {}",
        findings[0]
    );
}

#[test]
fn two_broken_slots_raise_two_findings() {
    // …and the per-slot rule cuts the other way: two broken subjects on the same
    // declared predicate are two findings, not one merged report.
    let world = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "nara", "admiral"),
        certain_fact("rank", "bryn", "ensign"),
        certain_fact("rank", "bryn", "lieutenant"),
    ]);
    let findings = fd_findings(&world);
    assert_eq!(findings.len(), 2, "one per broken slot: {findings:?}");
}

#[test]
fn a_clean_declared_functional_world_is_silent() {
    // The null case: the declaration alone changes nothing about honest canon.
    let world = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "bryn", "ensign"),
    ]);
    assert!(
        check::run(&world).is_empty(),
        "a clean declared-functional world raises nothing: {:?}",
        check::run(&world)
    );
}

#[test]
fn proposed_values_warn_for_declared_functional_slots_without_a_canon_violation() {
    let proposed = |value| {
        Decl::Fact(FactDecl {
            atom: atom("rank", &["nara", value]),
            triple: false,
            provenance: Some(marker(ProvenanceMarker::Proposed, &[])),
        })
    };
    let with_policy = world(vec![
        functional("rank"),
        proposed("captain"),
        proposed("admiral"),
    ]);
    let warnings: Vec<_> = check::run(&with_policy)
        .into_iter()
        .filter(|f| f.code == "continuity.pending-conflict")
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].severity, check::Severity::Warn);
    assert!(fd_findings(&with_policy).is_empty());
    let mixed = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        proposed("admiral"),
    ]);
    assert!(
        check::run(&mixed)
            .iter()
            .any(|f| f.code == "continuity.pending-conflict")
    );
    let broken_canon = world(vec![
        functional("rank"),
        certain_fact("rank", "nara", "captain"),
        certain_fact("rank", "nara", "admiral"),
        proposed("ensign"),
    ]);
    assert!(
        !check::run(&broken_canon)
            .iter()
            .any(|f| f.code == "continuity.pending-conflict")
    );
    assert_eq!(fd_findings(&broken_canon).len(), 1);
    let without_policy = world(vec![proposed("captain"), proposed("admiral")]);
    assert!(
        !check::run(&without_policy)
            .iter()
            .any(|f| f.code == "continuity.pending-conflict")
    );
    let identical = world(vec![
        functional("rank"),
        proposed("captain"),
        proposed("captain"),
    ]);
    assert!(
        !check::run(&identical)
            .iter()
            .any(|f| f.code == "continuity.pending-conflict")
    );
}
