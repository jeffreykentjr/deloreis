#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the world model.

use dli_world::draft::{DraftMeta, DraftPayload};
use dli_world::loader::{Loader, example_docs};

#[test]
fn draft_payload_round_trips() {
    let payload = DraftPayload {
        meta: DraftMeta {
            title: "the-door-after-rain".to_owned(),
            pack: "saltmarket".to_owned(),
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
#[ignore = "enabled at CHECK-1"]
fn golden_check_draft_findings_match() {
    // The CHECK-1 half: once check_draft lands, assert a draft over the
    // saltmarket pack produces the committed findings.
    todo!("CHECK-1: golden check_draft over the compiled saltmarket world")
}
