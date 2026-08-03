//! The bundled demo corpora, served from committed fact objects (roadmap
//! §LS-7).
//!
//! The three neutral teaching worlds the contract's fixtures exercise —
//! `camelot`, `round-table`, `warding-vows` — used to reach the read surface by
//! parsing `.dli` text (or building an AST in Rust) on every resolution. They now
//! reach it the way every other corpus will: as **stored fact objects**,
//! reconstructed through [`dli_lorestore::lower::reconstruct_pack`]. Nothing on
//! this path calls the parser.
//!
//! ## What is committed
//!
//! Each demo ships a `*.facts.json` bundle under `crates/dli-serve/demos/`: the
//! head [`Commit`] of a real [`Lorestore`](dli_lorestore::Lorestore) import (its
//! interner name table, known decisions, scope name, and the content addresses of
//! its facts) paired with the [`FactObject`]s that commit names, in commit order.
//! The bundle is the store's reachable state — `state ≡ fold(log)` — serialized
//! as one document, so the worker keeps the property the embedded sources gave
//! it: **no file read at runtime**, offline and self-contained.
//!
//! The bundles are generated from committed sources and re-derived on every gate
//! run (`tests/demo_fixtures.rs`), so a source edit without a regeneration is a
//! hard failure, never a silent drift.
//!
//! ## Sources of truth
//!
//! | demo | source | scope |
//! |---|---|---|
//! | `camelot` | `demos/camelot.source.json` | `camelot` |
//! | `round-table` | `examples/camelot.dli` | `camelot` |
//! | `warding-vows` | `demos/warding-vows.source.dli` | `warding-vows` |
//!
//! `camelot` and `round-table` are **different worlds** despite the shared scope
//! name: `camelot` is the small hand-built subset (a sort, three entities, two
//! facts) that the crate's doctests and the export fixtures use; `round-table` is
//! the full teaching corpus in `examples/camelot.dli`, whose `scope camelot`
//! header is what its hypothesis variables (`h.camelot#N`) are named after. The
//! demo id is the pack name; the scope header is the commit's `pack_name`.
//!
//! `camelot` alone has no `.dli` source — it was built directly as an AST — so
//! its committed source is the serialized [`Pack`], which is also the preimage of
//! its export-manifest source anchor (see [`source_bytes`]).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use dli_core::atom::Interner;
use dli_lorestore::{Commit, FactObject};
use dli_world::Pack;
use dli_world::language::LanguageModel;

/// The demo document ids this module resolves, in wire order.
pub const DEMO_DOCUMENTS: [&str; 3] = ["camelot", "round-table", "warding-vows"];

/// The scope-header name each demo's stored facts were committed under — the
/// commit's `pack_name`, which [`dli_lorestore::lower::reconstruct_pack`] replays
/// as the reconstructed document's `scope`. It is the demo id for every corpus
/// but `round-table`, whose source declares `scope camelot`.
#[must_use]
pub fn scope_name(document: &str) -> Option<&'static str> {
    match document {
        "camelot" | "round-table" => Some("camelot"),
        "warding-vows" => Some("warding-vows"),
        _ => None,
    }
}

/// The `camelot` demo's committed source: the hand-built [`Pack`] the world was
/// declared as, before it was lowered to fact objects. It is retained because it
/// is the preimage of the export manifest's source anchor (see [`source_bytes`]);
/// the served world comes from `camelot.facts.json`, not from here.
const CAMELOT_SOURCE: &str = include_str!("../demos/camelot.source.json");

/// The `warding-vows` demo's committed source: a linear-conserved proof
/// discipline with one worked casting that checks. It gives the `check-proof` and
/// `simulate.*` ops a hermetic document carrying both a practice and a proof —
/// the neutral `camelot` demo world carries neither. It lives beside its bundle
/// (not under `examples/`) because its bytes are the frozen preimage of its
/// source anchor, so the formatter must not rewrite them.
const WARDING_VOWS_SOURCE: &str = include_str!("../demos/warding-vows.source.dli");

/// The `round-table` demo's committed source: the neutral, wholly public-domain
/// Matter of Britain teaching corpus that ships with dli. It carries the one
/// shape the `camelot` demo lacks — a full chronology (an era spine, events with
/// a `before`-fact and a deliberately UNORDERED pair) plus a language dialect —
/// so the `timeline.show`/`timeline.order` and dialect-scoped `name.check` reads
/// have a resolvable, engine-owned world to run against. It is the neutral
/// counterpart to any one production world; the worker embeds no production canon
/// (the dli/TPtH separation invariant).
const ROUND_TABLE_SOURCE: &str = include_str!("../../../examples/camelot.dli");

const CAMELOT_FACTS: &str = include_str!("../demos/camelot.facts.json");
const ROUND_TABLE_FACTS: &str = include_str!("../demos/round-table.facts.json");
const WARDING_VOWS_FACTS: &str = include_str!("../demos/warding-vows.facts.json");

/// One demo corpus as committed: the head commit of its lorestore import, and
/// the fact objects that commit names.
///
/// This is the whole reachable state of a single-commit store — everything
/// [`dli_lorestore::lower::reconstruct_pack`] needs to rebuild the pack. The
/// commit's `facts` list is the content address of each object in `facts`, at the
/// same index, so a hand-edited bundle fails its address check rather than
/// serving a quietly different world.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoBundle {
    /// The head commit: interner name table, known decisions, scope name, and the
    /// content address of every fact below.
    pub commit: Commit,
    /// The fact objects the head commit names, in commit order.
    pub facts: Vec<FactObject>,
}

/// The committed bundle text for a demo id, or `None` if the id names no demo.
#[must_use]
pub fn bundle_json(document: &str) -> Option<&'static str> {
    match document {
        "camelot" => Some(CAMELOT_FACTS),
        "round-table" => Some(ROUND_TABLE_FACTS),
        "warding-vows" => Some(WARDING_VOWS_FACTS),
        _ => None,
    }
}

