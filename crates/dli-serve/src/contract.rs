//! Per-op parameter and result types.
//!
//! Every op has a real params struct and a real result struct, all deriving
//! `JsonSchema`. Mutating `author.*` ops share [`AuthorMutationResult`].

use std::collections::BTreeMap;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::language::{CompositionStep, DialectItem};
use dli_ast::literal::BodyLiteral;
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
use dli_ledger::ProofReport;
use dli_time::cycle::CyclePos;
use dli_time::fluent::{Projection, TemporalCover};
use dli_time::slack::EpisodeSlack;
use dli_world::attest::{
    Attestation, ContentHash, ModelRef, Policy, PromptRef, Question, Sampling,
};
use dli_world::episode::{EpisodeAnchor, Knower, OpenLoopLedger, Placement};
use dli_world::episode_order::OrderIntegrityFinding;

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

/// A conflict carried by either a fork diff or an authoring-seam staged-world diff.
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
    /// World and attestation findings, if any.
    pub findings: Vec<CheckFinding>,
}

/// A document-wide check finding without flattening domain-specific evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "domain", rename_all = "lowercase")]
pub enum CheckFinding {
    /// A world/compiler finding.
    World(Finding),
    /// A durable attestation dependency and its pass/fail state.
    Attestation(AttestationCheckFinding),
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
    /// Retired text form; use [`Self::goal_atom`].
    #[serde(default)]
    pub goal: String,
    /// The structured goal atom; a `Term::Var` argument is a query variable.
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
/// The replay context `(root_hash, valuation)` identifies the world state,
/// including any fork or as-of overlay, used to compute the answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryResult {
    /// The domain-prefixed digest (`world:sha256:<hex>`) binding the compiled
    /// world the query ran against: its fact set, rules, interner, and the
    /// full effective valuation (fork/as-of overlays folded in).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// The full effective valuation — every ruled atom with its boolean,
    /// sorted by atom name; absence from the list means unruled.
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
    /// independently verifiable.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// The result-level valuation, repeated per row.
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
    /// Retired text form; use [`Self::additions`].
    #[serde(default)]
    pub declarations: Vec<String>,
    /// Proposed facts as typed ground atoms, asserted in the fork.
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
    /// Optional era or event at which to project temporal states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
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
    /// Articy-shaped video-game JSON.
    Articy(ArticyExport),
    /// Producer-facing CSV.
    Csv(CsvExport),
    /// Static Markdown wiki bundle.
    StaticWiki(StaticWikiExport),
}

// -- Attestation types ------------------------------------------------------

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
    /// Optional authoring workspace whose base and staged dialects are used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
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
    /// Open against a named branch head instead of `main`. Requires `scope`;
    /// branch and fork cannot be combined.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub branch: Option<String>,
}

/// `author.open` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorOpenResult {
    /// The opened workspace id.
    pub workspace: String,
    /// The initial workspace digest.
    pub workspace_digest: String,
    /// The branch supplied when opening the workspace, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub branch: Option<String>,
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
    /// the moving global head. Absent when no lorestore is attached.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// Structured per-fact diff rows with before/after standing — the
    /// machine-readable review surface.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diff_rows: Option<Vec<AuthorDiffRow>>,
    /// The staged declarations and the structural key that names each one —
    /// what `author.discard` takes to reject a single declaration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub staged: Option<Vec<StagedDecl>>,
    /// Every current graded finding over the staged world.
    pub findings: Vec<Finding>,
    /// The current workspace digest.
    pub workspace_digest: String,
}

