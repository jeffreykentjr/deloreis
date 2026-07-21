#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `PAY-2`/`PAY-3` acceptance: the merged eight-family battery over a fat
//! [`Payload`](dli_world::payload::Payload), driven end-to-end through
//! [`check_payload`](dli_world::payload::check_payload).
//!
//! Each test runs a real [`Payload`] against a compiled world and asserts on the
//! exact finding codes, severities, and draft attributions the battery emits:
//! staleness, quote-anchoring, factcheck, continuity, the unparseable-atom defect,
//! proof-ledger checks, and the two-sided outline-conformance diff (`PAY-3`),
//! including that the diff is gated on an outline being supplied.
//!
//! The world is built with the same hand-rolled `dli_ast` builders the sibling
//! `check.rs` uses, so `gender`/`nature`/`origin` have known standings a claim can
//! be checked against.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, RatifyDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

use dli_core::grade::Grade;

use dli_world::craft::outline::{
    EpisodeOutline, Scene as OutlineScene, TryFailBeat, TryFailOutcome,
};
use dli_world::payload::{
    BatteryContext, BeatKind, BeatRow, Claim, Magnitude, Payload, PayloadFinding, ProofClose,
    ProofLedger, ProofLedgerRow, SceneRow, check_payload, outline_conformance, source_hash,
};
use dli_world::{CompiledWorld, Pack, Severity, compile};

// --- world + finding helpers -------------------------------------------------

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn atom(pred: &str, args: &[&str]) -> AtomExpr {
    AtomExpr {
        pred: id(pred),
        args: args.iter().map(|a| Term::Sym(id(a))).collect(),
    }
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
        body: Vec::new(),
    })
}

fn fact(pred: &str, args: &[&str], provenance: Option<Annotation>) -> Decl {
    Decl::Fact(FactDecl {
        atom: atom(pred, args),
        triple: false,
        provenance,
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

/// The saltcast world: `gender(anchor, male)` ratified-certain (D-0096),
/// `nature(hound, moor-spirit)` ratified-certain (D-0116), and
/// `origin(anchor, swartby)` proposed shadow lore (never certain).
fn saltcast_world() -> CompiledWorld {
    compile(&pack(
        "cast",
        vec![
            ratify_decl("D-0096"),
            ratify_decl("D-0116"),
            fact("gender", &["anchor", "male"], Some(ratified("D-0096"))),
            fact(
                "nature",
                &["hound", "moor-spirit"],
                Some(ratified("D-0116")),
            ),
            fact(
                "origin",
                &["anchor", "swartby"],
                Some(marker(ProvenanceMarker::Proposed, &[])),
            ),
        ],
    ))
    .expect("saltcast world compiles")
}

/// The single finding with `code`, or `None`.
fn finding<'a>(findings: &'a [PayloadFinding], code: &str) -> Option<&'a PayloadFinding> {
    findings.iter().find(|f| f.code == code)
}

// --- the battery -------------------------------------------------------------

#[test]
fn a_clean_payload_produces_no_findings() {
    let world = saltcast_world();
    let source = [
        "the anchor is a man, and canon holds it so", // line 1
        "the hound is a spirit of the moor",          // line 2
        "she set the withy and it held",              // line 3
    ]
    .join("\n");

    let mut payload = Payload::default();
    payload.meta.episode = "e01".to_owned();
    payload.meta.source.sha256 = source_hash(&source);
    payload.claims = vec![
        Claim {
            atom: "gender(anchor, male)".to_owned(),
            quote: "the anchor is a man, and canon holds it so".to_owned(),
            line: 1,
        },
        Claim {
            atom: "nature(hound, moor-spirit)".to_owned(),
            quote: "the hound is a spirit of the moor".to_owned(),
            line: 2,
        },
    ];
    // A minor try-beat trips no season join, and a well-formed proof shadow is clean.
    payload.beats = vec![BeatRow {
        id: "b1".to_owned(),
        scene: "s01".to_owned(),
        kind: BeatKind::Try,
        thread: "ward-the-door".to_owned(),
        magnitude: Magnitude::Minor,
        outcome: None,
        by: String::new(),
        assisted: false,
        quote: "she set the withy and it held".to_owned(),
        line: 3,
    }];
    payload.proof_ledgers = vec![ProofLedgerRow {
        scene: "s01".to_owned(),
        ledger: ProofLedger {
            premises: vec!["seated(withy)".to_owned()],
            steps: vec!["chain from seated(withy)".to_owned()],
            close: ProofClose {
                atom: "holds(ward(door))".to_owned(),
                expect: "holds".to_owned(),
            },
        },
    }];

    let findings = check_payload(&world, &payload, &BatteryContext::new(&source));
    assert!(
        findings.is_empty(),
        "a clean payload must be silent: {findings:?}"
    );
}

#[test]
fn a_contradicted_claim_is_attributed_to_its_line_and_quote() {
    let world = saltcast_world();
    let source = "the anchor was no man at all\n";
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(source);
    payload.claims = vec![Claim {
        atom: "gender(anchor, female)".to_owned(),
        quote: "the anchor was no man at all".to_owned(),
        line: 1,
    }];

    let findings = check_payload(&world, &payload, &BatteryContext::new(source));
    let f = finding(&findings, "factcheck.contradicted").expect("gender clash must contradict");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.grade, Grade::Checked);
    assert_eq!(f.line, Some(1));
    assert_eq!(f.quote.as_deref(), Some("the anchor was no man at all"));
    assert_eq!(f.citations, vec!["D-0096".to_owned()]);
    // No PAY-1 noise: the source hash matches and the quote anchors verbatim.
    assert!(
        !findings
            .iter()
            .any(|f| f.family() == "anchor" || f.code == "payload.stale"),
        "{findings:?}"
    );
}

