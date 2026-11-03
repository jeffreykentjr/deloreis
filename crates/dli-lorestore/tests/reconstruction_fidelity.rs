//! Regression cases for preserving authored provenance across store reconstruction.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use dli_ast::annot::ProvenanceMarker;
use dli_ast::decl::Decl;
use dli_core::atom::Interner;
use dli_lorestore::lower::{lower_decl, lower_pack, reconstruct_pack, render_polynomial};
use dli_world::{compile, pack::Pack};

fn parse(source: &str) -> dli_ast::decl::Document {
    let (document, findings) = dli_syntax::parse_document(source);
    assert!(findings.is_empty(), "{findings:?}");
    document
}

fn remove_carried_bundle_metadata(objects: &mut [dli_lorestore::FactObject]) {
    for object in objects {
        let Some(body) = object.expert_body.as_ref() else {
            continue;
        };
        if !matches!(
            body.kind,
            dli_lorestore::object::ExpertDeclKind::Entity
                | dli_lorestore::object::ExpertDeclKind::Rule
        ) {
            continue;
        }
        let mut decl: Decl = serde_json::from_str(object.carriage_json().unwrap()).unwrap();
        match &mut decl {
            Decl::Entity(entity) => entity.provenance.as_mut().unwrap().bundle = None,
            Decl::Rule(rule) => rule.provenance.as_mut().unwrap().bundle = None,
            _ => unreachable!(),
        }
        object.expert_body.as_mut().unwrap().body[0] = serde_json::to_string(&decl).unwrap();
        if let Some(provenance) = &mut object.provenance {
            provenance.located_body = None;
        }
    }
}

fn supports(pack: &Pack) -> Vec<String> {
    let world = compile(pack).expect("pack compiles");
    world
        .model
        .atoms()
        .iter()
        .map(|atom| {
            format!(
                "{}: {}",
                atom.atom.pred.text,
                render_polynomial(&atom.ann.support, world.model.interner())
            )
        })
        .collect()
}

#[test]
fn anonymous_language_proposal_without_its_own_interner_refuses_replay() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![parse(
            "scope a\ndialect harbor proposed {\n vowels a e i o u\n}\n",
        )],
    };
    let (mut objects, names) = lower_pack(&pack);
    let object = objects
        .iter_mut()
        .find(|object| object.expert_body.is_some())
        .unwrap();
    let mut decl: Decl = serde_json::from_str(object.carriage_json().unwrap()).unwrap();
    let Decl::Dialect(dialect) = &mut decl else {
        unreachable!()
    };
    dialect.provenance.as_mut().unwrap().bundle = None;
    object.expert_body.as_mut().unwrap().body[0] = serde_json::to_string(&decl).unwrap();
    object.provenance.as_mut().unwrap().located_body = None;
    let error = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("anonymous language proposal"),
        "{error}"
    );
}

#[test]
fn language_proposals_keep_their_separate_bundle_identities_through_text() {
    use dli_world::language::LanguageModel;
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse(
                "scope a\ndialect harbor proposed {\n name-part tide \"water\" role place proposed\n}\n",
            ),
            parse("scope b\nterm shore \"shore\" proposed\n"),
        ],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    let original = LanguageModel::from_documents(&pack.documents).model;
    let replay = LanguageModel::from_documents(&[parse(&rendered)]).model;
    let support = |model: &LanguageModel, ann: &dli_core::ann::Ann| {
        render_polynomial(&ann.support, &model.interner)
    };
    assert_eq!(
        support(&original, &original.registers[0].ann),
        support(&replay, &replay.registers[0].ann),
        "{rendered}"
    );
    assert_eq!(
        support(&original, &original.registers[0].morphemes[0].ann),
        support(&replay, &replay.registers[0].morphemes[0].ann),
        "{rendered}"
    );
    assert_eq!(
        support(&original, &original.terms[0].ann),
        support(&replay, &replay.terms[0].ann),
        "{rendered}"
    );
}