/// One staged declaration, named. The row an agent reads to reject a single
/// declaration without discarding its whole workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StagedDecl {
    /// The structural key: `jcs:sha256:<hex>` over the declaration's canonical
    /// JCS bytes — what `author.discard`'s `decl` accepts from contract 1.3.0.
    ///
    /// **What it is stable against**, and what it is not, because a consumer
    /// reading this decides how long to hold one. JCS fixes the *encoding*: keys
    /// sort and the number/string forms are specified, so the key does not
    /// depend on the order this build's structs happen to declare their fields,
    /// on which serializer produced the bytes, or on which build is running —
    /// a default build and a `syntax` build compute the same key for the same
    /// declaration, and it is recomputable by anyone holding the declaration.
    ///
    /// It is **not** stable across a schema change that adds a field which
    /// serializes. A new field is content, and content is what the key hashes —
    /// including a field that serializes as `null`, which is what an `Option`
    /// without `skip_serializing_if` does when absent, and none of the
    /// declaration types carry that attribute on their optional fields today.
    /// Only a field elided entirely from the bytes leaves the key unmoved.
    ///
    /// So: a key is valid **within a contract version**. Hold it across a call,
    /// not across an upgrade — re-read `author.status` after negotiating a new
    /// version rather than persisting keys through one.
    pub key: String,
    /// The declaration's kind (`entity`, `fact`, `dialect`, …) — enough to read
    /// the list without a formatter.
    pub kind: String,
    /// The canonical `.dli` line, when this build links a formatter. Absent on a
    /// default build, where [`Self::key`] is the only name a declaration has.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rendered: Option<String>,
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
/// `root_hash` pins the base commit this change-set is relative to; `rows`
/// provide structured per-fact diffs alongside the rendered
/// lines and findings. Accept/reject semantics: `author.materialize` is the
/// accept decision for the reviewed change-set; `author.discard` (with its
/// per-declaration `decl` parameter) is the reject decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorDiffResult {
    /// The rendered diff lines.
    pub rendered: Vec<String>,
    /// The base commit this diff is relative to, initially pinned at
    /// `author.open` and advanced by this workspace's own materialize.
    /// Absent without a lorestore.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// Structured per-fact diffs with before/after standing — the
    /// machine-readable review surface.
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
    /// The single staged declaration to discard, named by its structural key
    /// from `author.status` or, when a renderer is available, by its rendered
    /// canonical line. Omit to discard the whole workspace.
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
    /// For a lorestore-backed workspace, opt into the `.dli` rendering in the
    /// result. Committed fact objects are returned as JSON by default. A
    /// workspace without a lorestore returns rendered lines when a renderer is
    /// available and otherwise refuses.
    #[serde(default)]
    pub rendered: bool,
    /// When committing staged facts to a lorestore, record this defeasible
    /// decision and stamp previously uncited facts `Ratified` under it.
    /// Existing citations and declaration objects are preserved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
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
/// `entity.key = value` (e.g. `nara.home = riverside`).
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
    /// A ground claim supported by this option while the question is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implies: Option<AtomExpr>,
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

// -- the DM-4 expert authoring surface (contract 1.3.0) ----------------------
//
// Wave 1 (`dialect`, `name_part`, `name`, `term`, `banned`) is the consumer's
// actual corpus; tier 2 (`state`, `attest`) is their episode-continuity and
// question-ledger flow. Each params struct is a typed constructor over an AST
// declaration that already exists — no new staging machinery (DL-0058).
//
// **Span-free by construction.** The AST's LS-9 typed interiors pair each typed
// `item` with the normalized `Clause` text and span the parser captured for
// exact round-trip. A caller has no source to point into, so these params carry
// the typed items alone and the lowering synthesizes both the clause text and a
// synthetic span. The language compiler reads `item`, so nothing semantic is
// carried by what the wire omits.

/// `author.add_dialect` params: a dialect register and its typed interior.
///
/// [`Self::body`] is `dli_ast`'s own [`DialectItem`] sequence — the LS-9 typed
/// interior minus the span-bearing `Clause` wrapper — so the tool surface and
/// the compiler read the same shape rather than two that must be kept in step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddDialectParams {
    /// The workspace id.
    pub workspace: String,
    /// The dialect id.
    pub id: String,
    /// The dialect's display label (treated as data, never as an instruction).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<String>,
    /// The typed interior clauses, in order: vowels, endings, foreign clusters,
    /// ceilings, stress, name-kind grammars.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<DialectItem>,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

/// `author.add_name_part` params: one morpheme and the role it fills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddNamePartParams {
    /// The workspace id.
    pub workspace: String,
    /// The morpheme surface (the name-part id).
    pub id: String,
    /// The gloss — what the morpheme means (data, never an instruction).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gloss: Option<String>,
    /// The remaining header tokens, e.g. `["role", "given-proto"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub head: Vec<String>,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

/// `author.add_name` params: a composed name, optionally bound to its bearer.
///
/// The AST's [`NameItem`](dli_ast::language::NameItem) has four variants; two
/// are load-bearing here. `Composition` and `Bearer` are [`Self::composition`]
/// and [`Self::bearer`]. `Provenance` is not exposed — it would be a second,
/// competing spelling of [`Self::provenance`], and the header-first precedence
/// rule would silently pick one. `Other` is by definition a clause the name
/// grammar does not recognize, which a typed constructor cannot produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddNameParams {
    /// The workspace id.
    pub workspace: String,
    /// The name id.
    pub id: String,
    /// The rendered label (data, never an instruction).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<String>,
    /// The remaining header tokens, e.g. `["person.given", "in", "elder-tongue-fringe"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub head: Vec<String>,
    /// The composition steps: each a slot role filled by a named morpheme.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub composition: Vec<CompositionStep>,
    /// The entity that bears the name, if the name is bound to one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bearer: Option<String>,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

