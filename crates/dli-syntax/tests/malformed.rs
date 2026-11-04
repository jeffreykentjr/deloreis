#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Malformed declarations must diagnose, never silently drop (R3 H3), and the
//! keyword list must mirror the parser exactly (R3 H11).

use dli_syntax::diag::DiagClass;
use dli_syntax::{
    KEYWORDS, LINE_OPEN_KEYWORDS, format_document, format_source, line_open_keywords,
    parse_document, parse_proof_arguments, parse_proof_bindings, parse_proof_close_parts,
};

/// The error-severity classes a source produces.
fn error_classes(source: &str) -> Vec<DiagClass> {
    parse_document(source)
        .1
        .into_iter()
        .filter(|d| d.class.is_error())
        .map(|d| d.class)
        .collect()
}

// --- H3: no incomplete declaration is silently dropped ---------------------

#[test]
fn bare_fact_diagnoses() {
    assert!(error_classes("fact\n").contains(&DiagClass::Malformed));
}

#[test]
fn fact_missing_relation_and_object_diagnoses() {
    assert!(error_classes("fact a\n").contains(&DiagClass::Malformed));
}

#[test]
fn fact_missing_object_diagnoses() {
    assert!(error_classes("fact a b\n").contains(&DiagClass::Malformed));
}

#[test]
fn bare_question_diagnoses() {
    assert!(error_classes("question\n").contains(&DiagClass::Malformed));
}

#[test]
fn bare_option_diagnoses_at_top_level_and_in_a_question() {
    for source in ["option\n", "question q\n  option\n"] {
        assert!(
            error_classes(source).contains(&DiagClass::Malformed),
            "bare option was not diagnosed: {source:?}"
        );
    }
}

#[test]
fn every_non_option_question_child_diagnoses() {
    let source = "question q\n  fact lyra keeps ledger proposed\n  stray words\n";
    let malformed = error_classes(source)
        .into_iter()
        .filter(|class| *class == DiagClass::Malformed)
        .count();
    assert_eq!(malformed, 2, "each dropped child needs its own error");
    assert!(format_source(source).is_err(), "fmt must refuse the loss");
}

#[test]
fn fork_commands_are_preserved_without_error() {
    let source = "fork q\n  suppose h\n  add fact lyra keeps ledger\n";
    assert!(
        error_classes(source).is_empty(),
        "fork commands are intentional body clauses"
    );
    assert!(format_source(source).is_ok());
}

#[test]
fn malformed_fork_options_still_diagnose() {
    for source in ["fork q\n  option\n", "fork q\n  option x implies\n"] {
        assert!(
            error_classes(source).contains(&DiagClass::Malformed),
            "malformed fork option was not diagnosed: {source:?}"
        );
    }
}

#[test]
fn malformed_option_forms_diagnose() {
    for source in [
        "question q\n  option x implies\n",
        "question q\n  option x trailing\n",
    ] {
        assert!(
            error_classes(source).contains(&DiagClass::Malformed),
            "malformed option was not diagnosed: {source:?}"
        );
    }
}

#[test]
fn option_assumes_is_a_killed_form_with_claim_level_repair() {
    for source in [
        "question q\n  option x assumes h-x\n",
        "question q\n  option x implies p(x) assumes h-x\n",
        "question q\n  option x assumes\n",
        "question q {   option   x   assumes h-x }\n",
    ] {
        let diagnostics = parse_document(source).1;
        let killed = diagnostics
            .iter()
            .find(|diag| diag.class == DiagClass::KilledForm)
            .unwrap_or_else(|| panic!("retired option form was not diagnosed: {source:?}"));
        assert!(killed.message.contains("claim's provenance"));
        assert!(killed.message.contains("hypothesis gate"));
        let assumes = source.find("assumes").expect("assumes token");
        assert_eq!(killed.span.start as usize, assumes);
        assert_eq!(killed.span.end as usize, assumes + "assumes".len());
        assert!(
            format_source(source).is_err(),
            "fmt must refuse killed syntax"
        );
    }
}

#[test]
fn trailing_junk_after_fact_diagnoses() {
    assert!(error_classes("fact a b c d\n").contains(&DiagClass::Malformed));
}

#[test]
fn trailing_junk_after_entity_diagnoses() {
    assert!(error_classes("place weir junk\n").contains(&DiagClass::Malformed));
}

#[test]
fn trailing_junk_after_relation_diagnoses() {
    assert!(error_classes("adjacent(a, b) junk\n").contains(&DiagClass::Malformed));
}