#[test]
fn language_proposals_keep_citations_and_bundle_identity() {
    use dli_world::language::LanguageModel;
    let pack = Pack {
        name: "b".into(),
        documents: vec![parse("scope a\nterm shore \"shore\" proposed d-1\n")],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    let original = LanguageModel::from_documents(&pack.documents).model;
    let replay = LanguageModel::from_documents(&[parse(&rendered)]).model;
    assert_eq!(original.terms[0].citations, replay.terms[0].citations);
    assert_eq!(
        render_polynomial(&original.terms[0].ann.support, &original.interner),
        render_polynomial(&replay.terms[0].ann.support, &replay.interner)
    );
}

#[test]
fn proposed_fact_citations_survive_store_and_text_replay() {
    let pack = Pack {
        name: "a".into(),
        documents: vec![parse("scope a\nseed(wren) proposed d-1\n")],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "a",
    )
    .unwrap();
    let text = dli_syntax::format_document(&restored.documents[0]);
    let doc = parse(&text);
    let Decl::Fact(fact) = &doc.decls[0] else {
        panic!("fact: {text}")
    };
    let annotation = fact.provenance.as_ref().unwrap();
    assert_eq!(annotation.citations[0].id.text, "d-1");
    assert_eq!(annotation.bundle.as_deref(), Some("a#0"));
    assert_eq!(
        supports(&pack),
        supports(&Pack {
            name: "a".into(),
            documents: vec![doc]
        })
    );
}

#[test]
fn proposed_attribute_and_bare_entity_citations_survive_replay() {
    let pack = Pack {
        name: "a".into(),
        documents: vec![parse(
            "scope a\nperson wren proposed d-1\nwren.home = nest proposed d-2\n",
        )],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "a",
    )
    .unwrap();
    let text = dli_syntax::format_document(&restored.documents[0]);
    let reparsed = parse(&text);
    assert!(text.contains("d-1"), "{text}");
    assert!(text.contains("d-2"), "{text}");
    assert_eq!(
        supports(&pack),
        supports(&Pack {
            name: "a".into(),
            documents: vec![reparsed]
        })
    );
}

#[test]
fn distinct_citations_on_the_same_proposal_survive_store_deduplication() {
    let pack = Pack {
        name: "a".into(),
        documents: vec![parse(
            "scope a\nseed(wren) proposed-bundle \"a#0\" d-1\nseed(wren) proposed-bundle \"a#0\" d-2\n",
        )],
    };
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let head = store.import_pack(&pack).unwrap();
    let restored = store.reconstruct_at_uncached(&head).unwrap();
    let text = dli_syntax::format_document(&restored.documents[0]);
    let reparsed = parse(&text);
    let citations: BTreeSet<_> = reparsed
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fact(fact) => fact
                .provenance
                .as_ref()
                .and_then(|p| p.citations.first())
                .map(|c| c.id.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(citations, ["d-1", "d-2"].into_iter().collect(), "{text}");
}

#[test]
fn cited_proposal_carriage_cannot_regenerate_an_absent_fact() {
    for source in [
        "scope a\nseed(wren) proposed named-origin d-1\n",
        "scope a\nwren.home = nest proposed named-origin d-1\n",
        "scope a\nperson wren proposed named-origin d-1\n",
    ] {
        let pack = Pack {
            name: "a".into(),
            documents: vec![parse(source)],
        };
        let (mut objects, names) = lower_pack(&pack);
        objects.retain(|object| object.expert_body.is_some());
        let error = reconstruct_pack(
            &objects,
            &Interner::from_names(names),
            &BTreeSet::new(),
            "a",
        )
        .unwrap_err();
        assert!(error.to_string().contains("absent"), "{source}: {error}");
    }
}

#[test]
fn carried_entity_cannot_regenerate_a_missing_tagged_slot() {
    let pack = Pack {
        name: "a".into(),
        documents: vec![parse(
            "scope a\nperson wren proposed d-1\n  wren.home = nest proposed origin d-2\n",
        )],
    };
    let (mut objects, names) = lower_pack(&pack);
    objects.retain(|object| object.atom.pred != "home");
    let error = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "a",
    )
    .unwrap_err();
    assert!(error.to_string().contains("absent"), "{error}");
}

#[test]
fn incremental_language_lowering_uses_its_own_counter() {
    let doc = parse(
        "scope a\nseed(wren) proposed\ndialect harbor proposed {\n vowels a e i o u\n}\nterm shore \"shore\" proposed\n",
    );
    let mut interner = Interner::new();
    let mut world_counter = 0;
    let mut language_counter = 0;
    let mut objects = Vec::new();
    for decl in &doc.decls {
        objects.extend(dli_lorestore::lower::lower_decl_with_language_counter(
            decl,
            "a",
            &mut interner,
            &mut world_counter,
            &mut language_counter,
        ));
    }
    let pack = reconstruct_pack(&objects, &interner, &BTreeSet::new(), "a").unwrap();
    let text = dli_syntax::format_document(&pack.documents[0]);
    assert!(
        text.contains("dialect harbor proposed-bundle \"a#0\""),
        "{text}"
    );
    assert!(
        text.contains("term shore \"shore\" proposed-bundle \"a#1\""),
        "{text}"
    );
}

#[test]
fn reconstruct_preserves_anonymous_proposals_across_scopes() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse("scope a\nperson wren \"Wren\" proposed\n"),
            parse("scope b\nperson robin \"Robin\" proposed\n"),
        ],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .expect("reconstructs without changing provenance");
    assert_eq!(supports(&pack), supports(&restored));
}

