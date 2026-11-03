//! Explicit bundle identities survive parse and format.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use dli_ast::decl::Decl;
use dli_syntax::{format_document, parse_document};

#[test]
fn proposal_bundle_round_trips_and_keeps_its_exact_id() {
    let source = "scope b\nperson wren \"Wren\" proposed-bundle \"a#17\"\n";
    let (document, findings) = parse_document(source);
    assert!(findings.is_empty(), "{findings:?}");
    let Decl::Entity(entity) = &document.decls[0] else {
        panic!("entity")
    };
    assert_eq!(
        entity.provenance.as_ref().unwrap().bundle.as_deref(),
        Some("a#17")
    );
    let formatted = format_document(&document);
    let (again, findings) = parse_document(&formatted);
    assert!(findings.is_empty(), "{formatted}: {findings:?}");
    assert_eq!(format_document(&again), formatted);
}

#[test]
fn proposal_bundle_can_retain_a_citation() {
    let (doc, findings) =
        parse_document("scope a\nterm shore \"shore\" proposed-bundle \"a#0\" d-1\n");
    assert!(findings.is_empty(), "{findings:?}");
    let rendered = format_document(&doc);
    let (_, findings) = parse_document(&rendered);
    assert!(findings.is_empty(), "{rendered}: {findings:?}");
}

#[test]
fn malformed_or_ambiguous_bundle_annotations_are_errors() {
    for annotation in [
        "proposed-bundle \"not-numbered\"",
        "proposed-bundle \"a#+1\"",
        "proposed-bundle \"a#1\" note",
        "proposed-bundle \"a#1\" assumes h",
        "proposed-bundle \"a#1\" proposed-bundle \"a#2\"",
    ] {
        let (_, findings) =
            parse_document(&format!("scope story\nperson wren \"Wren\" {annotation}\n"));
        assert!(
            findings.iter().any(|d| d.class.is_error()),
            "{annotation}: {findings:?}"
        );
    }
}

#[test]
fn old_bundle_word_and_quoted_tag_keep_their_existing_meaning() {
    let (document, findings) =
        parse_document("scope story\nperson wren \"Wren\" proposed bundle \"a#1\"\n");
    assert!(findings.is_empty(), "{findings:?}");
    let Decl::Entity(entity) = &document.decls[0] else {
        panic!("entity")
    };
    let provenance = entity.provenance.as_ref().unwrap();
    assert_eq!(provenance.bundle, None);
    assert_eq!(provenance.tags, ["bundle", "\"a#1\""]);
}

#[test]
fn old_proposed_bundle_word_remains_a_tag_after_an_ordinary_marker() {
    let (doc, findings) = parse_document("scope story\nseed(wren) proposed proposed-bundle\n");
    assert!(findings.is_empty(), "{findings:?}");
    let Decl::Fact(fact) = &doc.decls[0] else {
        unreachable!()
    };
    assert_eq!(fact.provenance.as_ref().unwrap().tags, ["proposed-bundle"]);
    let (_, findings) = parse_document(&format_document(&doc));
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn typed_dialect_parts_render_their_actual_provenance() {
    for provenance in [
        "proposed named-origin",
        "proposed-bundle \"foreign#7\"",
        "ratified d-1",
    ] {
        let source = format!(
            "scope story\ndialect harbor proposed x {{\n name-part tide \"water\" role place {provenance}\n}}\n"
        );
        let (mut doc, findings) = parse_document(&source);
        assert!(findings.is_empty(), "{findings:?}");
        let Decl::Dialect(dialect) = &mut doc.decls[0] else {
            unreachable!()
        };
        dialect.body.clauses[0].clause.text = "name-part tide role place \"water\"".to_owned();
        let text = format_document(&doc);
        assert!(text.contains(provenance), "{text}");
        let (again, findings) = parse_document(&text);
        assert!(findings.is_empty(), "{text}: {findings:?}");
        let Decl::Dialect(dialect) = &again.decls[0] else {
            unreachable!()
        };
        let dli_ast::language::DialectItem::NamePart(part) = &dialect.body.clauses[0].item else {
            unreachable!()
        };
        assert!(part.provenance.is_some(), "{text}");
    }
}

#[test]
fn typed_name_clauses_render_their_actual_provenance() {
    for line in [
        "bearer wren proposed named-origin",
        "proposed-bundle \"a#7\"",
    ] {
        let source = format!("scope story\nname wren-name {{\n {line}\n}}\n");
        let (mut doc, findings) = parse_document(&source);
        assert!(findings.is_empty(), "{findings:?}");
        let Decl::Name(name) = &mut doc.decls[0] else {
            unreachable!()
        };
        name.body.clauses[0].clause.text = if line.starts_with("bearer") {
            "bearer wren proposed other".to_owned()
        } else {
            "proposed-bundle \"a#8\"".to_owned()
        };
        let text = format_document(&doc);
        assert!(text.contains(line), "{text}");
        let (_, findings) = parse_document(&text);
        assert!(findings.is_empty(), "{text}: {findings:?}");
    }
}

#[test]
fn quoted_name_bearers_remain_a_single_value() {
    for bearer in ["wren#bird", "decided person", "x proposed y"] {
        let source = format!("scope a\nname person-name {{\n bearer \"{bearer}\" proposed\n}}\n");
        let (doc, findings) = parse_document(&source);
        assert!(findings.is_empty(), "{findings:?}");
        let text = format_document(&doc);
        let (again, findings) = parse_document(&text);
        assert!(findings.is_empty(), "{text}: {findings:?}");
        let Decl::Name(name) = &again.decls[0] else {
            unreachable!()
        };
        let dli_ast::language::NameItem::Bearer { entity, provenance } = &name.body.clauses[0].item
        else {
            unreachable!()
        };
        assert_eq!(entity.as_deref(), Some(bearer), "{text}");
        assert!(provenance.is_some(), "{text}");
    }
}
