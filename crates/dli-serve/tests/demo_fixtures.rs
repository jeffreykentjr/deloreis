//! The demo-corpus drift guard and generator (roadmap §LS-7).
//!
//! The three bundled demo worlds reach the read surface as **committed fact
//! objects** (`crates/dli-serve/demos/*.facts.json`, served by
//! [`dli_serve::demo`]). Nothing on that runtime path parses. The bundles are
//! generated *here*, from committed sources, and this file is the only place in
//! the crate that may call the front end.
//!
//! Two obligations, both gate-run:
//!
//! - **Drift** — every committed bundle is re-derived from its source and
//!   compared byte-for-byte. A `.dli` edit without a regeneration is a hard
//!   failure, so a bundle can never quietly stop describing its source.
//! - **Parity** — the world the store serves must equal the world the parser
//!   builds: identical extensional facts with identical support, identical
//!   fixpoint, identical interner, identical valuation, identical findings
//!   (the LS-3 differential-parity bar, re-run per demo).
//!
//! To regenerate after an intentional source change:
//!
//! ```text
//! cargo test -p dli-serve --test demo_fixtures -- --ignored regenerate
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dli_eval::world::World;
use dli_lorestore::Lorestore;
use dli_serve::demo::{self, DEMO_DOCUMENTS, DemoBundle};
use dli_world::Pack;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn demos_dir() -> PathBuf {
    crate_root().join("demos")
}

fn parse_clean(source: &str, label: &str) -> dli_ast::decl::Document {
    let (document, diagnostics) = dli_syntax::parse_document(source);
    assert!(
        diagnostics.iter().all(|d| !d.class.is_error()),
        "{label}: the committed demo source must parse clean, got {diagnostics:?}"
    );
    document
}

/// The `camelot` demo's source pack: the hand-built AST it was declared as. It
/// has no `.dli` text, so the committed `camelot.source.json` is generated from
/// here — which is what keeps the committed file and `example_docs` coupled.
fn camelot_source_pack() -> Pack {
    Pack {
        name: "camelot".to_owned(),
        documents: dli_world::loader::example_docs(),
    }
}

/// Every demo, as `(document id, source pack)`. The source pack is named by its
/// **scope**, not by the demo id: the name rides into the commit's `pack_name`,
/// which reconstruction replays as the document's `scope` header, and the
/// `round-table` corpus declares `scope camelot`.
fn demo_source_packs() -> Vec<(&'static str, Pack)> {
    let round_table = crate_root().join("../../examples/camelot.dli");
    let round_table_src = std::fs::read_to_string(&round_table).expect("read examples/camelot.dli");
    let warding_vows_src = std::fs::read_to_string(demos_dir().join("warding-vows.source.dli"))
        .expect("read demos/warding-vows.source.dli");

    vec![
        ("camelot", camelot_source_pack()),
        (
            "round-table",
            Pack {
                name: "camelot".to_owned(),
                documents: vec![parse_clean(&round_table_src, "examples/camelot.dli")],
            },
        ),
        (
            "warding-vows",
            Pack {
                name: "warding-vows".to_owned(),
                documents: vec![parse_clean(&warding_vows_src, "warding-vows.source.dli")],
            },
        ),
    ]
}

/// Generate one demo's bundle text: import the source pack into a real
/// lorestore, then serialize the head commit together with the fact objects it
/// names, in commit order. The result is the store's whole reachable state.
fn generate_bundle(pack: &Pack) -> String {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Lorestore::open(tmp.path().to_path_buf()).expect("open lorestore");
    let head = store.import_pack(pack).expect("import demo pack");
    let commit: dli_lorestore::Commit = store
        .read_object(&head)
        .expect("read head commit")
        .expect("head commit is present");
    let facts = commit
        .facts
        .iter()
        .map(|hash| {
            store
                .read_object::<dli_lorestore::FactObject>(hash)
                .expect("read fact object")
                .expect("fact object is present")
        })
        .collect();
    let mut text =
        serde_json::to_string_pretty(&DemoBundle { commit, facts }).expect("serialize bundle");
    text.push('\n');
    text
}

/// The committed `camelot.source.json` text.
fn generate_camelot_source() -> String {
    let mut text =
        serde_json::to_string_pretty(&camelot_source_pack()).expect("serialize camelot pack");
    text.push('\n');
    text
}