#[test]
fn unterminated_relation_atom_diagnoses() {
    let source = "adjacent(a, b\n";
    assert!(error_classes(source).iter().any(|class| class.is_error()));
    assert!(format_source(source).is_err());
}

#[test]
fn trailing_junk_after_slot_fact_diagnoses() {
    assert!(error_classes("weir.trade = eel junk\n").contains(&DiagClass::Malformed));
}

// --- H3: the fmt path refuses to rewrite a lossy document ------------------

#[test]
fn fmt_refuses_a_lossy_document() {
    // The malformed second line parses to a dropped declaration; rendering the
    // document back would delete the author's line. `format_source` must refuse.
    let source = "place a \"A\" proposed\nfact lyra lives-in\n";
    let outcome = format_source(source);
    assert!(outcome.is_err(), "fmt must refuse a lossy document");
    assert!(
        outcome
            .unwrap_err()
            .iter()
            .any(|d| d.class == DiagClass::Malformed)
    );
}

#[test]
fn fmt_accepts_a_clean_document() {
    assert!(format_source("place a \"A\" proposed\n").is_ok());
}

#[test]
fn fmt_accepts_advisory_only_documents() {
    // A marker-less claim is only a warning — the AST is complete, so formatting
    // is lossless and must not be refused.
    assert!(format_source("place a\n").is_ok());
}

// --- H11: the keyword list mirrors the parser, with no stale entries --------

#[test]
fn probe_keywords_are_the_authoritative_set() {
    // Every keyword in the KEYWORDS constant must be dispatched by the parser
    // (i.e., not produce an UnknownKeyword diagnostic). This test was originally
    // `assert_eq!(KEYWORDS, LINE_OPEN_KEYWORDS)` which is a tautology — KEYWORDS
    // is defined as an alias of LINE_OPEN_KEYWORDS. We now verify against the
    // actual parser behaviour (an independent oracle).
    for kw in KEYWORDS {
        let source = format!("{kw} x\n");
        let (_doc, diags) = parse_document(&source);
        assert!(
            !diags.iter().any(|d| d.class == DiagClass::UnknownKeyword),
            "KEYWORDS lists `{kw}` but the parser rejects it as unknown"
        );
    }
    // Also verify that line_open_keywords() returns the same slice.
    assert_eq!(
        line_open_keywords(),
        LINE_OPEN_KEYWORDS,
        "line_open_keywords() must return LINE_OPEN_KEYWORDS"
    );
}

#[test]
fn no_stale_keywords_remain() {
    // The twelve forms R3 found listed but rejected by the real parser.
    for stale in [
        "hyp",
        "cite",
        "adjacent",
        "close",
        "decided",
        "premise",
        "proposed",
        "provisional",
        "ratified",
        "spend",
        "step",
        "when",
    ] {
        assert!(
            !LINE_OPEN_KEYWORDS.contains(&stale),
            "stale keyword still listed: {stale}"
        );
        // …and the parser really does reject it as a line opener.
        let (_doc, diags) = parse_document(&format!("{stale} x\n"));
        assert!(
            diags.iter().any(|d| d.class.is_error()),
            "`{stale}` should not open a valid declaration"
        );
    }
}

#[test]
fn every_listed_keyword_opens_a_line() {
    for keyword in line_open_keywords() {
        let source = format!("{keyword} placeholder\n");
        let (_doc, diags) = parse_document(&source);
        assert!(
            !diags.iter().any(|d| d.class == DiagClass::UnknownKeyword),
            "parser rejects its own listed keyword `{keyword}`"
        );
    }
    // A word not in the set is an unknown keyword.
    let (_doc, diags) = parse_document("zorble x\n");
    assert!(diags.iter().any(|d| d.class == DiagClass::UnknownKeyword));
}

