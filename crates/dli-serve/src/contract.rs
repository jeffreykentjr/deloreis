//! Per-op parameter and result types.
//!
//! Every op has a real params struct and a real result struct, all deriving
//! `JsonSchema`. Mutating `author.*` ops share [`AuthorMutationResult`].

use std::collections::BTreeMap;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::term::AtomExpr;
use dli_core::ann::Status;
use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_eval::fork::{AtomChange, ConflictKind};
use dli_eval::query::{Layer, QueryRow};
use dli_eval::why::{Why, WhyNot};
use dli_eval::world::GroundAtom;
use dli_export::{
    ArticyExport, ContextPack, CsvExport, ExportManifest, ExportProfile, StaticWikiExport,
};
use dli_game::export::{CardDeck, LudiiExport};
use dli_ledger::ProofReport;
use dli_sim::RunManifest;
use dli_time::cycle::CyclePos;
use dli_time::slack::EpisodeSlack;
use dli_world::attest::{
    Attestation, ContentHash, ModelRef, Policy, PromptRef, Question, Sampling,
};
use dli_world::episode::{EpisodeAnchor, Knower, OpenLoopLedger, Placement};
use dli_world::episode_order::OrderIntegrityFinding;
use dli_world::payload::{Payload, PayloadFinding, RuntimeContinuity};

use crate::finding::{Finding, Provenance};

/// The result of any mutating `author.*` op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorMutationResult {
    /// Findings raised by the mutation (data, never instructions).
    pub findings: Vec<Finding>,
    /// The rendered lines the mutation produced. On a 0.6.0 `materialize`
    /// this is empty unless the request opted in with `rendered: true` —
    /// the JSON-first return carries `facts` instead.
    pub rendered: Vec<String>,
    /// The workspace digest after the mutation — always in the fence-valid
    /// `jcs:sha256:` regime, so fencing the next call with this value works.
    pub workspace_digest: String,
    /// The lorestore commit hash a `materialize` advanced the workspace's
    /// branch to. A separate field from `workspace_digest` — the two digest
    /// regimes never share a field. 0.6.0 materialize only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// The committed fact objects as JSON — the 0.6.0 materialize JSON-first
    /// return (`.dli` rendering is the opt-in, DL-0045 ruling 1).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub facts: Option<Vec<serde_json::Value>>,
}

/// A derived-consequence diff delta, shared by the fork mutation ops.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffDelta {
    /// Atoms that became certain, with before/after standing and rendering.
    pub newly_certain: Vec<AtomChange>,
    /// Atoms that became merely possible, with before/after standing and rendering.
    pub newly_possible: Vec<AtomChange>,
    /// Atoms that became dead, with before/after standing and rendering.
    pub newly_dead: Vec<AtomChange>,
    /// Atoms whose grade changed, with before/after standing and rendering.
    pub grade_changed: Vec<AtomChange>,
    /// Typed conflicts surfaced by the delta.
    pub conflicts: Vec<DiffConflict>,
}

/// A conflict carried by either a fork diff or a Door-T staged-world diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffConflict {
    /// Stable finding code for machine routing.
    pub code: String,
    /// Fork conflict family, when the conflict comes from the fork engine.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub kind: Option<ConflictKind>,
    /// The claims, assignments, or findings that cannot co-hold.
    pub blame: Vec<String>,
    /// Deterministic domain rendering.
    pub rendered: String,
}

/// A ruling-request draft emitted by `dli.fork.promote`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RqDraft {
    /// The slug of the ruling-request.
    pub slug: String,
    /// The question posed to the author.
    pub ask: String,
    /// The atoms/sections the ruling affects.
    pub affects: Vec<String>,
    /// The rendered consequence diff.
    pub diff: Vec<String>,
    /// The evidence gathered for the ask.
    pub evidence: Vec<String>,
}

/// The closed entity kind for `author.add_entity`. Events are not entities —
/// chronology points go through `author.add_event`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum EntityKind {
    /// A person.
    Person,
    /// A place.
    Place,
    /// A thing.
    Thing,
    /// An era (a named span of time).
    Era,
}

// -- dli.check ---------------------------------------------------------------

/// `dli.check` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    /// The document id to check.
    pub document: String,
}

/// `dli.check` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    /// Whether the document has no blocking error or attestation outcome.
    /// Advisory findings may be present when this is true.
    pub ok: bool,
    /// The weakest procedure grade used by the complete check.
    pub grade: Grade,
    /// World, payload, and attestation findings, if any.
    pub findings: Vec<CheckFinding>,
}

/// A document-wide check finding without flattening domain-specific evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "domain", rename_all = "lowercase")]
pub enum CheckFinding {
    /// A world/compiler finding.
    World(Finding),
    /// A replayed payload finding with durable payload and source identity.
    Payload(PayloadCheckFinding),
    /// A durable attestation dependency and its pass/fail state.
    Attestation(AttestationCheckFinding),
}

/// A replayed payload finding tied to one committed evidence record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PayloadCheckFinding {
    /// SHA-256 address of the committed payload record.
    pub payload_digest: String,
    /// Episode identifier carried by the payload.
    pub episode: String,
    /// Source filename carried by the payload.
    pub source_file: String,
    /// Domain-specific finding with line, quote, principle, and citations.
    #[serde(flatten)]
    pub finding: PayloadFinding,
}

/// The outcome-specific attestation evidence consumed by `dli.check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum AttestationCheckFinding {
    /// A current affirmative answer.
    Pass(Box<AttestationPassFinding>),
    /// A current negative answer.
    Fail(Box<AttestationFailFinding>),
    /// No answer has been committed.
    Missing(Box<AttestationMissingFinding>),
    /// The committed answer is not current.
    Stale(Box<AttestationStaleFinding>),
}

/// The singleton code serialized by an attestation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AttestationPassCode {
    /// `attest.pass`.
    #[serde(rename = "attest.pass")]
    Value,
}

