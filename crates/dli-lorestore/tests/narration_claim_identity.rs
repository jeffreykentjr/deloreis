#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Pinned defect: `carriage_kind` keyed every `Decl::Narration` on the EMPTY
//! STRING (`lower.rs:173`, pre-fix: `Some((K::Narration, String::new()))`),
//! so two distinct narrations in one pack minted the SAME claim id
//! (`decl:Narration:`) — a identity collision, not two distinct claims. Fixed
//! by keying the carriage name on the narration's own content
//! (`head.join(" ")`) instead of the constant empty string.

use dli_ast::decl::{Decl, Document, NarrationDecl, ScopeHeader};
use dli_ast::ident::Ident;
use dli_ast::span::Span;

use dli_lorestore::lower::lower_pack;
use dli_world::pack::Pack;

fn id(text: &str) -> Ident {
    Ident::synthetic(text)
}

fn narration(head: &[&str]) -> Decl {
    Decl::Narration(NarrationDecl {
        head: head.iter().map(|s| (*s).to_owned()).collect(),
        provenance: None,
    })
}

fn pack(name: &str, decls: Vec<Decl>) -> Pack {
    Pack {
        name: name.to_owned(),
        documents: vec![Document {
            scope: Some(ScopeHeader {
                name: id(name),
                span: Span::synthetic(),
            }),
            decls,
        }],
    }
}

/// Two narrations naming different vantages must carry distinct claim ids —
/// they are two distinct assertions, not one claim overwriting another.
#[test]
fn two_narrations_in_one_pack_carry_distinct_claim_ids() {
    let p = pack(
        "chronicle",
        vec![
            narration(&["vantage", "the-narrators-era"]),
            narration(&["vantage", "the-heralds-era"]),
        ],
    );
    let (objects, _names) = lower_pack(&p);

    let narration_ids: Vec<_> = objects
        .iter()
        .filter(|o| {
            o.expert_body
                .as_ref()
                .is_some_and(|b| b.kind == dli_lorestore::ExpertDeclKind::Narration)
        })
        .map(|o| o.claim_id.clone())
        .collect();

    assert_eq!(
        narration_ids.len(),
        2,
        "expected one carriage object per narration decl, got {narration_ids:?}"
    );
    assert_ne!(
        narration_ids[0], narration_ids[1],
        "two distinct narrations collided on one claim id: {narration_ids:?}"
    );
}
