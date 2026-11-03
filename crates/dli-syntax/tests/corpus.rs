#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The committed examples and representative expert-tier blocks parse cleanly.
//!
//! This is the in-crate mirror of the repo-wide docs-oracle: it guards the
//! surface forms the spec teaches so a regression is caught in `dli-syntax`'s
//! own suite, not only in the xtask gate.

use std::path::PathBuf;

use dli_syntax::{Token, TokenKind, format_document, lex, parse_document};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn committed_examples_parse_clean() {
    let examples = repo_root().join("examples");
    let mut saw = false;
    for entry in walkdir::WalkDir::new(&examples)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path().to_path_buf();
        if path.extension().and_then(|e| e.to_str()) != Some("dli") {
            continue;
        }
        saw = true;
        let source = std::fs::read_to_string(&path).unwrap();
        let (_doc, diags) = parse_document(&source);
        let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
        assert!(
            errs.is_empty(),
            "{} produced errors: {errs:?}",
            path.display()
        );
    }
    assert!(saw, "expected at least one example .dli file");
}

/// The expert practice block from `docs/spec/04-practices.md`.
const PRACTICE: &str = "\
scope magic

practice spoken-oath  ratified D-0146 D-0148 {
  kind ground = water | field | ore | tide | hearth | boundary
  role witness : entity    requires standing(witness), consent(witness)
  resource debt   : linear conserved
  rule ward-form(ground, fibre)  ratified D-0146 :- ...
  condition seated requires phase(tide) in allowed-hours(rule)
  error fizzle             refines L2 premise-not-available where absent
  cost bodily | cognitive | material | temporal
  }
";

const TIME: &str = "\
scope chronology

era elder-cycles < the-equilibrium < the-first-enclosure
    < the-long-creep < the-beleaguered-present < the-narrators-era

event first-storehouse-lie    during the-first-enclosure   provisional D-0072
before breaking-of-the-stones -> tithe-of-minds-sweeps     provisional D-0072

cycle tide(4) = ebb | slack | flood | high        ratified D-0146
cycle moon(29)                                     ratified D-0148

state joren.water-status = commons  until enclosure-of-the-parish-water  ratified D-0114
";

const LANGUAGE: &str = "\
scope language

dialect elder-tongue-fringe  \"the fringe tongue\"  provisional D-0181 {
  vowels a e i o u w y
  name-part en   \"spirit\"        role given-proto     ratified D-0134
  }

name linet \"Linet\"  person.given in elder-tongue-fringe
  = given-proto en + given-deutero fys
  bearer protagonist   ratified D-0134

term orchard-lane \"the orchard-lane\"  ratified D-0143
banned machine   D-0056  \"the allegory is structural, never lexical\"

hypothesis h-elder-tongue \"the western fringe speaks a Brythonic-shaped tongue\"

dialect speculative  proposed agent:author/naming-run-0712  assumes h-elder-tongue {
  inventory placeholder
  }

ratify D-0195 { h-elder-tongue := true }
";

const CRAFT: &str = "\
scope craft

principle tryfail-before-triumph  family plot  mission pass
  source sanderson  cites D-0128
  checkable deterministic -> accomplishment-earned

rule accomplishment-earned  over season  severity error
  : beat $b kind accomplishment magnitude >= major
    requires beat $f  thread($f) = thread($b)

rule full-team-floor  over order  ratified D-0162
  : listener-has(configuration full-team) requires slot >= 10
";

const PROOF: &str = "\
scope magic

proof e01-s04  speaker protagonist  in spoken-oath {
  premise p1  holds(linet, consent(household, door))  cited D-0113  cue \"line 112\"
  step   s1  instantiate ward-form  with ground = door, fibre = withy
  spend  p3
  close  holds(ward(door))  expect holds
  }
";

#[test]
fn expert_blocks_parse_clean() {
    for (label, source) in [
        ("practice", PRACTICE),
        ("time", TIME),
        ("language", LANGUAGE),
        ("craft", CRAFT),
        ("proof", PROOF),
    ] {
        let (_doc, diags) = parse_document(source);
        let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
        assert!(errs.is_empty(), "{label} produced errors: {errs:?}");
    }
}

#[test]
fn practice_block_captures_its_body() {
    let (doc, diags) = parse_document(PRACTICE);
    let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let dli_ast::decl::Decl::Practice(practice) = &doc.decls[0] else {
        panic!("expected a practice");
    };
    assert_eq!(practice.name.text, "spoken-oath");
    // Every non-blank interior line is captured as a clause.
    assert_eq!(practice.body.clauses.len(), 7);
}