/// `author.add_term` params: a glossary term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddTermParams {
    /// The workspace id.
    pub workspace: String,
    /// The term id.
    pub id: String,
    /// The term's rendered label (data, never an instruction).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub label: Option<String>,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

/// `author.add_banned` params: a word refused on the lore surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddBannedParams {
    /// The workspace id.
    pub workspace: String,
    /// The banned word.
    pub word: String,
    /// Why it is banned (data, never an instruction).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

/// `author.declare_functional` params: declare a predicate single-valued.
///
/// This stages a `Decl::Functional`, marking `pred` a functional dependency —
/// each subject may carry at most one certain value for it, so two distinct
/// certain values are a contradiction (`continuity.fd-violation`). It lets an
/// author judge a *first-seen* predicate single-valued, which canon precedent
/// alone cannot (issue 49, sub-claim 4; contract 1.6.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorDeclareFunctionalParams {
    /// The workspace id.
    pub workspace: String,
    /// The predicate declared single-valued.
    pub pred: String,
    /// The provenance of this declaration.
    pub provenance: Provenance,
}

/// `author.declare_inertial` params: declare a predicate's fluent values
/// persistent.
///
/// This stages a `Decl::Inertial`, marking `pred` inertial — a fluent value
/// asserted `since t` with no authored `until` persists forward until a
/// later, distinct, certain value on the same slot defeats it. The continuity
/// check reads the staged declaration to reshape the fluent projection,
/// bounding that persistence at the next distinct certain value (contract
/// 1.8.0, DL-0081).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorDeclareInertialParams {
    /// The workspace id.
    pub workspace: String,
    /// The predicate declared inertial (fluent-persistent).
    pub pred: String,
    /// The provenance of this declaration.
    pub provenance: Provenance,
}

/// `author.assert_same_as` params: assert two entity ids name the same bearer
/// (WS-1, contract 1.7.0).
///
/// This stages a `Decl::SameAs`, closed symmetric-transitively by the
/// compiled world (`dli_eval::world::synthesize_alias_rules`): every other
/// predicate's tuples merge across `left` and `right`, and the merge's
/// standing is carried entirely by this claim's own provenance — defeating
/// the citing decision un-merges every consequence algebraically, with no
/// recompile (DL-0078). A plain `author.add_relation {pred: "same-as", …}`
/// does NOT merge anything; only this declaration form does (koan 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAssertSameAsParams {
    /// The workspace id.
    pub workspace: String,
    /// One side of the identity claim.
    pub left: String,
    /// The other side of the identity claim.
    pub right: String,
    /// The provenance of this claim.
    pub provenance: Provenance,
}

/// `author.assert_predicate_same_as` params: assert two predicate names denote
/// one property (contract 1.12.0).
///
/// This is deliberately a typed wire carriage only. No `.dli` spelling is
/// added; the evaluator receives the dedicated `Decl::PredicateSameAs`
/// variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAssertPredicateSameAsParams {
    /// The workspace id.
    pub workspace: String,
    /// One predicate name in the identity claim.
    pub left: String,
    /// The other predicate name in the identity claim.
    pub right: String,
    /// The provenance and requested standing of this claim.
    pub provenance: Provenance,
}

/// `author.add_rule` params: declare one datalog derivation rule (WS-2,
/// contract 1.7.0).
///
/// `head`/`body` are the already-typed wire forms `author.query`'s
/// `goal_atom` and `author.add_rule` share — never a text field. One clause
/// per call; an N-clause rule is N calls under distinct names, which is also
/// finer-grained for `author.defeat` (DL-0058's rule-authoring trigger,
/// satisfied here). Provenance is required, exactly as `author.add_before`'s
/// is: a rule is a ruling like any other, and every fact it derives carries
/// this claim's own provenance into its support (`fire_rules` multiplies it
/// in already; `author.add_rule` is what gives that multiplication something
/// real to work with).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddRuleParams {
    /// The workspace id.
    pub workspace: String,
    /// The rule's name — the unit `author.defeat` cites and `rule.duplicate`
    /// guards.
    pub name: String,
    /// The head atom the rule derives when the body holds.
    pub head: AtomExpr,
    /// The body conjunction of literals (a negated literal is `not <atom>`).
    pub body: Vec<BodyLiteral>,
    /// The provenance of this rule.
    pub provenance: Provenance,
}

/// `author.add_state` params: a time-varying fluent.
///
/// A state is not an attribute: it holds over an interval rather than forever,
/// which is why it carries [`Self::time`] and lowers to `Decl::State` instead of
/// `Decl::Attr`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddStateParams {
    /// The workspace id.
    pub workspace: String,
    /// The state name; may be dotted, e.g. `joren.water-status`.
    pub id: String,
    /// The value the state takes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub value: Option<String>,
    /// When the state holds, as a temporal clause.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub time: Option<StateTime>,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

