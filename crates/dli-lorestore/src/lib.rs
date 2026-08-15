//! Deloreis Lore interchange store: content-addressed fact objects.
//!
//! The fact-object model is deloreis's storage substrate (roadmap LS-1). Facts
//! become immutable JSON objects addressed by their SHA-256 content hash under
//! RFC 8785 JCS canonicalization. The schemars-derived JSON Schema of these
//! objects is the language definition; `.dli` text is a rendering, not the
//! canonical form.
//!
//! ## Three-way identity split
//!
//! The model keeps three identity concerns apart (DL-0041, roadmap §LS-1):
//! - **Authority distinctness** — the provenance polynomial and `authority` field
//!   are part of the object, so the same proposition asserted by two authorities
//!   is two distinct objects. No parent hash is needed.
//! - **Claim continuity** — a stable [`ClaimId`] accompanies the content hash,
//!   tracking "this claim across revisions" (the Wikibase datom split). A parent
//!   chain serves this badly: hashes change on every revision.
//! - **Supersession / retcon history** — first-class [`LineageEdge`] objects
//!   carry their own `(P⁺, P⁻)`, because in a multi-authority world whether B
//!   supersedes A is itself contested and valuation-dependent.
//!
//! Only the bare parent pointer stays out of the content hash: it belongs to the
//! commit DAG (as in git), not to the fact.

pub mod cache;
pub mod canonical;
pub mod evidence;
pub mod identity;
pub mod lorestore;
pub mod lower;
pub mod merge;
pub mod object;
pub mod storage;
pub mod store;
pub mod upcast;

pub use cache::{CacheStats, DEFAULT_CACHE_CAPACITY};
pub use canonical::{
    CanonicalError, canonical_bytes, content_hash, digest_bytes, validate_content_address,
};
pub use evidence::{EvidenceEnvelope, EvidenceKind};
pub use identity::{Authority, ClaimId, ContentHash, LineageEdge};
pub use lorestore::{HEADS_PREFIX, Lorestore, MAIN_REF, branch_ref_name};
pub use merge::{MergeReport, StandingChange};
pub use object::{CondPair, ExpertBody, ExpertDeclKind, FactObject};
pub use storage::{FilesystemStorage, Storage, StoreError};
pub use store::{Commit, Ref};

/// The schema version carried by every stored object. Bumped when the stored
/// shape changes; old versions remain readable via typed upcast-on-read
/// ([`upcast`]).
///
/// - **v1** — the LS-1 fact-object model. Expert-tier declaration carriages
///   embed the whole `Decl` as JSON with `Vec<Clause>` string bodies (named,
///   versioned debt, roadmap §LS-1).
/// - **v2** — LS-9 expert-tier structuring: the `rule` carriage embeds a `Decl`
///   whose body is the typed `RuleBody`, and the `practice` carriage a `Decl`
///   whose body is the typed `PracticeBody`. A v1 rule or practice carriage
///   deepens to v2 on read via the front-end lowering ([`upcast::upcast_value`]);
///   every other carriage is byte-identical across v1 and v2. No stored fact is
///   rewritten — the deepening happens in memory, at the load boundary.
/// - **v3** — DM-2 (DL-0055): source position leaves the identity projection,
///   and provenance metadata is recorded beside the fact. A carriage body is
///   hashed span-normalized, so the same declaration at a different offset is
///   the same object (v2 hashed the located body, which made position
///   destabilize content addressing and defeat dedup and merge convergence);
///   the located original moves to
///   [`FactProvenance::located_body`](object::FactProvenance::located_body),
///   outside the hash, so nothing is lost. The authored ledger marker is
///   recorded in [`FactProvenance::marker`](object::FactProvenance::marker),
///   because `decided`/`ratified`/`provisional` share one support polynomial
///   and reverse lowering could not otherwise tell them apart. A v2 object
///   upcasts on read by normalizing its carriage body — which lands it on the
///   **same identity** as a freshly lowered v3 object, so an existing store is
///   recognized as the same lineage rather than refused as foreign — and
///   records no marker, since v2 never stored one.
/// - **v4** — Contract 1.5: the retired `decided` marker is upcast to
///   `provisional` at the load boundary. Marker metadata remains outside the
///   identity projection, so this migration does not change fact addresses.
pub const SCHEMA_VERSION: u32 = 4;

/// Return the canonical standing vocabulary for a stored marker.
///
/// `Decided` remains deserializable for one compatibility cycle, but a current
/// store must not expose it after the v4 upcast. Unknown markers are rejected
/// by callers rather than guessed into this vocabulary.
#[must_use]
pub fn status_of(marker: Option<dli_ast::annot::ProvenanceMarker>) -> Option<&'static str> {
    match marker {
        None => Some("asserted"),
        Some(dli_ast::annot::ProvenanceMarker::Proposed) => Some("proposed"),
        Some(dli_ast::annot::ProvenanceMarker::Provisional) => Some("provisional"),
        Some(dli_ast::annot::ProvenanceMarker::Ratified) => Some("ratified"),
        Some(dli_ast::annot::ProvenanceMarker::Decided) => None,
    }
}

/// All schemars-derived JSON schemas for this crate's stored objects, paired
/// with their schema name. Used by `xtask schema` to emit and drift-check the
/// committed `.schema.json` files.
pub fn schemas() -> Vec<(&'static str, schemars::Schema)> {
    use schemars::schema_for;
    vec![
        ("fact-object-v1", schema_for!(FactObject)),
        ("commit-v1", schema_for!(Commit)),
        ("ref-v1", schema_for!(Ref)),
        ("evidence-envelope-v1", schema_for!(EvidenceEnvelope)),
        ("lineage-edge-v1", schema_for!(LineageEdge)),
    ]
}
