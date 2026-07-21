#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Export coverage: the frozen wire types, the export core, the shared keep-stamp
//! visibility filter, and the generic context-pack and articy scaffolds. The
//! world-specific card/Ludii render (the Turning) is exercised in the `tpth` crate;
//! everything here is world-agnostic engine mechanism, tested on neutral profiles.

use std::collections::BTreeMap;

use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Ann;
use dli_eval::fork::{KeepStamp, Visibility};
use dli_world::compile;
use dli_world::loader::example_docs;
use dli_world::model::{LoadedAtom, Provenance};
use dli_world::pack::Pack;

use dli_export::profile::{
    ExportOpts, ExportTarget, ProfileSpec, ProvenanceMode, Registry, Selection, apply_keep_stamps,
    certain_atoms, export, export_with, refuse_write_back,
};
use dli_export::{ExportError, ExportManifest, ExportProfile, export_wire};

fn camelot() -> dli_world::CompiledWorld {
    compile(&Pack {
        name: "camelot".to_owned(),
        documents: example_docs(),
    })
    .expect("compiles")
}

/// A neutral articy profile: a generic format selector with no world content — it
/// names no sorts, relations, or region, exactly as the engine's base context-pack
/// profile does. Enough to exercise the generic export mechanism (profile-hash
/// stability, the articy scaffold map) without leaning on any world's card deck.
fn neutral_articy_spec() -> ProfileSpec {
    ProfileSpec {
        target: ExportTarget::Articy,
        name: "video/articy".to_owned(),
        version: 1,
        selects: Selection::default(),
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::DevMetadata,
    }
}

/// The engine's neutral base catalog plus the neutral articy profile — two
/// world-agnostic format selectors, no world data loaded from disk.
fn neutral_registry() -> Registry {
    Registry::builtin().merge(Registry {
        profiles: vec![neutral_articy_spec()],
    })
}

// -- the frozen wire contract ------------------------------------------------

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
    assert_eq!(
        serde_json::to_string(&ExportProfile::ContextPack).unwrap(),
        "\"context-pack\""
    );
    assert_eq!(
        serde_json::to_string(&ExportProfile::Cards).unwrap(),
        "\"cards\""
    );
}

#[test]
fn wire_manifest_has_exactly_the_frozen_fields() {
    // The golden schema (contracts/dli-worker/schemas/dli.export.run.response)
    // pins these four field names; `xtask schema --check` diffs it in the gate.
    let value = serde_json::to_value(ExportManifest::default()).unwrap();
    let keys: std::collections::BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        ["commit", "layer", "profile_hash", "valuation"]
            .into_iter()
            .collect()
    );
}

// -- the export core (EXP-1) -------------------------------------------------

#[test]
fn context_pack_export_reads_the_certain_layer_and_pins_a_manifest() {
    let world = camelot();
    let outcome = export_wire(
        ExportProfile::ContextPack,
        &world,
        &ExportOpts {
            source_commit: "3e1320a".to_owned(),
            ..ExportOpts::default()
        },
    )
    .expect("context pack always exports");

    // Clause 1: the certain layer by default. Clause 4: the manifest traces it.
    assert_eq!(outcome.manifest.layer, "certain");
    assert_eq!(outcome.manifest.commit, "3e1320a");
    assert!(outcome.manifest.valuation.starts_with("sha256:"));
    assert!(outcome.manifest.profile_hash.starts_with("sha256:"));
    // The artifact is a context pack carrying real canon text.
    let pack = outcome.artifact.as_context_pack().expect("a context pack");
    assert!(!pack.text.is_empty());
    assert_eq!(pack.est_tokens, pack.text.len() / 4);
}

#[test]
fn a_profile_hash_is_stable_and_profile_specific() {
    let world = camelot();
    let registry = neutral_registry();
    // Two neutral, world-agnostic profiles — the engine's context-pack and a
    // neutral articy scaffold. The profile hash digests the ProfileSpec, so it is
    // deterministic per profile and distinct across profiles, with no world deck
    // in the picture.
    let pack_hash = export_with(
        &registry,
        ExportTarget::ContextPack,
        &world,
        &ExportOpts::default(),
    )
    .unwrap()
    .manifest
    .profile_hash;
    let articy_hash = export_with(
        &registry,
        ExportTarget::Articy,
        &world,
        &ExportOpts::default(),
    )
    .unwrap()
    .manifest
    .profile_hash;
    // Deterministic per profile, distinct across profiles.
    assert_eq!(
        pack_hash,
        export_with(
            &registry,
            ExportTarget::ContextPack,
            &world,
            &ExportOpts::default()
        )
        .unwrap()
        .manifest
        .profile_hash
    );
    assert_ne!(pack_hash, articy_hash);
}

