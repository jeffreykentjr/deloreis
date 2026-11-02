#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Generic language checks over the public Norse example and invalid names.

use dli_syntax::parse_document;
use dli_world::language::{self, LanguageModel, ratified_names, vet_candidate};

fn norse_model() -> LanguageModel {
    let source = include_str!("../../../examples/norse/runes.dli");
    let (document, diagnostics) = parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    LanguageModel::from_documents(&[document]).model
}

#[test]
fn public_example_compiles_and_promotes_names() {
    let model = norse_model();
    assert!(model.register("old-norse").is_some());
    let names: Vec<_> = ratified_names(&model).into_iter().map(|r| r.name).collect();
    for expected in ["Sigrun", "Thorulf", "Gudmund", "Harmund"] {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
    let findings = language::check_language(&model);
    assert!(findings.is_empty(), "{findings:?}");
    assert!(language::check(&model).is_empty());
}

#[test]
fn off_register_composition_is_rejected() {
    let source = "scope language\n\
        dialect harbor proposed {\n\
        vowels a e i o u\n\
        foreign z\n\
        cluster-max 2\n\
        name-kind place = place-qualifier place-generic\n\
        name-part bright role place-qualifier proposed\n\
        name-part cove role place-generic proposed\n\
        name-part zed role place-generic proposed\n\
        }\n\
        name bad \"Bad\" place in harbor = place-qualifier bright + place-generic zed proposed\n\
        name wrong \"Wrong\" place in harbor = place-generic bright + place-qualifier cove proposed\n";
    let (document, diagnostics) = parse_document(source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let model = LanguageModel::from_documents(&[document]).model;
    let findings = language::check_language(&model);
    let codes: Vec<_> = findings.iter().map(language::LangFinding::code).collect();
    assert!(codes.contains(&"name.phonotactics"), "{codes:?}");
    assert!(
        codes.contains(&"name.slot-sequence") || codes.contains(&"name.role-membership"),
        "{codes:?}"
    );
}

#[test]
fn duplicate_and_near_homophone_are_detected() {
    let model = norse_model();
    let duplicate = vet_candidate(&model, "Sigrun");
    assert!(
        duplicate
            .iter()
            .any(|finding| finding.code() == "name.uniqueness"),
        "{duplicate:?}"
    );
    let near = vet_candidate(&model, "Sigrunn");
    assert!(
        near.iter()
            .any(|finding| finding.code() == "name.aural-clash"),
        "{near:?}"
    );
}
