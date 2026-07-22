//! Citation-guard soundness for `term` / `proof` / `ratify`-block — the `B1`
//! invariant "authority is declared, not minted" extended over the three
//! declarations the loader keeps outside `atoms()`/`sorts()`. A forged decision id
//! cited by any of them must now raise `citation.unknown-decision`; every shipped
//! citation (all declared) must stay clean.

use dli_syntax::parse_document;

use dli_world::check;
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
fn a_term_citing_an_undeclared_decision_is_caught() {
    let world = world_of(
        "scope demo\n\
         ratify d-1\n\
         term the-drawing \"the freeing of the sword from the stone\"  decided d-9999\n",
    );
    let findings = unknown_decision_findings(&world);
    assert!(
        findings.iter().any(|m| m.contains("d-9999")),
        "a forged term citation is caught: {findings:?}",
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