/// The temporal clause of an [`AuthorAddStateParams`]: which anchor, and which
/// point it names. Mirrors `dli_ast`'s `TimeClause` minus the multi-point
/// `order` form, which orders events rather than bounding a state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub struct StateTime {
    /// The temporal relation.
    pub kind: StateTimeKind,
    /// The era or event the relation anchors to.
    pub point: String,
}

/// The temporal relation an [`StateTime`] asserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StateTimeKind {
    /// Holds after the point.
    After,
    /// Holds throughout the point.
    During,
    /// Holds from the point onward.
    Since,
    /// Holds up to the point.
    Until,
}

/// `author.add_attest` params: an attestation request against an open question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAddAttestParams {
    /// The workspace id.
    pub workspace: String,
    /// The question being attested.
    pub question: String,
    /// The provenance of this addition.
    pub provenance: Provenance,
}

/// `author.set_standing` promotes one provisional stored claim to ratified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorSetStandingParams {
    /// The authoring workspace whose lorestore is being updated.
    pub workspace: String,
    /// The stable claim id of the stored fact to promote.
    pub claim_id: String,
    /// The decision that records the ratification.
    pub decision: String,
}

/// `author.record_decision` records a baseline ratified (supporting) decision
/// on the workspace's branch (contract 1.9.0, DL-0083).
///
/// It writes a `ratify` carriage object for `decision` and grows the commit's
/// known-decision list, so a fact that cites `decision` reads `certain` on the
/// next compile — the wire way to reach certainty and make contradiction checks
/// (`continuity.fd-violation` and its kin) fire, closing the bootstrap gap the
/// out-of-band `fixture-authority` binary used to fill. The id is
/// caller-supplied and matched by exact equality (re-recording a known id is
/// idempotent), the recorded decision is defeasible by `author.defeat`, and
/// this does NOT move the DL-0054 boundary: it records the caller's claim that a
/// ruling was made, exactly as `cited`/`set_standing`/`defeat` already record
/// caller-supplied decision state as data — it never decides *who* may rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorRecordDecisionParams {
    /// The authoring workspace whose branch records the decision.
    pub workspace: String,
    /// The decision id to record (caller-supplied, exact-equality matched).
    pub decision: String,
    /// Optional human rationale for the ruling — recorded as review metadata,
    /// scanned for control characters, never interpreted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Optional provenance for a decision that itself rests on prior decisions:
    /// its cited ids are integrity-gated (an unresolved citation raises
    /// `citation.unknown-decision` and bars the record), exactly as a fact's
    /// citation is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

/// One decision record in an atomic [`AuthorRecordDecisionsParams`] batch.
/// The fields intentionally mirror [`AuthorRecordDecisionParams`] minus the
/// workspace: bulk recording changes commit granularity, not decision meaning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorDecisionRecord {
    /// The caller-supplied decision id, matched by exact equality.
    pub decision: String,
    /// Optional review rationale. It receives the same control-character guard
    /// as the single operation and otherwise preserves that operation's current
    /// metadata behaviour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Optional citation-bearing provenance for the decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

/// `author.record_decisions` atomically records several supporting decisions
/// on one workspace branch (contract 1.13.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorRecordDecisionsParams {
    /// The authoring workspace whose branch records the decisions.
    pub workspace: String,
    /// Decision records in durable append order. Empty batches are refused by
    /// schema; duplicate ids are a typed, atomic validation finding.
    #[schemars(length(min = 1))]
    pub records: Vec<AuthorDecisionRecord>,
}

/// The ordinary author mutation receipt plus deterministic batch counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorRecordDecisionsResult {
    /// The ordinary mutation receipt, flattened so the wire retains the common
    /// `findings`/`rendered`/digest/root fields rather than nesting them.
    #[serde(flatten)]
    pub mutation: AuthorMutationResult,
    /// Number of records supplied by the caller.
    pub requested: usize,
    /// Number already present at the target branch head.
    pub already_known: usize,
    /// Number newly written by this call.
    pub recorded: usize,
}

/// `author.defeat` records a conditional attack against a supporting decision.
/// Defeating an attacker removes its effect; other live attacks may still block
/// the target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorDefeatParams {
    /// The authoring workspace whose lorestore is being updated.
    pub workspace: String,
    /// The stable claim id of the stored fact to defeat.
    pub claim_id: String,
    /// The decision that defeats the claim.
    pub decision: String,
    /// The claim's existing supporting decision to attack.
    pub defeated_decision: String,
}