/// The singleton code serialized by an attestation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AttestationFailCode {
    /// `attest.fail`.
    #[serde(rename = "attest.fail")]
    Value,
}

/// The singleton code serialized by a missing attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AttestationMissingCode {
    /// `attest.missing`.
    #[serde(rename = "attest.missing")]
    Value,
}

/// The singleton code serialized by a stale attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AttestationStaleCode {
    /// `attest.stale`.
    #[serde(rename = "attest.stale")]
    Value,
}

/// The only severity carried by a blocking attestation outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AttestationErrorSeverity {
    /// The outcome blocks the document verdict.
    Error,
}

/// The only grade carried by a current model judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AttestedProcedureGrade {
    /// The procedure consumed a stored model judgment.
    Attested,
}

/// The only grade carried by deterministic absence or staleness detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CheckedProcedureGrade {
    /// The procedure re-derived the outcome exactly.
    Checked,
}

/// A current affirmative attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestationPassFinding {
    /// Stable code, fixed to `attest.pass`.
    pub code: AttestationPassCode,
    /// Procedure grade, fixed to `attested`.
    pub grade: AttestedProcedureGrade,
    /// Domain-spoken rendering.
    pub message: String,
    /// The configured bounded question.
    pub question: Question,
    /// Identity of the stored head record.
    pub record_id: String,
    /// Identity of the record superseded by the head, when one exists.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub supersedes: Option<String>,
    /// The previous bounded answer, when the head supersedes a typed record.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub previous_answer: Option<AttestationAnswer>,
    /// Fingerprint captured by the stored head.
    pub stored_fingerprint: ContentHash,
    /// Fingerprint recomputed from the current compiled world.
    pub current_fingerprint: ContentHash,
}

/// A current negative attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestationFailFinding {
    /// Stable code, fixed to `attest.fail`.
    pub code: AttestationFailCode,
    /// Blocking severity, fixed to `error`.
    pub severity: AttestationErrorSeverity,
    /// Procedure grade, fixed to `attested`.
    pub grade: AttestedProcedureGrade,
    /// Domain-spoken rendering.
    pub message: String,
    /// The configured bounded question.
    pub question: Question,
    /// Identity of the stored head record.
    pub record_id: String,
    /// Identity of the record superseded by the head, when one exists.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub supersedes: Option<String>,
    /// The previous bounded answer, when the head supersedes a typed record.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub previous_answer: Option<AttestationAnswer>,
    /// Fingerprint captured by the stored head.
    pub stored_fingerprint: ContentHash,
    /// Fingerprint recomputed from the current compiled world.
    pub current_fingerprint: ContentHash,
}

/// A configured question with no committed answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestationMissingFinding {
    /// Stable code, fixed to `attest.missing`.
    pub code: AttestationMissingCode,
    /// Blocking severity, fixed to `error`.
    pub severity: AttestationErrorSeverity,
    /// Procedure grade, fixed to `checked`.
    pub grade: CheckedProcedureGrade,
    /// Domain-spoken rendering.
    pub message: String,
    /// The configured bounded question.
    pub question: Question,
    /// Fingerprint recomputed from the current compiled world.
    pub current_fingerprint: ContentHash,
}

/// A committed answer that requires re-attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestationStaleFinding {
    /// Stable code, fixed to `attest.stale`.
    pub code: AttestationStaleCode,
    /// Blocking severity, fixed to `error`.
    pub severity: AttestationErrorSeverity,
    /// Procedure grade, fixed to `checked`.
    pub grade: CheckedProcedureGrade,
    /// Domain-spoken rendering.
    pub message: String,
    /// The configured bounded question.
    pub question: Question,
    /// Identity of the stored head record.
    pub record_id: String,
    /// Identity of the record superseded by the head, when one exists.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub supersedes: Option<String>,
    /// The previous bounded answer, when the head supersedes a typed record.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub previous_answer: Option<AttestationAnswer>,
    /// Fingerprint captured by the stored head.
    pub stored_fingerprint: ContentHash,
    /// Fingerprint recomputed from the current compiled world.
    pub current_fingerprint: ContentHash,
}

impl AttestationCheckFinding {
    /// Whether this outcome permits the document verdict to pass.
    #[must_use]
    pub const fn passes(&self) -> bool {
        matches!(self, Self::Pass(_))
    }

    /// Procedure grade represented by this closed outcome.
    #[must_use]
    pub const fn procedure_grade(&self) -> Grade {
        match self {
            Self::Pass(_) | Self::Fail(_) => Grade::Attested,
            Self::Missing(_) | Self::Stale(_) => Grade::Checked,
        }
    }

    /// Stable finding code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Pass(_) => "attest.pass",
            Self::Fail(_) => "attest.fail",
            Self::Missing(_) => "attest.missing",
            Self::Stale(_) => "attest.stale",
        }
    }

    /// Domain-spoken message.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Pass(finding) => &finding.message,
            Self::Fail(finding) => &finding.message,
            Self::Missing(finding) => &finding.message,
            Self::Stale(finding) => &finding.message,
        }
    }
}

/// The closed bounded answer accepted by new attestation records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AttestationAnswer {
    /// The model affirms the bounded proposition.
    Yes,
    /// The model rejects the bounded proposition.
    No,
}

// -- dli.query ---------------------------------------------------------------