/// Whether `document` names one of the bundled demo corpora.
#[must_use]
pub fn is_demo_document(document: &str) -> bool {
    DEMO_DOCUMENTS.contains(&document)
}

/// Reconstruct one demo's pack from its committed bundle.
///
/// Returns `None` if the bundle does not deserialize, if any fact object's
/// content address disagrees with the commit that names it, or if the fact stream
/// cannot be reconstructed — a compiled-in bundle that fails any of these is a
/// build-integrity fault, and resolving the id to nothing is the fail-closed
/// answer.
fn reconstruct(document: &str) -> Option<Pack> {
    let bundle: DemoBundle = serde_json::from_str(bundle_json(document)?).ok()?;

    // The bundle is content-addressed exactly as the store is: each object must
    // hash to the address its commit records for it.
    if bundle.facts.len() != bundle.commit.facts.len() {
        return None;
    }
    for (object, address) in bundle.facts.iter().zip(&bundle.commit.facts) {
        if dli_lorestore::content_hash(object).ok()?.as_str() != address.as_str() {
            return None;
        }
    }

    let interner = Interner::from_names(bundle.commit.interner.clone());
    let decisions: BTreeSet<String> = bundle.commit.decisions.iter().cloned().collect();
    let mut pack = dli_lorestore::lower::reconstruct_pack(
        &bundle.facts,
        &interner,
        &decisions,
        &bundle.commit.pack_name,
    )
    .ok()?;
    // The commit's `pack_name` is the *scope* the facts were committed under;
    // the pack this session serves is named by its demo id (they differ for
    // `round-table`, whose source declares `scope camelot`).
    document.clone_into(&mut pack.name);
    Some(pack)
}

/// Every demo pack, reconstructed once per process.
///
/// Reconstruction reads a compiled-in constant and touches no clock, filesystem,
/// or network, so caching it changes nothing observable — it only keeps a
/// per-request resolution (`dli.export.run`, `name.check`) from re-walking the
/// whole fact stream.
fn packs() -> &'static BTreeMap<&'static str, Pack> {
    static PACKS: OnceLock<BTreeMap<&'static str, Pack>> = OnceLock::new();
    PACKS.get_or_init(|| {
        DEMO_DOCUMENTS
            .iter()
            .filter_map(|&document| reconstruct(document).map(|pack| (document, pack)))
            .collect()
    })
}

/// The demo corpus pack for a doc-id, or `None` if the id names no demo.
#[must_use]
pub fn pack(document: &str) -> Option<Pack> {
    packs().get(document).cloned()
}

/// The worker's demo language model — the compiled language layer of the
/// `round-table` world (its `old-speech` dialect and name-parts). The `name.check`
/// op carries no corpus reference, so a dialect-scoped vet resolves its register
/// here, against the one engine-owned neutral corpus that declares a dialect. An
/// empty dialect keeps the register-free default behavior.
#[must_use]
pub fn language_model() -> LanguageModel {
    packs()
        .get("round-table")
        .map(|pack| LanguageModel::from_documents(&pack.documents).model)
        .unwrap_or_default()
}

/// The bytes a demo's export-manifest source anchor is taken over — its committed
/// source, not its stored facts.
///
/// The anchor is a wire-visible, contract-pinned value, so its preimage is frozen
/// at the source: the `.dli` text for the two parsed corpora, and for `camelot`
/// (which has no `.dli` source) the compact serialization of its committed
/// [`Pack`], which is exactly what the pre-lorestore path hashed.
#[must_use]
pub fn source_bytes(document: &str) -> Option<Cow<'static, [u8]>> {
    match document {
        "round-table" => Some(Cow::Borrowed(ROUND_TABLE_SOURCE.as_bytes())),
        "warding-vows" => Some(Cow::Borrowed(WARDING_VOWS_SOURCE.as_bytes())),
        "camelot" => {
            let pack: Pack = serde_json::from_str(CAMELOT_SOURCE).ok()?;
            serde_json::to_vec(&pack).ok().map(Cow::Owned)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DEMO_DOCUMENTS, is_demo_document, language_model, pack, scope_name, source_bytes};

    #[test]
    fn every_demo_bundle_reconstructs() {
        for document in DEMO_DOCUMENTS {
            let resolved = pack(document).expect("demo bundle reconstructs");
            assert_eq!(resolved.name, document, "the pack is named by its demo id");
            assert_eq!(
                resolved.documents.len(),
                1,
                "{document}: one reconstructed document"
            );
            let scope = resolved.documents[0]
                .scope
                .as_ref()
                .expect("reconstructed document carries its scope header");
            assert_eq!(
                scope.name.text,
                scope_name(document).expect("known demo"),
                "{document}: the scope header replays the committed pack name"
            );
            dli_world::compile(&resolved).expect("a demo world compiles");
        }
    }

    #[test]
    fn an_unknown_id_resolves_to_nothing() {
        assert!(pack("saltmarket").is_none());
        assert!(!is_demo_document("saltmarket"));
        assert!(source_bytes("saltmarket").is_none());
        assert!(scope_name("saltmarket").is_none());
    }

    #[test]
    fn the_demo_language_model_carries_the_round_table_dialect() {
        assert!(
            !language_model().registers.is_empty(),
            "the round-table world declares a dialect register"
        );
    }

    #[test]
    fn every_demo_carries_source_bytes() {
        for document in DEMO_DOCUMENTS {
            assert!(
                !source_bytes(document)
                    .expect("demo has a source")
                    .is_empty(),
                "{document}: the source anchor has a preimage"
            );
        }
    }
}
