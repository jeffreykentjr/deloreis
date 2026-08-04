//! Source position must never reach the identity projection (DL-0055).
//!
//! A declaration asserts the same thing wherever it sits in the file. If its
//! offset reached the content hash, the same declaration authored at two
//! offsets would be two objects — dedup would miss, and the "same assertion
//! reached on two forks is the same object" invariant that makes merge a union
//! (see `identity.rs`) would not hold.
//!
//! This is the standing guard for that property. Before DM-2 the store hashed
//! the located declaration JSON, so `spans_never_change_identity` failed with
//! two different hashes; `located_body_survives_the_round_trip` is the other
//! half — position is kept, just kept out of the hash.

use dli_ast::decl::{Decl, UseDecl};
use dli_ast::ident::Ident;
use dli_ast::span::{FileId, Span};
use dli_core::atom::Interner;
use dli_lorestore::lower::lower_decl;

fn span_at(start: u32, end: u32) -> Span {
    Span {
        file: FileId(0),
        start,
        end,
    }
}

fn use_decl_at(start: u32, end: u32) -> Decl {
    Decl::Use(UseDecl {
        target: Ident {
            text: "harbour".to_owned(),
            span: span_at(start, end),
        },
        span: span_at(start, end),
    })
}

fn carriage_for(decl: &Decl) -> dli_lorestore::FactObject {
    let mut interner = Interner::default();
    let mut counter = 0;
    let objects = lower_decl(decl, "span-identity", &mut interner, &mut counter);
    objects
        .into_iter()
        .next()
        .expect("a carried declaration lowers to a carriage object")
}

#[test]
fn spans_never_change_identity() {
    let early = carriage_for(&use_decl_at(0, 13));
    let late = carriage_for(&use_decl_at(4096, 4109));

    assert_eq!(
        early.content_hash().expect("canonical encoding"),
        late.content_hash().expect("canonical encoding"),
        "the same declaration at two source offsets must be one object"
    );
}

#[test]
fn located_body_survives_the_round_trip() {
    let carriage = carriage_for(&use_decl_at(4096, 4109));

    let hashed = &carriage
        .expert_body
        .as_ref()
        .expect("a carriage object carries an expert body")
        .body[0];
    assert!(
        !hashed.contains("4096"),
        "the hashed body must be span-normalized, got {hashed}"
    );

    let located = carriage
        .provenance
        .as_ref()
        .and_then(|p| p.located_body.as_deref())
        .expect("the located body is recorded beside the fact");
    assert!(
        located.contains("4096"),
        "the located body must keep the authored position, got {located}"
    );

    // And it is still a declaration, not just a string we kept around.
    let decl: Decl = serde_json::from_str(located).expect("the located body decodes as a Decl");
    let Decl::Use(use_decl) = decl else {
        panic!("expected the Use declaration back");
    };
    assert_eq!(use_decl.span, span_at(4096, 4109));
}

#[test]
fn a_position_free_declaration_carries_no_located_copy() {
    let carriage = carriage_for(&use_decl_at(0, 0));

    assert!(
        carriage
            .provenance
            .as_ref()
            .and_then(|p| p.located_body.as_deref())
            .is_none(),
        "a declaration with synthetic spans needs no located duplicate"
    );
}