/// `dli.query` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryParams {
    /// The document id to query.
    pub document: String,
    /// The goal atom, rendered. The 0.6.3 typed successor is [`Self::goal_atom`];
    /// this string form is a registered field-level killed form, refused from
    /// contract 1.0 and still accepted — and still **required** — before it.
    ///
    /// `default` is what lets a 1.0 caller send [`Self::goal_atom`] alone: a
    /// retired field cannot also be a mandatory one. The pre-1.0 requirement is
    /// not dropped, only moved — `serve::required_retired_field` restores it
    /// verbatim on every `0.x` wire.
    #[serde(default)]
    pub goal: String,
    /// The goal as a typed atom — the 0.6.3 typed successor to [`Self::goal`],
    /// carried structurally instead of re-parsed from text. When present it is
    /// used and `goal` is ignored; when absent `goal` is parsed exactly as
    /// before. A `Term::Var` argument is a query variable (the string form's
    /// `?who`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goal_atom: Option<AtomExpr>,
    /// The standing floor for the returned rows (`certain` | `possible` | `all`).
    /// Absent means `all` — no floor. (`--layer`.)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub layer: Option<Layer>,
    /// Drop rows anchored strictly after this era or event. (`--as-of`.)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub as_of: Option<String>,
    /// Evaluate under a stored fork's valuation overlay, named by its handle; the
    /// server resolves the handle to the overlay. (`--fork`.)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fork: Option<String>,
    /// Attach the support antichain (`--why`) to every row.
    #[serde(default)]
    pub why: bool,
}

/// `dli.query` result.
///
/// Contract 0.6.0 adds the replay context `(root_hash, valuation)`: the pair
/// identifies exactly which world state — including any fork/as-of overlay
/// folded into the evaluation — the answer was computed against, so a
/// consumer can replay the triple `(root_hash, valuation, query)`. Both
/// fields are absent on a `0.5.1` request (dual-mode).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryResult {
    /// The domain-prefixed digest (`world:sha256:<hex>`) binding the compiled
    /// world the query ran against: its fact set, rules, interner, and the
    /// full effective valuation (fork/as-of overlays folded in). 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// The full effective valuation — every ruled atom with its boolean,
    /// sorted by atom name; absence from the list means unruled. 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub valuation: Option<Vec<ValuationEntry>>,
    /// Every row matching the query and its requested filters.
    pub rows: Vec<QueryResultRow>,
}

/// One ruled event variable in a wire valuation: the atom's resolved name and
/// the boolean it was ruled to. Explicit-false rulings are semantically
/// distinct from unruled atoms (a false kills a monomial; unruled leaves it
/// possible), so the wire form carries both — never a true-only list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValuationEntry {
    /// The resolved event-variable name.
    pub atom: String,
    /// The ruled boolean.
    pub value: bool,
}

/// One wire query row. It retains evaluator data and adds resolved citations
/// plus the deterministic line a text caller may display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryResultRow {
    /// The matched ground atom.
    pub atom: String,
    /// Query-variable bindings for this row.
    pub bindings: BTreeMap<String, String>,
    /// The atom's standing under the selected valuation.
    pub status: Status,
    /// The support formula.
    pub support: PosBool,
    /// The procedure grade.
    pub grade: Grade,
    /// Supporting variables resolved to stable corpus names.
    pub citations: Vec<String>,
    /// The optional support explanation requested with `why`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub why: Option<Why>,
    /// The kernel's deterministic one-line rendering of the row.
    pub rendered: String,
    /// The result-level `root_hash`, repeated per row so a single row is
    /// independently verifiable. 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// The result-level valuation, repeated per row. 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub valuation: Option<Vec<ValuationEntry>>,
}

// -- dli.fork.list -----------------------------------------------------------

/// `dli.fork.list` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkListParams {
    /// The document id.
    pub document: String,
}

/// `dli.fork.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkListResult {
    /// The open forks and their current private-state summary.
    pub forks: Vec<ForkSummary>,
}

/// One open fork as exposed to callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkSummary {
    /// The stable wire handle.
    pub fork: String,
    /// The parent wire handle, when this fork branches another.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub base: Option<String>,
    /// Named valuation assumptions local to the fork.
    pub assumes: BTreeMap<String, bool>,
    /// Number of fork-local proposed facts.
    pub local_atoms: usize,
    /// Number of active visibility restrictions.
    pub restrictions: usize,
}

// -- dli.fork.new ------------------------------------------------------------

/// `dli.fork.new` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkNewParams {
    /// The document id the fork is over.
    pub document: String,
    /// The base fork to branch from, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub base: Option<String>,
}

/// `dli.fork.new` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkNewResult {
    /// The new fork handle.
    pub fork: String,
}

// -- dli.fork.diff -----------------------------------------------------------

/// `dli.fork.diff` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkDiffParams {
    /// The fork handle.
    pub fork: String,
}

// `dli.fork.diff` result is [`DiffDelta`].

// -- dli.fork.suppose --------------------------------------------------------

/// `dli.fork.suppose` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkSupposeParams {
    /// The fork handle.
    pub fork: String,
    /// The option/event ids to assume true.
    pub assume: Vec<String>,
}

// `dli.fork.suppose` result is [`DiffDelta`].

// -- dli.fork.add ------------------------------------------------------------

/// `dli.fork.add` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkAddParams {
    /// The fork handle.
    pub fork: String,
    /// The rendered proposed declarations to add. The 0.6.3 typed successor is
    /// [`Self::additions`]; this string form is a registered field-level killed
    /// form, refused from contract 1.0 and still accepted — and still
    /// **required** — before it.
    ///
    /// `default` is what lets a 1.0 caller send [`Self::additions`] alone; the
    /// pre-1.0 requirement is restored verbatim by
    /// `serve::required_retired_field`.
    #[serde(default)]
    pub declarations: Vec<String>,
    /// The proposed facts as typed ground atoms — the 0.6.3 typed successor to
    /// [`Self::declarations`]. When present it is used and `declarations` is
    /// ignored; when absent the declaration lines are parsed and lowered
    /// exactly as before.
    ///
    /// A ground atom is the whole of what the string path can express: both a
    /// fact line and a slot line lower to a ground fact, and every lowered
    /// addition is asserted by fiat with an unconditional annotation, so the
    /// typed form reaches exactly the same additions the text form did.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub additions: Option<Vec<GroundAtom>>,
}

