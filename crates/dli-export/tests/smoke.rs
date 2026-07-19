#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for export manifests.

use dli_export::{ExportManifest, ExportProfile};

#[test]
fn manifest_round_trips() {
    let manifest = ExportManifest {
        commit: "deadbeef".to_owned(),
        layer: "ratified".to_owned(),
        valuation: "sha256:0000".to_owned(),
        profile_hash: "sha256:1111".to_owned(),
    };
    let json = serde_json::to_string(&manifest).unwrap();
    let back: ExportManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(manifest, back);
}

#[test]
fn profile_serializes_kebab_case() {
    assert_eq!(
        serde_json::to_string(&ExportProfile::LudiiGdl).unwrap(),
        "\"ludii-gdl\""
    );
}

#[test]
#[ignore = "enabled at EXP-1"]
fn golden_ludii_export_matches() {
    // Once export() renders GDL, snapshot the emitted program + manifest.
    todo!("EXP-1: golden Ludii GDL export")
}
