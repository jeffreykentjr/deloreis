#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the AST crate.

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{Decl, Document, FactDecl, KindDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

fn sample_document() -> Document {
    Document {
        scope: Some(ScopeHeader {
            name: Ident::synthetic("camelot"),
            span: Span::synthetic(),
        }),
        decls: vec![
            Decl::Kind(KindDecl {
                name: Ident::synthetic("place"),
                provenance: None,
            }),
            Decl::Fact(FactDecl {
                atom: AtomExpr {
                    pred: Ident::synthetic("adjacent"),
                    args: vec![
                        Term::Sym(Ident::synthetic("the-lake")),
                        Term::Sym(Ident::synthetic("camelot")),
                    ],
                },
                triple: false,
                provenance: Some(Annotation {
                    marker: ProvenanceMarker::Decided,
                    citations: vec![CitationRef {
                        id: Ident::synthetic("D-0042"),
                    }],
                    condition: None,
                    bundle: None,
                    tags: vec![],
                    span: Span::synthetic(),
                }),
            }),
        ],
    }
}

#[test]
fn document_round_trips_through_canonical_json() {
    let doc = sample_document();
    let bytes = dli_ast::canonical::to_canonical_json(&doc);
    let back: Document = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc, back);
}