#[test]
fn reconstruct_preserves_current_standing_of_carried_entity() {
    let pack = Pack {
        name: "story".into(),
        documents: vec![parse(
            "scope story\nratify d-1\nperson wren \"Wren\" decided d-1\n",
        )],
    };
    let (mut objects, names) = lower_pack(&pack);
    let fact = objects
        .iter_mut()
        .find(|object| object.expert_body.is_none())
        .unwrap();
    fact.provenance.as_mut().unwrap().marker = Some(ProvenanceMarker::Provisional);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &["d-1".to_owned()].into_iter().collect(),
        "story",
    )
    .expect("reconstructs without losing standing");
    let marker = restored
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .find_map(|decl| match decl {
            Decl::Entity(entity) => entity.provenance.as_ref().map(|ann| ann.marker),
            _ => None,
        });
    assert_eq!(marker, Some(ProvenanceMarker::Provisional));
}

#[test]
fn reconstruct_preserves_anonymous_proposal_after_an_interner_gap() {
    let document = parse("scope story\nperson wren \"Wren\" proposed\n");
    let mut interner = Interner::from_names(vec!["h.story#0".into()]);
    let objects = lower_decl(&document.decls[0], "story", &mut interner, &mut 1);
    let restored = reconstruct_pack(&objects, &interner, &BTreeSet::new(), "story")
        .expect("reconstructs without renumbering proposal");
    assert!(
        supports(&restored)
            .iter()
            .any(|support| support.contains("h.story#1"))
    );
}

#[test]
fn multi_scope_proposals_remain_identical_after_text_round_trip() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse("scope a\nperson wren \"Wren\" proposed\n"),
            parse("scope b\nperson robin \"Robin\" proposed\n"),
        ],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    let reparsed = parse(&rendered);
    let text_pack = Pack {
        name: "b".into(),
        documents: vec![reparsed],
    };
    assert_eq!(supports(&pack), supports(&text_pack), "{rendered}");
}

#[test]
fn multi_scope_import_reopens_and_renders_the_same_proposal_supports() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse("scope a\nperson wren \"Wren\" proposed\n"),
            parse("scope b\nperson robin \"Robin\" proposed\n"),
        ],
    };
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let head = store.import_pack(&pack).unwrap();
    drop(store);
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let reopened = store.reconstruct_at_uncached(&head).unwrap();
    assert_eq!(supports(&pack), supports(&reopened));
    let rendered = dli_syntax::format_document(&reopened.documents[0]);
    let reparsed = Pack {
        name: "b".into(),
        documents: vec![parse(&rendered)],
    };
    assert_eq!(supports(&pack), supports(&reparsed));
}

#[test]
fn proposed_ordering_from_another_scope_keeps_its_support() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse(
                "scope a\nevent dawn proposed\nevent noon proposed\nbefore dawn -> noon proposed\n",
            ),
            parse("scope b\nperson wren \"Wren\" proposed\n"),
        ],
    };
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    assert_eq!(supports(&pack), supports(&restored));
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    let roundtrip = Pack {
        name: "b".into(),
        documents: vec![parse(&rendered)],
    };
    assert_eq!(supports(&pack), supports(&roundtrip), "{rendered}");
}