/// `dli.fork.add` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkAddResult {
    /// Whether the declarations were accepted into the fork.
    pub ok: bool,
    /// The resulting consequence delta.
    pub delta: DiffDelta,
}

// -- dli.fork.suppress -------------------------------------------------------

/// `dli.fork.suppress` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkSuppressParams {
    /// The fork handle.
    pub fork: String,
    /// The assumption id to retract.
    pub assumption: String,
}

// `dli.fork.suppress` result is [`DiffDelta`].

// -- dli.fork.restrict -------------------------------------------------------

/// `dli.fork.restrict` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkRestrictParams {
    /// The fork handle.
    pub fork: String,
    /// The atoms the fork's valuation scope is restricted to.
    pub scope: Vec<String>,
}

// `dli.fork.restrict` result is [`DiffDelta`].

// -- dli.fork.preflight ------------------------------------------------------

/// `dli.fork.preflight` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkPreflightParams {
    /// The fork handle.
    pub fork: String,
}

/// `dli.fork.preflight` result: the ratification pre-flight scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkPreflightResult {
    /// The candidate valuation, keyed by stable corpus variable name.
    pub candidate: BTreeMap<String, bool>,
    /// Conflicts the candidate valuation would introduce.
    pub conflicts: Vec<DiffConflict>,
    /// The atoms the ratification would affect.
    pub affected: Vec<String>,
}

// -- dli.fork.promote --------------------------------------------------------

/// `dli.fork.promote` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForkPromoteParams {
    /// The fork handle.
    pub fork: String,
}

/// `dli.fork.promote` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkPromoteResult {
    /// The emitted ruling-request draft.
    pub rq_draft: RqDraft,
}

// -- dli.check-proof ---------------------------------------------------------

/// `dli.check-proof` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckProofParams {
    /// The document id.
    pub document: String,
    /// The proof declaration to replay. May be omitted only when the document
    /// declares exactly one proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<String>,
}

/// `dli.check-proof` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckProofResult {
    /// The rich proof report: per-step L1–L7 verdicts, the reached verdict, the
    /// declared expectation, and the weakest-link grade.
    pub report: ProofReport,
}

// -- dli.check-draft ---------------------------------------------------------

/// `dli.check-draft` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckDraftParams {
    /// The project document the draft belongs to.
    pub document: String,
    /// The complete structured shadow to check.
    pub payload: Payload,
    /// Caller-owned current/prior episode state for cross-episode continuity.
    /// This context is checked but never committed as part of `payload/v1`.
    #[serde(default)]
    pub continuity: Option<RuntimeContinuity>,
    /// The draft prose the payload quotes and hashes.
    pub source: String,
}

/// `dli.check-draft` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckDraftResult {
    /// The merged, deterministically ordered payload battery.
    pub findings: Vec<PayloadFinding>,
}

// -- dli.simulate.start ------------------------------------------------------

/// `dli.simulate.start` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulateStartParams {
    /// The logical document or game id.
    pub document: String,
    /// The RNG seed.
    pub seed: u64,
    /// The number of trials to simulate.
    pub trials: u64,
    /// Trials folded between durable checkpoints.
    #[schemars(range(min = 1))]
    pub checkpoint_every: u64,
}

/// `dli.simulate.start` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateStartResult {
    /// The job id to poll.
    pub job_id: String,
    /// The initial job status.
    pub status: SimulateJobStatus,
}

/// A durable simulation job's lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SimulateJobStatus {
    /// Reserved and ready to advance.
    Pending,
    /// At least one checkpoint has been committed.
    Running,
    /// A final manifest has been committed.
    Complete,
    /// Cancellation was committed before completion.
    Cancelled,
}

// -- dli.simulate.status -----------------------------------------------------

/// `dli.simulate.status` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulateStatusParams {
    /// The job id.
    pub job_id: String,
}

/// `dli.simulate.status` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateStatusResult {
    /// The content-addressed job id.
    pub job_id: String,
    /// The job status.
    pub status: SimulateJobStatus,
    /// Trials durably folded so far.
    pub trials_done: u64,
    /// Total trials requested.
    pub trials_total: u64,
    /// The final manifest, present once the job is complete.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub manifest: Option<RunManifest>,
    /// JCS SHA-256 of the final manifest.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub manifest_digest: Option<String>,
}

// -- dli.simulate.cancel -----------------------------------------------------

/// `dli.simulate.cancel` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimulateCancelParams {
    /// The job id.
    pub job_id: String,
}

/// `dli.simulate.cancel` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateCancelResult {
    /// The content-addressed job id.
    pub job_id: String,
    /// The job status after cancellation.
    pub status: SimulateJobStatus,
}

// -- dli.timeline.* ----------------------------------------------------------

/// `dli.timeline.show` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelineShowParams {
    /// The document id.
    pub document: String,
    /// The position to show the timeline at, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub at: Option<String>,
}

/// `dli.timeline.order` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelineOrderParams {
    /// The document id.
    pub document: String,
}

/// `dli.timeline.knows` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelineKnowsParams {
    /// The document id.
    pub document: String,
    /// The position to evaluate knowledge at, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub at: Option<String>,
}

/// `dli.timeline.episodes` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelineEpisodesParams {
    /// The document id.
    pub document: String,
}

/// `dli.timeline.phases` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelinePhasesParams {
    /// The document id.
    pub document: String,
    /// The episode to walk phases for.
    pub episode: String,
}

/// One topological layer in a chronology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineLayer {
    /// Zero-based topological rank.
    pub rank: u32,
    /// Era and event names in this layer.
    pub points: Vec<String>,
}

/// One directed chronology edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEdge {
    /// Earlier point.
    pub before: String,
    /// Later point.
    pub after: String,
}

