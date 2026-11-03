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

/// The old rendered-lines contract, retained internally for decoding legacy data.
/// Requests naming it are refused at the server boundary.
pub const COMPAT_CONTRACT_VERSION: &str = "0.5.1";

/// The contract versions accepted at the server boundary.
pub const ACCEPTED_CONTRACT_VERSIONS: &[&str] = &[CONTRACT_VERSION];

/// Whether a negotiated version speaks the JSON-first ("v2") wire shape — the
/// 0.6.0 replay-context fields (`root_hash`, `valuation`) and the JSON-first
/// `materialize`. Only the legacy `0.5.1` wire omits them; `0.6.0` and `0.6.1`
/// share the shape.
#[must_use]
pub fn speaks_v2(dli: &str) -> bool {
    dli != COMPAT_CONTRACT_VERSION
}

/// A negotiated version's `(major, minor, patch)` ordering key, or `None` if it
/// is not three dotted numbers. Only versions in [`ACCEPTED_CONTRACT_VERSIONS`]
/// ever reach the gates below — dispatch refuses anything else before routing —
/// so an unparseable version answers `false` to every floor rather than needing
/// a fallible signature.
fn version_key(dli: &str) -> Option<(u32, u32, u32)> {
    let mut parts = dli.split('.');
    let mut next = || parts.next()?.parse::<u32>().ok();
    let key = (next()?, next()?, next()?);
    parts.next().is_none().then_some(key)
}

/// Whether the negotiated version is at or above a floor.
///
/// **A floor, deliberately, not an equality.** Every version gate here started
/// as an exact match against the newest version, which is correct exactly until
/// the next bump and then silently wrong: adding `1.3.0` to the accepted list
/// with `matches!(dli, "1.2.0")` still guarding the corpus ops would have
/// dropped `dli.corpus.import`/`export` from a `1.3.0` caller's capability list
/// — a surface added in DM-3 disappearing in DM-4, with no test failing, since
/// the tier assertion was written against the same exact match. A floor cannot
/// fail that way: a version added later inherits every capability by
/// construction, which is the promise `capabilities` documents.
fn at_or_above(dli: &str, floor: (u32, u32, u32)) -> bool {
    version_key(dli).is_some_and(|key| key >= floor)
}

/// Whether the negotiated version carries Contract 1.5's standing tools.
#[must_use]
pub fn has_standing_tools(dli: &str) -> bool {
    at_or_above(dli, (1, 5, 0))
}

/// Whether predicate identity is ready to advertise as a capability. The wire
/// lane owns this explicit seam so a future durable-store regression cannot
/// silently turn a partial implementation into a live promise.
#[must_use]
pub const fn predicate_identity_wired() -> bool {
    true
}

/// Whether the negotiated version carries the `Finding.data` structured-
/// payload rider (≥ `1.7.0`, the CAMPAIGN DL). Additive and field-level, not
/// an op gate: a finding's `data` is present only on specific classes that
/// document one (`continuity.fd-violation` and its derived sibling,
/// `continuity.fluent-contradiction`, `continuity.fluent-order-undeclared`)
/// and only when this floor is met; every earlier negotiation's findings —
/// and the fixtures pinning their bytes — stay byte-identical, since the
/// field is absent rather than `null` (`skip_serializing_if`).
#[must_use]
pub fn has_finding_data(dli: &str) -> bool {
    at_or_above(dli, (1, 7, 0))
}

pub use envelope::{ErrorBody, ErrorCode, Request, Response};
pub use finding::{Finding, Provenance, ProvenanceState, Severity};
pub use ops::Op;
pub use schema::contract_schemas;