#[test]
fn indistinguishable_carried_proposals_are_refused_without_guessing() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse("scope a\nperson wren \"Wren\" proposed\n"),
            parse("scope b\nperson wren \"Wren\" proposed\n"),
        ],
    };
    let (mut objects, names) = lower_pack(&pack);
    remove_carried_bundle_metadata(&mut objects);
    let error = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("more than one anonymous proposal bundle"),
        "{error}"
    );
}

#[test]
fn proposed_rule_from_another_scope_keeps_its_support() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse("scope a\nseed(wren) proposed\nrule infer proposed\n  seen($x) :- seed($x)\n"),
            parse("scope b\nperson robin \"Robin\" proposed\n"),
        ],
    };
    let signature = |world: &dli_world::CompiledWorld| {
        let fact = world
            .model
            .world
            .derived
            .iter()
            .find(|row| row.atom.pred == "seen")
            .expect("rule derives seen");
        render_polynomial(&fact.ann().support, &world.model.world.interner)
    };
    let original = compile(&pack).unwrap();
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    let replay = compile(&restored).unwrap();
    assert_eq!(signature(&original), signature(&replay));
}

#[test]
fn missing_fact_makes_unattached_rule_bundle_ambiguous() {
    let pack = Pack {
        name: "story".into(),
        documents: vec![parse(
            "scope story\nseed(wren) proposed\nrule infer proposed\n  seen($x) :- seed($x)\n",
        )],
    };
    let (mut objects, names) = lower_pack(&pack);
    objects.retain(|object| object.atom.pred != "seed");
    remove_carried_bundle_metadata(&mut objects);
    let error = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "story",
    )
    .unwrap_err();
    assert!(error.to_string().contains("ambiguous"), "{error}");
}

#[test]
fn ordered_rule_bundles_from_two_scopes_survive_replay() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse("scope a\nseed(wren) proposed\nrule first proposed\n  seen($x) :- seed($x)\n"),
            parse("scope b\nrule second proposed\n  known($x) :- seen($x)\n"),
        ],
    };
    let signature = |world: &dli_world::CompiledWorld| {
        let fact = world
            .model
            .world
            .derived
            .iter()
            .find(|row| row.atom.pred == "known")
            .unwrap();
        render_polynomial(&fact.ann().support, &world.model.world.interner)
    };
    let original = compile(&pack).unwrap();
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    assert_eq!(
        signature(&original),
        signature(&compile(&restored).unwrap())
    );
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    let reparsed = Pack {
        name: "b".into(),
        documents: vec![parse(&rendered)],
    };
    assert_eq!(
        signature(&original),
        signature(&compile(&reparsed).unwrap()),
        "{rendered}"
    );
}

#[test]
fn explicit_and_implicit_rule_bundles_replay_together() {
    let pack = Pack {
        name: "story".into(),
        documents: vec![parse(
            "scope story\nseed(wren) proposed\nrule first proposed-bundle \"other#3\"\n  seen($x) :- seed($x)\nrule second proposed\n  known($x) :- seed($x)\n",
        )],
    };
    let original = compile(&pack).unwrap();
    let (objects, names) = lower_pack(&pack);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "story",
    )
    .unwrap();
    let replay = compile(&restored).unwrap();
    for predicate in ["seen", "known"] {
        let support = |world: &dli_world::CompiledWorld| {
            let row = world
                .model
                .world
                .derived
                .iter()
                .find(|row| row.atom.pred == predicate)
                .unwrap();
            render_polynomial(&row.ann().support, &world.model.world.interner)
        };
        assert_eq!(support(&original), support(&replay), "{predicate}");
    }
}