#[test]
fn an_unsupported_claim_surfaces_factcheck_unsupported() {
    let world = saltcast_world();
    // Canon bears no certain `origin` fact for the anchor, so any origin claim is
    // unsupported (not contradicted — nothing certain to clash with).
    let source = "some say the anchor hails from gwern\n";
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(source);
    payload.claims = vec![Claim {
        atom: "origin(anchor, gwern)".to_owned(),
        quote: "some say the anchor hails from gwern".to_owned(),
        line: 1,
    }];

    let findings = check_payload(&world, &payload, &BatteryContext::new(source));
    let f = finding(&findings, "factcheck.unsupported").expect("origin gwern is unsupported");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.line, Some(1));
    assert_eq!(
        f.quote.as_deref(),
        Some("some say the anchor hails from gwern")
    );
}

#[test]
fn stating_shadow_lore_as_settled_flags_continuity() {
    let world = saltcast_world();
    // `origin(anchor, swartby)` is proposed shadow lore. Stated as settled fact it
    // draws the continuity class AND stays unsupported (a shadow is never certain).
    let source = "the anchor was born in swartby, plainly\n";
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(source);
    payload.claims = vec![Claim {
        atom: "origin(anchor, swartby)".to_owned(),
        quote: "the anchor was born in swartby, plainly".to_owned(),
        line: 1,
    }];

    let findings = check_payload(&world, &payload, &BatteryContext::new(source));
    let cont = finding(&findings, "continuity.proposed-as-fact").expect("shadow-stated-as-settled");
    assert_eq!(
        cont.line,
        Some(1),
        "continuity finding carries its claim's line"
    );
    assert_eq!(
        cont.quote.as_deref(),
        Some("the anchor was born in swartby, plainly")
    );
    assert!(
        finding(&findings, "factcheck.unsupported").is_some(),
        "a shadow slot is not certain, so the claim is also unsupported: {findings:?}"
    );
}

#[test]
fn an_unparseable_claim_atom_is_a_defect() {
    let world = saltcast_world();
    let source = "here is a line the extractor mis-shadowed\n";
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(source);
    payload.claims = vec![Claim {
        // A bare token has no `pred(args)` / triple / slot shape: not an atom.
        atom: "not-an-atom".to_owned(),
        quote: "here is a line the extractor mis-shadowed".to_owned(),
        line: 1,
    }];

    let findings = check_payload(&world, &payload, &BatteryContext::new(source));
    let f = finding(&findings, "payload.unparseable-claim").expect("a bare token is unparseable");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.line, Some(1));
    assert_eq!(
        f.quote.as_deref(),
        Some("here is a line the extractor mis-shadowed")
    );
    // The unparseable atom never reaches the fact-checker (it would be silently skipped).
    assert!(
        !findings.iter().any(|f| f.family() == "factcheck"),
        "{findings:?}"
    );
}

