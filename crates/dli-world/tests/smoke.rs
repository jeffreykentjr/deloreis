#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the world model.

use dli_world::draft::{DraftMeta, DraftPayload};

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
#[ignore = "enabled at FRONT-1"]
fn golden_check_draft_findings_match() {
    // Once compile + check_draft land, assert a draft over the saltmarket pack
    // produces the committed findings.
    todo!("FRONT-1 + CHECK-1: golden compile + check_draft")
}
