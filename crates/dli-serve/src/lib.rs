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
/// `1.3.0` is DM-4: the expert authoring surface (wave 1 — `author.add_dialect`,
/// `add_name_part`, `add_name`, `add_term`, `add_banned`; tier 2 —
/// `add_state`, `add_attest`) and the **authority scrub** (DL-0054, DL-0058).
/// The ops are additive and advertised only from `1.3.0` up. The scrub is the
/// one behavior change, and it is gated at this version precisely so it is not
/// one: `0.5.1` through `1.2.0` keep the `provenance.cited-refused` refusal
/// byte-exact, so every tagged contract before this one is frozen.
/// `1.4.0` adds the canon-normalized corpus export and its `canon` response
/// field; it is a new wire version because the response shape is additive.
/// `1.5.0` unifies standing markers and adds ordinary promotion/defeat tools.
pub const CONTRACT_VERSION: &str = "1.5.0";

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
/// above so an older negotiation's capability list is unmoved), and `1.3.0`
/// (DM-4: the expert authoring ops and the authority scrub, DL-0058 — the ops
/// additive and advertised only from `1.3.0` up, the scrub gated so that every
/// version below keeps `provenance.cited-refused` byte-exact), and `1.4.0`
/// (LS-7: canon-normalized corpus export), and `1.5.0` (standing vocabulary
/// unification plus ordinary promotion/defeat tools).
///
/// The whole `0.x` prefix stays here. LS-8 adds a major, it does not retire a
/// minor: a consumer pinned to any of them keeps its byte-identical wire until
/// EXT-F4, and `1.0.0` is opt-in by negotiation like every version before it.
/// The `serve::KILLED_FIELDS` retirement is keyed on the major, so it is live
/// on exactly one member of this list and dormant on the older versions.
pub const ACCEPTED_CONTRACT_VERSIONS: &[&str] = &[
    "0.5.1", "0.6.0", "0.6.1", "0.6.2", "0.6.3", "0.6.4", "1.0.0", "1.1.0", "1.2.0", "1.3.0",
    "1.4.0", "1.5.0",
];

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

/// Whether the negotiated version carries the LS-6 branch/merge ops (≥ `0.6.1`).
#[must_use]
pub fn has_branch_merge(dli: &str) -> bool {
    at_or_above(dli, (0, 6, 1))
}

/// Whether the negotiated version carries the DM-3 corpus-exchange ops
/// (≥ `1.2.0`, DL-0056).
#[must_use]
pub fn has_corpus_ops(dli: &str) -> bool {
    at_or_above(dli, (1, 2, 0))
}

/// Whether the negotiated version carries LS-7's canon-normalized corpus
/// export and additive `canon` response field (>= `1.4.0`).
#[must_use]
pub fn has_canon_export(dli: &str) -> bool {
    at_or_above(dli, (1, 4, 0))
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

/// Whether the negotiated version checks that an imported object's `claim_id`
/// matches the identity its content mints (≥ `1.3.0`, DL-0059).
///
/// Gated for the same reason every other behavior change here is: `1.2.0` is a
/// **tagged** contract that shipped accepting a mislabelled object, and its
/// golden fixture records that it does. Correcting the fixture would rewrite
/// what a released version is on record as doing; refusing on a `1.2.0` wire
/// would change a released version's behavior. The check therefore lands on the
/// living wire and `1.2.0` keeps its bytes — including the mislabelled object in
/// `golden/corpus-import-v1`, which stays exactly as recorded because it is a
/// truthful record of the version it was recorded under.
#[must_use]
pub fn validates_claim_ids(dli: &str) -> bool {
    at_or_above(dli, (1, 3, 0))
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
