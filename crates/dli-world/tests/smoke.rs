#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the world model.

use dli_ast::ident::Ident;
use dli_ast::term::{AtomExpr, Term};

use dli_world::draft::{DraftMeta, DraftPayload};
use dli_world::loader::{Loader, example_docs};
use dli_world::pack::Pack;
use dli_world::{check_draft, compile};

#[test]
fn draft_payload_round_trips() {
    let payload = DraftPayload {
        meta: DraftMeta {
            title: "the-door-after-rain".to_owned(),
            scope: "saltmarket".to_owned(),
            version: 1,
        },
        scenes: vec!["s1".to_owned(), "s2".to_owned()],
        beats: vec!["b1".to_owned()],
        names: vec!["mara".to_owned()],
        ..DraftPayload::default()
    };
    let json = serde_json::to_string(&payload).unwrap();
    let back: DraftPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(payload, back);
}

#[test]
fn golden_compile_loads_saltmarket_clean() {
    // The FRONT-1 half of the original golden test: a hand-built pack fronts as
    // a compiled world with zero findings and the expected shape.
    let out = Loader::load(&example_docs());
    assert!(
        out.findings.is_empty(),
        "golden pack must load clean: {:?}",
        out.findings
    );
    assert_eq!(out.world.packs().len(), 1);
    assert_eq!(out.world.entities().len(), 3);
    assert_eq!(out.world.world.facts.len(), 2);
}

#[test]
fn golden_check_draft_findings_match() {
    // The CHECK-1 half: a draft over the compiled saltmarket world produces the
    // committed findings. The pack's `rings(tide-bell, saltmarket)` is proposed
    // (shadow) lore, so a draft that states it as settled draws two findings — a
    // fact-check `unsupported` (no certain fact) and a continuity
    // `proposed-as-fact` — while the ratified `adjacent` claim is supported and
    // stays silent.
    // `example_docs` declares the decisions it cites (`ratify`), so its ratified
    // facts carry authority and compile certain (B1).
    let world = compile(&Pack {
        name: "saltmarket".to_owned(),
        documents: example_docs(),
    })
    .unwrap();

    let claim = |pred: &str, args: &[&str]| AtomExpr {
        pred: Ident::synthetic(pred),
        args: args
            .iter()
            .map(|a| Term::Sym(Ident::synthetic(*a)))
            .collect(),
    };
    let draft = DraftPayload {
        meta: DraftMeta {
            title: "the-door-after-rain".to_owned(),
            scope: "saltmarket".to_owned(),
            version: 1,
        },
        claims: vec![
            claim("adjacent", &["saltmarket", "the-shallows"]), // ratified → supported, silent
            claim("rings", &["tide-bell", "saltmarket"]),       // proposed → two findings
        ],
        ..DraftPayload::default()
    };

    let findings = check_draft(&world, &draft);
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert!(
        findings
            .iter()
            .any(|f| f.claim == "rings(tide-bell, saltmarket)"),
        "expected a fact-check finding for the proposed claim: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.claim == "continuity.proposed-as-fact"),
        "expected a proposed-as-fact finding: {findings:?}"
    );
    assert!(findings.iter().all(|f| f.grade == dli_core::Grade::Checked));
}