#[test]
fn a_moved_source_flags_the_payload_stale() {
    let world = saltcast_world();
    let source = [
        "the anchor is a man, and canon holds it so", // line 1
        "she set the withy and it held",              // line 2
    ]
    .join("\n");
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(&source);
    payload.claims = vec![Claim {
        atom: "gender(anchor, male)".to_owned(),
        quote: "the anchor is a man, and canon holds it so".to_owned(),
        line: 1,
    }];
    payload.beats = vec![BeatRow {
        id: "b1".to_owned(),
        scene: "s01".to_owned(),
        kind: BeatKind::Try,
        thread: "t".to_owned(),
        magnitude: Magnitude::Minor,
        outcome: None,
        by: String::new(),
        assisted: false,
        quote: "she set the withy and it held".to_owned(),
        line: 2,
    }];

    // Append a line: the two existing anchors still land and the claim still holds,
    // so staleness is the SOLE finding — the hash catches a move the anchors miss.
    let edited = format!("{source}\nand the tide came in");
    let findings = check_payload(&world, &payload, &BatteryContext::new(&edited));
    assert_eq!(
        findings.len(),
        1,
        "staleness must be the only finding: {findings:?}"
    );
    assert_eq!(findings[0].code, "payload.stale");
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn a_forged_quote_anchor_is_a_hard_error() {
    let world = saltcast_world();
    let source = "the anchor is a man, and canon holds it so\n";
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(source);
    payload.claims = vec![Claim {
        atom: "gender(anchor, male)".to_owned(),
        quote: "the anchor rode a dragon over the fen".to_owned(), // absent from the draft
        line: 1,
    }];

    let findings = check_payload(&world, &payload, &BatteryContext::new(source));
    let f = finding(&findings, "anchor.quote-missing").expect("a forged quote is a hard error");
    assert_eq!(f.family(), "anchor");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.grade, Grade::Checked);
    assert_eq!(f.line, Some(1));

    // The real quote anchored to the wrong line is the misplaced sibling error.
    payload.claims[0].quote = "the anchor is a man, and canon holds it so".to_owned();
    payload.claims[0].line = 5;
    let findings = check_payload(&world, &payload, &BatteryContext::new(source));
    let f = finding(&findings, "anchor.quote-misplaced").expect("wrong line is misplaced");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.line, Some(5));
}

#[test]
fn outline_conformance_is_gated_on_the_outline_and_states_both_readings() {
    let world = saltcast_world();
    let source = "she tried the latch, and it did not give\n";
    let mut payload = Payload::default();
    payload.meta.episode = "e01".to_owned();
    payload.meta.source.sha256 = source_hash(source);
    payload.beats = vec![BeatRow {
        id: "b-drafted".to_owned(),
        scene: "s01".to_owned(),
        kind: BeatKind::Try,
        thread: "the-latch".to_owned(),
        magnitude: Magnitude::Minor,
        outcome: Some(TryFailOutcome::NoBut),
        by: String::new(),
        assisted: false,
        quote: "she tried the latch, and it did not give".to_owned(),
        line: 1,
    }];

    let outline = EpisodeOutline {
        id: "e01".to_owned(),
        title: "T".to_owned(),
        tryfail_beats: vec![TryFailBeat {
            id: "b-outlined".to_owned(),
            attempt: "force the door".to_owned(),
            outcome: TryFailOutcome::NoAnd,
        }],
        ..EpisodeOutline::default()
    };

    // WITHOUT an outline the diff is gated off — no outline findings at all.
    let bare = check_payload(&world, &payload, &BatteryContext::new(source));
    assert!(
        bare.iter().all(|f| f.family() != "outline"),
        "the outline diff must be gated on an outline being supplied: {bare:?}"
    );
    assert!(
        bare.is_empty(),
        "a well-formed payload with no outline is otherwise silent: {bare:?}"
    );

    // WITH an outline both readings fire, each stated two-sidedly.
    let ctx = BatteryContext::new(source).with_outline(&outline);
    let findings = check_payload(&world, &payload, &ctx);

    let unaccounted =
        finding(&findings, "outline.beat-unaccounted").expect("the outlined beat is unaccounted");
    assert!(unaccounted.message.contains("b-outlined"));
    assert!(
        unaccounted.message.contains("dropped") && unaccounted.message.contains("missed"),
        "the unaccounted reading must state BOTH: {}",
        unaccounted.message
    );

    let undeclared =
        finding(&findings, "outline.beat-undeclared").expect("the drafted beat is undeclared");
    assert_eq!(
        undeclared.line,
        Some(1),
        "the undeclared finding carries the payload beat's anchor"
    );
    assert_eq!(
        undeclared.quote.as_deref(),
        Some("she tried the latch, and it did not give")
    );
    assert!(
        undeclared.message.contains("drift") && undeclared.message.contains("stale"),
        "the undeclared reading must state BOTH: {}",
        undeclared.message
    );
}

