//! Citation-guard soundness for `term` / `proof` / `ratify`-block — the `B1`
//! invariant "authority is declared, not minted" extended over the three
//! declarations the loader keeps outside `atoms()`/`sorts()`. A forged decision id
//! cited by any of them must now raise `citation.unknown-decision`; every shipped
//! citation (all declared) must stay clean.

use dli_syntax::parse_document;

use dli_world::check::{self, Severity};
use dli_world::pack::Pack;
use dli_world::{CompiledWorld, compile};

fn world_of(src: &str) -> CompiledWorld {
    let (doc, diags) = parse_document(src);
    assert!(diags.is_empty(), "the world parses: {diags:?}");
    compile(&Pack {
        name: "guard".to_owned(),
        documents: vec![doc],
    })
    .expect("the world compiles")
}

fn unknown_decision_findings(world: &CompiledWorld) -> Vec<String> {
    check::run(world)
        .into_iter()
        .filter(|f| f.code == "citation.unknown-decision")
        .map(|f| f.message)
        .collect()
}

#[test]
fn unknown_proof_and_ratify_citations_keep_their_source_lines() {
    let source = "scope demo\nproof oath speaker p in practice {\n  premise p1 holds(p) cited d-9999\n  close holds(p) expect holds\n}\nhypothesis h-x\nratify d-1 { h-x := true decided d-9998 }\n";
    let world = world_of(source);
    let findings = check::run(&world);
    for (decision, expected_line) in [("d-9999", 3), ("d-9998", 7)] {
        let finding = findings
            .iter()
            .find(|f| f.code == "citation.unknown-decision" && f.message.contains(decision))
            .expect("the test fixture must provide this value");
        let span = finding
            .span
            .expect("the test fixture must provide this value");
        let line = source[..span.start as usize]
            .bytes()
            .filter(|&b| b == b'\n')
            .count()
            + 1;
        assert_eq!(line, expected_line, "{finding:?}");
    }
}

#[test]
fn inherited_unknown_citations_point_to_the_enclosing_provenance() {
    let source =
        "scope demo\nperson wren ratified d-9999 {\n  wren.home = shore\n  wren.role = guide\n}\n";
    let world = world_of(source);
    let findings: Vec<_> = check::run(&world)
        .into_iter()
        .filter(|f| f.code == "citation.unknown-decision")
        .collect();
    assert!(!findings.is_empty());
    for finding in findings {
        let span = finding
            .span
            .expect("the test fixture must provide this value");
        let line = source[..span.start as usize]
            .bytes()
            .filter(|&b| b == b'\n')
            .count()
            + 1;
        assert_eq!(line, 2, "{finding:?}");
    }
}

#[test]
fn unknown_kind_citation_points_to_its_provenance() {
    let source = "scope demo\nkind spirit ratified d-9999\n";
    let world = world_of(source);
    let finding = check::run(&world)
        .into_iter()
        .find(|f| f.code == "citation.unknown-decision")
        .expect("the test fixture must provide this value");
    let span = finding
        .span
        .expect("the test fixture must provide this value");
    assert_eq!(
        source[..span.start as usize]
            .bytes()
            .filter(|&b| b == b'\n')
            .count()
            + 1,
        2
    );
}

#[test]
fn repeated_ratify_citations_point_to_each_body_line() {
    let source = "scope demo\nhypothesis h-x\nhypothesis h-y\nratify d-1 {\n  h-x := true decided d-9999\n  h-y := true decided d-9999\n}\n";
    let world = world_of(source);
    let lines: Vec<_> = check::run(&world)
        .into_iter()
        .filter(|f| f.code == "citation.unknown-decision")
        .map(|f| {
            let span = f.span.expect("the test fixture must provide this value");
            source[..span.start as usize]
                .bytes()
                .filter(|&b| b == b'\n')
                .count()
                + 1
        })
        .collect();
    assert_eq!(lines, vec![5, 6]);
}

#[test]
fn a_synthetic_citation_does_not_claim_a_source_location() {
    let (mut doc, diagnostics) =
        parse_document("scope demo\nfact wren home shore ratified d-9999\n");
    assert!(diagnostics.is_empty());
    for decl in &mut doc.decls {
        if let dli_ast::decl::Decl::Fact(fact) = decl {
            fact.provenance
                .as_mut()
                .expect("the test fixture must provide this value")
                .span = dli_ast::span::Span::synthetic();
        }
    }
    let world = compile(&Pack {
        name: "guard".to_owned(),
        documents: vec![doc],
    })
    .expect("the test fixture must provide this value");
    let finding = check::run(&world)
        .into_iter()
        .find(|f| f.code == "citation.unknown-decision")
        .expect("the test fixture must provide this value");
    assert!(finding.span.is_none());
}

// --- term ------------------------------------------------------------------

#[test]
fn a_term_citing_a_declared_decision_is_clean() {
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         term the-drawing \"the freeing of the sword from the stone\"  decided d-1\n",
    );
    assert!(
        unknown_decision_findings(&world).is_empty(),
        "a declared citation is clean: {:?}",
        unknown_decision_findings(&world),
    );
}

