#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Property tests: the formatter round-trips with the parser and is idempotent.
//!
//! Two properties, both over generated documents (spec `01-syntax.md` §1):
//!
//! * `parse ∘ format == identity` on the AST, modulo source spans — a formatted
//!   document parses back to the same tree.
//! * `format ∘ parse ∘ format == format` — the canonical form is a fixed point.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    AttrDecl, BeforeDecl, CycleDecl, Decl, Document, EntityDecl, EntityKind, FactDecl,
    HypothesisDecl, OptionClause, QuestionDecl, ScopeHeader, StateDecl, TimeClause,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};
use dli_syntax::{format_document, parse_document};
use proptest::prelude::*;

/// Serialize a document and zero every source span, so two trees that differ
/// only in where they point compare equal.
fn span_blind(doc: &Document) -> serde_json::Value {
    let mut value = serde_json::to_value(doc).unwrap();
    zero_spans(&mut value);
    value
}

fn zero_spans(value: &mut serde_json::Value) {
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
                    zero_spans(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                zero_spans(item);
            }
        }
        _ => {}
    }
}

fn ident() -> impl Strategy<Value = Ident> {
    (0u32..10).prop_map(|n| Ident::synthetic(format!("x{n}")))
}

fn label() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "The Weir".to_owned(),
        "Mara of the Weirs".to_owned(),
        "Old Fen".to_owned(),
        "salt".to_owned(),
    ])
}

fn term() -> impl Strategy<Value = Term> {
    prop_oneof![
        ident().prop_map(Term::Sym),
        (0i64..1000).prop_map(Term::Int),
    ]
}

fn citation() -> impl Strategy<Value = CitationRef> {
    (1u32..40).prop_map(|n| CitationRef {
        id: Ident::synthetic(format!("d-{n:04}")),
    })
}

fn provenance() -> impl Strategy<Value = Option<Annotation>> {
    let ledger = prop::sample::select(vec![
        ProvenanceMarker::Decided,
        ProvenanceMarker::Ratified,
        ProvenanceMarker::Provisional,
    ]);
    prop_oneof![
        Just(None),
        Just(Some(Annotation {
            marker: ProvenanceMarker::Proposed,
            citations: vec![],
            condition: None,
            tags: vec![],
            span: Span::synthetic(),
        })),
        (ledger, prop::collection::vec(citation(), 1..3)).prop_map(|(marker, citations)| {
            Some(Annotation {
                marker,
                citations,
                condition: None,
                tags: vec![],
                span: Span::synthetic(),
            })
        }),
    ]
}

fn entity_kind() -> impl Strategy<Value = EntityKind> {
    prop::sample::select(vec![
        EntityKind::Era,
        EntityKind::Place,
        EntityKind::Person,
        EntityKind::Thing,
        EntityKind::Event,
    ])
}

fn time_clause() -> impl Strategy<Value = TimeClause> {
    prop_oneof![
        ident().prop_map(TimeClause::After),
        ident().prop_map(TimeClause::During),
        ident().prop_map(TimeClause::Since),
        ident().prop_map(TimeClause::Until),
    ]
}

fn attr() -> impl Strategy<Value = AttrDecl> {
    (ident(), ident(), term(), provenance()).prop_map(|(subject, attr, value, provenance)| {
        AttrDecl {
            subject,
            attr,
            value,
            provenance,
        }
    })
}

fn atom() -> impl Strategy<Value = AtomExpr> {
    (
        ident(),
        prop::collection::vec(ident().prop_map(Term::Sym), 1..3),
    )
        .prop_map(|(pred, args)| AtomExpr { pred, args })
}

fn option_clause() -> impl Strategy<Value = OptionClause> {
    (ident(), prop::option::of(atom()), prop::option::of(ident())).prop_map(
        |(name, implies, assumes)| OptionClause {
            name,
            implies,
            assumes,
        },
    )
}

fn entity() -> impl Strategy<Value = Decl> {
    (
        entity_kind(),
        ident(),
        prop::option::of(label()),
        prop::option::of(time_clause()),
        prop::collection::vec(attr(), 0..3),
        provenance(),
    )
        .prop_map(|(kind, name, label, time, slots, provenance)| {
            Decl::Entity(EntityDecl {
                kind,
                name,
                label,
                time,
                slots,
                provenance,
            })
        })
}