// -- author.query (read) -----------------------------------------------------

/// `author.query` params: query the **staged** world (base + staged
/// declarations). The agent asks "what holds about `nara` now?" over its
/// in-progress work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorQueryParams {
    /// The workspace id.
    pub workspace: String,
    /// Retired text form; use [`Self::goal_atom`].
    #[serde(default)]
    pub goal: String,
    /// The structured goal atom; a `Term::Var` argument is a query variable.
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
    /// The full effective valuation of the staged world — every ruled atom
    /// with its boolean, sorted by name.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub valuation: Option<Vec<ValuationEntry>>,
    /// The answer rows over the staged world.
    pub rows: Vec<AuthorQueryRow>,
}

/// A query row with IDs of directly matching committed claims. Derived rows
/// may have none; these IDs name matching source objects, not a proof of support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorQueryRow {
    /// The ordinary query answer.
    #[serde(flatten)]
    pub row: QueryRow,
    /// Stable IDs of directly matching committed claims, sorted and deduplicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claim_ids: Vec<String>,
}

// -- author.as_of (read, contract 1.9.0, DL-0082) -----------------------------

/// `author.as_of` params: project the inertially-current fluent value(s) of
/// one `(pred, subject)` slot at a query point over the staged world (base +
/// staged declarations). This is the ONE inertia-aware read
/// (`docs/design-notes/inertia-fluent-persistence.md` A.3, lifted by
/// DL-0082): `author.query` still returns the raw extensional value(s)
/// unmodified, so a silent inertial transition — otherwise unreadable
/// outside a `continuity.fluent-*` finding — becomes directly observable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorAsOfParams {
    /// The workspace id.
    pub workspace: String,
    /// The predicate.
    pub pred: String,
    /// The subject; may be any alias a certain `same-as` merges into the
    /// canonical class.
    pub subject: String,
    /// The query point's name — an era or event the staged world's chronology
    /// declares. An unknown name is a `schema-violation`, not a silent empty
    /// answer.
    pub at: String,
}

/// `author.as_of` result: the point-in-time projection `check::slot_state_at`
/// derives for one `(pred, subject)` fluent slot, kernel-side — never routed
/// through `dli_eval::world::World::query_with` or the saturated derived
/// layer (the DL-0081/DL-0082 hard invariant). A `(pred, subject)` with no
/// fluent timeline answers with empty `holdings`, no fallback to the raw
/// extensional facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorAsOfResult {
    /// The predicate, echoed back.
    pub pred: String,
    /// The subject, canonicalized through the identity map.
    pub subject: String,
    /// The query point's name, echoed back.
    pub at: String,
    /// Whether `pred` carries an `inertial` declaration (any standing).
    pub inertial: bool,
    /// Whether that declaration is certain under the staged world's
    /// valuation, so the projection is actually inertia-shaped rather than
    /// byte-identical to the plain open-interval projection.
    pub capping: bool,
    /// Whether two or more distinct values hold certainly at `at` at once —
    /// [`dli_time::fluent::FluentState::has_contradiction`] over this same
    /// projection.
    pub contested: bool,
    /// The inertia declaration's own decision citations; empty when `pred`
    /// was never declared inertial.
    pub inertial_citations: Vec<String>,
    /// The values whose span could be in force at the query point, each with
    /// its factorial verdict and, when capped, the defeater that bounds it.
    pub holdings: Vec<AsOfHolding>,
    /// The workspace digest the read was evaluated against (`jcs:sha256:`
    /// regime — the staged world is workspace state, not a store commit).
    /// Like [`AuthorQueryResult::root_hash`], this names workspace state.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
}

/// One [`AuthorAsOfResult`] value's standing at the query point: its
/// factorial verdict, its temporal anchors, and — for a capped inertial value
/// — the competitor that defeats it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AsOfHolding {
    /// The value in force.
    pub value: String,
    /// The collapsed three-valued projection.
    pub projection: Projection,
    /// The temporal half: whether the span's interval covers the query
    /// point.
    pub temporal: TemporalCover,
    /// The support half: the anchoring ruling's standing (dead spans are
    /// dropped, never reported as a holding).
    pub support: Status,
    /// The originating span's authored `since` anchor.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub since: Option<String>,
    /// The originating span's upper bound: its authored `until` when one
    /// exists, else — for a capped inertial value — the DERIVED boundary;
    /// `None` means unbounded.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub until: Option<String>,
    /// The originating span's own decision citations.
    pub citations: Vec<String>,
    /// The certain, distinct competitor that caps this value (inertial only,
    /// present exactly when [`Self::until`] is a derived rather than authored
    /// boundary).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub defeater: Option<AsOfDefeater>,
}