#[test]
fn a_term_citing_an_undeclared_decision_is_not_gated() {
    // A `term`'s `decided <d-id>` is language-layer *provenance*, not assertability
    // authority: it may name a decision recorded outside this corpus (the vendored
    // storybook packs cite external canon ids like `D-0181` on terms this way). So an
    // undeclared term citation is NOT a finding — unlike a proof/ratify citation.
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         term the-drawing \"the freeing of the sword from the stone\"  decided d-9999\n",
    );
    let findings = unknown_decision_findings(&world);
    assert!(
        findings.is_empty(),
        "an undeclared term citation is provenance, not gated: {findings:?}",
    );
}

// --- proof -----------------------------------------------------------------

#[test]
fn a_proof_citing_a_declared_decision_is_clean() {
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         standing(lancelot)  decided d-1\n\
         proof lancelots-oath  speaker lancelot  in oath-binding {\n\
           premise p1  standing(lancelot)  cited d-1  cue \"comes to court\"\n\
           close pledged(lancelot, fealty) expect holds\n\
         }\n",
    );
    assert!(
        unknown_decision_findings(&world).is_empty(),
        "a declared premise citation is clean: {:?}",
        unknown_decision_findings(&world),
    );
}

#[test]
fn a_proof_citing_an_undeclared_decision_is_caught() {
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         standing(lancelot)  decided d-1\n\
         proof lancelots-oath  speaker lancelot  in oath-binding {\n\
           premise p1  standing(lancelot)  cited d-9999  cue \"comes to court\"\n\
           close pledged(lancelot, fealty) expect holds\n\
         }\n",
    );
    let findings = unknown_decision_findings(&world);
    assert!(
        findings.iter().any(|m| m.contains("d-9999")),
        "a forged proof-premise citation is caught: {findings:?}",
    );
    let finding = check::run(&world)
        .into_iter()
        .find(|finding| finding.code == "citation.unknown-decision")
        .expect("citation finding");
    assert_eq!(finding.severity, Severity::Error);
}

#[test]
fn a_cited_keyword_inside_a_nested_atom_is_not_a_citation() {
    // `cited` appearing inside a premise atom's parens is part of the atom, not a
    // provenance keyword — the paren-depth guard must not read it as a decision id.
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         proof edge  speaker p  in prac {\n\
           premise p1  holds(cited, nothing)  cited d-1\n\
           close holds(cited, nothing) expect holds\n\
         }\n",
    );
    assert!(
        unknown_decision_findings(&world).is_empty(),
        "only the top-level `cited d-1` is a citation, and d-1 is declared: {:?}",
        unknown_decision_findings(&world),
    );
}

#[test]
fn a_forged_citation_hidden_behind_a_cue_paren_is_still_caught() {
    // R-A3 F1a: an unbalanced `(` inside a cue STRING must not hide a real forged
    // `cited` after it — the harvest tracks string state, not only paren depth.
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         proof forged  speaker p  in prac {\n\
           premise p1  holds(p)  cue \"a (parenthetical remark\"  cited d-9999\n\
           close holds(p) expect holds\n\
         }\n",
    );
    let findings = unknown_decision_findings(&world);
    assert!(
        findings.iter().any(|m| m.contains("d-9999")),
        "a forged cited behind a cue paren is caught: {findings:?}",
    );
}

#[test]
fn a_cite_keyword_in_cue_prose_is_not_a_citation() {
    // R-A3 F1b: `decided`/`ratified` in ordinary cue PROSE must not mint a spurious
    // citation.
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         proof benign  speaker p  in prac {\n\
           premise p1  holds(p)  cited d-1  cue \"the king decided to grant it\"\n\
           close holds(p) expect holds\n\
         }\n",
    );
    assert!(
        unknown_decision_findings(&world).is_empty(),
        "a cite keyword in cue prose is not a citation: {:?}",
        unknown_decision_findings(&world),
    );
}

// --- ratify block ----------------------------------------------------------

#[test]
fn a_ratify_block_settling_a_hypothesis_is_clean() {
    // The shipped shape: a block mints its decision and settles a hypothesis; it
    // cites no other decision, so it is clean.
    let world = world_of(
        "scope demo\n\
         hypothesis h-x \"a supposition\"\n\
         ratify d-1 { h-x := true }\n",
    );
    assert!(
        unknown_decision_findings(&world).is_empty(),
        "a hypothesis-settling block is clean: {:?}",
        unknown_decision_findings(&world),
    );
}

#[test]
fn a_ratify_block_citing_an_undeclared_decision_is_caught() {
    // A block whose body cites a decision the corpus never declares forges
    // authority — caught the same way as any other citation.
    let world = world_of(
        "scope demo\n\
         hypothesis h-x \"a supposition\"\n\
         ratify d-1 { h-x := true  decided d-9999 }\n",
    );
    let findings = unknown_decision_findings(&world);
    assert!(
        findings.iter().any(|m| m.contains("d-9999")),
        "a forged ratify-body citation is caught: {findings:?}",
    );
}