/// The relation of every known point to a requested cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineCursor {
    /// The requested point.
    pub at: String,
    /// Points known to precede it.
    pub before: Vec<String>,
    /// Points known to follow it.
    pub after: Vec<String>,
    /// Points with no declared order relative to it.
    pub unordered: Vec<String>,
}

/// `dli.timeline.show` result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineShowResult {
    /// Topological layers; empty when the chronology is cyclic.
    pub layers: Vec<TimelineLayer>,
    /// Cycle blame edges, if the chronology is cyclic.
    pub cycle: Vec<TimelineEdge>,
    /// Cursor-relative ordering, when `at` was supplied and the chronology is acyclic.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cursor: Option<TimelineCursor>,
    /// Episode fact placements projected onto story-time.
    pub placements: Vec<Placement>,
}

/// One unordered pair in a partial chronology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelinePair {
    /// The first point.
    pub left: String,
    /// The second point.
    pub right: String,
}

/// `dli.timeline.order` result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineOrderResult {
    /// Whether no cycle exists.
    pub acyclic: bool,
    /// Cycle blame edges, when not acyclic.
    pub cycle: Vec<TimelineEdge>,
    /// Every pair for which neither order is declared.
    pub unordered: Vec<TimelinePair>,
}

/// One row of the who-knows-what matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeRow {
    /// Listener or named cast knower.
    pub knower: Knower,
    /// Facts known at the requested production slot.
    pub facts: Vec<String>,
}

/// `dli.timeline.knows` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineKnowsResult {
    /// Production slot queried.
    pub at: u32,
    /// Complete matrix rows for the listener and every season-declared cast knower.
    pub knowers: Vec<KnowledgeRow>,
}

/// One episode's production/story-time summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EpisodeSummary {
    /// Episode id.
    pub episode: String,
    /// Listener-facing production slot.
    pub production_order: u32,
    /// Story-time anchor.
    pub story_anchor: EpisodeAnchor,
}

/// `dli.timeline.episodes` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEpisodesResult {
    /// Episodes in production order.
    pub episodes: Vec<EpisodeSummary>,
    /// Final open/closed loop ledger.
    pub loops: OpenLoopLedger,
    /// Duplicate and gapped production-slot findings.
    pub integrity: Vec<OrderIntegrityFinding>,
    /// Earliest/latest legal production slots.
    pub slack: Vec<EpisodeSlack>,
}

/// One cyclic phase pinned by an episode or scene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PhaseAnchor {
    /// `episode` or `scene:<id>`.
    pub owner: String,
    /// Declared cyclic position.
    pub position: CyclePos,
    /// Resolved spoke label, absent when the cycle or index is invalid.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub spoke: Option<String>,
    /// Whether the cycle exists and the index is in range.
    pub valid: bool,
}

/// `dli.timeline.phases` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelinePhasesResult {
    /// Episode inspected.
    pub episode: String,
    /// Whether every pinned phase is valid.
    pub consistent: bool,
    /// Episode and scene anchors.
    pub anchors: Vec<PhaseAnchor>,
    /// Domain-spoken consistency findings.
    pub findings: Vec<String>,
}

// -- dli.export.run ----------------------------------------------------------

/// `dli.export.run` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportRunParams {
    /// The document id.
    pub document: String,
    /// The export profile.
    pub profile: ExportProfile,
    /// Optional qualified profile name from the caller project's catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
}

/// `dli.export.run` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportRunResult {
    /// The target-shaped emitted artifact.
    pub emission: ExportEmission,
    /// The export manifest.
    pub manifest: ExportManifest,
}

/// The target-shaped artifacts available through this worker contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "profile", content = "artifact", rename_all = "kebab-case")]
pub enum ExportEmission {
    /// A context pack.
    ContextPack(ContextPack),
    /// A rendered card deck.
    Cards(CardDeck),
    /// A Ludii program and differential status.
    LudiiGdl(LudiiExport),
    /// Articy-shaped video-game JSON.
    Articy(ArticyExport),
    /// Producer-facing CSV.
    Csv(CsvExport),
    /// Static Markdown wiki bundle.
    StaticWiki(StaticWikiExport),
}

// -- dli.attestation-sweep ---------------------------------------------------

/// `dli.attestation-sweep` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestationSweepParams {
    /// The project document whose demand these answers satisfy.
    pub document: String,
    /// New records to append transactionally.
    #[schemars(length(min = 1))]
    pub records: Vec<AttestationRecord>,
}

impl AttestationAnswer {
    /// The canonical wire and storage token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
        }
    }
}

impl fmt::Display for AttestationAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for AttestationAnswer {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "yes" => Ok(Self::Yes),
            "no" => Ok(Self::No),
            _ => Err(()),
        }
    }
}

/// A new attestation accepted at the wire boundary.
///
/// The durable version-1 record retains a string answer so pre-`0.4.1`
/// evidence can still be loaded. This request type closes all new writes to
/// `yes` or `no`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestationRecord {
    /// The bounded question this record answers.
    pub question: Question,
    /// The bounded answer.
    pub answer: AttestationAnswer,
    /// SHA-256 identity of the model rationale.
    pub rationale_hash: String,
    /// The model that produced the judgment.
    pub model: ModelRef,
    /// The prompt posed to the model.
    pub prompt: PromptRef,
    /// Sampling parameters and agreement.
    pub params: Sampling,
    /// Re-attestation policy.
    pub policy: Policy,
    /// Record id superseded within this question chain.
    pub supersedes: Option<String>,
    /// Sweep manifest reference.
    pub sweep: String,
    /// Fingerprint of the world content judged.
    pub fingerprint: ContentHash,
}

impl AttestationRecord {
    /// Convert a validated wire record into the backward-compatible durable
    /// representation.
    #[must_use]
    pub fn into_stored(self) -> Attestation {
        Attestation {
            question: self.question,
            answer: self.answer.as_str().to_owned(),
            rationale_hash: self.rationale_hash,
            model: self.model,
            prompt: self.prompt,
            params: self.params,
            policy: self.policy,
            supersedes: self.supersedes,
            sweep: self.sweep,
            fingerprint: self.fingerprint,
        }
    }
}

