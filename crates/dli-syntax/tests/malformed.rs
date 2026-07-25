#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Malformed declarations must diagnose, never silently drop (R3 H3), and the
//! keyword list must mirror the parser exactly (R3 H11).

use dli_syntax::diag::DiagClass;
use dli_syntax::{KEYWORDS, LINE_OPEN_KEYWORDS, format_source, line_open_keywords, parse_document};

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
    let source = "question q\n  fact mara keeps ledger proposed\n  stray words\n";
    let malformed = error_classes(source)
        .into_iter()
        .filter(|class| *class == DiagClass::Malformed)
        .count();
    assert_eq!(malformed, 2, "each dropped child needs its own error");
    assert!(format_source(source).is_err(), "fmt must refuse the loss");
}

#[test]
fn fork_commands_are_preserved_without_error() {
    let source = "fork q\n  suppose h\n  add fact mara keeps ledger\n";
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
    ] {
        let diagnostics = parse_document(source).1;
        let killed = diagnostics
            .iter()
            .find(|diag| diag.class == DiagClass::KilledForm)
            .unwrap_or_else(|| panic!("retired option form was not diagnosed: {source:?}"));
        assert!(killed.message.contains("claim's provenance"));
        assert!(killed.message.contains("hypothesis gate"));
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
fn trailing_junk_after_slot_fact_diagnoses() {
    assert!(error_classes("weir.trade = eel junk\n").contains(&DiagClass::Malformed));
}

// --- H3: the fmt path refuses to rewrite a lossy document ------------------

#[test]
fn fmt_refuses_a_lossy_document() {
    // The malformed second line parses to a dropped declaration; rendering the
    // document back would delete the author's line. `format_source` must refuse.
    let source = "place a \"A\" proposed\nfact mara lives-in\n";
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