#[test]
fn newly_stored_rules_keep_bundles_when_carriages_are_reordered() {
    let pack = Pack {
        name: "story".into(),
        documents: vec![parse(
            "scope story\nseed(wren) proposed\nrule first proposed\n  seen($x) :- seed($x)\nrule second proposed\n  known($x) :- seed($x)\n",
        )],
    };
    let original = compile(&pack).unwrap();
    let (mut objects, names) = lower_pack(&pack);
    let indices: Vec<_> = objects
        .iter()
        .enumerate()
        .filter(|(_, object)| {
            object
                .expert_body
                .as_ref()
                .is_some_and(|body| body.kind == dli_lorestore::object::ExpertDeclKind::Rule)
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(indices.len(), 2);
    objects.swap(indices[0], indices[1]);
    let restored = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "story",
    )
    .unwrap();
    let replay = compile(&restored).unwrap();
    for pred in ["seen", "known"] {
        let support = |world: &dli_world::CompiledWorld| {
            let row = world
                .model
                .world
                .derived
                .iter()
                .find(|row| row.atom.pred == pred)
                .unwrap();
            render_polynomial(&row.ann().support, &world.model.world.interner)
        };
        assert_eq!(support(&original), support(&replay), "{pred}");
    }
}

#[test]
fn legacy_carriages_with_ambiguous_rule_bundles_are_refused() {
    let pack = Pack {
        name: "story".into(),
        documents: vec![parse(
            "scope story\nseed(wren) proposed\nrule first proposed\n  seen($x) :- seed($x)\nrule second proposed\n  known($x) :- seed($x)\n",
        )],
    };
    let (mut objects, names) = lower_pack(&pack);
    for object in &mut objects {
        if object
            .expert_body
            .as_ref()
            .is_some_and(|body| body.kind == dli_lorestore::object::ExpertDeclKind::Rule)
        {
            let mut decl: Decl = serde_json::from_str(object.carriage_json().unwrap()).unwrap();
            let Decl::Rule(rule) = &mut decl else {
                unreachable!()
            };
            rule.provenance.as_mut().unwrap().bundle = None;
            object.expert_body.as_mut().unwrap().body[0] = serde_json::to_string(&decl).unwrap();
            if let Some(provenance) = &mut object.provenance {
                provenance.located_body = None;
            }
        }
    }
    let error = reconstruct_pack(
        &objects,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "story",
    )
    .unwrap_err();
    assert!(error.to_string().contains("ambiguous"), "{error}");
}

#[test]
fn new_same_entity_carriages_keep_their_own_slots_and_supports_when_reordered() {
    let pack = Pack {
        name: "b".into(),
        documents: vec![
            parse("scope a\nperson wren \"Wren\" proposed\n  wren.home = hill\n"),
            parse("scope b\nperson wren \"Wren\" proposed\n  wren.home = shore\n"),
        ],
    };
    let signature = |pack: &Pack| -> BTreeSet<String> {
        let world = compile(pack).unwrap();
        world
            .model
            .atoms()
            .iter()
            .map(|atom| {
                format!(
                    "{}: {}",
                    dli_eval::world::GroundAtom::from_pattern(&atom.atom)
                        .unwrap()
                        .render(),
                    render_polynomial(&atom.ann.support, world.model.interner())
                )
            })
            .collect()
    };
    let expected = signature(&pack);
    let (objects, names) = lower_pack(&pack);
    assert_eq!(objects.len(), 6);
    let reordered = [
        objects[3].clone(),
        objects[4].clone(),
        objects[5].clone(),
        objects[0].clone(),
        objects[1].clone(),
        objects[2].clone(),
    ];
    let restored = reconstruct_pack(
        &reordered,
        &Interner::from_names(names),
        &BTreeSet::new(),
        "b",
    )
    .unwrap();
    assert_eq!(signature(&restored), expected);
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    let reparsed = Pack {
        name: "b".into(),
        documents: vec![parse(&rendered)],
    };
    assert_eq!(signature(&reparsed), expected, "{rendered}");
}

#[test]
fn scope_less_proposal_survives_store_and_text_replay() {
    let pack = Pack {
        name: "story".into(),
        documents: vec![parse("person wren \"Wren\" proposed\n")],
    };
    let expected = supports(&pack);
    let root = tempfile::tempdir().unwrap();
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let head = store.import_pack(&pack).unwrap();
    drop(store);
    let store = dli_lorestore::Lorestore::open(root.path().to_path_buf()).unwrap();
    let restored = store.reconstruct_at_uncached(&head).unwrap();
    assert_eq!(supports(&restored), expected);
    let rendered = dli_syntax::format_document(&restored.documents[0]);
    assert!(rendered.contains("proposed-bundle \"#0\""), "{rendered}");
    let reparsed = Pack {
        name: "story".into(),
        documents: vec![parse(&rendered)],
    };
    assert_eq!(supports(&reparsed), expected);
}
