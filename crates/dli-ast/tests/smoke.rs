#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the AST crate.

use dli_ast::annot::{Annotation, CitationRef};
use dli_ast::decl::{Decl, Document, FactDecl, PackHeader, SortDecl};
use dli_ast::ident::Ident;
use dli_ast::span::Span;
use dli_ast::term::{AtomExpr, Term};

fn sample_document() -> Document {
    Document {
        pack: Some(PackHeader {
            name: Ident::synthetic("saltmarket"),
        }),
        decls: vec![
            Decl::Sort(SortDecl {
                name: Ident::synthetic("place"),
                provenance: None,
            }),
            Decl::Fact(FactDecl {
                atom: AtomExpr {
                    pred: Ident::synthetic("adjacent"),
                    args: vec![
                        Term::Sym(Ident::synthetic("the-shallows")),
                        Term::Sym(Ident::synthetic("saltmarket")),
                    ],
                },
                provenance: Some(Annotation {
                    citations: vec![CitationRef {
                        id: Ident::synthetic("D-0042"),
                    }],
                    condition: None,
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

#[test]
#[ignore = "enabled at SURF-1"]
fn golden_document_matches_committed_snapshot() {
    // The golden AST snapshot lands with the SURF-1 parser milestone, once a
    // concrete-syntax document lowers deterministically into these types.
    todo!("SURF-1: assert parse(saltmarket.dli) == committed golden AST")
}
