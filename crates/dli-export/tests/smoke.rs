#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Export coverage: the frozen wire types, the export core, the shared keep-stamp
//! visibility filter, and the generic context-pack and articy backends, tested
//! with neutral profiles.

use std::collections::BTreeMap;

use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term, TimeRef};
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

fn anchored_opts() -> ExportOpts {
    ExportOpts {
        source_commit: "content-sha256:test-source".to_owned(),
        ..ExportOpts::default()
    }
}

/// A neutral articy profile: a generic format selector with no world content — it
/// names no sorts, relations, or region, exactly as the engine's base context-pack
/// profile does. Enough to exercise the generic export mechanism (profile-hash
/// stability, the articy mapping) without leaning on any world's card deck.
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
    Registry::builtin()
}

// -- the frozen wire contract ------------------------------------------------

#[test]
fn manifest_round_trips() {
    let manifest = ExportManifest {
        as_of: None,
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
        serde_json::to_string(&ExportProfile::ContextPack).unwrap(),
        "\"context-pack\""
    );
    assert_eq!(
        serde_json::to_string(&ExportProfile::StaticWiki).unwrap(),
        "\"static-wiki\""
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
            ..anchored_opts()
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
fn every_export_requires_a_source_anchor() {
    let error = export(
        ExportTarget::ContextPack,
        &camelot(),
        &ExportOpts::default(),
    )
    .unwrap_err();
    assert_eq!(error, ExportError::MissingSourceAnchor);
}

#[test]
fn a_profile_hash_is_stable_and_profile_specific() {
    let world = camelot();
    let registry = neutral_registry();
    // Two neutral, world-agnostic profiles — the engine's context-pack and a
    // neutral articy backend. The profile hash digests the ProfileSpec, so it is
    // deterministic per profile and distinct across profiles, with no world deck
    // in the picture.
    let pack_hash = export_with(
        &registry,
        ExportTarget::ContextPack,
        &world,
        &anchored_opts(),
    )
    .unwrap()
    .manifest
    .profile_hash;
    let articy_hash = export_with(&registry, ExportTarget::Articy, &world, &anchored_opts())
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
            &anchored_opts()
        )
        .unwrap()
        .manifest
        .profile_hash
    );
    assert_ne!(pack_hash, articy_hash);

    let named = export_with(
        &registry,
        ExportTarget::ContextPack,
        &world,
        &ExportOpts {
            profile_name: Some("context-pack/default@1".to_owned()),
            ..anchored_opts()
        },
    )
    .unwrap()
    .manifest
    .profile_hash;
    assert_eq!(pack_hash, named);
}

#[test]
fn generic_lore_exports_reject_unapplied_delta_selectors() {
    let world = camelot();
    let region = export(
        ExportTarget::ContextPack,
        &world,
        &ExportOpts {
            region: Some("alternate".to_owned()),
            ..anchored_opts()
        },
    )
    .unwrap_err();
    assert_eq!(
        region,
        ExportError::UnsupportedOption {
            target: ExportTarget::ContextPack,
            option: "region",
        }
    );

    let variant = export(
        ExportTarget::ContextPack,
        &world,
        &ExportOpts {
            variant: Some("alternate".to_owned()),
            ..anchored_opts()
        },
    )
    .unwrap_err();
    assert_eq!(
        variant,
        ExportError::UnsupportedOption {
            target: ExportTarget::ContextPack,
            option: "variant",
        }
    );
}

#[test]
fn an_unknown_named_profile_is_an_error() {
    let world = camelot();
    let err = export(
        ExportTarget::ContextPack,
        &world,
        &ExportOpts {
            profile_name: Some("wiki/nowhere".to_owned()),
            ..anchored_opts()
        },
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::UnknownProfile(_)));
}