#[test]
fn practice_body_is_structured_by_the_front_end() {
    // LS-9: the parser is the single producer of the typed practice interior.
    // Fails-before-fix — pre-LS-9 `PracticeDecl.body` was `Vec<Clause>` with no
    // typed `def`, so `body.def` did not exist.
    let source = "\
scope magic
cycle tide(4) proposed
practice spoken-oath proposed {
  kind ground = door | field | ore
  resource debt : linear conserved
  resource strain : affine bounded body-limit
  rule ward-form(ground, fibre)
  error fizzle refines L2 premise-not-available where absent
  error overload refines L6 resource-violation on strain signals collapse
  cost bodily | material
}
";
    let (doc, diags) = parse_document(source);
    let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let dli_ast::decl::Decl::Practice(practice) = &doc.decls[1] else {
        panic!("expected a practice");
    };

    let def = &practice.body.def;
    assert_eq!(def.name.text, "spoken-oath");
    assert_eq!(def.marker, Some(dli_ast::annot::ProvenanceMarker::Proposed));
    assert_eq!(def.sorts.len(), 1);
    assert_eq!(def.sorts[0].variants.len(), 3);
    assert_eq!(def.resources.len(), 2);
    assert_eq!(def.rules.len(), 1);
    assert_eq!(def.rules[0].form.text, "ward-form");
    assert_eq!(def.rules[0].args.len(), 2);
    assert_eq!(def.errors.len(), 2);
    assert_eq!(
        def.errors[1].on_resource.as_ref().map(|r| r.text.as_str()),
        Some("strain")
    );
    assert_eq!(def.costs.len(), 2);
    // The front end leaves cycle scope for the ledger to overlay at compile time.
    assert!(def.cycles_in_scope.is_empty());

    // Store fidelity: the typed interior survives a JSON round-trip byte-for-byte
    // (this is the shape the lorestore carriage stores and reconstructs).
    let json = serde_json::to_string(practice).expect("serialize");
    let back: dli_ast::decl::PracticeDecl = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        &back, practice,
        "the typed practice interior must round-trip through JSON"
    );
}

#[test]
fn practice_label_round_trips() {
    let source = "practice p \"The Rite\" {\n  claim x\n}\n";
    let (doc, diags) = parse_document(source);
    let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
    assert!(errs.is_empty(), "parse errors: {errs:?}");

    let dli_ast::decl::Decl::Practice(practice) = &doc.decls[0] else {
        panic!("expected a practice declaration");
    };
    assert_eq!(practice.name.text, "p");
    assert_eq!(practice.label.as_deref(), Some("The Rite"));
    assert_eq!(practice.body.clauses.len(), 1);

    // Round-trip: format → re-parse → same label.
    let formatted = dli_syntax::format::format_document(&doc);
    assert!(
        formatted.contains("\"The Rite\""),
        "label missing from formatted output: {formatted}"
    );
    let (doc2, diags2) = parse_document(&formatted);
    let errs2: Vec<_> = diags2.iter().filter(|d| d.class.is_error()).collect();
    assert!(errs2.is_empty(), "re-parse errors: {errs2:?}");
    let dli_ast::decl::Decl::Practice(practice2) = &doc2.decls[0] else {
        panic!("expected a practice declaration on re-parse");
    };
    assert_eq!(practice2.label.as_deref(), Some("The Rite"));
}

#[test]
fn draft_and_export_labels_round_trip() {
    let source =
        "draft d \"Working Copy\" {\n  line one\n}\nexport e \"Public\" {\n  format json\n}\n";
    let (doc, diags) = parse_document(source);
    let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
    assert!(errs.is_empty(), "parse errors: {errs:?}");

    let dli_ast::decl::Decl::Draft(draft) = &doc.decls[0] else {
        panic!("expected a draft");
    };
    assert_eq!(draft.label.as_deref(), Some("Working Copy"));

    let dli_ast::decl::Decl::Export(export) = &doc.decls[1] else {
        panic!("expected an export");
    };
    assert_eq!(export.label.as_deref(), Some("Public"));

    let formatted = dli_syntax::format::format_document(&doc);
    assert!(formatted.contains("\"Working Copy\""));
    assert!(formatted.contains("\"Public\""));

    let (doc2, _) = parse_document(&formatted);
    let dli_ast::decl::Decl::Draft(draft2) = &doc2.decls[0] else {
        panic!("expected a draft on re-parse");
    };
    assert_eq!(draft2.label.as_deref(), Some("Working Copy"));
    let dli_ast::decl::Decl::Export(export2) = &doc2.decls[1] else {
        panic!("expected an export on re-parse");
    };
    assert_eq!(export2.label.as_deref(), Some("Public"));
}

