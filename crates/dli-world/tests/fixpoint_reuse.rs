#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Fixpoint reuse agrees with a fresh compile and rejects truncation.

use dli_ast::decl::Decl;
use dli_ast::ident::Ident;
use dli_world::loader::example_docs;
use dli_world::{Pack, compile, compile_reusing_fixpoint};

#[test]
fn fixpoint_reuse_matches_full_compile_and_refuses_truncation() {
    let base_pack = Pack {
        name: "camelot".to_owned(),
        documents: example_docs(),
    };
    let base = compile(&base_pack).unwrap();
    let mut extended_pack = base_pack.clone();
    let mut fact = extended_pack.documents[0]
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Fact(fact) => Some(fact.clone()),
            _ => None,
        })
        .unwrap();
    fact.atom.pred = Ident::synthetic("new-relation");
    extended_pack.documents[0].decls.push(Decl::Fact(fact));

    let expected = compile(&extended_pack).unwrap();
    let (reused, report) = compile_reusing_fixpoint(&base, &extended_pack).unwrap();
    assert_eq!(reused, expected);
    assert!(
        report.fixpoint.reused,
        "the extension should reuse the fixpoint"
    );

    let (narrowed, report) = compile_reusing_fixpoint(&expected, &base_pack).unwrap();
    assert_eq!(narrowed, base);
    assert!(
        !report.fixpoint.reused,
        "a truncation cannot reuse the fixpoint"
    );
}