#[test]
fn a_malformed_profile_version_does_not_fall_back() {
    assert!(Registry::builtin().resolve("wiki/static@latest").is_none());
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

#[test]
fn context_pack_honors_exact_profile_selection() {
    let world = camelot();
    let spec = ProfileSpec {
        target: ExportTarget::ContextPack,
        name: "context-pack/selected".to_owned(),
        version: 1,
        selects: Selection {
            sorts: Vec::new(),
            relations: vec!["adjacent".to_owned()],
        },
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::DevMetadata,
    };
    let registry = Registry {
        profiles: vec![spec],
    };
    let outcome = export_with(
        &registry,
        ExportTarget::ContextPack,
        &world,
        &anchored_opts(),
    )
    .unwrap();
    let text = &outcome.artifact.as_context_pack().unwrap().text;
    assert!(text.contains("adjacent("), "{text}");
    assert!(!text.contains("place("), "{text}");
}

// -- the articy v1 export (EXP-4) --------------------------------------------

#[test]
fn articy_v1_maps_entities_scopes_and_quests() {
    let atoms = [
        loaded_fact("person", &["the-reeve"]),
        loaded_fact("place", &["the-mill"]),
        loaded_relation("owes", &["the-reeve", "the-mill"]),
        loaded_relation("knows", &["moorlands", "owes(the-reeve, the-mill)"]),
        loaded_relation("promise", &["mend-north-gate"]),
    ];
    let refs: Vec<&LoadedAtom> = atoms.iter().collect();
    let spec = neutral_articy_spec();
    let export = dli_export::articy::compile(&spec, "certain", &refs).unwrap();

    // A real v1: no longer a scaffold.
    assert!(!export.pending);
    assert_eq!(export.profile, "video/articy@1");

    // Two typed entities, each carrying the fact it participates in (its scope).
    assert_eq!(export.entities.len(), 2);
    let reeve = export
        .entities
        .iter()
        .find(|e| e.id == "the-reeve")
        .expect("the reeve entity");
    assert_eq!(reeve.sort, "person");
    assert_eq!(reeve.facts, vec!["owes(the-reeve, the-mill)".to_owned()]);

    // The `owes` relation is modelled with ordered args.
    assert!(
        export
            .relations
            .iter()
            .any(|r| r.kind == "owes" && r.args == vec!["the-reeve", "the-mill"])
    );

    // The promise becomes a quest whose flow starts at the hook stage.
    let quest = export
        .quests
        .iter()
        .find(|q| q.id == "mend-north-gate")
        .expect("the promise quest");
    assert_eq!(quest.state, "hook");
    assert_eq!(quest.flow, vec!["hook".to_owned()]);

    // The `knows` atom licenses the moorlands scope to know the owes fact.
    let owes = export
        .facts
        .iter()
        .find(|fact| fact.claim == "owes(the-reeve, the-mill)")
        .expect("the owes fact");
    assert_eq!(owes.who_may_know, vec!["moorlands".to_owned()]);
}

#[test]
fn articy_quest_folds_stages_into_one_flow() {
    // A single quest id that is promised, owed, then discharged folds into one
    // quest whose state is the furthest reached and whose flow is ordered.
    let atoms = [
        loaded_relation("promise", &["mend-north-gate"]),
        loaded_relation("debt", &["mend-north-gate"]),
        loaded_relation("discharge", &["mend-north-gate"]),
    ];
    let refs: Vec<&LoadedAtom> = atoms.iter().collect();
    let export = dli_export::articy::compile(&neutral_articy_spec(), "certain", &refs).unwrap();

    assert_eq!(export.quests.len(), 1);
    let quest = &export.quests[0];
    assert_eq!(quest.id, "mend-north-gate");
    assert_eq!(quest.state, "complete");
    assert_eq!(
        quest.flow,
        vec![
            "hook".to_owned(),
            "objective".to_owned(),
            "complete".to_owned()
        ]
    );
}

#[test]
fn articy_output_is_deterministic() {
    let atoms = [
        loaded_fact("person", &["the-reeve"]),
        loaded_relation("owes", &["the-reeve", "the-mill"]),
        loaded_relation("promise", &["mend-north-gate"]),
    ];
    let refs: Vec<&LoadedAtom> = atoms.iter().collect();
    let spec = neutral_articy_spec();
    let a = dli_export::articy::compile(&spec, "certain", &refs).unwrap();
    let b = dli_export::articy::compile(&spec, "certain", &refs).unwrap();
    assert_eq!(a, b);
}

#[test]
fn articy_honors_exact_selection_and_entity_rendering() {
    let atoms = [
        loaded_fact("person", &["arden"]),
        loaded_fact("place", &["mill"]),
        loaded_relation("owes", &["arden", "mill"]),
        loaded_relation("ignored", &["arden", "mill"]),
    ];
    let refs: Vec<&LoadedAtom> = atoms.iter().collect();
    let mut renders = BTreeMap::new();
    renders.insert(
        "person".to_owned(),
        dli_export::RenderRule {
            id: "actor-{id}".to_owned(),
            keep: vec!["id".to_owned(), "sort".to_owned()],
        },
    );
    let spec = ProfileSpec {
        target: ExportTarget::Articy,
        name: "video/selected".to_owned(),
        version: 1,
        selects: Selection {
            sorts: vec!["person".to_owned(), "place".to_owned()],
            relations: vec!["owes".to_owned()],
        },
        renders,
        carries_provenance: ProvenanceMode::DevMetadata,
    };
    let export = dli_export::articy::compile(&spec, "certain", &refs).unwrap();
    assert_eq!(export.entities.len(), 2);
    let arden = export
        .entities
        .iter()
        .find(|entity| entity.id == "actor-arden")
        .unwrap();
    assert!(arden.facts.is_empty());
    assert_eq!(export.relations.len(), 1);
    assert_eq!(export.relations[0].kind, "owes");
    assert_eq!(export.relations[0].args[0], "actor-arden");
    assert!(
        export
            .facts
            .iter()
            .all(|fact| !fact.claim.contains("ignored"))
    );
}

#[test]
fn articy_rejects_an_unknown_kept_field() {
    let atom = loaded_fact("person", &["arden"]);
    let mut renders = BTreeMap::new();
    renders.insert(
        "person".to_owned(),
        dli_export::RenderRule {
            id: "{id}".to_owned(),
            keep: vec!["invented".to_owned()],
        },
    );
    let spec = ProfileSpec {
        target: ExportTarget::Articy,
        name: "video/bad".to_owned(),
        version: 1,
        selects: Selection {
            sorts: vec!["person".to_owned()],
            relations: Vec::new(),
        },
        renders,
        carries_provenance: ProvenanceMode::DevMetadata,
    };
    assert!(matches!(
        dli_export::articy::compile(&spec, "certain", &[&atom]),
        Err(ExportError::InvalidProfile { .. })
    ));
}

#[test]
fn articy_rejects_colliding_rendered_entity_ids() {
    let atoms = [
        loaded_fact("person", &["arden"]),
        loaded_fact("person", &["bedivere"]),
    ];
    let mut renders = BTreeMap::new();
    renders.insert(
        "person".to_owned(),
        dli_export::RenderRule {
            id: "actor".to_owned(),
            keep: Vec::new(),
        },
    );
    let spec = ProfileSpec {
        target: ExportTarget::Articy,
        name: "video/collision".to_owned(),
        version: 1,
        selects: Selection {
            sorts: vec!["person".to_owned()],
            relations: Vec::new(),
        },
        renders,
        carries_provenance: ProvenanceMode::DevMetadata,
    };

    assert!(matches!(
        dli_export::articy::compile(&spec, "certain", &[&atoms[0], &atoms[1]]),
        Err(ExportError::Render { .. })
    ));
}

#[test]
fn articy_rejects_relations_to_unselected_declared_entities() {
    let atoms = [
        loaded_fact("person", &["arden"]),
        loaded_fact("place", &["mill"]),
        loaded_relation("owes", &["arden", "mill"]),
    ];
    let spec = ProfileSpec {
        target: ExportTarget::Articy,
        name: "video/dangling".to_owned(),
        version: 1,
        selects: Selection {
            sorts: vec!["person".to_owned()],
            relations: vec!["owes".to_owned()],
        },
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::Stripped,
    };

    assert!(matches!(
        dli_export::articy::compile(&spec, "certain", &atoms.iter().collect::<Vec<_>>()),
        Err(ExportError::Render { .. })
    ));
}

#[test]
fn articy_entity_and_quest_provenance_obeys_the_profile() {
    let mut person = loaded_fact("person", &["arden"]);
    person.citations = vec!["d-0001".to_owned()];
    let mut promise = loaded_relation("promise", &["mend-north-gate"]);
    promise.citations = vec!["d-0002".to_owned()];
    let atoms = [&person, &promise];

    let dev = dli_export::articy::compile(&neutral_articy_spec(), "certain", &atoms).unwrap();
    assert_eq!(dev.entities[0].cited, vec!["d-0001"]);
    assert_eq!(dev.quests[0].cited, vec!["d-0002"]);

    let mut ship = neutral_articy_spec();
    ship.carries_provenance = ProvenanceMode::Stripped;
    let ship = dli_export::articy::compile(&ship, "certain", &atoms).unwrap();
    assert!(ship.entities[0].cited.is_empty());
    assert!(ship.quests[0].cited.is_empty());
}

#[test]
fn articy_stripping_never_removes_knowledge_from_knows_only_rows() {
    let mut knows = loaded_relation("knows", &["wardens", "owes(arden, mill)"]);
    knows.citations = vec!["d-0001".to_owned()];

    let dev = dli_export::articy::compile(&neutral_articy_spec(), "certain", &[&knows]).unwrap();
    let mut ship_spec = neutral_articy_spec();
    ship_spec.carries_provenance = ProvenanceMode::Stripped;
    let ship = dli_export::articy::compile(&ship_spec, "certain", &[&knows]).unwrap();

    assert_eq!(dev.facts.len(), 1);
    assert_eq!(ship.facts.len(), 1);
    assert_eq!(dev.facts[0].claim, ship.facts[0].claim);
    assert_eq!(dev.facts[0].who_may_know, ship.facts[0].who_may_know);
    assert_eq!(dev.facts[0].cited, vec!["d-0001"]);
    assert!(ship.facts[0].cited.is_empty());
}

#[test]
fn articy_preserves_time_arguments_and_refuses_non_ground_or_wrong_arity_rows() {
    let timed = loaded_atom(
        "during",
        vec![
            Term::Sym(Ident::synthetic("vigil")),
            Term::Time(TimeRef::Era(Ident::synthetic("first-age"))),
        ],
    );
    let output = dli_export::articy::compile(&neutral_articy_spec(), "certain", &[&timed]).unwrap();
    assert_eq!(
        output.relations[0].args,
        vec!["vigil".to_owned(), "era:first-age".to_owned()]
    );

    let variable = loaded_atom("during", vec![Term::Var(Ident::synthetic("when"))]);
    assert!(matches!(
        dli_export::articy::compile(&neutral_articy_spec(), "certain", &[&variable]),
        Err(ExportError::Render { .. })
    ));

    let malformed_knows = loaded_relation("knows", &["wardens"]);
    assert!(matches!(
        dli_export::articy::compile(&neutral_articy_spec(), "certain", &[&malformed_knows]),
        Err(ExportError::Render { .. })
    ));

    let malformed_person = loaded_relation("person", &["arden", "lost-field"]);
    assert!(matches!(
        dli_export::articy::compile(&neutral_articy_spec(), "certain", &[&malformed_person]),
        Err(ExportError::Render { .. })
    ));
}

#[test]
fn csv_is_exact_deterministic_and_rfc4180_quoted() {
    let mut selected = loaded_relation("note", &["alpha", "line,\n\"quoted\""]);
    selected.citations = vec!["d-2".to_owned(), "d-1".to_owned(), "d-1".to_owned()];
    let ignored = loaded_fact("ignored", &["beta"]);
    let mut renders = BTreeMap::new();
    renders.insert(
        "note".to_owned(),
        dli_export::RenderRule {
            id: "{predicate}-{arg0}".to_owned(),
            keep: vec!["arg1".to_owned()],
        },
    );
    let spec = ProfileSpec {
        target: ExportTarget::Csv,
        name: "tabular/notes".to_owned(),
        version: 2,
        selects: Selection {
            sorts: Vec::new(),
            relations: vec!["note".to_owned()],
        },
        renders,
        carries_provenance: ProvenanceMode::DevMetadata,
    };
    let refs = [&selected, &ignored];
    let csv = dli_export::csv::compile(&spec, "certain", &refs).unwrap();
    assert_eq!(csv.row_count, 1);
    assert!(
        csv.content
            .starts_with("id,kind,predicate,arguments,claim,cited\r\n")
    );
    assert!(csv.content.contains("note-alpha"));
    assert!(csv.content.contains("\"\"quoted\"\""));
    assert!(csv.content.contains("[\"\"d-1\"\",\"\"d-2\"\"]"));
    assert!(!csv.content.contains("ignored"));
    assert_eq!(
        csv,
        dli_export::csv::compile(&spec, "certain", &refs).unwrap()
    );
}

#[test]
fn flat_projection_rejects_selected_non_ground_atoms() {
    let variable = loaded_atom("note", vec![Term::Var(Ident::synthetic("value"))]);
    let spec = ProfileSpec {
        target: ExportTarget::Csv,
        name: "tabular/ground-only".to_owned(),
        version: 1,
        selects: Selection {
            sorts: Vec::new(),
            relations: vec!["note".to_owned()],
        },
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::Stripped,
    };

    assert!(matches!(
        dli_export::csv::compile(&spec, "certain", &[&variable]),
        Err(ExportError::Render { target, .. }) if target == "csv"
    ));
}

#[test]
fn static_wiki_strips_provenance_and_groups_predicates() {
    let mut alpha = loaded_fact("note", &["alpha"]);
    alpha.citations = vec!["d-1".to_owned()];
    let beta = loaded_fact("place", &["beta"]);
    let spec = ProfileSpec {
        target: ExportTarget::StaticWiki,
        name: "wiki/ship".to_owned(),
        version: 1,
        selects: Selection::default(),
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::Stripped,
    };
    let wiki = dli_export::static_wiki::compile(&spec, "certain", &[&beta, &alpha]).unwrap();
    assert_eq!(wiki.pages[0].path, "index.md");
    assert_eq!(
        wiki.pages
            .iter()
            .map(|page| page.path.as_str())
            .collect::<Vec<_>>(),
        vec!["index.md", "predicate-note.md", "predicate-place.md"]
    );
    assert!(wiki.pages.iter().all(|page| !page.content.contains("d-1")));
}

#[test]
fn all_standing_wiki_is_explicit_and_respects_visibility() {
    let world = camelot();
    let default = export(ExportTarget::StaticWiki, &world, &anchored_opts()).unwrap();
    let default_json = serde_json::to_string(&default.artifact).unwrap();
    assert!(!default_json.contains("kept-at"));

    let opts = ExportOpts {
        profile_name: Some("wiki/all-standing".to_owned()),
        ..anchored_opts()
    };
    let all = export(ExportTarget::StaticWiki, &world, &opts).unwrap();
    assert_eq!(all.manifest.layer, "all");
    let json = serde_json::to_string(&all.artifact).unwrap();
    assert!(json.contains("kept-at"));
    assert!(json.contains("possible"));
    assert!(json.contains("certain"));
    assert!(json.contains("D-0001"));

    let hidden = export(
        ExportTarget::StaticWiki,
        &world,
        &ExportOpts {
            keep_stamps: vec![KeepStamp {
                scope: vec!["kept-at".to_owned()],
                visibility: Visibility::Removed,
            }],
            ..opts
        },
    )
    .unwrap();
    assert!(
        !serde_json::to_string(&hidden.artifact)
            .unwrap()
            .contains("kept-at")
    );
    let at_time = export(
        ExportTarget::StaticWiki,
        &world,
        &ExportOpts {
            profile_name: Some("wiki/all-standing".to_owned()),
            as_of: Some("any-point".to_owned()),
            ..anchored_opts()
        },
    )
    .unwrap_err();
    assert_eq!(
        at_time,
        ExportError::UnsupportedOption {
            target: ExportTarget::StaticWiki,
            option: "as_of"
        }
    );
}

#[test]
fn all_standing_wiki_labels_individual_source_support() {
    use dli_core::ann::Theory;
    use dli_core::atom::Atom;
    use dli_core::posbool::PosBool;
    use dli_core::valuation::Valuation;

    let certain = loaded_fact("note", &["certain"]);
    let mut possible = loaded_fact("note", &["possible"]);
    possible.ann = Ann::supported(PosBool::pos(Atom(42)));
    possible.kind = Provenance::Provisional;
    possible.citations = vec!["d-0002".to_owned()];
    let mut dead = loaded_fact("note", &["dead"]);
    dead.ann = Ann::supported(PosBool::FALSE);
    let spec = Registry::builtin()
        .resolve("wiki/all-standing")
        .unwrap()
        .clone();
    let wiki = dli_export::static_wiki::compile_all(
        &spec,
        "all",
        &[&dead, &possible, &certain],
        &Valuation::default(),
        &Theory::default(),
    )
    .unwrap();
    let page = &wiki.pages[1].content;
    assert!(page.contains("`note(certain)` — **certain**"));
    assert!(
        page.contains(
            "`note(possible)` — **possible** (provenance: provisional) (decisions: d-0002)"
        )
    );
    assert!(page.contains("`note(dead)` — **dead**"));
    assert!(page.contains("[Back to index](index.md)"));
    let mut stripped = spec;
    stripped.carries_provenance = ProvenanceMode::Stripped;
    let wiki = dli_export::static_wiki::compile_all(
        &stripped,
        "all",
        &[&possible],
        &Valuation::default(),
        &Theory::default(),
    )
    .unwrap();
    assert!(!wiki.pages[1].content.contains("provenance:"));
    assert!(!wiki.pages[1].content.contains("d-0002"));
}

#[test]
fn static_wiki_paths_cannot_escape_the_bundle() {
    let atom = loaded_fact("../notes", &["alpha"]);
    let spec = ProfileSpec {
        target: ExportTarget::StaticWiki,
        name: "wiki/safe-paths".to_owned(),
        version: 1,
        selects: Selection::default(),
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::Stripped,
    };
    let wiki = dli_export::static_wiki::compile(&spec, "certain", &[&atom]).unwrap();

    assert_eq!(wiki.pages[1].path, "predicate-%2E%2E%2Fnotes.md");
    assert!(
        wiki.pages[0]
            .content
            .contains("predicate-%252E%252E%252Fnotes.md")
    );
    assert!(wiki.pages.iter().all(|page| !page.path.contains('/')));
}

#[test]
fn static_wiki_escapes_html_and_reserves_the_index_path() {
    let html = loaded_relation("index", &["<img src=x onerror=alert(1)>"]);
    let spec = ProfileSpec {
        target: ExportTarget::StaticWiki,
        name: "wiki/html-safe".to_owned(),
        version: 1,
        selects: Selection::default(),
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::Stripped,
    };
    let wiki = dli_export::static_wiki::compile(&spec, "certain", &[&html]).unwrap();

    assert_eq!(wiki.pages[0].path, "index.md");
    assert_eq!(wiki.pages[1].path, "predicate-index.md");
    assert!(!wiki.pages[1].content.contains("<img"));
    assert!(wiki.pages[1].content.contains("&lt;img"));
}

#[test]
fn csv_neutralizes_spreadsheet_formula_cells() {
    let atom = loaded_fact("note", &["alpha"]);
    let mut renders = BTreeMap::new();
    renders.insert(
        "note".to_owned(),
        dli_export::RenderRule {
            id: "=1+1".to_owned(),
            keep: Vec::new(),
        },
    );
    let spec = ProfileSpec {
        target: ExportTarget::Csv,
        name: "tabular/formula-safe".to_owned(),
        version: 1,
        selects: Selection {
            sorts: Vec::new(),
            relations: vec!["note".to_owned()],
        },
        renders,
        carries_provenance: ProvenanceMode::Stripped,
    };
    let csv = dli_export::csv::compile(&spec, "certain", &[&atom]).unwrap();

    assert!(csv.content.contains("\r\n'=1+1,relation,"));

    let mut newline_rule = spec;
    newline_rule.renders.insert(
        "note".to_owned(),
        dli_export::RenderRule {
            id: "\r=1+1".to_owned(),
            keep: Vec::new(),
        },
    );
    let csv = dli_export::csv::compile(&newline_rule, "certain", &[&atom]).unwrap();
    assert!(csv.content.contains("\"'\r=1+1\""));
}

#[test]
fn caller_registry_overrides_neutral_default_without_ambiguity() {
    let custom = ProfileSpec {
        target: ExportTarget::Csv,
        name: "tabular/csv".to_owned(),
        version: 1,
        selects: Selection {
            sorts: Vec::new(),
            relations: vec!["note".to_owned()],
        },
        renders: BTreeMap::new(),
        carries_provenance: ProvenanceMode::Stripped,
    };
    let registry = Registry::builtin().merge(Registry {
        profiles: vec![custom.clone()],
    });
    assert_eq!(
        registry.default_for(ExportTarget::Csv),
        Some(&custom),
        "the caller profile must replace the neutral identity"
    );
    registry.validate().unwrap();
}

#[test]
fn duplicate_in_memory_profile_identities_fail_closed() {
    let profile = neutral_articy_spec();
    let registry = Registry {
        profiles: vec![profile.clone(), profile],
    };
    assert!(registry.validate().is_err());
    assert!(matches!(
        export_with(
            &registry,
            ExportTarget::Articy,
            &camelot(),
            &anchored_opts()
        ),
        Err(ExportError::InvalidProfileRegistry(_))
    ));
}

#[test]
fn merge_preserves_duplicate_incoming_identities_for_validation() {
    let profile = neutral_articy_spec();
    let registry = Registry::builtin().merge(Registry {
        profiles: vec![profile.clone(), profile],
    });
    assert!(registry.validate().is_err());
    assert!(matches!(
        export_with(
            &registry,
            ExportTarget::Articy,
            &camelot(),
            &anchored_opts()
        ),
        Err(ExportError::InvalidProfileRegistry(_))
    ));
}

#[test]
fn malformed_templates_fail_before_row_projection() {
    for template in ["{arg0", "stray}-then-{arg0}"] {
        let mut renders = BTreeMap::new();
        renders.insert(
            "absent".to_owned(),
            dli_export::RenderRule {
                id: template.to_owned(),
                keep: vec!["arg0".to_owned()],
            },
        );
        let registry = Registry {
            profiles: vec![ProfileSpec {
                target: ExportTarget::Csv,
                name: "tabular/malformed".to_owned(),
                version: 1,
                selects: Selection {
                    sorts: Vec::new(),
                    relations: vec!["absent".to_owned()],
                },
                renders,
                carries_provenance: ProvenanceMode::Stripped,
            }],
        };

        assert!(matches!(
            export_with(&registry, ExportTarget::Csv, &camelot(), &anchored_opts()),
            Err(ExportError::InvalidProfileRegistry(_))
        ));
    }
}

#[test]
fn render_rules_without_selected_certain_atoms_fail_closed() {
    let mut renders = BTreeMap::new();
    renders.insert(
        "absent".to_owned(),
        dli_export::RenderRule {
            id: "{arg0}".to_owned(),
            keep: vec!["arg0".to_owned()],
        },
    );
    let registry = Registry {
        profiles: vec![ProfileSpec {
            target: ExportTarget::Csv,
            name: "tabular/absent".to_owned(),
            version: 1,
            selects: Selection {
                sorts: Vec::new(),
                relations: vec!["absent".to_owned()],
            },
            renders,
            carries_provenance: ProvenanceMode::Stripped,
        }],
    };

    assert!(matches!(
        export_with(&registry, ExportTarget::Csv, &camelot(), &anchored_opts()),
        Err(ExportError::InvalidProfile { .. })
    ));
}

#[test]
fn profile_catalog_rejects_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profiles.json");
    std::fs::write(
        &path,
        r#"{
          "profiles": [{
            "target": "csv",
            "name": "tabular/strict",
            "version": 1,
            "selects": {"sorts": [], "relations": [], "typo": []},
            "renders": {},
            "carries_provenance": "stripped"
          }]
        }"#,
    )
    .unwrap();

    assert!(matches!(
        Registry::from_path(path),
        Err(dli_export::profile::RegistryLoadError::Parse { .. })
    ));
}