/// The certain, distinct competitor that defeats a capped [`AsOfHolding`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AsOfDefeater {
    /// The defeater's value.
    pub value: String,
    /// The defeater's `since` — the same point as the capped holding's
    /// derived `until`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub since: Option<String>,
    /// The defeater span's own decision citations.
    pub citations: Vec<String>,
}

// -- author.predicates (read, contract 1.10.0) --------------------------------

/// `author.predicates` params: the distinct predicates present in the staged
/// world (base + staged declarations).
///
/// The predicate census a canonicalizing extractor reads before minting a new
/// predicate name, so a fresh mention can be matched against what the store
/// already holds rather than lowered to a novel slot the FD/continuity check
/// can never pair with an existing one (the incremental-canonicalization
/// reads). The set is small, bounded, and stabilizes quickly, so a full list
/// is the whole answer — completeness over precision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorPredicatesParams {
    /// The workspace id.
    pub workspace: String,
}

/// `author.predicates` result: every distinct predicate the staged world
/// carries, sorted by name, each annotated with its observed arity and whether
/// the author has declared it functional or inertial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorPredicatesResult {
    /// Every distinct predicate present, sorted by `pred`.
    pub predicates: Vec<PredicateEntry>,
    /// The workspace digest the read was evaluated against (`jcs:sha256:`
    /// regime — the staged world is workspace state, not a store commit). v2
    /// only, mirroring [`AuthorQueryResult::root_hash`].
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
}

/// One predicate in the staged world's census.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PredicateEntry {
    /// The predicate name.
    pub pred: String,
    /// The arity observed at the predicate's first occurrence. The
    /// FD/continuity slot check keys only binary predicates (`arity == 2`), so
    /// this lets a caller filter to the FD-eligible ones.
    pub arity: usize,
    /// Whether the author has **declared** this predicate functional
    /// (`author.declare_functional`). This is the declared set the load-check
    /// consults — never canon's inferred-functional set — so it agrees with
    /// what `continuity.fd-violation` fires on.
    pub functional: bool,
    /// Whether the author has declared this predicate inertial
    /// (`author.declare_inertial`).
    pub inertial: bool,
}

// -- author.entity_candidates (read, contract 1.10.0) -------------------------

/// `author.entity_candidates` params: entities in the staged world whose id is
/// close to a surface form, by edit distance. The candidate universe contains
/// declared entities only; symbols referenced only by facts are not candidates.
///
/// The bounded coreference candidate read a canonicalizing extractor runs once
/// per mention: given the surface form it is about to mint, return the existing
/// entities it most plausibly names, so the merge decision (`assert_same_as`)
/// is made against what the store already holds. `limit` is caller-specified
/// and bounds the result — this read must never dump the full entity set, which
/// grows without bound — and the engine caps it at a hard ceiling so an
/// over-large `limit` cannot degenerate into that dump.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorEntityCandidatesParams {
    /// The workspace id.
    pub workspace: String,
    /// The surface form to match existing entities against.
    pub surface: String,
    /// The maximum number of candidates to return. Capped at the engine's
    /// ceiling so the read stays bounded regardless of the value.
    pub limit: usize,
}

/// `author.entity_candidates` result: the nearest candidate entities to the
/// surface form, nearest first, each with its edit distance. Only declared
/// entities are returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorEntityCandidatesResult {
    /// The candidate entities, nearest first (lexical on ties), at most `limit`.
    pub candidates: Vec<EntityCandidate>,
    /// The workspace digest the read was evaluated against (`jcs:sha256:`
    /// regime — the staged world is workspace state, not a store commit). v2
    /// only, mirroring [`AuthorQueryResult::root_hash`].
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub root_hash: Option<String>,
}

