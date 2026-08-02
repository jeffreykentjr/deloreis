//! `LS-1` total-lowering oracle.
//!
//! This integration test closes the loop the roadmap (doc/PLAN.md § LS-1,
//! `docs/design-notes/lorestore-roadmap.md` §LS-1) demands:
//!
//! > every AST construct lowers to fact objects, and lowering the vendored
//! > corpus then compiling reproduces identical fixpoint, findings, and digests
//!
//! Flow — shadow-read harness idiom (same as the MIG/PROOF lanes in
//! `crates/dli-world/tests/check.rs`):
//!
//! 1.  Bridge the vendored `facts.yaml` into `Decl`s (same `corpus_pack`
//!     logic the MIG lane uses).
//! 2.  Compile via `dli_world::compile` → **reference A**.
//! 3.  Lower A's `LoadedAtom`s to `FactObject`s and compute their
//!     content-address digests (verifying determinism by hashing twice).
//! 4.  Reconstruct source `Decl`s from those `FactObject`s — deriving each
//!     `Annotation` from the support polynomial via the committed interner.
//! 5.  Recompile the reconstructed documents → **reference B**.
//! 6.  Compare A and B: rendered fixpoint (`World::derived`), findings, and
//!     the set of `FactObject` digests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
use common::{corpus_pack, load_corpus, render_derived, render_facts, render_findings};

use dli_ast::decl::Decl;

use dli_core::posbool::PosBool;

use dli_lorestore::identity::ClaimId;
use dli_lorestore::lower::{declared_decisions, derive_annotation, lower_atoms, reconstruct_pack};
use dli_lorestore::object::FactObject;

use dli_world::compile;

// ── comparison helpers are shared via `common/mod.rs` ───────────────────

// ── the oracle ────────────────────────────────────────────────────────

