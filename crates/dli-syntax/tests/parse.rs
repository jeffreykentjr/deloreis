#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Golden parses, canonical rendering, and error recovery for the front end.

use dli_ast::annot::ProvenanceMarker;
use dli_ast::clause::Clause;
use dli_ast::decl::{Decl, TimeClause};
use dli_ast::span::Span;
use dli_ast::term::Term;
use dli_syntax::diag::{DiagClass, Diagnostic};
use dli_syntax::{format_document, parse_clause_atom, parse_document, render};

/// The error-severity diagnostics only — advisory warnings (origin-missing) are
/// legal surface (koan 5) and never a parse failure.
fn errors(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    diags.iter().filter(|d| d.class.is_error()).collect()
}

const SALTMARKET_CARD: &str = "\
pack saltmarket

era founding   \"The Landing\"      decided d-0001
era now        \"The Ninth Tide\"   after founding

place saltmarket   \"Saltmarket\"     decided d-0001
place the-shallows \"The Shallows\"   decided d-0003
place breakwater   \"The Breakwater\" proposed

adjacent(saltmarket, the-shallows)  decided d-0003
";

const ENTITY_SLOTS: &str = "\
person mara \"Mara of the Weirs\"     decided d-0002
  mara.trade = eel-fisher           decided d-0002
  mara.home  = the-shallows         proposed

thing tide-bell \"The Tide-Bell\"     decided d-0004
  tide-bell.kept-at = saltmarket    decided d-0004
";

const QUESTION_CARD: &str = "\
question q-0001 \"Who raised the Tide-Bell?\"
  option guild      implies raised-by(tide-bell, guild)
  option marsh-folk implies raised-by(tide-bell, marsh-folk)
";

#[test]
fn parses_the_saltmarket_card() {
    let (doc, diags) = parse_document(SALTMARKET_CARD);
    assert!(
        errors(&diags).is_empty(),
        "unexpected errors: {:?}",
        errors(&diags)
    );
    assert_eq!(doc.pack.as_ref().unwrap().name.text, "saltmarket");
    // Two eras, three places, one relation fact.
    assert_eq!(doc.decls.len(), 6);
    assert!(matches!(doc.decls[0], Decl::Entity(_)));
    // The last declaration is the `adjacent(..)` relation fact.
    let Decl::Fact(fact) = doc.decls.last().unwrap() else {
        panic!("expected a relation fact");
    };
    assert_eq!(fact.atom.pred.text, "adjacent");
    assert_eq!(fact.atom.args.len(), 2);
    assert!(!fact.triple);
}

#[test]
fn entity_slots_lower_into_the_entity() {
    let (doc, diags) = parse_document(ENTITY_SLOTS);
    assert!(
        errors(&diags).is_empty(),
        "unexpected errors: {:?}",
        errors(&diags)
    );
    let Decl::Entity(mara) = &doc.decls[0] else {
        panic!("expected an entity");
    };
    assert_eq!(mara.name.text, "mara");
    assert_eq!(mara.slots.len(), 2);
    assert_eq!(mara.slots[0].subject.text, "mara");
    assert_eq!(mara.slots[0].attr.text, "trade");
}

#[test]
fn question_options_lower_into_the_question() {
    let (doc, diags) = parse_document(QUESTION_CARD);
    assert!(
        errors(&diags).is_empty(),
        "unexpected errors: {:?}",
        errors(&diags)
    );
    let Decl::Question(q) = &doc.decls[0] else {
        panic!("expected a question");
    };
    assert_eq!(q.options.len(), 2);
    assert_eq!(q.options[0].name.text, "guild");
    assert!(q.options[0].implies.is_some());
}

#[test]
fn column_free_options_fold_into_the_question() {
    // The committed examples write options at column zero, not indented.
    let source = "question who-keeps-the-ledger\noption old-fen\noption mara\n";
    let (doc, diags) = parse_document(source);
    assert!(
        errors(&diags).is_empty(),
        "unexpected errors: {:?}",
        errors(&diags)
    );
    let Decl::Question(q) = &doc.decls[0] else {
        panic!("expected a question");
    };
    assert_eq!(q.options.len(), 2);
}