#[test]
fn malformed_proof_children_and_surplus_tokens_are_refused() {
    let cases = [
        "spend p1 trailing",
        "spend \"p1\"",
        "release p1 trailing",
        "redistribute p1 trailing",
        "reveal p1 trailing",
        "close holds(a) expect holds trailing",
        "close holds(a) trailing",
        "premise p1 holds(a) cited d-1 trailing",
        "premise p1 holds(a) trailing",
        "assume p1 holds(a) trailing",
        "assume p1 holds(a) checked",
        "premise p1 holds(a) asserted attested",
        "step s1 instantiate form with x = a trailing",
        "step s1 instantiate form with x = a,",
        "step s1 instantiate form with x = a, dangling",
        "step s1 chain from p1 by law conclusion holds(a) trailing",
        "step s1 chain from p1, by law conclusion holds(a)",
        "step s1 chain from p1 by law conclusion holds(a",
        "step s1 weave = = nonsense",
        "step s1 weave a b",
        "step s1 weave a,",
        "\"assume\" p1 holds(a)",
        "\"step\" s1 form a",
        "wholly-invalid proof syntax",
    ];
    for child in cases {
        let source = format!(
            "proof p speaker a in practice {{\n  premise p1 holds(a)\n  {child}\n  close holds(a) expect holds\n}}\n"
        );
        let (_, diagnostics) = parse_document(&source);
        assert!(
            diagnostics.iter().any(|diag| diag.class.is_error()),
            "proof child was accepted: {child}"
        );
        assert!(
            format_source(&source).is_err(),
            "formatter would discard malformed proof child: {child}"
        );
    }
}

#[test]
fn nested_chain_conclusions_and_quoted_commas_are_well_formed() {
    let source = "\
proof p speaker a in practice {
  premise p1 holds(a)
  step s1 instantiate form with role = \"the, witness\"
  step s2 chain from p1, s1 by law conclusion holds(a, consent(group, act))
  step s3 form a, \"b, c\"
  close holds(a, consent(group, act)) expect holds
}
";
    let (_, diagnostics) = parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
}

#[test]
fn proof_term_parsers_preserve_quoted_commas_exactly() {
    assert_eq!(
        parse_proof_bindings("with role = \"the, witness\", place = threshold"),
        Some(vec![
            ("role".to_owned(), "the, witness".to_owned()),
            ("place".to_owned(), "threshold".to_owned()),
        ])
    );
    assert_eq!(
        parse_proof_arguments("threshold, \"withy, twisted\""),
        Some(vec!["threshold".to_owned(), "withy, twisted".to_owned()])
    );
    assert_eq!(
        parse_proof_close_parts("says(witness, \"we expect victory\") expect holds"),
        Some((
            "says(witness, \"we expect victory\")".to_owned(),
            Some("holds".to_owned()),
        ))
    );
    for malformed in ["with role = a,", "with role = a, dangling", "with role a"] {
        assert_eq!(parse_proof_bindings(malformed), None, "{malformed}");
    }
    for malformed in ["a,", "a,,b", "a b"] {
        assert_eq!(parse_proof_arguments(malformed), None, "{malformed}");
    }
}

#[test]
fn proof_strings_preserve_whitespace_and_escaped_delimiters() {
    let source = r#"proof p speaker witness in practice {
  assume p1 says(witness, "quoted  \" } # value")
  close says(witness, "quoted  \" } # value") expect holds
}
"#;
    let (document, diagnostics) = parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let rendered = format_document(&document);
    assert!(rendered.contains(r#""quoted  \" } # value""#), "{rendered}");
    let (_, reparsed) = parse_document(&rendered);
    assert!(
        reparsed
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{reparsed:?}"
    );
}

#[test]
fn quoted_proof_words_are_data_not_operations_or_grades() {
    let source = r#"proof p speaker witness in practice {
  premise p1 says(witness, "checked") cited checked tested
  premise p2 says(witness, "attested") cue "asserted checked" attested
  close says(witness, "checked") expect holds
}
"#;
    let (_, diagnostics) = parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
}

