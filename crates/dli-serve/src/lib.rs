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

/// The current worker contract and envelope protocol version. `1.0.0` is the
/// LS-8 major: the JSON-first surface, and the version at which the retired
/// string forms registered in `serve::KILLED_FIELDS` stop being accepted. The
/// registry that LS-7 left dormant is live from here — a `1.0` request carrying
/// `goal`, `declarations`, or a payload claim's `atom` is refused with
/// `killed-form` naming its typed successor, and the typed successor is sent
/// alone (the string form is no longer a required field). The retirement is
/// two-sided and version-gated in one place, so nothing about it reaches a
/// `0.x` wire: before `1.0` the string form is still **required**, exactly as
/// its params struct required it, and still answered.
///
/// LS-8 is otherwise **local prep**. Every `0.5.1`–`0.6.4` response is
/// byte-identical, every dual-mode path is intact, and the consumer cutover is
/// external-blocked on EXT-F4.
///
/// `0.6.4` was the LS-7 runtime-continuity typed-successor bump: the fourth and
/// last place the wire carried `.dli` text — `dli.check-draft`'s `continuity`
/// rows, whose `predicate`/`subject`/`value` strings were assembled into
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
/// `serve::KILLED_FIELDS` and gated on contract 1.0 — dormant on that wire.
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
pub const CONTRACT_VERSION: &str = "1.2.0";

/// The legacy dual-mode contract version: a `0.5.1` request receives
/// byte-identical `0.5.1`-shaped responses (no 0.6.0 fields) until the consumer
/// completes EXT-F4 — nothing removed, nothing reshaped (DL-0010,
/// DL-0031..DL-0037 discipline).
pub const COMPAT_CONTRACT_VERSION: &str = "0.5.1";

/// Every contract version this build answers. Multi-version dual-mode
/// negotiation (DL-0046, extended by LS-6, LS-9, LS-7, and LS-8): `0.5.1`
/// (legacy), `0.6.0` (JSON-first), `0.6.1` (JSON-first + branch/merge), `0.6.2`
/// (JSON-first + LS-9 typed interiors), `0.6.3` (JSON-first + LS-7 typed
/// successors), `0.6.4` (JSON-first + LS-7 typed runtime-continuity rows), and
/// `1.0.0` (LS-8: the typed successors are the only form), and `1.1.0` (DM-2:
/// `SCHEMA_VERSION` 3 — the stored fact-object canonical bytes changed, which
/// is a contract change under schema-first, DL-0003/DL-0041/DL-0055), and
/// `1.2.0` (DM-3: the corpus-exchange ops `dli.corpus.import` and
/// `dli.corpus.export`, DL-0056 — additive, and advertised only at `1.2.0` and
/// above so an older negotiation's capability list is unmoved).
///
/// The whole `0.x` prefix stays here. LS-8 adds a major, it does not retire a
/// minor: a consumer pinned to any of them keeps its byte-identical wire until
/// EXT-F4, and `1.0.0` is opt-in by negotiation like every version before it.
/// The `serve::KILLED_FIELDS` retirement is keyed on the major, so it is live
/// on exactly one member of this list and dormant on the other six.
pub const ACCEPTED_CONTRACT_VERSIONS: &[&str] = &[
    "0.5.1", "0.6.0", "0.6.1", "0.6.2", "0.6.3", "0.6.4", "1.0.0", "1.1.0", "1.2.0",
];

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