#[test]
fn canonical_rendering_of_the_card() {
    let (doc, diags) = parse_document(SALTMARKET_CARD);
    assert!(errors(&diags).is_empty());
    insta::assert_snapshot!("saltmarket_card_canonical", format_document(&doc));
}

#[test]
fn canonical_rendering_of_entity_slots() {
    let (doc, _) = parse_document(ENTITY_SLOTS);
    insta::assert_snapshot!("entity_slots_canonical", format_document(&doc));
}

// --- error recovery: diagnostics speak the domain --------------------------

#[test]
fn unknown_keyword_diagnoses_and_recovers() {
    let source = "place saltmarket\nzorble the-thing\nplace weir\n";
    let (doc, diags) = parse_document(source);
    // The two good declarations still parse — recovery is per record.
    assert_eq!(doc.decls.len(), 2);
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].class, DiagClass::UnknownKeyword);
}

#[test]
fn a_ledger_marker_needs_a_decision() {
    let source = "place weir \"The Weir\" ratified\n";
    let (_doc, diags) = parse_document(source);
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].class, DiagClass::OriginRequired);
}

#[test]
fn a_stray_option_has_no_question() {
    let source = "option lonely\n";
    let (_doc, diags) = parse_document(source);
    let errs = errors(&diags);
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].class, DiagClass::StrayOption);
}

// --- parse_clause_atom: the one way captured clauses become typed atoms ------

