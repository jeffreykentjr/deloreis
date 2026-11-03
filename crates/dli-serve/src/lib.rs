//! Deloreis Lore Interchange wire contract.
//!
//! This crate is the single source of truth for the request/response envelope,
//! the closed [`ops::Op`] set, and the per-op parameter and result schemas. The
//! `dli-worker` contract fixtures are validated against exactly the schemas
//! [`schema::contract_schemas`] emits, so a wire change here is a reviewable
//! schema diff there. The serve loop and MCP adapter are the SERVE-1 milestone.

pub mod author;
pub mod contract;
pub mod demo;
pub mod envelope;
pub mod evidence;
pub mod finding;
pub mod handlers;
pub mod mcp;
pub mod ops;
pub mod project;
mod query_goal;
pub mod schema;
pub mod serve;
pub mod session;

/// The only served worker contract and envelope protocol version.
pub const CONTRACT_VERSION: &str = "2.0.0";

/// The contract versions accepted at the server boundary.
pub const ACCEPTED_CONTRACT_VERSIONS: &[&str] = &[CONTRACT_VERSION];

/// Whether predicate identity is ready to advertise as a capability. The wire
/// lane owns this explicit seam so a future durable-store regression cannot
/// silently turn a partial implementation into a live promise.
#[must_use]
pub const fn predicate_identity_wired() -> bool {
    true
}

pub use envelope::{ErrorBody, ErrorCode, Request, Response};
pub use finding::{Finding, Provenance, ProvenanceState, Severity};
pub use ops::Op;
pub use schema::contract_schemas;
