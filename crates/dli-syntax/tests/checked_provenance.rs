//! A checked text rendering must preserve every provenance annotation.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use dli_ast::decl::Decl;
use dli_syntax::{format::format_document_checked, parse_document};

#[test]
fn unrepresentable_origin_is_reported_instead_of_silently_changed() {
    let (mut document, _) = parse_document("scope story\nseed(wren) proposed origin\n");
    let Decl::Fact(fact) = &mut document.decls[0] else {
        panic!("fact")
    };
    fact.provenance.as_mut().unwrap().tags = vec!["group#draft".into()];
    let error = format_document_checked(&document).unwrap_err();
    assert!(error.contains("provenance"), "{error}");
}

#[test]
fn ordinary_and_quoted_origin_tokens_remain_renderable() {
    for source in [
        "scope story\nseed(wren) proposed group\n",
        "scope story\nseed(wren) proposed \"group#draft\"\n",
        "scope story\ndialect a proposed x {\n name-part tide \"water\" role place proposed origin\n}\n",
    ] {
        let (document, findings) = parse_document(source);
        assert!(findings.is_empty(), "{findings:?}");
        format_document_checked(&document).unwrap();
    }
}