#[test]
fn word_label_and_parts_round_trip() {
    let source = "word runestave \"rune-stave\"  = given-deutero run   decided d-0203\n";
    let (doc, diags) = parse_document(source);
    let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
    assert!(errs.is_empty(), "parse errors: {errs:?}");

    let dli_ast::decl::Decl::Word(word) = &doc.decls[0] else {
        panic!("expected a word");
    };
    assert_eq!(word.name.text, "runestave");
    assert_eq!(word.label.as_deref(), Some("rune-stave"));
    assert_eq!(word.name_parts.len(), 2);
    assert_eq!(word.name_parts[0].text, "given-deutero");
    assert_eq!(word.name_parts[1].text, "run");
    assert!(word.provenance.is_some());

    let formatted = dli_syntax::format::format_document(&doc);
    assert!(formatted.contains("\"rune-stave\""));
    assert!(formatted.contains("= given-deutero run"));

    let (doc2, _) = parse_document(&formatted);
    let dli_ast::decl::Decl::Word(word2) = &doc2.decls[0] else {
        panic!("expected a word on re-parse");
    };
    assert_eq!(word2.label.as_deref(), Some("rune-stave"));
    assert_eq!(word2.name_parts.len(), 2);
}

// ---------------------------------------------------------------------------
// Safety tests: no source token is silently dropped by format(parse(src)).
// ---------------------------------------------------------------------------

/// Collect the non-whitespace, non-comment token texts from a token stream.
fn meaningful_tokens(tokens: &[Token]) -> Vec<&str> {
    tokens
        .iter()
        .filter(|t| t.kind != TokenKind::Newline)
        .map(|t| t.text.as_str())
        .collect()
}

/// For every committed `.dli` file, the non-whitespace token sequence of the
/// formatted output must equal the non-whitespace token sequence of the source.
/// This guards against the formatter silently dropping any surface token.
#[test]
fn source_token_invariant() {
    let root = repo_root();
    let dirs = [root.join("examples"), root.join("worlds")];
    let mut saw = false;
    for dir in &dirs {
        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path().to_path_buf();
            if path.extension().and_then(|e| e.to_str()) != Some("dli") {
                continue;
            }
            saw = true;
            let source = std::fs::read_to_string(&path).unwrap();
            let src_tokens = lex(&source, dli_ast::span::FileId(0));
            let (doc, _) = parse_document(&source);
            let formatted = format_document(&doc);
            let fmt_tokens = lex(&formatted, dli_ast::span::FileId(0));
            let src_meaningful = meaningful_tokens(&src_tokens);
            let fmt_meaningful = meaningful_tokens(&fmt_tokens);
            assert_eq!(
                src_meaningful,
                fmt_meaningful,
                "token mismatch in {}: source tokens != format(parse(source)) tokens",
                path.display()
            );
        }
    }
    assert!(saw, "expected at least one .dli file");
}

/// For every committed `.dli` file, `parse(format(parse(src)))` must equal
/// `parse(src)` when compared ignoring source spans.
#[test]
fn format_parse_round_trip() {
    let root = repo_root();
    let dirs = [root.join("examples"), root.join("worlds")];
    let mut saw = false;
    for dir in &dirs {
        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path().to_path_buf();
            if path.extension().and_then(|e| e.to_str()) != Some("dli") {
                continue;
            }
            saw = true;
            let source = std::fs::read_to_string(&path).unwrap();
            let (doc1, _) = parse_document(&source);
            let formatted = format_document(&doc1);
            let (doc2, _) = parse_document(&formatted);

            // Compare ASTs ignoring spans by serializing and zeroing spans.
            let v1 = span_blind_value(&doc1);
            let v2 = span_blind_value(&doc2);
            assert_eq!(
                v1,
                v2,
                "round-trip mismatch in {}: parse(format(parse(src))) != parse(src)",
                path.display()
            );
        }
    }
    assert!(saw, "expected at least one .dli file");
}

/// For every committed `.dli` file, `parse(src)` must produce zero error-class
/// diagnostics.
#[test]
fn committed_files_zero_diagnostics() {
    let root = repo_root();
    let dirs = [root.join("examples"), root.join("worlds")];
    let mut saw = false;
    for dir in &dirs {
        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path().to_path_buf();
            if path.extension().and_then(|e| e.to_str()) != Some("dli") {
                continue;
            }
            saw = true;
            let source = std::fs::read_to_string(&path).unwrap();
            let (_, diags) = parse_document(&source);
            let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
            assert!(
                errs.is_empty(),
                "{} produced diagnostics: {errs:?}",
                path.display()
            );
        }
    }
    assert!(saw, "expected at least one .dli file");
}

/// Zero all spans in a serialized AST value so structural comparison ignores
/// position information.
fn span_blind_value(doc: &dli_ast::decl::Document) -> serde_json::Value {
    let mut value = serde_json::to_value(doc).unwrap();
    zero_spans_value(&mut value);
    value
}

fn zero_spans_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let is_span = map.len() == 3
                && map.contains_key("file")
                && map.contains_key("start")
                && map.contains_key("end");
            if is_span {
                map.insert("file".into(), 0.into());
                map.insert("start".into(), 0.into());
                map.insert("end".into(), 0.into());
            } else {
                for child in map.values_mut() {
                    zero_spans_value(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                zero_spans_value(item);
            }
        }
        _ => {}
    }
}