#[test]
fn an_unknown_named_profile_is_an_error() {
    let world = camelot();
    let err = export(
        ExportTarget::ContextPack,
        &world,
        &ExportOpts {
            profile_name: Some("cards/nowhere".to_owned()),
            ..ExportOpts::default()
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::UnknownProfile(_)));
}

#[test]
fn write_back_is_refused_one_way_absolutely() {
    assert_eq!(refuse_write_back(), ExportError::WriteBackRefused);
}

// -- the shared keep-stamp visibility filter (spec 09 §3) --------------------

#[test]
fn keep_stamp_filter_hides_scoped_atoms_but_leaves_the_rest() {
    let atoms = [
        loaded_fact("hidden", &["a"]),
        loaded_fact("hidden", &["b"]),
        loaded_fact("shown", &["c"]),
    ];
    let refs: Vec<&LoadedAtom> = atoms.iter().collect();
    let stamp = KeepStamp {
        scope: vec!["hidden".to_owned()],
        visibility: Visibility::Removed,
    };
    let visible = apply_keep_stamps(refs, std::slice::from_ref(&stamp));
    assert_eq!(visible.len(), 1);
    assert!(visible.iter().all(|atom| atom.atom.pred.text == "shown"));
}

#[test]
fn kept_present_never_hides() {
    let atom = loaded_fact("secret", &["x"]);
    let refs = vec![&atom];
    let stamp = KeepStamp {
        scope: vec!["secret".to_owned()],
        visibility: Visibility::KeptPresent,
    };
    assert_eq!(apply_keep_stamps(refs, &[stamp]).len(), 1);
}

// -- context-pack parity invariants (EXP-2) ----------------------------------

#[test]
fn context_pack_budget_drops_from_the_tail() {
    let world = camelot();
    let atoms = certain_atoms(&world);
    let registry = Registry::builtin();
    let spec = registry.default_for(ExportTarget::ContextPack).unwrap();

    let full = dli_export::context_pack::build_with_budget(spec, "certain", &atoms, 30_000);
    let starved = dli_export::context_pack::build_with_budget(spec, "certain", &atoms, 0);
    assert!(starved.included.is_empty());
    assert_eq!(
        starved.skipped.len(),
        full.included.len() + full.skipped.len()
    );
    // est_tokens is always len(text)//4.
    assert_eq!(full.est_tokens, full.text.len() / 4);
}

// -- the articy scaffold (EXP-4) ---------------------------------------------

#[test]
fn articy_scaffold_maps_knowledge_scopes_and_quests() {
    let atoms = [
        loaded_fact("person", &["the-reeve"]),
        loaded_relation("owes", &["the-reeve", "the-mill"]),
        loaded_relation("knows", &["lowlands", "owes(the-reeve, the-mill)"]),
        loaded_relation("promise", &["mend-the-weir"]),
    ];
    let refs: Vec<&LoadedAtom> = atoms.iter().collect();
    let spec = neutral_articy_spec();
    let export = dli_export::articy::compile_scaffold(&spec, "certain", &refs);

    assert!(export.pending);
    assert_eq!(export.entities.len(), 1);
    assert!(export.quests.iter().any(|quest| quest.state == "hook"));
    // The `knows` atom licenses the lowlands scope to know the owes fact.
    let owes = export
        .facts
        .iter()
        .find(|fact| fact.claim == "owes(the-reeve, the-mill)")
        .expect("the owes fact");
    assert_eq!(owes.who_may_know, vec!["lowlands".to_owned()]);
}

// -- helpers -----------------------------------------------------------------

fn loaded_fact(pred: &str, args: &[&str]) -> LoadedAtom {
    loaded_relation(pred, args)
}

fn loaded_relation(pred: &str, args: &[&str]) -> LoadedAtom {
    let expr = AtomExpr {
        pred: Ident::synthetic(pred),
        args: args
            .iter()
            .map(|arg| Term::Sym(Ident::synthetic(*arg)))
            .collect(),
    };
    LoadedAtom {
        atom: expr,
        ann: Ann::unconditional(),
        kind: Provenance::Ratified,
        citations: Vec::new(),
        pack: "turning".to_owned(),
        span: Span::synthetic(),
    }
}