fn clause(text: &str) -> Clause {
    Clause {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

#[test]
fn clause_atom_reads_a_relation() {
    let atom = parse_clause_atom(&clause("adjacent(the-shallows, saltmarket)")).unwrap();
    assert_eq!(atom.pred.text, "adjacent");
    assert_eq!(atom.args.len(), 2);
    assert!(matches!(&atom.args[0], Term::Sym(s) if s.text == "the-shallows"));
}

#[test]
fn clause_atom_reads_a_slot_fact() {
    // `subject.slot = value` lowers to `slot(subject, value)`, matching the loader.
    let atom = parse_clause_atom(&clause("mara.trade = eel-fisher")).unwrap();
    assert_eq!(atom.pred.text, "trade");
    assert!(matches!(&atom.args[0], Term::Sym(s) if s.text == "mara"));
    assert!(matches!(&atom.args[1], Term::Sym(s) if s.text == "eel-fisher"));
}

#[test]
fn clause_atom_reads_a_triple() {
    let atom = parse_clause_atom(&clause("fact mara lives-in saltmarket")).unwrap();
    assert_eq!(atom.pred.text, "lives-in");
    assert_eq!(atom.args.len(), 2);
}

#[test]
fn clause_atom_rejects_non_atoms() {
    // A rule body, an operator expression, and free text are all not single atoms.
    assert!(parse_clause_atom(&clause("adjacent(A, B) :- adjacent(B, A)")).is_none());
    assert!(parse_clause_atom(&clause(": beat $b requires slot >= 10")).is_none());
    assert!(parse_clause_atom(&clause("resource debt : linear conserved")).is_none());
}

// --- era chains keep provenance out of the chain (V1) ----------------------

#[test]
fn era_chain_keeps_provenance_out_of_the_chain() {
    // The exact repro: the trailing marker + citation must be provenance, not
    // two fake era members.
    let source = "era first-tides < the-silting < the-ledger-days decided d-0001\n";
    let (doc, diags) = parse_document(source);
    assert!(errors(&diags).is_empty(), "{:?}", errors(&diags));
    let Decl::Entity(era) = &doc.decls[0] else {
        panic!("expected an era");
    };
    let Some(TimeClause::Order(chain)) = &era.time else {
        panic!("expected an order chain");
    };
    let members: Vec<&str> = chain.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(
        members,
        ["the-silting", "the-ledger-days"],
        "no fake members"
    );
    let prov = era.provenance.as_ref().expect("provenance preserved");
    assert_eq!(prov.marker, ProvenanceMarker::Decided);
    assert_eq!(prov.citations.len(), 1);
    assert_eq!(prov.citations[0].id.text, "d-0001");
}

#[test]
fn era_chain_round_trips_with_provenance() {
    let source = "era first-tides < the-silting < the-ledger-days decided d-0001\n";
    let (doc, _) = parse_document(source);
    let text = format_document(&doc);
    assert!(
        text.contains("era first-tides < the-silting < the-ledger-days decided d-0001"),
        "canonical line lost provenance: {text}"
    );
    let (doc2, _) = parse_document(&text);
    assert_eq!(format_document(&doc2), text, "not idempotent");
}

#[test]
fn a_marker_less_era_chain_still_warns() {
    let (doc, diags) = parse_document("era a < b < c\n");
    assert_eq!(doc.decls.len(), 1);
    assert!(errors(&diags).is_empty());
    assert!(diags.iter().any(|d| d.class == DiagClass::OriginMissing));
}

// --- lexical hazards never lose records (R1 fixes) -------------------------

#[test]
fn a_brace_in_a_label_does_not_swallow_records() {
    // H2 repro: an unbalanced `{` inside a string must stay text, not open a
    // block that eats every following declaration.
    let source =
        "place a \"a {big weir\" proposed\nplace b \"B\" proposed\nplace c \"C\" proposed\n";
    let (doc, diags) = parse_document(source);
    assert_eq!(doc.decls.len(), 3, "records were lost: {:?}", doc.decls);
    assert!(errors(&diags).is_empty(), "{:?}", errors(&diags));
}

#[test]
fn balanced_braces_in_a_label_survive_intact() {
    let source = "place a \"tide {ebb} bell\" proposed\nplace b \"B\" proposed\n";
    let (doc, _diags) = parse_document(source);
    assert_eq!(doc.decls.len(), 2);
    let Decl::Entity(entity) = &doc.decls[0] else {
        panic!("expected an entity");
    };
    assert_eq!(entity.label.as_deref(), Some("tide {ebb} bell"));
}

#[test]
fn conflicting_markers_diagnose() {
    // M2: a second marker is flagged, not silently swallowed into tags.
    let (_doc, diags) = parse_document("place weir \"W\" proposed ratified d-1\n");
    assert!(
        diags
            .iter()
            .any(|d| d.class == DiagClass::ConflictingMarkers),
        "{diags:?}"
    );
}

#[test]
fn a_leading_bom_is_stripped() {
    // M7: a UTF-8 BOM must not fuse into the first token and lose the pack.
    let source = "\u{feff}pack p\nplace weir \"W\" proposed\n";
    let (doc, _diags) = parse_document(source);
    assert_eq!(doc.pack.as_ref().unwrap().name.text, "p");
    assert_eq!(doc.decls.len(), 1);
}

#[test]
fn a_curly_quote_diagnoses_without_dropping_the_record() {
    // M7: a smart-quoted label is flagged; the record still stands.
    let source = "place weir \u{201C}The Weir\u{201D} proposed\n";
    let (doc, diags) = parse_document(source);
    assert!(diags.iter().any(|d| d.class == DiagClass::CurlyQuote));
    assert_eq!(doc.decls.len(), 1);
}

#[test]
fn an_unterminated_string_diagnoses_and_recovers() {
    // L1: an open string is flagged and the next record still parses.
    let source = "place weir \"The Weir proposed\nplace other \"O\" proposed\n";
    let (doc, diags) = parse_document(source);
    assert!(
        diags
            .iter()
            .any(|d| d.class == DiagClass::UnterminatedString)
    );
    assert_eq!(doc.decls.len(), 2);
}

#[test]
fn a_marker_less_claim_warns_but_still_parses() {
    // M1: koan 1 — the surface says a claim has no origin, but it is advisory.
    let (doc, diags) = parse_document("place weir\n");
    assert_eq!(doc.decls.len(), 1);
    assert!(errors(&diags).is_empty(), "origin-missing must be advisory");
    assert!(diags.iter().any(|d| d.class == DiagClass::OriginMissing));
}

#[test]
fn diagnostics_render_in_the_domain() {
    let source = "place weir \"The Weir\" ratified\n";
    let (_doc, diags) = parse_document(source);
    insta::assert_snapshot!("origin_required_rendered", render(source, &diags));
}

#[test]
fn unknown_keyword_renders_in_the_domain() {
    let source = "zorble frobnicate\n";
    let (_doc, diags) = parse_document(source);
    insta::assert_snapshot!("unknown_keyword_rendered", render(source, &diags));
}