/// One coreference candidate: an existing entity close to the surface form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntityCandidate {
    /// The candidate entity's id — its **canonical representative** under the
    /// identity map, so a surfaced candidate keys the way the FD/continuity
    /// check would and aliases never spend the `limit` budget twice.
    pub name: String,
    /// The declared entity's sort when the negotiated contract uses the
    /// declared-only candidate universe.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sort: Option<String>,
    /// The edit distance from the surface form to `name`, so the caller can
    /// threshold.
    pub distance: usize,
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
    /// The atom to explain, rendered `pred(a, b)` (ground). The typed successor
    /// is [`Self::goal_atom`]; this string form is parsed through the `.dli`
    /// front end, so it is inert in a syntax-free (default) build. `atom` is not
    /// a killed form — it stays accepted on every wire — but `default` is what
    /// lets a caller send [`Self::goal_atom`] alone.
    #[serde(default)]
    pub atom: String,
    /// The atom to explain as a typed atom — the successor to [`Self::atom`],
    /// carried structurally instead of re-parsed from text, so `supporting_decisions`
    /// is reachable in a syntax-free build. When present it is used and `atom` is
    /// ignored; when absent `atom` is parsed exactly as before. Mirrors the sibling
    /// `author.query`'s `goal_atom` verbatim.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goal_atom: Option<AtomExpr>,
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
    /// The recorded decisions in the atom's support antichain — the support
    /// monomials' claim ids filtered to the known-decision set (contract 1.9.0,
    /// DL-0083). The auditability affordance: it lets a caller confirm a fact
    /// reached certainty through recorded decisions. The field is omitted when
    /// there are no supporting decisions.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub supporting_decisions: Vec<String>,
    /// Currently admissible minimal support monomials, with exact recorded
    /// decision names and path boundaries preserved. An empty list means no
    /// support paths are currently admissible; it does not establish standing.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active_supporting_decision_paths: Option<Vec<Vec<String>>>,
    /// The active direct decision defeats that explain a currently blocked
    /// conclusion. Live conclusions report an explicit empty list; their
    /// blocked alternatives remain available through [`Self::blocked_supports`].
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blocking_defeats: Option<Vec<BlockingDefeat>>,
    /// Minimal symbolic support paths that would be active but for the listed
    /// current direct defeats. This remains separate from active provenance so
    /// a defeated redundant alternative never contaminates a live path.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blocked_supports: Option<Vec<BlockedSupport>>,
}

/// One currently active direct decision attack, named exactly as recorded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockingDefeat {
    /// The decision whose supporting path is disabled.
    pub defeated_decision: String,
    /// The undefeated decision that directly attacks it.
    pub defeating_decision: String,
}

/// One otherwise-admissible minimal derivation path and its active blockers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockedSupport {
    /// Exact recorded decision ids carried by the fact/rule support monomial.
    pub supporting_decision_path: Vec<String>,
    /// The minimal active direct-defeat pairs disabling this path.
    pub blocking_defeats: Vec<BlockingDefeat>,
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
    /// The branch this workspace targets, if any (WS-3). Emitted only on a
    /// ≥ 1.7.0 request.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub branch: Option<String>,
}

/// `author.list_workspaces` result: every open authoring workspace and its
/// digest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorListWorkspacesResult {
    /// The open workspaces, in id order.
    pub workspaces: Vec<WorkspaceDigest>,
}

// -- author.branch (LS-6, contract 0.6.1) ------------------------------------

/// The branch action: create a new branch, list the existing ones, or delete
/// one (`delete`, WS-3, ≥ contract 1.7.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BranchAction {
    /// Create a new branch at the source branch's head.
    Create,
    /// List every branch.
    List,
    /// Delete a branch (WS-3, ≥ contract 1.7.0): pointer removal only, no
    /// object GC. Refuses `main`, and refuses while any open workspace still
    /// targets the branch.
    Delete,
}

/// `author.branch` params. `name`/`from` apply to `create`; `name` alone
/// applies to `delete`; `list` ignores both. `expected_ref` fences `create`'s
/// FROM branch or `delete`'s target branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthorBranchParams {
    /// The workspace whose lorestore the branch lives in.
    pub workspace: String,
    /// Create, list, or delete.
    pub action: BranchAction,
    /// The branch name to create or delete (required for `create`/`delete`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    /// The source branch to create from (default `main`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub from: Option<String>,
    /// Fence `create`'s FROM branch, or `delete`'s target branch, on its
    /// current whole-ref digest (WS-3, ≥ contract 1.7.0). A mismatch is
    /// `stale-fence`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub expected_ref: Option<String>,
}

/// One branch's head, in an `author.branch` result (WS-3, ≥ contract 1.7.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BranchHead {
    /// The branch name.
    pub name: String,
    /// The branch's head commit hash.
    pub commit: String,
    /// The branch ref's whole-ref digest — the fence `author.merge`'s and
    /// `author.branch`'s `expected_ref` check against.
    pub ref_digest: String,
}

/// `author.branch` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorBranchResult {
    /// Every branch name after the action, `main` first.
    pub branches: Vec<String>,
    /// The branch just created (`create` only).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub created: Option<String>,
    /// Every branch's head commit and ref digest (WS-3, ≥ contract 1.7.0) —
    /// `expected_ref`'s source material for a subsequent `author.merge` or
    /// `author.branch` `delete`/`create` fence.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub heads: Option<Vec<BranchHead>>,
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
    /// Inspect the merge without committing it (WS-3, ≥ contract 1.7.0): writes
    /// the same content-addressed merge commit a real merge would (so a later
    /// real merge of the same two heads produces the identical hash) and runs
    /// the full conflict scan, but skips the target-ref CAS — nothing
    /// observable moves.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub dry_run: Option<bool>,
    /// Fence the TARGET branch's whole-ref digest (WS-3, ≥ contract 1.7.0). A
    /// mismatch is `stale-fence`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub expected_ref: Option<String>,
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

