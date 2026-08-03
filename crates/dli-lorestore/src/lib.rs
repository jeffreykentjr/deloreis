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
pub mod merge;
pub mod object;
pub mod storage;
pub mod store;
pub mod upcast;

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
///   whose body is the typed `RuleBody`. A v1 rule carriage deepens to v2 on
///   read via the front-end lowering ([`upcast::upcast_value`]); every other
///   carriage is byte-identical across v1 and v2. No stored fact is rewritten —
///   the deepening happens in memory, at the load boundary.
pub const SCHEMA_VERSION: u32 = 2;

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