/// `dli.attestation-sweep` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestationSweepResult {
    /// Number of records accepted.
    pub accepted: usize,
    /// SHA-256 address of the immutable append batch.
    pub batch_digest: String,
}

// -- dli.payload-submit ------------------------------------------------------

/// `dli.payload-submit` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PayloadSubmitParams {
    /// The project document the draft belongs to.
    pub document: String,
    /// The complete structured shadow to validate and store.
    pub payload: Payload,
    /// The draft prose the payload quotes and hashes.
    pub source: String,
}

/// `dli.payload-submit` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PayloadSubmitResult {
    /// Whether the payload was accepted.
    pub accepted: bool,
    /// Findings raised during acceptance.
    pub findings: Vec<PayloadFinding>,
    /// SHA-256 address of the immutable payload, when accepted.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub payload_digest: Option<String>,
}

// -- dli.version -------------------------------------------------------------

/// `dli.version` params (empty).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VersionParams {}

/// `dli.version` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VersionResult {
    /// The contract name.
    pub contract: String,
    /// The contract version.
    pub contract_version: String,
    /// The server version.
    pub server: String,
    /// The wire op names this build actually implements — capability negotiation
    /// (spec `10-serving.md` §2), so a consumer can degrade rather than call an op
    /// the running build has not wired. Defaulted for N-1 payloads without it.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

// -- dli.name.check ----------------------------------------------------------

/// `dli.name.check` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NameCheckParams {
    /// The proposed name.
    pub name: String,
    /// The dialect to vet against.
    pub dialect: String,
}

/// `dli.name.check` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NameCheckResult {
    /// Whether the name is admissible.
    pub ok: bool,
    /// Findings from the kernel-half dialect and phonotactic vet.
    pub findings: Vec<Finding>,
}

// -- dli.attest.missing ------------------------------------------------------

/// `dli.attest.missing` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestMissingParams {
    /// The document id.
    pub document: String,
}

/// `dli.attest.missing` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestMissingResult {
    /// Questions lacking a current durable answer.
    pub questions: Vec<AttestationDemand>,
}

/// Why a configured attestation question is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AttestationDemandReason {
    /// No record has ever answered this question.
    Missing,
    /// The current record's subject fingerprint no longer matches.
    Stale,
}

/// One model-judgment request emitted by the deterministic kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttestationDemand {
    /// The full bounded question to pose.
    pub question: Question,
    /// The fingerprint an accepted record must carry.
    pub fingerprint: ContentHash,
    /// Why the question is in the demand queue.
    pub reason: AttestationDemandReason,
}

// -- author.open -------------------------------------------------------------

/// `author.open` params. Exactly one of `scope` or `fork` names the mode:
/// staging over an existing scope, or fork-scoped exploration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorOpenParams {
    /// Stage over an existing scope.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scope: Option<String>,
    /// Open a fork-scoped workspace, named by its question.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fork: Option<String>,
}

/// `author.open` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorOpenResult {
    /// The opened workspace id.
    pub workspace: String,
    /// The initial workspace digest.
    pub workspace_digest: String,
}

// -- author.status -----------------------------------------------------------

/// `author.status` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorStatusParams {
    /// The workspace id.
    pub workspace: String,
}

/// `author.status` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorStatusResult {
    /// The rendered workspace state.
    pub rendered: Vec<String>,
    /// The lorestore commit the workspace was opened against — pinned at
    /// `author.open`, advanced by this workspace's own `materialize`, never
    /// the moving global head. Absent on a `0.5.1` request or when no
    /// lorestore is attached. 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// Structured per-fact diff rows with before/after standing — the
    /// machine-readable review surface. 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diff_rows: Option<Vec<AuthorDiffRow>>,
    /// Every current graded finding over the staged world.
    pub findings: Vec<Finding>,
    /// The current workspace digest.
    pub workspace_digest: String,
}

// -- author.diff -------------------------------------------------------------

/// `author.diff` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorDiffParams {
    /// The workspace id.
    pub workspace: String,
}

/// `author.diff` result.
///
/// Contract 0.6.0 adds `root_hash` (the pinned base commit this change-set is
/// relative to) and `rows` (structured per-fact diffs) alongside the rendered
/// lines and findings. Accept/reject semantics: `author.materialize` is the
/// accept decision for the reviewed change-set; `author.discard` (with its
/// per-declaration `decl` parameter) is the reject decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorDiffResult {
    /// The rendered diff lines.
    pub rendered: Vec<String>,
    /// The lorestore commit this diff is relative to — pinned at
    /// `author.open`. Absent on a `0.5.1` request or without a lorestore.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// Structured per-fact diffs with before/after standing — the
    /// machine-readable review surface. 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rows: Option<Vec<AuthorDiffRow>>,
    /// Every current graded finding over the staged world.
    pub findings: Vec<Finding>,
    /// The base-to-staged derived-consequence diff.
    pub consequences: DiffDelta,
    /// The current workspace digest.
    pub workspace_digest: String,
}

/// One structured row in the review diff: a fact-level change with
/// before/after standing. The consumer renders this for review; accepting is
/// `author.materialize`, rejecting is `author.discard` (whole workspace or
/// per-declaration via `decl`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorDiffRow {
    /// The atom this row describes, rendered `pred(a, b)`.
    pub atom: String,
    /// The before standing — absent for a newly-introduced fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<AuthorDiffStanding>,
    /// The after standing — absent for a retracted fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<AuthorDiffStanding>,
    /// Whether the row represents a newly-introduced fact.
    pub introduced: bool,
    /// Whether the row represents a retracted fact.
    pub retracted: bool,
    /// Deterministic rendering for the human review surface.
    pub rendered: String,
}

