//! The synthetic-prelude / carriage byte-identity parity for
//! `author.record_decision` (DL-0083).
//!
//! `reconstruct_pack` re-declares a commit's decisions one of two ways: a
//! synthesized `ratify` prelude from the commit's decisions list, or the carried
//! `ratify` objects a `defeat`/`record_decision` write leaves in the stream.
//! `record_decision` may write the FIRST carried `ratify` over a base whose
//! earlier decisions were only ever in the decisions list — the exact commit
//! shape that used to flip reconstruction from "prelude" to "carriage" mode.
//!
//! The adversarial invariant this pins: adding ONE carried `ratify` must not
//! change how the OTHER decisions reconstruct. They must stay declared,
//! byte-identically, whether by the list (prelude) or by carriage.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use dli_ast::decl::{Decl, RatifyDecl};
use dli_ast::ident::Ident;
use dli_ast::language::RatifyBody;
use dli_ast::span::Span;

use dli_lorestore::lower::{lower_decl, reconstruct_pack};

fn id(text: &str) -> Ident {
    Ident {
        text: text.to_owned(),
        span: Span::synthetic(),
    }
}

fn ratify_decls(pack: &dli_world::pack::Pack) -> Vec<RatifyDecl> {
    pack.documents
        .iter()
        .flat_map(|document| &document.decls)
        .filter_map(|decl| match decl {
            Decl::Ratify(ratify) => Some(ratify.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_carried_ratify_does_not_drop_the_list_only_decisions() {
    let interner = dli_core::Interner::new();

    // BEFORE: a carriage-free stream whose two decisions reconstruct via the
    // synthesized prelude — the shape a base with list-only decisions has.
    let known_before: BTreeSet<String> = ["d-1", "d-2"].iter().map(|s| (*s).to_owned()).collect();
    let before = reconstruct_pack(&[], &interner, &known_before, "demo")
        .expect("the prelude-only stream reconstructs");
    let before_ratifies = ratify_decls(&before);
    assert_eq!(
        before_ratifies.len(),
        2,
        "both decisions reconstruct via the prelude"
    );

    // record_decision writes ONE carried `ratify` (for a fresh d-3) and appends
    // d-3 to the commit's decisions list.
    let mut lower_interner = interner.clone();
    let mut bundle_counter = 0u32;
    let carriage = lower_decl(
        &Decl::Ratify(RatifyDecl {
            decision: id("d-3"),
            body: RatifyBody::default(),
        }),
        "demo",
        &mut lower_interner,
        &mut bundle_counter,
    );

    // AFTER: the mixed stream — one carried `ratify` (d-3) beside the list-only
    // d-1/d-2.
    let known_after: BTreeSet<String> = ["d-1", "d-2", "d-3"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let after = reconstruct_pack(&carriage, &interner, &known_after, "demo")
        .expect("the mixed stream reconstructs");
    let after_ratifies = ratify_decls(&after);

    // Every decision — the two list-only ones AND the carried one — is still
    // declared. The old all-or-nothing rule would have dropped d-1 and d-2 the
    // moment the d-3 carriage appeared.
    let declared: BTreeSet<&str> = after_ratifies
        .iter()
        .map(|ratify| ratify.decision.text.as_str())
        .collect();
    assert!(
        declared.contains("d-1") && declared.contains("d-2") && declared.contains("d-3"),
        "all three decisions remain declared: {declared:?}"
    );

    // And the list-only decisions reconstruct byte-identically — the carried
    // `ratify` changed nothing about how d-1 and d-2 come back.
    for wanted in &before_ratifies {
        assert!(
            after_ratifies.iter().any(|got| got == wanted),
            "decision {} reconstructs byte-identically before and after the carriage",
            wanted.decision.text
        );
    }
}
