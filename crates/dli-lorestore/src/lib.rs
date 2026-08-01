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

pub mod canonical;
pub mod evidence;
pub mod identity;
pub mod lorestore;
pub mod lower;
pub mod object;
pub mod storage;
pub mod store;

pub use canonical::{
    CanonicalError, canonical_bytes, content_hash, digest_bytes, validate_content_address,
};
pub use evidence::{EvidenceEnvelope, EvidenceKind};
pub use identity::{Authority, ClaimId, ContentHash, LineageEdge};
pub use lorestore::{Lorestore, MAIN_REF};
pub use object::{CondPair, ExpertBody, ExpertDeclKind, FactObject};
pub use storage::{FilesystemStorage, Storage, StoreError};
pub use store::{Commit, Ref};

/// The schema version carried by every stored object. Bumped when the stored
/// shape changes; old versions remain readable via typed upcast-on-read.
pub const SCHEMA_VERSION: u32 = 1;

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
