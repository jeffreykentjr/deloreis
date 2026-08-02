//! `LS-3` differential parity oracle.
//!
//! Verifies the read path: a `Pack` compiled directly via `dli_world::compile`
//! (reference A) must produce an identical `CompiledWorld` when the same pack is
//! first imported into a `Lorestore` and then recompiled via `Lorestore::compile`
//! (reference B).
//!
//! This closes the `.dli → lorestore → recompile` round-trip at the concept
//! level: the content-addressed store must be a lossless conduit for the
//! compiler front-end.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
use common::{corpus_pack, load_corpus, render_derived, render_facts, render_findings};

use tempfile::TempDir;

use dli_lorestore::Lorestore;
use dli_world::compile;

#[test]
#[allow(clippy::too_many_lines, clippy::print_stdout)]
fn compile_from_lorestore_matches_direct_compile() {
    // 1 — build the reference Pack from the vendored corpus.
    let pack = corpus_pack(&load_corpus());

    // 2 — compile reference A (direct path).
    let compiled_a = compile(&pack).expect("reference A compiles");
    let model_a = compiled_a.model();
    let world_a = &model_a.world;

    // 3 — import the same pack into a lorestore, then compile from storage.
    let tmp = TempDir::new().expect("tempdir");
    let store = Lorestore::open(tmp.path().to_path_buf()).expect("open lorestore");
    store
        .import_pack(&pack)
        .expect("import pack into lorestore");

    // 4 — compile reference B (read path through lorestore).
    let compiled_b = store.compile().expect("reference B compiles");
    let model_b = compiled_b.model();
    let world_b = &model_b.world;

    // ── parity checks ─────────────────────────────────────────────

    // (a) extensional facts match.
    let facts_a = render_facts(world_a);
    let facts_b = render_facts(world_b);
    assert_eq!(
        facts_a, facts_b,
        "extensional facts must be identical after lorestore round-trip"
    );

    // (b) fixpoint (derived relation) is identical.
    let derived_a = render_derived(world_a);
    let derived_b = render_derived(world_b);
    assert_eq!(
        derived_a, derived_b,
        "fixpoint must be identical after lorestore round-trip"
    );

    // (c) load findings are identical.
    assert_eq!(
        render_findings(model_a.findings()),
        render_findings(model_b.findings()),
        "load findings must be identical after lorestore round-trip"
    );

    // (d) interner (variable set) matches.
    let names_a = model_a.interner().names().to_vec();
    let names_b = model_b.interner().names().to_vec();
    assert_eq!(names_a, names_b, "interned variable sets must match");

    // (e) valuation (truth assignment over resolved names) matches.
    for (&atom_idx, val_a) in &world_a.valuation.0 {
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
        let val_b = world_b.valuation.get(b_idx).unwrap_or(false);
        assert_eq!(
            *val_a, val_b,
            "valuation disagree on variable '{name}': A={val_a}, B={val_b}"
        );
    }

    // (f) loaded atom set matches (the core of the read path: the store must
    //     reproduce every atom the compiler loaded).
    assert_eq!(
        model_a.atoms().len(),
        model_b.atoms().len(),
        "loaded atom count must match after lorestore round-trip"
    );

    println!(
        "differential parity passed: {} atoms, {} facts, {} derived, {} findings",
        model_a.atoms().len(),
        facts_a.len(),
        derived_a.len(),
        render_findings(model_a.findings()).len(),
    );
}