/// A fact's standing at a point in time: its provenance, grade, and status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorDiffStanding {
    /// The support antichain as resolved names: one inner list per monomial
    /// (an alternative derivation), each literal signed `+name`/`-name` —
    /// citations, hypothesis variables, and proposal-bundle variables alike.
    /// Never filtered: a gated claim must never render as unconditional.
    pub support: Vec<Vec<String>>,
    /// The procedure grade.
    pub grade: Grade,
    /// The atom's standing: `certain` | `possible` | `dead`.
    pub status: String,
}

// -- author.discard ----------------------------------------------------------

/// `author.discard` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorDiscardParams {
    /// The workspace id.
    pub workspace: String,
    /// The single staged declaration to discard, named by its rendered canonical
    /// line. Omit to discard the whole workspace.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub decl: Option<String>,
}

/// `author.discard` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorDiscardResult {
    /// Whether the workspace was discarded.
    pub discarded: bool,
    /// The workspace digest after a single-declaration discard; absent when the
    /// whole workspace was discarded.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub workspace_digest: Option<String>,
}

// -- author.materialize ------------------------------------------------------

/// `author.materialize` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorMaterializeParams {
    /// The workspace id.
    pub workspace: String,
    /// Opt into the `.dli` rendering in the result. Contract 0.6.0 returns
    /// the committed fact objects as JSON by default (the JSON-first return);
    /// the rendered lines are produced only on request. Ignored (always
    /// rendered) on a `0.5.1` request.
    #[serde(default)]
    pub rendered: bool,
}

// -- author.add_entity -------------------------------------------------------

/// `author.add_entity` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddEntityParams {
    /// The workspace id.
    pub workspace: String,
    /// The entity kind (person/place/thing/era).
    pub kind: EntityKind,
    /// The entity id.
    pub id: String,
    /// The entity's display name (treated as data, never as an instruction).
    pub name: String,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

// -- author.add_fact ---------------------------------------------------------

/// `author.add_fact` params: an attribute-intent fact that renders
/// `entity.key = value` (e.g. `mara.home = the-shallows`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddFactParams {
    /// The workspace id.
    pub workspace: String,
    /// The entity the attribute is set on.
    pub entity: String,
    /// The attribute key.
    pub key: String,
    /// The attribute value.
    pub value: String,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

// -- author.add_relation -----------------------------------------------------

/// `author.add_relation` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddRelationParams {
    /// The workspace id.
    pub workspace: String,
    /// The relation name.
    pub relation: String,
    /// The source entity id.
    pub from: String,
    /// The target entity id.
    pub to: String,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

// -- author.add_event --------------------------------------------------------

/// `author.add_event` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddEventParams {
    /// The workspace id.
    pub workspace: String,
    /// The event id.
    pub id: String,
    /// Optional event display name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    /// The era that contains the event.
    pub era: String,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

// -- author.add_before -------------------------------------------------------

/// `author.add_before` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddBeforeParams {
    /// The workspace id.
    pub workspace: String,
    /// The earlier event id.
    pub earlier: String,
    /// The later event id.
    pub later: String,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

// -- author.open_question ----------------------------------------------------

/// `author.open_question` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorOpenQuestionParams {
    /// The workspace id.
    pub workspace: String,
    /// The question id.
    pub id: String,
    /// The question prompt.
    pub prompt: String,
}

// -- author.add_option -------------------------------------------------------

/// `author.add_option` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddOptionParams {
    /// The workspace id.
    pub workspace: String,
    /// The question id this option belongs to.
    pub question: String,
    /// The option id.
    pub id: String,
    /// The option label.
    pub label: String,
}

// -- author.add_hypothesis ---------------------------------------------------

/// `author.add_hypothesis` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddHypothesisParams {
    /// The workspace id.
    pub workspace: String,
    /// The hypothesis id.
    pub id: String,
    /// The hypothesis statement.
    pub statement: String,
}

// -- author.suppose ----------------------------------------------------------

/// `author.suppose` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorSupposeParams {
    /// The workspace id.
    pub workspace: String,
    /// The hypothesis ids to pin true.
    pub hypotheses: Vec<String>,
}

// -- author.query (read) -----------------------------------------------------

/// `author.query` params: query the **staged** world (base + staged
/// declarations). The agent asks "what holds about `mara` now?" over its
/// in-progress work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorQueryParams {
    /// The workspace id.
    pub workspace: String,
    /// The goal atom, rendered `pred(a, b)`. An argument written `?` or `?name`
    /// is a query variable that enumerates matches (`serves(mara, ?who)`); a bare
    /// symbol is a ground constant. The 0.6.3 typed successor is
    /// [`Self::goal_atom`]; this string form is a registered field-level killed
    /// form, refused from contract 1.0 and still accepted — and still
    /// **required** — before it.
    ///
    /// `default` is what lets a 1.0 caller send [`Self::goal_atom`] alone; the
    /// pre-1.0 requirement is restored verbatim by
    /// `serve::required_retired_field`.
    #[serde(default)]
    pub goal: String,
    /// The goal as a typed atom — the 0.6.3 typed successor to [`Self::goal`],
    /// carried structurally instead of re-parsed from text. When present it is
    /// used and `goal` is ignored; when absent `goal` is parsed exactly as
    /// before. A `Term::Var` argument is a query variable (the string form's
    /// `?who`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goal_atom: Option<AtomExpr>,
    /// The standing floor for the returned rows (`certain` | `possible` | `all`).
    /// Absent means `all` — no floor.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub layer: Option<Layer>,
    /// Attach the support antichain (`why`) to every row.
    #[serde(default)]
    pub why: bool,
}

