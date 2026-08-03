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
mod simulation;

/// The current worker contract and envelope protocol version. `0.6.4` is the
/// LS-7 runtime-continuity typed-successor bump: the fourth and last place the
/// wire carried `.dli` text — `dli.check-draft`'s `continuity` rows, whose
/// `predicate`/`subject`/`value` strings were assembled into
/// `predicate(subject, value)` and parsed — gains the typed successors
/// `atom_expr` (on an established or prior fact) and `to_atom_expr` (on a
/// change, whose target atom is what canon is checked against). Both are
/// `skip_serializing_if = "Option::is_none"`, so a `0.6.3` request serializes
/// byte-identically and the string path is untouched.
///
/// `0.6.3` was the LS-7 typed-successor bump for the other three text-on-wire
/// remnants: `goal_atom` beside `dli.query`/`author.query`'s `goal`,
/// `additions` beside `dli.fork.add`'s `declarations`, and `atom_expr` beside a
/// payload claim's `atom`. The retirement of the string forms is registered in
/// `serve::KILLED_FIELDS` and gated on contract 1.0 — dormant here.
/// `0.6.2` was the LS-9 typed-interior structuring bump: typing the `ratify`
/// carriage (`RatifyDecl`) shifted the ratify serialization, which re-derives
/// the embedded commit/merge hashes in the affected golden fixtures — a
/// fixture-byte change the C4 guard couples to a `contract_version` change. The
/// wire shape is unchanged and additive: a request pinned to `0.6.1`/`0.6.0`
/// (or legacy `0.5.1`) still receives byte-identical responses via dual-mode.
/// `0.6.1` added the branch/merge ops (`author.branch`, `author.merge`) — the
/// LS-6 merge engine on the tool surface (roadmap LS-6). `0.6.0` through
/// `0.6.4` share the JSON-first wire shape; `0.5.1` is the legacy dual-mode
/// shape.
pub const CONTRACT_VERSION: &str = "0.6.4";

/// The legacy dual-mode contract version: a `0.5.1` request receives
/// byte-identical `0.5.1`-shaped responses (no 0.6.0 fields) until the consumer
/// completes EXT-F4 — nothing removed, nothing reshaped (DL-0010,
/// DL-0031..DL-0037 discipline).
pub const COMPAT_CONTRACT_VERSION: &str = "0.5.1";

/// Every contract version this build answers. Multi-version dual-mode
/// negotiation (DL-0046, extended by LS-6, LS-9, and LS-7): `0.5.1` (legacy),
/// `0.6.0` (JSON-first), `0.6.1` (JSON-first + branch/merge), `0.6.2`
/// (JSON-first + LS-9 typed interiors), `0.6.3` (JSON-first + LS-7 typed
/// successors), and `0.6.4` (JSON-first + LS-7 typed runtime-continuity rows).
/// Contract `1.0` — where the retired string forms in `serve::KILLED_FIELDS`
/// start being refused — is deliberately absent: no version this build speaks
/// reaches it, so the field-level killed forms are dormant.
pub const ACCEPTED_CONTRACT_VERSIONS: &[&str] =
    &["0.5.1", "0.6.0", "0.6.1", "0.6.2", "0.6.3", "0.6.4"];

/// Whether a negotiated version speaks the JSON-first ("v2") wire shape — the
/// 0.6.0 replay-context fields (`root_hash`, `valuation`) and the JSON-first
/// `materialize`. Only the legacy `0.5.1` wire omits them; `0.6.0` and `0.6.1`
/// share the shape.
#[must_use]
pub fn speaks_v2(dli: &str) -> bool {
    dli != COMPAT_CONTRACT_VERSION
}

pub use envelope::{ErrorBody, ErrorCode, Request, Response};
pub use finding::{Finding, Provenance, Severity};
pub use ops::Op;
pub use schema::contract_schemas;