// -- dli.corpus.import / dli.corpus.export (DM-3, contract 1.2.0) ------------

/// `dli.corpus.import` params: seed or extend a document's lorestore.
///
/// Exactly one source form, mirroring the manifest's own rule (DL-0055):
/// `objects` is the fact-object array and works on **any** build, `dli_documents`
/// is `.dli` text and needs a build with the front end linked. Both, or neither,
/// is refused — there is no rule for which would win.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CorpusImportParams {
    /// The logical document whose store receives the corpus.
    pub document: String,
    /// The corpus as stored fact objects — the build-independent form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<Vec<serde_json::Value>>,
    /// The exporting store's interner: the name table those objects' support
    /// polynomials index into. Required with `objects`, because a stored fact
    /// records its support as `±<index>` — without the table the indices
    /// resolve against whatever the receiving store happens to hold, which
    /// silently rewrites provenance rather than failing (DL-0056).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interner: Option<Vec<String>>,
    /// The decision ids the exporting store holds as known. Standing is read
    /// against this set: the same fact reads `decided d-1` where `d-1` is known
    /// and `proposed assumes d-1` where it is not, so an import that dropped it
    /// would demote every ratified fact it carried.
    #[serde(default)]
    pub decisions: Vec<String>,
    /// Conditional decision-attack objects to import. Defaults to empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attacks: Vec<serde_json::Value>,
    /// The corpus as `.dli` documents. Syntax builds only; elsewhere this is a
    /// loud refusal naming the one-shot syntax-build path, never a silent skip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dli_documents: Option<Vec<CorpusDocument>>,
    /// Whether this seeds an empty store or extends a populated one.
    pub mode: CorpusImportMode,
}

/// One `.dli` document in an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CorpusDocument {
    /// The document's logical id.
    pub id: String,
    /// Its `.dli` source text.
    pub text: String,
}

/// Whether an import seeds or extends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CorpusImportMode {
    /// Seed an empty store. Refused into a store that already has a head — an
    /// init that silently extended would make "init" mean two things.
    Init,
    /// Extend a store that already holds a corpus.
    Append,
}

/// `dli.corpus.import` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CorpusImportResult {
    /// The store commit this import advanced the document to.
    pub root_hash: String,
    /// How many fact objects the store holds at that commit.
    pub imported_count: usize,
    /// Graded findings raised by the import.
    pub findings: Vec<Finding>,
}

/// `dli.corpus.export` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CorpusExportParams {
    /// The logical document to export.
    pub document: String,
    /// The store commit to export at. Defaults to the served head.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// The emission form.
    #[serde(default)]
    pub format: CorpusExportFormat,
}

/// The form an export emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CorpusExportFormat {
    /// The folded facts, plus historical attack carriers required for replay.
    #[default]
    Objects,
    /// The `.dli` rendering. Syntax builds only; elsewhere a loud refusal.
    Rendered,
    /// Live fact objects grouped by predicate, with the interner, decisions,
    /// and conditional attacks needed to interpret their support. Dead facts
    /// remain available in the ordinary object export and store history.
    Canon,
}

/// `dli.corpus.export` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CorpusExportResult {
    /// The commit these facts were folded at.
    pub root_hash: String,
    /// Folded fact objects at that commit, preceded when needed by superseded
    /// metadata revisions that are exact carriers of decision attacks. Import
    /// preserves those revisions as evidence and folds the current objects last.
    pub objects: Vec<serde_json::Value>,
    /// The interner those objects' support polynomials index into. Feed it back
    /// to `dli.corpus.import` alongside `objects`; the pair is the corpus, and
    /// the objects alone are not (DL-0056).
    pub interner: Vec<String>,
    /// The decision ids known at that commit — what makes a fact read `decided`
    /// rather than `proposed assumes`. Round-trips with `objects` and
    /// `interner`; the three together are the exported state.
    pub decisions: Vec<String>,
    /// Folded conditional decision attacks at this commit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attacks: Vec<serde_json::Value>,
    /// The `.dli` rendering, when `format: rendered` was asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rendered: Vec<String>,
    /// The canon projection, when `format: canon` was asked
    /// for. `null` (omitted from the serialized response) for non-canon
    /// exports. The accompanying objects, interner, decisions, and attacks
    /// carry the corpus data needed to reconstruct the exported state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canon: Option<serde_json::Value>,
}