#[test]
fn source_fingerprint_is_order_independent_and_unambiguously_framed() {
    let first = [b"alpha".as_slice(), b"beta".as_slice()];
    let reversed = [b"beta".as_slice(), b"alpha".as_slice()];
    assert_eq!(
        dli_export::source_fingerprint(first),
        dli_export::source_fingerprint(reversed)
    );

    let left = [b"a\0".as_slice(), b"b".as_slice()];
    let right = [b"a".as_slice(), b"\0b".as_slice()];
    assert_ne!(
        dli_export::source_fingerprint(left),
        dli_export::source_fingerprint(right)
    );
}

// -- helpers -----------------------------------------------------------------

fn loaded_fact(pred: &str, args: &[&str]) -> LoadedAtom {
    loaded_relation(pred, args)
}

fn loaded_relation(pred: &str, args: &[&str]) -> LoadedAtom {
    loaded_atom(
        pred,
        args.iter()
            .map(|arg| Term::Sym(Ident::synthetic(*arg)))
            .collect(),
    )
}

fn loaded_atom(pred: &str, args: Vec<Term>) -> LoadedAtom {
    let expr = AtomExpr {
        pred: Ident::synthetic(pred),
        args,
    };
    LoadedAtom {
        atom: expr,
        ann: Ann::unconditional(),
        kind: Provenance::Ratified,
        citations: Vec::new(),
        pack: "moon-cycle".to_owned(),
        span: Span::synthetic(),
        provenance_span: None,
    }
}
