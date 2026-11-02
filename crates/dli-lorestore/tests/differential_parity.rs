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
    // 1 — build the reference Pack from the test corpus.
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

/// Assert full compiled parity between a direct compile and the
/// import → store → recompile path, for one pack.
#[allow(clippy::too_many_lines)]
fn assert_store_parity(pack: &dli_world::Pack, label: &str) {
    let compiled_a = compile(pack).unwrap_or_else(|e| panic!("{label}: direct compile: {e}"));
    let model_a = compiled_a.model();

    let tmp = TempDir::new().expect("tempdir");
    let store = Lorestore::open(tmp.path().to_path_buf()).expect("open lorestore");
    store
        .import_pack(pack)
        .unwrap_or_else(|e| panic!("{label}: import refused: {e}"));
    let compiled_b = store
        .compile()
        .unwrap_or_else(|e| panic!("{label}: store compile: {e}"));
    let model_b = compiled_b.model();

    assert_eq!(
        render_facts(&model_a.world),
        render_facts(&model_b.world),
        "{label}: extensional facts must be identical"
    );
    assert_eq!(
        render_derived(&model_a.world),
        render_derived(&model_b.world),
        "{label}: fixpoint must be identical"
    );
    assert_eq!(
        render_findings(model_a.findings()),
        render_findings(model_b.findings()),
        "{label}: findings must be identical"
    );
    assert_eq!(
        model_a.interner().names(),
        model_b.interner().names(),
        "{label}: interned variable sets must match"
    );
    let names_b = model_b.interner().names().to_vec();
    for (&atom_idx, val_a) in &model_a.world.valuation.0 {
        let name = model_a.interner().resolve(atom_idx).expect("atom has name");
        let b_idx = dli_core::atom::Atom(
            u32::try_from(
                names_b
                    .iter()
                    .position(|n| n == name)
                    .unwrap_or_else(|| panic!("{label}: `{name}` missing in B interner")),
            )
            .expect("fits in u32"),
        );
        assert_eq!(
            *val_a,
            model_b.world.valuation.get(b_idx).unwrap_or(false),
            "{label}: valuation disagrees on `{name}`"
        );
    }
    assert_eq!(
        model_a.atoms().len(),
        model_b.atoms().len(),
        "{label}: loaded atom count must match"
    );
}

/// The corpus-wide oracle (audit A7, DL-0045 ruling 5): every example `.dli`
/// pack — rules, cycles, practices, dialects, proofs, forks and all — must
/// survive the `.dli → parse → import → store → recompile` round-trip with an
/// identical compiled world, or be *refused* by the import. Nothing may pass
/// through the store and come out silently different.
#[test]
#[allow(clippy::print_stdout)]
fn every_example_dli_pack_round_trips_through_the_store() {
    let worlds = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut dli_files: Vec<std::path::PathBuf> = walk_dli(&worlds);
    dli_files.sort();
    assert!(
        !dli_files.is_empty(),
        "the example tree contains .dli packs"
    );

    for path in &dli_files {
        let source = std::fs::read_to_string(path).expect("read .dli file");
        let (document, diagnostics) = dli_syntax::parse_document(&source);
        assert!(
            diagnostics.is_empty(),
            "{}: example pack must parse clean, got {diagnostics:?}",
            path.display()
        );
        let name = document.scope.as_ref().map_or_else(
            || {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("pack")
                    .to_owned()
            },
            |s| s.name.text.clone(),
        );
        let pack = dli_world::Pack {
            name,
            documents: vec![document],
        };
        assert_store_parity(&pack, &path.display().to_string());
    }
    println!(
        "example differential parity passed over {} packs",
        dli_files.len()
    );
}

fn walk_dli(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_dli(&path));
        } else if path.extension().is_some_and(|e| e == "dli") {
            out.push(path);
        }
    }
    out
}

/// **The import-side claim-id validator agrees with the write-side minter, over
/// every example pack.**
///
/// `expected_claim_id` is what `dli.corpus.import` checks a caller's object
/// against from contract 1.3.0 (DL-0059 ruling 4). It re-derives the id by
/// delegating to the same two minting functions the write paths use, so the
/// *label format* has one reader and cannot drift. What it does re-decide is the
/// **dispatch**: it picks the declaration minter or the atom minter by looking at
/// whether the stored object carries an `expert_body`, while the write paths pick
/// theirs at six separate call sites. Nothing structurally forces those two
/// choices to agree, and if they ever diverge the validator would enforce an id
/// the engine never mints — turning a guard into the bug.
///
/// So the agreement is asserted end-to-end rather than argued: lower every
/// example pack and require that the validator re-derives exactly the id the
/// lowering minted, for every object. A future write path that pairs a claim id
/// with the wrong carriage shape reds here, whether or not anyone remembers this
/// function exists.
#[allow(clippy::print_stdout)]
#[test]
fn the_claim_id_validator_agrees_with_the_minter_on_every_example_object() {
    let worlds = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut dli_files: Vec<std::path::PathBuf> = walk_dli(&worlds);
    dli_files.sort();
    assert!(!dli_files.is_empty(), "the example tree has packs");

    let (mut objects, mut carried, mut bare) = (0usize, 0usize, 0usize);
    for path in &dli_files {
        let source = std::fs::read_to_string(path).expect("read .dli file");
        let (document, diagnostics) = dli_syntax::parse_document(&source);
        assert!(diagnostics.is_empty(), "{}: parses", path.display());
        let name = document.scope.as_ref().map_or_else(
            || {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("pack")
                    .to_owned()
            },
            |s| s.name.text.clone(),
        );
        let pack = dli_world::Pack {
            name,
            documents: vec![document],
        };
        let (lowered, _interner) = dli_lorestore::lower::lower_pack(&pack);
        for object in &lowered {
            objects += 1;
            if object.expert_body.is_some() {
                carried += 1;
            } else {
                bare += 1;
            }
            assert_eq!(
                dli_lorestore::lower::expected_claim_id(object),
                object.claim_id,
                "{}: the import validator would refuse an object this engine minted \
                 (atom {})",
                path.display(),
                object.atom.render()
            );
        }
    }
    // Non-vacuous in both directions: the dispatch has two arms and a corpus
    // that exercised only one would prove nothing about the other.
    assert!(
        carried > 0 && bare > 0,
        "the corpus must exercise both claim-id arms (carried {carried}, bare {bare})"
    );
    println!(
        "claim-id validator agrees with the minter on {objects} objects \
         ({carried} carried, {bare} bare) across {} packs",
        dli_files.len()
    );
}