#[test]
#[allow(clippy::too_many_lines, clippy::print_stdout)]
fn total_lowering_reproduces_fixpoint_findings_and_digests() {
    // 1 — bridge the vendored corpus.
    let pack = corpus_pack(&load_corpus());

    // 2 — compile reference A.
    let compiled_a = compile(&pack).expect("reference A compiles");
    let model_a = compiled_a.model();

    // 3 — lower A's LoadedAtoms to FactObjects.
    //    Using LoadedAtom (not just World::facts) captures entity declarations
    //    too, which is what the loader interns as proposal-bundle variables.
    let fact_objects = lower_atoms(model_a.atoms());

    //    Totality: every loaded atom lowers to exactly one FactObject.
    assert_eq!(
        fact_objects.len(),
        model_a.atoms().len(),
        "every LoadedAtom must lower to exactly one FactObject (DL-0041)"
    );

    //    Determinism: hashing the objects twice must yield identical digests.
    let digests_a: Vec<String> = fact_objects
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    let digests_a_again: Vec<String> = fact_objects
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    assert_eq!(
        digests_a, digests_a_again,
        "FactObject digests must be deterministic over canonical bytes"
    );

    //    No two FactObjects in the same pack may share a digest.
    let mut unique = std::collections::HashSet::new();
    for d in &digests_a {
        assert!(
            unique.insert(d.clone()),
            "duplicate FactObject digest — content-address collision: {d}"
        );
    }

    // 4 — collect known decisions (minted via Decl::Ratify) so the
    //    reconstructed pack can re-emit them.
    let known_decisions = declared_decisions(&pack);

    //    Derive annotations from FactObject support polynomials and verify
    //    that every citation resolves to a known decision.
    let interner_a = model_a.interner();
    for fo in &fact_objects {
        if let Some(ann) = derive_annotation(&fo.support, interner_a, &known_decisions) {
            for cite in &ann.citations {
                assert!(
                    known_decisions.contains(&cite.id.text),
                    "reconstructed citation {} was not a declared decision",
                    cite.id.text
                );
            }
        }
    }

    // 5 — reconstruct source Decls from the FactObjects.
    let reconstructed = reconstruct_pack(&fact_objects, interner_a, &known_decisions, &pack.name);

    //    The reconstructed documents carry ratify decls first, then entity decls,
    //    then fact decls — matching the original pack's ordering.
    let ratify_count = reconstructed
        .documents
        .iter()
        .flat_map(|doc| &doc.decls)
        .filter(|d| matches!(d, Decl::Ratify(_)))
        .count();
    assert_eq!(ratify_count, known_decisions.len());

    // 6 — compile the reconstructed pack (reference B).
    let compiled_b = compile(&reconstructed).expect("reference B compiles");
    let model_b = compiled_b.model();

    // ---- fidelity checks ---------------------------------------------------

    // (a) extensional facts match (by surface form + support name-strings).
    let facts_a = render_facts(&model_a.world);
    let facts_b = render_facts(&model_b.world);
    assert_eq!(
        facts_a, facts_b,
        "extensional facts must be identical after round-trip"
    );

    // (b) fixpoint (derived relation) is identical.
    let derived_a = render_derived(&model_a.world);
    let derived_b = render_derived(&model_b.world);
    assert_eq!(
        derived_a, derived_b,
        "fixpoint must be identical after round-trip"
    );
    assert_eq!(
        model_a.world.derived.len(),
        model_b.world.derived.len(),
        "derived relation row count must match"
    );

    // (c) load findings are identical (the clean corpus must be clean on both
    //     sides — charter koan 4).
    assert_eq!(
        render_findings(model_a.findings()),
        render_findings(model_b.findings()),
        "load findings must be identical after round-trip"
    );

    // (d) valuation (truth assignment over resolved names) matches.
    let names_a = model_a.interner().names().to_vec();
    let names_b = model_b.interner().names().to_vec();
    assert_eq!(names_a, names_b, "interned variable sets must match");
    for (&atom_idx, val_a) in &model_a.world.valuation.0 {
        let name = model_a.interner().resolve(atom_idx).expect("atom has name");
        let b_idx = dli_core::atom::Atom(
            u32::try_from(
                names_b
                    .iter()
                    .position(|n| n == name)
                    .expect("atom exists in B interner"),
            )
            .expect("fits in u32"),
        );
        let val_b = model_b.world.valuation.get(b_idx).unwrap_or(false);
        assert_eq!(
            *val_a, val_b,
            "valuation disagree on variable '{name}': A={val_a}, B={val_b}"
        );
    }

    // (e) FactObject digests from A and B must match — the interner is
    //     rebuilt identically (ratify-first ordering) so atom indices line up
    //     and canonical bytes are byte-for-byte stable.
    let fact_objects_b = lower_atoms(model_b.atoms());
    let digests_b: Vec<String> = fact_objects_b
        .iter()
        .map(|fo| fo.content_hash().expect("canonical encoding").to_string())
        .collect();
    assert_eq!(
        digests_a, digests_b,
        "FactObject digests must match after round-trip (interner-stable)"
    );

    println!(
        "oracle passed: {} atoms, {} facts, {} derived, {} findings, {} digests identical",
        fact_objects.len(),
        facts_a.len(),
        derived_a.len(),
        render_findings(model_a.findings()).len(),
        digests_a.len(),
    );
}

#[test]
fn fact_object_is_content_addressed_and_upcast_safe() {
    // A FactObject's ContentHash must depend only on its canonical bytes,
    // never on ClaimId (the three-way split — DL-0041).
    let atom = dli_eval::world::GroundAtom {
        pred: "place".to_owned(),
        args: vec!["camelot".to_owned()],
    };
    let support = PosBool::pos(dli_core::atom::Atom(0));

    let fo = FactObject::new(
        ClaimId::from_label("test/a"),
        atom.clone(),
        support.clone(),
        PosBool::FALSE,
        Vec::new(),
        dli_core::grade::Grade::Asserted,
    );
    let h = fo.content_hash().expect("canonical encoding").to_string();

    // Same content, different claim_id → same hash (claim_id is not in the
    // canonical content bytes).
    let mut fo_renamed = fo.clone();
    fo_renamed.claim_id = ClaimId::from_label("test/a-renamed");
    assert_eq!(
        fo_renamed
            .content_hash()
            .expect("canonical encoding")
            .to_string(),
        h,
        "content hash must be stable regardless of ClaimId label"
    );

    // Different atoms produce different hashes.
    let mut fo_diff = fo.clone();
    fo_diff.atom.args.push("guild".to_owned());
    assert_ne!(
        fo_diff
            .content_hash()
            .expect("canonical encoding")
            .to_string(),
        h,
        "different atoms must produce different hashes"
    );
}
