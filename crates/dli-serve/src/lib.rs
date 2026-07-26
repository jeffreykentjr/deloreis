//! Deloreis Lore Interchange wire contract.
//!
//! This crate is the single source of truth for the request/response envelope,
//! the closed [`ops::Op`] set, and the per-op parameter and result schemas. The
//! `dli-worker` contract fixtures are validated against exactly the schemas
//! [`schema::contract_schemas`] emits, so a wire change here is a reviewable
//! schema diff there. The serve loop and MCP adapter are the SERVE-1 milestone.

pub mod author;
pub mod contract;
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
mod simulation;

/// The pre-v1 worker contract and envelope protocol version.
pub const CONTRACT_VERSION: &str = "0.5.1";

pub use envelope::{ErrorBody, ErrorCode, Request, Response};
pub use finding::{Finding, Provenance, Severity};
pub use ops::Op;
pub use schema::contract_schemas;
