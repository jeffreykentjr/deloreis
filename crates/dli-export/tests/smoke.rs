#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Export coverage: the frozen wire types, the export core, and the worked
//! card + Ludii export off the real Turning ruleset — the honest-vs-keep-stamped
//! diff is the test (spec 09 §6).

use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_core::ann::Ann;
use dli_eval::fork::{KeepStamp, Visibility};
use dli_sim::turning::Ruleset;
use dli_world::compile;
use dli_world::loader::example_docs;
use dli_world::model::{LoadedAtom, Provenance};
use dli_world::pack::Pack;

use std::path::PathBuf;

use dli_export::games::{
    DifferentialStatus, compile_cards, compile_ludii, diff_decks, follow_edge_count,
};
use dli_export::profile::{
    ExportOpts, ExportTarget, Registry, apply_keep_stamps, certain_atoms, export, export_with,
    refuse_write_back,
};
use dli_export::{ExportError, ExportManifest, ExportProfile, export_wire};

fn saltmarket() -> dli_world::CompiledWorld {
    compile(&Pack {
        name: "saltmarket".to_owned(),
        documents: example_docs(),
    })
    .expect("compiles")
}

/// The engine's neutral base catalog merged with the `tpth` world's export
/// profiles, loaded from data (`worlds/tpth/export/profiles.json`) — the same
/// `cards/lowlands@3`, `games/ludii@1`, `video/articy@1` that used to be hardcoded
/// in `Registry::builtin`, now sourced from the world, not the kernel.
fn tpth_registry() -> Registry {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../worlds/tpth/export/profiles.json");
    Registry::builtin().merge(
        Registry::from_path(&path).expect("worlds/tpth/export/profiles.json loads and parses"),
    )
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
    let world = saltmarket();
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
    let world = saltmarket();
    let registry = tpth_registry();
    let pack_hash = export_with(
        &registry,
        ExportTarget::ContextPack,
        &world,
        &ExportOpts::default(),
    )
    .unwrap()
    .manifest
    .profile_hash;
    let cards_hash = export_with(
        &registry,
        ExportTarget::Cards,
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
    assert_ne!(pack_hash, cards_hash);
}

#[test]
fn an_unknown_named_profile_is_an_error() {
    let world = saltmarket();
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

// -- the worked card export (EXP-3, spec 09 §6) ------------------------------

#[test]
fn worked_card_export_honest_deck() {
    let world = saltmarket();
    let outcome = export_with(
        &tpth_registry(),
        ExportTarget::Cards,
        &world,
        &ExportOpts::default(),
    )
    .unwrap();
    let deck = outcome.artifact.as_cards().expect("a card deck");

    assert_eq!(deck.deck, "turning-lowlands-honest");
    assert_eq!(deck.compiled_from.profile, "cards/lowlands@3");
    assert_eq!(deck.compiled_from.variant, None);
    assert_eq!(deck.leaf_count, 29);
    assert!(deck.layouts.contains(&"the-debt-square".to_owned()));

    // A Mender hand leaf carries the Pleacher and its cited provenance (spec 09 §6).
    let mender = deck
        .leaves
        .iter()
        .find(|leaf| leaf.role.as_deref() == Some("mender"))
        .expect("a mender hand leaf");
    assert_eq!(mender.figure.as_deref(), Some("the-pleacher"));
    assert_eq!(
        mender.meta.as_ref().map(|meta| meta.cited.clone()),
        Some(vec!["D-0147".to_owned(), "D-0148".to_owned()])
    );
}

#[test]
fn worked_card_export_keep_stamped_diff_is_computed_not_asserted() {
    let world = saltmarket();
    let registry = tpth_registry();

    let honest = export_with(
        &registry,
        ExportTarget::Cards,
        &world,
        &ExportOpts::default(),
    )
    .unwrap();
    let honest = honest.artifact.as_cards().unwrap();

    // The keep-stamped variant is lore (a variant selector); the ruleset cuts the
    // hard Weathers, and the debt-square drops out of `available_layouts` by compile.
    let keep_stamped = export_with(
        &registry,
        ExportTarget::Cards,
        &world,
        &ExportOpts {
            variant: Some("keep-stamped".to_owned()),
            ..ExportOpts::default()
        },
    )
    .unwrap();
    let keep_stamped = keep_stamped.artifact.as_cards().unwrap();

    assert_eq!(keep_stamped.deck, "turning-lowlands-keep-stamped");
    assert_eq!(
        keep_stamped.compiled_from.variant.as_deref(),
        Some("turning/keep-stamped")
    );
    assert_eq!(keep_stamped.leaf_count, 26);
    assert!(!keep_stamped.layouts.contains(&"the-debt-square".to_owned()));

    // The diff itself is the shippable artifact (spec 09 §6).
    let diff = diff_decks(honest, keep_stamped);
    assert_eq!(diff.leaf_count_before, 29);
    assert_eq!(diff.leaf_count_after, 26);
    assert_eq!(diff.layouts_removed, vec!["the-debt-square".to_owned()]);
    // Exactly the three hard Weathers (the Debt, the Sundering, the Bourn) fall out.
    assert_eq!(
        diff.leaves_removed,
        vec![
            "weather-bourn".to_owned(),
            "weather-debt".to_owned(),
            "weather-sundering".to_owned(),
        ]
    );
}

#[test]
fn worked_card_export_salt_fringe_regional_delta() {
    let world = saltmarket();
    let outcome = export_with(
        &tpth_registry(),
        ExportTarget::Cards,
        &world,
        &ExportOpts {
            region: Some("salt-fringe".to_owned()),
            ..ExportOpts::default()
        },
    )
    .unwrap();
    let deck = outcome.artifact.as_cards().unwrap();

    assert_eq!(deck.deck, "turning-salt-fringe-honest");
    // The Tide and the Catch re-suit Grounds; the moon-count holds (a re-suit).
    assert_eq!(deck.leaf_count, 29);
    let ground_names: Vec<&str> = deck
        .leaves
        .iter()
        .filter(|leaf| leaf.layer == "ground")
        .filter_map(|leaf| leaf.name.as_deref())
        .collect();
    assert!(ground_names.contains(&"The Tide"));
    assert!(ground_names.contains(&"The Catch"));
    // The Net-mender takes the Mender's face.
    let mender = deck
        .leaves
        .iter()
        .find(|leaf| leaf.role.as_deref() == Some("mender"))
        .unwrap();
    assert_eq!(mender.figure.as_deref(), Some("the-net-mender"));
}

// -- the Ludii L-GDL export (EXP-3, DL-0007) ---------------------------------

#[test]
fn ludii_export_is_a_structural_render_with_pending_differential() {
    let world = saltmarket();
    let outcome = export_with(
        &tpth_registry(),
        ExportTarget::LudiiGdl,
        &world,
        &ExportOpts::default(),
    )
    .unwrap();
    let dli_export::ExportArtifact::LudiiGdl(gdl) = outcome.artifact else {
        panic!("expected a Ludii export");
    };

    let honest = Ruleset::lowlands_honest();
    // Every leaf and every available layout appears — a straight render.
    for leaf in honest.leaves() {
        assert!(
            gdl.program.contains(&leaf.id()),
            "leaf {} missing from L-GDL",
            leaf.id()
        );
    }
    for layout in honest.available_layouts() {
        assert!(gdl.program.contains(&layout.name));
    }
    // The successor relation is enumerated, not computed.
    assert!(follow_edge_count(&honest) > 0);
    // The trace-agreement battery is a non-gating lab lane, recorded pending.
    assert!(matches!(
        gdl.differential,
        DifferentialStatus::Pending { .. }
    ));
}

#[test]
fn compile_ludii_directly_renders_each_leaf() {
    let registry = tpth_registry();
    let spec = registry.default_for(ExportTarget::LudiiGdl).unwrap();
    let gdl = compile_ludii(&Ruleset::lowlands_honest(), spec, "certain");
    assert!(gdl.program.starts_with("(game \"turning-lowlands-honest\""));
    assert_eq!(gdl.profile, "games/ludii@1");
}

// -- direct card compile off the ruleset -------------------------------------

#[test]
fn compile_cards_directly_matches_the_ruleset_counts() {
    let registry = tpth_registry();
    let spec = registry.default_for(ExportTarget::Cards).unwrap();
    let honest = compile_cards(
        &Ruleset::lowlands_honest(),
        spec,
        "certain",
        &ExportOpts::default(),
    );
    let keep = compile_cards(
        &Ruleset::keep_stamped(),
        spec,
        "certain",
        &ExportOpts::default(),
    );
    assert_eq!(honest.leaf_count, 29);
    assert_eq!(keep.leaf_count, 26);
    // Ground emblems are surfaced but not part of the moon-count.
    assert_eq!(
        honest.leaves.iter().filter(|l| l.layer == "ground").count(),
        4
    );
}

// -- context-pack parity invariants (EXP-2) ----------------------------------

#[test]
fn context_pack_budget_drops_from_the_tail() {
    let world = saltmarket();
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
    let registry = tpth_registry();
    let spec = registry.default_for(ExportTarget::Articy).unwrap();
    let export = dli_export::articy::compile_scaffold(spec, "certain", &refs);

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
