#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Pinned defect: `World::lower` used to route every `Decl` variant it did
//! not explicitly name through a `_ => {}` catch-all (world.rs:424,
//! pre-fix), silently dropping 18 of 28 variants — issue #47's shape ("an
//! accepted declaration lowers to nothing and nobody is told"), one layer
//! out. The match is now exhaustive over all 28 variants: a 29th variant
//! breaks the build until it is routed.
//!
//! The per-form question of whether Use/Draft/Export/Variant/Narration
//! (deprecated-only, `.dli`-text-reachable forms) should raise
//! `load.inert-declaration` (koan 4 / DL-0029) is DEFERRED to the
//! deprecation audit (PR-FINAL, with its own per-form DL ruling): a first
//! pass flagged them error-severity and a red team caught that the shipped
//! round-table/camelot reference corpus declares `use language`, `use
//! chronology`, and `narration vantage after-camlann` — regressing a clean
//! `dli.check` on shipped content from zero findings to three (SEV-A). PR-0c
//! stays behavior-neutral instead: the exhaustive match is the only change,
//! so this test pins that these five forms still raise NOTHING here either,
//! mirroring `dli-world`'s loader-side pin in `decl_exhaustiveness.rs`.
//!
//! `Attest` carries no atom either but for a different, load-bearing reason:
//! `author.add_attest` (contract 1.3.0) is a live, materializable wire
//! capability, and `load.inert-declaration` is error-severity, so flagging
//! it would break `author.materialize` (verified against
//! `framing/author-expert-framing.json` case 6).

use dli_ast::annot::{Annotation, CitationRef, ProvenanceMarker};
use dli_ast::decl::{
    Decl, Document, DraftDecl, ExportDecl, NarrationDecl, ScopeHeader, UseDecl, VariantDecl,
};
use dli_ast::ident::Ident;
use dli_ast::span::Span;

use dli_eval::world::World;

fn id(t: &str) -> Ident {
    Ident::synthetic(t)
}

fn marker(m: ProvenanceMarker, cites: &[&str]) -> Annotation {
    Annotation {
        marker: m,
        citations: cites.iter().map(|c| CitationRef { id: id(c) }).collect(),
        condition: None,
        bundle: None,
        tags: Vec::new(),
        span: Span::synthetic(),
    }
}

fn proposed() -> Annotation {
    marker(ProvenanceMarker::Proposed, &[])
}

fn doc(pack: &str, decls: Vec<Decl>) -> Document {
    Document {
        scope: Some(ScopeHeader {
            name: id(pack),
            span: Span::synthetic(),
        }),
        decls,
    }
}

/// The five deprecated-only, disposition-deferred forms (WS-6 / PR-FINAL):
/// accepted by the grammar, naming nothing this direct lowering — or the
/// loader — reads today, and deliberately left silent rather than flagged
/// because the shipped round-table/camelot corpus declares three of them.
fn deferred_forms() -> Vec<Decl> {
    vec![
        Decl::Use(UseDecl {
            target: id("elder-tongue-fringe"),
            span: Span::synthetic(),
        }),
        Decl::Draft(DraftDecl {
            name: id("first-pass"),
            label: None,
            body: Vec::new(),
            provenance: Some(proposed()),
        }),
        Decl::Export(ExportDecl {
            profile: id("teaching-card"),
            label: None,
            body: Vec::new(),
            provenance: Some(proposed()),
        }),
        Decl::Variant(VariantDecl {
            path: id("moon-cycle/winter"),
            body: Vec::new(),
            provenance: Some(proposed()),
        }),
        Decl::Narration(NarrationDecl {
            head: vec!["vantage".to_owned(), "the-narrators-era".to_owned()],
            provenance: None,
        }),
    ]
}

#[test]
fn the_deferred_forms_still_raise_no_findings() {
    let document = doc("weir", deferred_forms());
    let world = World::lower(&document).expect("the five deferred forms lower cleanly");

    assert!(
        world.findings.is_empty(),
        "PR-0c is behavior-neutral on findings; the deferred forms unexpectedly raised: {:?}",
        world.findings
    );
    // None of the five carries a ground fact — inert, not merely unlucky.
    assert!(
        world.facts.is_empty(),
        "deferred forms must not lower to facts: {:?}",
        world.facts
    );
}