#[test]
fn outline_conformance_cast_diff_states_both_readings() {
    // The scene-cast branch of the two-sided diff, exercised directly (PAY-3 unit):
    // the outline casts `aled`, the payload casts `carys`, they share `bryn`.
    let payload = Payload {
        scenes: vec![SceneRow {
            id: "s1".to_owned(),
            cast: vec!["bryn".to_owned(), "carys".to_owned()],
            ..SceneRow::default()
        }],
        ..Payload::default()
    };
    let outline = EpisodeOutline {
        id: "e01".to_owned(),
        title: "T".to_owned(),
        scenes: vec![OutlineScene {
            id: "s1".to_owned(),
            summary: "s".to_owned(),
            speaking_cast: vec!["bryn".to_owned(), "aled".to_owned()],
            advances: Vec::new(),
            silence_beat: false,
        }],
        ..EpisodeOutline::default()
    };

    let findings = outline_conformance(&payload, &outline);

    let unaccounted = finding(&findings, "outline.cast-unaccounted").expect("aled is unaccounted");
    assert!(
        unaccounted.message.contains("aled") && unaccounted.message.contains("s1"),
        "{}",
        unaccounted.message
    );
    assert!(
        unaccounted.message.contains("dropped") && unaccounted.message.contains("missed"),
        "both readings: {}",
        unaccounted.message
    );

    let undeclared = finding(&findings, "outline.cast-undeclared").expect("carys is undeclared");
    assert!(
        undeclared.message.contains("carys") && undeclared.message.contains("s1"),
        "{}",
        undeclared.message
    );
    assert!(
        undeclared.message.contains("drift") && undeclared.message.contains("stale"),
        "both readings: {}",
        undeclared.message
    );
}

#[test]
fn findings_are_returned_in_sorted_order() {
    let world = saltcast_world();
    let source = [
        "the anchor was no man at all", // line 1
        "and here the shadow broke",    // line 2
    ]
    .join("\n");
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(&source);
    payload.claims = vec![
        Claim {
            atom: "gender(anchor, female)".to_owned(),
            quote: "the anchor was no man at all".to_owned(),
            line: 1,
        },
        Claim {
            atom: "not-an-atom".to_owned(),
            quote: "and here the shadow broke".to_owned(),
            line: 2,
        },
    ];

    let findings = check_payload(&world, &payload, &BatteryContext::new(&source));
    assert!(
        findings.len() >= 2,
        "need at least two findings to test ordering: {findings:?}"
    );

    // The list is sorted by (code, line, message) — the fn's declared order.
    let sorted = findings
        .windows(2)
        .all(|w| (&w[0].code, w[0].line, &w[0].message) <= (&w[1].code, w[1].line, &w[1].message));
    assert!(sorted, "findings must be returned sorted: {findings:?}");

    // Concretely, `factcheck.contradicted` sorts before `payload.unparseable-claim`.
    let pos = |code: &str| findings.iter().position(|f| f.code == code);
    let contradicted = pos("factcheck.contradicted").expect("contradicted present");
    let unparseable = pos("payload.unparseable-claim").expect("unparseable present");
    assert!(
        contradicted < unparseable,
        "factcheck.* must sort before payload.*: {findings:?}"
    );
}

#[test]
fn a_proof_ledger_unknown_verdict_surfaces_through_the_battery() {
    let world = saltcast_world();
    let source = "the withy was seated and the ward closed\n";
    let mut payload = Payload::default();
    payload.meta.source.sha256 = source_hash(source);
    payload.proof_ledgers = vec![ProofLedgerRow {
        scene: "s01".to_owned(),
        ledger: ProofLedger {
            premises: vec!["seated(withy)".to_owned()],
            steps: Vec::new(),
            close: ProofClose {
                atom: "holds(ward(door))".to_owned(),
                expect: "hodls".to_owned(), // typo: not a real verdict token
            },
        },
    }];

    let findings = check_payload(&world, &payload, &BatteryContext::new(source));
    let f = finding(&findings, "proof.unknown-verdict").expect("a typo'd verdict is caught");
    assert_eq!(f.severity, Severity::Error);
    assert!(
        f.message.contains("s01"),
        "the proof finding names its scene: {}",
        f.message
    );
}