fn bundle_path(document: &str) -> PathBuf {
    demos_dir().join(format!("{document}.facts.json"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

const REGENERATE: &str = "run `cargo test -p dli-serve --test demo_fixtures -- --ignored regenerate` \
     and commit the result";

// ── the drift guard ──────────────────────────────────────────────────────

/// Every committed demo bundle is exactly what its committed source lowers to.
#[test]
fn demo_bundles_match_their_sources() {
    for (document, pack) in demo_source_packs() {
        let path = bundle_path(document);
        assert_eq!(
            read(&path),
            generate_bundle(&pack),
            "{document}: the committed fact-object bundle has drifted from its source — {REGENERATE}"
        );
    }
    let source = demos_dir().join("camelot.source.json");
    assert_eq!(
        read(&source),
        generate_camelot_source(),
        "camelot: the committed source pack has drifted from `example_docs` — {REGENERATE}"
    );
}

/// Lowering is deterministic: two imports of one pack produce the same bytes.
/// Without this a "regenerate" would be a coin flip and the guard above would
/// merely record which flip landed.
#[test]
fn generation_is_deterministic() {
    for (document, pack) in demo_source_packs() {
        assert_eq!(
            generate_bundle(&pack),
            generate_bundle(&pack),
            "{document}: two imports of one pack must produce identical bundles"
        );
    }
}

/// The committed bundle deserializes, its objects hash to the addresses its
/// commit records, and the served pack is named by its demo id under its source's
/// scope header.
#[test]
fn every_committed_bundle_is_content_addressed() {
    for document in DEMO_DOCUMENTS {
        let text = demo::bundle_json(document).expect("a demo has a bundle");
        let bundle: DemoBundle = serde_json::from_str(text).expect("bundle deserializes");
        assert_eq!(
            bundle.facts.len(),
            bundle.commit.facts.len(),
            "{document}: the commit names exactly the facts the bundle carries"
        );
        assert!(
            !bundle.facts.is_empty(),
            "{document}: a demo world lowers to at least one fact object"
        );
        for (object, address) in bundle.facts.iter().zip(&bundle.commit.facts) {
            assert_eq!(
                dli_lorestore::content_hash(object)
                    .expect("hash object")
                    .as_str(),
                address.as_str(),
                "{document}: a fact object does not hash to its recorded address"
            );
        }
        assert_eq!(
            bundle.commit.pack_name,
            demo::scope_name(document).expect("known demo"),
            "{document}: the commit records the source's scope name"
        );
    }
}

// ── the parity oracle ────────────────────────────────────────────────────

fn render_facts(world: &World, interner: &dli_core::atom::Interner) -> Vec<String> {
    let mut rows: Vec<String> = world
        .facts
        .iter()
        .map(|fact| {
            format!(
                "{}|{}",
                fact.atom.render(),
                dli_lorestore::lower::render_polynomial(&fact.ann.support, interner)
            )
        })
        .collect();
    rows.sort();
    rows
}

fn render_derived(world: &World) -> Vec<String> {
    let mut rows: Vec<String> = world.derived.iter().map(|d| d.atom.render()).collect();
    rows.sort();
    rows
}

/// The world each demo now serves from its stored facts is the world its source
/// compiles to — same facts under the same support, same fixpoint, same interner,
/// same valuation, same findings. This is the LS-3 differential-parity bar
/// applied per demo; it is what makes the reroute a no-op on the wire.
#[test]
fn every_demo_world_matches_its_parsed_source() {
    for (document, source) in demo_source_packs() {
        let direct = dli_world::compile(&source).expect("source pack compiles");
        let served =
            dli_world::compile(&demo::pack(document).expect("demo resolves")).expect("store world");

        let (a, b) = (direct.model(), served.model());
        assert_eq!(
            a.interner().names(),
            b.interner().names(),
            "{document}: interned variable sets must match"
        );
        assert_eq!(
            render_facts(&a.world, a.interner()),
            render_facts(&b.world, b.interner()),
            "{document}: extensional facts must be identical"
        );
        assert_eq!(
            render_derived(&a.world),
            render_derived(&b.world),
            "{document}: the fixpoint must be identical"
        );
        assert_eq!(
            a.world.valuation, b.world.valuation,
            "{document}: the valuation must be identical"
        );
        assert_eq!(
            serde_json::to_value(a.findings()).unwrap(),
            serde_json::to_value(b.findings()).unwrap(),
            "{document}: load findings must be identical"
        );
        assert_eq!(
            a.atoms().len(),
            b.atoms().len(),
            "{document}: the loaded atom count must match"
        );
    }
}

/// The demo language model — the `name.check` register source — is the one the
/// parsed `round-table` corpus builds.
#[test]
fn the_demo_language_model_matches_its_parsed_source() {
    let source = demo_source_packs()
        .into_iter()
        .find(|(id, _)| *id == "round-table")
        .expect("round-table is a demo")
        .1;
    let parsed = dli_world::language::LanguageModel::from_documents(&source.documents).model;
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        serde_json::to_value(demo::language_model()).unwrap(),
        "the served language model must equal the parsed one"
    );
}

/// The export-manifest source anchor is taken over the committed source bytes —
/// for `camelot`, the compact serialization of the pack `example_docs` builds,
/// which is exactly the preimage the pre-lorestore path hashed.
#[test]
fn the_camelot_source_anchor_preimage_is_unchanged() {
    assert_eq!(
        demo::source_bytes("camelot").expect("camelot has a source"),
        serde_json::to_vec(&camelot_source_pack()).unwrap(),
        "the camelot source anchor's preimage must stay byte-identical — {REGENERATE}"
    );
}

/// The committed sources are the ones the demos declare, and the demo id set is
/// closed: a fourth id resolves to nothing.
#[test]
fn the_demo_id_set_is_closed() {
    let ids: BTreeSet<&str> = DEMO_DOCUMENTS.into_iter().collect();
    let sourced: BTreeSet<&str> = demo_source_packs().into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, sourced, "every demo id has a committed source");
}

// ── the generator ────────────────────────────────────────────────────────

/// Rewrite every committed demo artifact from its source. Ignored by default;
/// run it deliberately after an intentional source edit.
#[test]
#[ignore = "generator: rewrites committed fixtures"]
fn regenerate_demo_fixtures() {
    for (document, pack) in demo_source_packs() {
        let path = bundle_path(document);
        std::fs::write(&path, generate_bundle(&pack))
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }
    let source = demos_dir().join("camelot.source.json");
    std::fs::write(&source, generate_camelot_source())
        .unwrap_or_else(|e| panic!("write {}: {e}", source.display()));
}