fn era_chain() -> impl Strategy<Value = Decl> {
    (ident(), prop::collection::vec(ident(), 1..4), provenance()).prop_map(
        |(name, chain, provenance)| {
            Decl::Entity(EntityDecl {
                kind: EntityKind::Era,
                name,
                label: None,
                time: Some(TimeClause::Order(chain)),
                slots: vec![],
                provenance,
            })
        },
    )
}

fn question() -> impl Strategy<Value = Decl> {
    (
        ident(),
        prop::option::of(label()),
        prop::collection::vec(option_clause(), 0..3),
        provenance(),
    )
        .prop_map(|(name, prompt, options, provenance)| {
            Decl::Question(QuestionDecl {
                name,
                prompt,
                options,
                provenance,
            })
        })
}

fn fact_relation() -> impl Strategy<Value = Decl> {
    (atom(), provenance()).prop_map(|(atom, provenance)| {
        Decl::Fact(FactDecl {
            atom,
            triple: false,
            provenance,
        })
    })
}

fn fact_triple() -> impl Strategy<Value = Decl> {
    (ident(), ident(), ident(), provenance()).prop_map(|(subject, pred, object, provenance)| {
        Decl::Fact(FactDecl {
            atom: AtomExpr {
                pred,
                args: vec![Term::Sym(subject), Term::Sym(object)],
            },
            triple: true,
            provenance,
        })
    })
}

fn before() -> impl Strategy<Value = Decl> {
    (ident(), ident(), provenance()).prop_map(|(earlier, later, provenance)| {
        Decl::Before(BeforeDecl {
            earlier,
            later,
            provenance,
        })
    })
}

fn hypothesis() -> impl Strategy<Value = Decl> {
    (ident(), prop::option::of(label()))
        .prop_map(|(name, statement)| Decl::Hypothesis(HypothesisDecl { name, statement }))
}

fn fluent() -> impl Strategy<Value = Decl> {
    (
        ident(),
        prop::option::of(term()),
        prop::option::of(time_clause()),
        provenance(),
    )
        .prop_map(|(name, value, time, provenance)| {
            Decl::State(StateDecl {
                name,
                value,
                time,
                provenance,
            })
        })
}

fn wheel() -> impl Strategy<Value = Decl> {
    (
        ident(),
        prop::option::of(0i64..40),
        prop::collection::vec(ident(), 0..4),
        provenance(),
    )
        .prop_map(|(name, modulus, spokes, provenance)| {
            Decl::Cycle(CycleDecl {
                name,
                modulus,
                spokes,
                provenance,
            })
        })
}

fn decl() -> impl Strategy<Value = Decl> {
    prop_oneof![
        entity(),
        era_chain(),
        attr().prop_map(Decl::Attr),
        question(),
        fact_relation(),
        fact_triple(),
        before(),
        hypothesis(),
        fluent(),
        wheel(),
    ]
}

fn document() -> impl Strategy<Value = Document> {
    (
        prop::option::of((0u32..5).prop_map(|n| ScopeHeader {
            name: Ident::synthetic(format!("p{n}")),
            span: Span::synthetic(),
        })),
        prop::collection::vec(decl(), 0..6),
    )
        .prop_map(|(scope, decls)| Document { scope, decls })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn parse_of_format_is_identity(doc in document()) {
        let text = format_document(&doc);
        let (parsed, diags) = parse_document(&text);
        let errs: Vec<_> = diags.iter().filter(|d| d.class.is_error()).collect();
        prop_assert!(errs.is_empty(), "errors on generated doc:\n{text}\n{errs:?}");
        prop_assert_eq!(
            span_blind(&parsed),
            span_blind(&doc),
            "round-trip changed the tree:\n{}",
            text
        );
    }

    #[test]
    fn format_is_idempotent(doc in document()) {
        let text = format_document(&doc);
        let (parsed, _) = parse_document(&text);
        prop_assert_eq!(format_document(&parsed), text);
    }
}

/// Label content built from lexical hazards — `{ } #` plus letters and spaces
/// — that must stay text and never open a block, start a comment, or otherwise
/// change how many records parse (H2 regression).
fn hazard_label() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec!['{', '}', '#', 'a', 'b', ' ']),
        0..6,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn hazard_chars_in_labels_never_change_record_count(labels in prop::collection::vec(hazard_label(), 1..5)) {
        use std::fmt::Write;
        let mut source = String::new();
        for (i, label) in labels.iter().enumerate() {
            let _ = writeln!(source, "place x{i} \"{label}\" proposed");
        }
        let (doc, _diags) = parse_document(&source);
        prop_assert_eq!(doc.decls.len(), labels.len(), "source:\n{}", source);
    }
}
