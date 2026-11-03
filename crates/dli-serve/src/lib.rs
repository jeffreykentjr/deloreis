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

/// Whether the negotiated version carries the DM-4 expert authoring surface —
/// wave 1 (`dialect`, `name_part`, `name`, `term`, `banned`) and tier 2
/// (`state`, `attest`) — at ≥ `1.3.0` (DL-0058).
#[must_use]
pub fn has_expert_authoring(dli: &str) -> bool {
    at_or_above(dli, (1, 3, 0))
}

/// Whether the negotiated version carries Contract 1.5's standing tools.
#[must_use]
pub fn has_standing_tools(dli: &str) -> bool {
    at_or_above(dli, (1, 5, 0))
}

/// Whether the negotiated version carries the baseline-decision recorder
/// (`author.record_decision`, ≥ `1.9.0`, DL-0083) — the wire op that writes a
/// `ratify` carriage for a caller-named decision so citing facts read certain,
/// and the floor `author.materialize`'s optional `decision` stamping and
/// `author.why`'s `supporting_decisions` field are gated on. A floor like every
/// gate here, not an equality: `serve::dispatch` gates `author.record_decision`
/// on it and `capabilities()` filters the op's advertisement by it, so a
/// `1.8.0`-or-earlier negotiation's capability list stays byte-identical (audit
/// C4).
#[must_use]
pub fn has_decisions(dli: &str) -> bool {
    at_or_above(dli, (1, 9, 0))
}

/// Whether the negotiated version gets the corrected declared-entity universe
/// for `author.entity_candidates` (GH #88, ≥ `1.11.0`). The operation itself
/// remains available from `1.10.0`; this separate floor preserves the released
/// 1.10.0 result semantics while making the corrected ranking explicit.
#[must_use]
pub fn has_declared_entity_candidates(dli: &str) -> bool {
    at_or_above(dli, (1, 11, 0))
}

/// Whether `author.defeat` records conditional attack edges and reads standing
/// through grounded labelling. Versions through 1.12 retain one-way false Ref
/// assignments for replay compatibility.
#[must_use]
pub fn has_conditional_defeat(dli: &str) -> bool {
    at_or_above(dli, (1, 13, 0))
}

/// Whether predicate identity is ready to advertise as a capability. The wire
/// lane owns this explicit seam so a future durable-store regression cannot
/// silently turn a partial implementation into a live promise.
#[must_use]
pub const fn predicate_identity_wired() -> bool {
    true
}

/// Whether the negotiated version carries the campaign's branch-scoped write
/// surface (`author.open`'s `branch` param, branch-routed materialize/standing
/// writes, `author.merge`'s `dry_run`/`expected_ref`, and `author.branch`'s
/// `delete` action, ≥ `1.7.0`, the CAMPAIGN DL). Params-only — no new op — so
/// this predicate gates request fields, not `capabilities()` entries; a
/// caller below `1.7.0` supplying `branch` (or the other gated params) meets a
/// schema-violation refusal, not a silent no-op.
#[must_use]
pub fn has_branch_writes(dli: &str) -> bool {
    at_or_above(dli, (1, 7, 0))
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

/// Whether the negotiated version **records** caller-supplied citations as data
/// rather than refusing them (the authority scrub: DL-0054, landed at the
/// version this predicate names by DL-0058).
///
/// At and above `1.3.0` a `cited` provenance lowers to a real `decided`
/// annotation carrying its citation ids, and referential integrity — the one
/// thing the engine can actually check — is what decides the claim's standing:
/// an id the corpus does not declare leaves its decision variable open, so the
/// claim can never read certain, and raises `citation.unknown-decision`.
///
/// Below `1.3.0` the pre-scrub `provenance.cited-refused` refusal is served
/// **byte-exact**, permanently. That is not a transition period: DL-0054 clause
/// 4 freezes every `0.x` wire until version retirement, and DL-0058 extends the
/// same freeze to `1.0.0`–`1.2.0`, which shipped with the refusal and are
/// pinned by released fixtures. A caller opts into the scrub by negotiating up,
/// exactly as it opts into every other version-gated change.
#[must_use]
pub fn records_citations(dli: &str) -> bool {
    at_or_above(dli, (1, 3, 0))
}

pub use envelope::{ErrorBody, ErrorCode, Request, Response};
pub use finding::{Finding, Provenance, ProvenanceState, Severity};
pub use ops::Op;
pub use schema::contract_schemas;