/// `author.query` result: one row per match over the staged world, each carrying
/// its support, grade, standing, and (when asked) its why-antichain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorQueryResult {
    /// The workspace digest the query was evaluated against (`jcs:sha256:`
    /// regime — the staged world is workspace state, not a store commit).
    /// 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// The full effective valuation of the staged world — every ruled atom
    /// with its boolean, sorted by name. 0.6.0 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub valuation: Option<Vec<ValuationEntry>>,
    /// The answer rows over the staged world.
    pub rows: Vec<QueryRow>,
}

// -- author.check (read) -----------------------------------------------------

/// `author.check` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorCheckParams {
    /// The workspace id.
    pub workspace: String,
}

/// `author.check` result: the full-workspace check battery (continuity +
/// factcheck + rejected-lore) over the staged world, one graded finding list of
/// the same shape the mutating tools return. The agent verifies its whole draft
/// before materialize.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorCheckResult {
    /// Whether the staged world is clean of error-severity findings (the
    /// materialize gate).
    pub ok: bool,
    /// The graded findings raised over the staged world (data, never
    /// instructions), deduplicated.
    pub findings: Vec<Finding>,
}

// -- author.why (read) -------------------------------------------------------

/// `author.why` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorWhyParams {
    /// The workspace id.
    pub workspace: String,
    /// The atom to explain, rendered `pred(a, b)` (ground).
    pub atom: String,
}

/// `author.why` result: the actionable explanation of a staged atom's standing —
/// the blocking rulings and defeater (why certain / why dead / why possible),
/// plus the support antichain when the atom is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorWhyResult {
    /// The negative explanation: what stands between the atom and certainty (the
    /// closest support, the unruled variables blocking it, the defeater).
    pub why_not: WhyNot,
    /// The positive support antichain, present when the world derives the atom.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub why: Option<Why>,
}

// -- author.findings (read) --------------------------------------------------

/// `author.findings` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorFindingsParams {
    /// The workspace id.
    pub workspace: String,
}

/// `author.findings` result: the workspace's open findings partitioned by
/// severity — the standing pre-flight of what `author.materialize` would refuse
/// on, each with its typed repair data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorFindingsResult {
    /// Error-severity findings — these bar materialization.
    pub errors: Vec<Finding>,
    /// Warn-severity findings — advisory; they pass materialization through.
    pub warnings: Vec<Finding>,
    /// Whether the workspace could materialize now (no error-severity findings).
    pub materializable: bool,
}

// -- author.list_workspaces (read) -------------------------------------------

/// `author.list_workspaces` params (empty).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorListWorkspacesParams {}

/// The authoring door a workspace was opened in, on the wire. Mirrors the
/// internal `author::workspace::Mode` but is a wire-local type so the contract
/// layer stays independent of the workspace engine's internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceMode {
    /// `{"scope": …}` — staging over an existing scope.
    Scope,
    /// `{"fork": …}` — a fork-scoped workspace.
    Fork,
}

/// One open workspace's digest line — enough for an agent to recover context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceDigest {
    /// The workspace id.
    pub workspace: String,
    /// The scope or fork name the staging is over.
    pub base: String,
    /// The door the workspace was opened in.
    pub mode: WorkspaceMode,
    /// How many declarations are staged.
    pub staged: usize,
    /// The current workspace digest.
    pub workspace_digest: String,
}

/// `author.list_workspaces` result: every open authoring workspace and its
/// digest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorListWorkspacesResult {
    /// The open workspaces, in id order.
    pub workspaces: Vec<WorkspaceDigest>,
}

// -- author.branch (LS-6, contract 0.6.1) ------------------------------------

/// The branch action: create a new branch or list the existing ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BranchAction {
    /// Create a new branch at the source branch's head.
    Create,
    /// List every branch.
    List,
}

/// `author.branch` params. `name`/`from` apply to `create`; `list` ignores them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorBranchParams {
    /// The workspace whose lorestore the branch lives in.
    pub workspace: String,
    /// Create a branch or list branches.
    pub action: BranchAction,
    /// The branch name to create (required for `create`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    /// The source branch to create from (default `main`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub from: Option<String>,
}

/// `author.branch` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorBranchResult {
    /// Every branch name after the action, `main` first.
    pub branches: Vec<String>,
    /// The branch just created (`create` only).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created: Option<String>,
}

// -- author.merge (LS-6, contract 0.6.1) -------------------------------------

/// `author.merge` params: merge `source` into `target` (default `main`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorMergeParams {
    /// The workspace whose lorestore the branches live in.
    pub workspace: String,
    /// The branch merged in.
    pub source: String,
    /// The branch merged into (default `main`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target: Option<String>,
}

/// One atom whose standing changed downward in the merged world vs. a pre-merge
/// branch — the stratified-negation effect surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MergeStandingChange {
    /// The affected atom, rendered `pred(a, b)`.
    pub atom: String,
    /// Which pre-merge branch the atom was stronger on.
    pub branch: String,
    /// Its standing on that branch.
    pub before: String,
    /// Its standing in the merged world.
    pub after: String,
}

/// `author.merge` result: the merge commit hash and a structural conflict report
/// across the three classes. The fact-set union is confluent, so the merge
/// commit is always produced; a non-empty `conflicts`/`standing_changes` is the
/// sovereign's to rule on (rendering is consumer-owned, DL-0043).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorMergeResult {
    /// The lorestore commit hash of the merge commit.
    pub merge_commit: String,
    /// The merge base (lowest common ancestor), when the branches share history.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub merge_base: Option<String>,
    /// The structural conflicts — valuation disagreement (`kind` = valuation),
    /// exclusivity, and functional-dependency / certain-layer (`kind` = certain).
    pub conflicts: Vec<DiffConflict>,
    /// Graded findings over the recompiled merged world, with repair data.
    pub findings: Vec<Finding>,
    /// Whether the merged rule set stratifies (`false` = a negation cycle).
    pub stratifies: bool,
    /// Atoms whose standing changed downward vs. a pre-merge branch.
    pub standing_changes: Vec<MergeStandingChange>,
}
