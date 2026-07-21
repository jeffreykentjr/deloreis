//! The `TPtH` round-trip GOLDEN — the hard gate of Step 5 (design §4).
//!
//! `apply(load_config("worlds/tpth/craft"), doctrines())` must equal the compiled
//! 107-principle corpus **field-for-field**. The expectation is a committed golden
//! snapshot ([`GOLDEN`]) captured from `kb::PRINCIPLES` *before* the const was
//! removed, so this parity is checkable after the flip: the engine ships zero world
//! content, and the `TPtH` corpus is reconstructed purely from the shipped generic
//! packs + the world craft config.
//!
//! Corpus **order** is not semantically load-bearing (every engine query is by id or
//! a family/binding filter), so the comparison is by id: same set of ids, and each
//! id's [`Principle`] equal field-for-field.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use dli_world::craft::kb::Principle;
use dli_world::craft::load::{doctrines, parse_pack};
use dli_world::craft::worldcfg::{apply, load_config, validate};

/// The committed golden: the 107 principles serialized from the compiled corpus.
const GOLDEN: &str = include_str!("golden/tpth-principles.json");

fn tpth_craft_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../worlds/tpth/craft")
        .canonicalize()
        .expect("worlds/tpth/craft exists at the repo root")
}

fn by_id(ps: &[Principle]) -> BTreeMap<String, &Principle> {
    ps.iter().map(|p| (p.id.to_string(), p)).collect()
}

#[test]
fn apply_reconstructs_the_107_field_for_field() {
    let cfg = load_config(&tpth_craft_dir()).expect("the TPtH craft config loads");
    let packs = doctrines();

    // The config is structurally clean against the shipped packs.
    let faults = validate(&cfg, &packs);
    assert!(faults.is_empty(), "config validate() faults: {faults:?}");

    let got = apply(&cfg, &packs);
    let want = parse_pack(GOLDEN)
        .expect("the golden snapshot parses")
        .principles;

    assert_eq!(
        want.len(),
        107,
        "the golden snapshot carries 107 principles"
    );
    assert_eq!(
        got.len(),
        107,
        "apply(config, packs) reconstructs 107 principles, got {}",
        got.len()
    );

    let got_map = by_id(&got);
    let want_map = by_id(&want);

    // Same set of ids.
    let got_ids: Vec<&String> = got_map.keys().collect();
    let want_ids: Vec<&String> = want_map.keys().collect();
    assert_eq!(got_ids, want_ids, "reconstructed ids match the golden ids");

    // Each principle equal field-for-field.
    let mut mismatches = Vec::new();
    for (id, want_p) in &want_map {
        let got_p = got_map.get(id).expect("id present in both");
        if got_p != want_p {
            mismatches.push(format!(
                "\n  {id}:\n    want={want_p:?}\n    got ={got_p:?}"
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} principle(s) did not round-trip field-for-field:{}",
        mismatches.len(),
        mismatches.join("")
    );
}

/// The composition shape the round-trip depends on: 83 shipped generic records
/// (26 pure-generic + 57 stripped generic+overlay) fold with the world config's
/// overlays; 24 config-only world principles are appended. 83 + 24 = 107.
#[test]
fn composition_partitions_as_83_shipped_plus_24_world() {
    let cfg = load_config(&tpth_craft_dir()).expect("the TPtH craft config loads");
    let packs = doctrines();
    let shipped: usize = packs.iter().map(|p| p.principles.len()).sum();
    assert_eq!(shipped, 83, "the shipped generic library is 83 records");
    assert_eq!(
        cfg.world_principles.len(),
        24,
        "TPtH contributes 24 config-only world principles"
    );
    assert_eq!(shipped + cfg.world_principles.len(), 107);
}