#[test]
fn proof_headers_and_close_layout_are_closed() {
    let cases = [
        "proof p speaker a garbage ignored in practice trailing ignored {\n  close x expect blocked\n}\n",
        "proof p speaker \"quoted \\\" actor\" in practice {\n  close x expect blocked\n}\n",
        "proof p speaker a in practice {\n}\n",
        "proof p speaker a in practice {\n  close x expect blocked\n  close y expect blocked\n}\n",
        "proof p speaker a in practice {\n  close x expect blocked\n  assume p1 later\n}\n",
    ];
    for source in cases {
        let (_, diagnostics) = parse_document(source);
        assert!(
            diagnostics.iter().any(|diag| diag.class.is_error()),
            "malformed proof shape was accepted: {source}"
        );
        assert!(format_source(source).is_err(), "fmt accepted: {source}");
        let (document, _) = parse_document(source);
        let rendered = format_document(&document);
        let (_, reparsed) = parse_document(&rendered);
        assert!(
            reparsed
                .iter()
                .any(|diagnostic| diagnostic.class.is_error()),
            "malformed proof header became valid after formatting: {rendered}"
        );
    }

    let custom_role = "proof p reader old-fen in tide-cycle {\n  close x expect blocked\n}\n";
    assert!(
        error_classes(custom_role).is_empty(),
        "a caller-declared role remains valid"
    );
}
#[test]
fn bearer_named_like_a_marker_keeps_its_actual_provenance() {
    let source = "scope storybook\nperson decided \"Decided\" proposed\nname bay-name \"Bay\" place in harbor\n  bearer decided proposed\n";
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let name = document
        .decls
        .iter()
        .find_map(|decl| match decl {
            dli_ast::decl::Decl::Name(name) => Some(name),
            _ => None,
        })
        .unwrap();
    assert!(name.body.clauses.iter().any(|clause| matches!(&clause.item, dli_ast::language::NameItem::Bearer { entity: Some(entity), provenance: Some(annotation) } if entity == "decided" && annotation.marker == dli_ast::annot::ProvenanceMarker::Proposed)));
    let (quoted, diagnostics) = dli_syntax::parse_document(
        "scope storybook\nname bay \"Bay\" place in harbor\n  bearer \"decided person\" proposed\n",
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.class.is_error()),
        "{diagnostics:?}"
    );
    let name = quoted
        .decls
        .iter()
        .find_map(|decl| match decl {
            dli_ast::decl::Decl::Name(name) => Some(name),
            _ => None,
        })
        .unwrap();
    assert!(name.body.clauses.iter().any(|clause| matches!(&clause.item, dli_ast::language::NameItem::Bearer { entity: Some(entity), provenance: Some(annotation) } if entity == "decided person" && annotation.marker == dli_ast::annot::ProvenanceMarker::Proposed)));
}

#[test]
fn ratify_rejects_unknown_clauses_instead_of_dropping_them() {
    for body in [
        "defeats d-1",
        "h-test := true extra",
        "h-test := maybe",
        "@ := true",
    ] {
        let source = format!("scope story\nratify d-3 {{ {body} }}\n");
        let diagnostics = parse_document(&source).1;
        assert!(
            diagnostics
                .iter()
                .any(|d| d.class.is_error() && d.message.contains("author.defeat")),
            "{body}: {diagnostics:?}"
        );
    }
    for body in [
        "h-test := true",
        "h-test = false",
        "h-test := true decided d-1",
        "h-test.junk := true",
    ] {
        let source = format!("scope story\nratify d-3 {{ {body} }}\n");
        assert!(error_classes(&source).is_empty(), "{body}");
    }
}

#[test]
fn ratify_assignment_preserves_compact_names_and_boolean_values() {
    for (body, expected_name, expected_value) in [
        ("h-x:=true", "h-x", true),
        ("h-x := false decided true", "h-x", false),
        ("h-x.junk := true", "h-x.junk", true),
    ] {
        let (doc, diagnostics) = parse_document(&format!("scope story\nratify d-3 {{ {body} }}\n"));
        assert!(diagnostics.is_empty(), "{body}: {diagnostics:?}");
        let decl = doc
            .decls
            .iter()
            .find_map(|d| match d {
                dli_ast::decl::Decl::Ratify(r) => Some(r),
                _ => None,
            })
            .unwrap();
        let assignment = decl.body.clauses[0].assignment.as_ref().unwrap();
        assert_eq!(assignment.hyp, expected_name);
        assert_eq!(assignment.value, expected_value);
    }
}

#[test]
fn unreadable_assumption_is_an_error_not_a_tag() {
    for source in [
        "scope story\nfact wren home den proposed assumes\n",
        "scope story\nfact wren home den proposed assumes (h-x and h-y)\n",
        "scope story\nfact wren home den proposed assumes h-x and (h-y)\n",
        "scope story\nfact wren home den proposed assumes h-x and\n",
        "scope story\nfact wren home den proposed assumes h-x and h-y or h-z\n",
        "scope story\nfact wren home den proposed assumes h-x assumes h-y\n",
        "scope story\nfact wren home den proposed assumes h-x and assumes h-y\n",
        "scope story\nfact wren home den proposed assumes h-x and or h-y\n",
    ] {
        let diagnostics = parse_document(source).1;
        assert!(
            diagnostics
                .iter()
                .any(|d| d.class.is_error() && d.message.contains("assumes")),
            "{diagnostics:?}"
        );
        assert!(
            format_source(source).is_err(),
            "formatter must not discard {source:?}"
        );
    }
}
