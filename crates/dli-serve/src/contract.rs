//! Per-op parameter and result types.
//!
//! Every op has a real params struct and a real result struct, all deriving
//! `JsonSchema`. Mutating `author.*` ops share [`AuthorMutationResult`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::grade::Grade;
use dli_core::posbool::PosBool;
use dli_eval::query::{Layer, QueryRow};
use dli_eval::why::{Why, WhyNot};
use dli_export::{ExportManifest, ExportProfile};
use dli_ledger::LedgerReport;
use dli_world::attest::{Attestation, AttestationStore};
use dli_world::draft::{DraftFinding, DraftPayload};

use crate::finding::{Finding, Provenance};

/// The result of any mutating `author.*` op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorMutationResult {
    /// Findings raised by the mutation (data, never instructions).
    pub findings: Vec<Finding>,
    /// The rendered lines the mutation produced.
    pub rendered: Vec<String>,
    /// The workspace digest after the mutation.
    pub workspace_digest: String,
}

/// A derived-consequence diff delta, shared by the fork mutation ops. Each field
/// lists rendered atoms whose status changed relative to the fork's base.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffDelta {
    /// Atoms that became certain.
    pub newly_certain: Vec<String>,
    /// Atoms that became merely possible.
    pub newly_possible: Vec<String>,
    /// Atoms that became dead (no longer derivable).
    pub newly_dead: Vec<String>,
    /// Atoms whose grade changed.
    pub grade_changed: Vec<String>,
    /// Conflicts surfaced by the delta.
    pub conflicts: Vec<String>,
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
pub struct CheckParams {
    /// The document id to check.
    pub document: String,
}

/// `dli.check` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    /// Whether the document is clean.
    pub ok: bool,
    /// Findings, if any.
    pub findings: Vec<Finding>,
}

// -- dli.query ---------------------------------------------------------------

/// `dli.query` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryParams {
    /// The document id to query.
    pub document: String,
    /// The goal atom, rendered.
    pub goal: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryResult {
    /// The support formula.
    pub support: PosBool,
    /// The grade.
    pub grade: Grade,
    /// Supporting citations.
    pub citations: Vec<String>,
}

// -- dli.fork.list -----------------------------------------------------------

/// `dli.fork.list` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkListParams {
    /// The document id.
    pub document: String,
}

/// `dli.fork.list` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkListResult {
    /// The open fork question ids.
    pub forks: Vec<String>,
}

// -- dli.fork.new ------------------------------------------------------------

/// `dli.fork.new` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
pub struct ForkDiffParams {
    /// The fork handle.
    pub fork: String,
}

// `dli.fork.diff` result is [`DiffDelta`].

// -- dli.fork.suppose --------------------------------------------------------

/// `dli.fork.suppose` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
pub struct ForkAddParams {
    /// The fork handle.
    pub fork: String,
    /// The rendered proposed declarations to add.
    pub declarations: Vec<String>,
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
pub struct ForkPreflightParams {
    /// The fork handle.
    pub fork: String,
}

/// `dli.fork.preflight` result: the ratification pre-flight scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForkPreflightResult {
    /// Conflicts the candidate valuation would introduce.
    pub conflicts: Vec<String>,
    /// The atoms the ratification would affect.
    pub affected: Vec<String>,
}

// -- dli.fork.promote --------------------------------------------------------

/// `dli.fork.promote` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
pub struct CheckProofParams {
    /// The document id.
    pub document: String,
}

/// `dli.check-proof` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckProofResult {
    /// The ledger report.
    pub report: LedgerReport,
}

// -- dli.check-draft ---------------------------------------------------------

/// `dli.check-draft` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckDraftParams {
    /// The draft payload to check.
    pub payload: DraftPayload,
}

/// `dli.check-draft` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckDraftResult {
    /// One finding per checked claim.
    pub findings: Vec<DraftFinding>,
}

// -- dli.simulate.start ------------------------------------------------------

/// `dli.simulate.start` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateStartParams {
    /// The document id.
    pub document: String,
    /// The RNG seed.
    pub seed: u64,
    /// The number of turns to simulate.
    pub turns: u32,
}

/// `dli.simulate.start` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateStartResult {
    /// The job id to poll.
    pub job_id: String,
    /// The initial job status.
    pub status: String,
}

// -- dli.simulate.status -----------------------------------------------------

/// `dli.simulate.status` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateStatusParams {
    /// The job id.
    pub job_id: String,
}

/// `dli.simulate.status` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateStatusResult {
    /// The job status.
    pub status: String,
    /// One line per simulated turn so far.
    pub trace: Vec<String>,
    /// The final manifest, present once the job is complete.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub manifest: Option<serde_json::Value>,
}

// -- dli.simulate.cancel -----------------------------------------------------

/// `dli.simulate.cancel` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateCancelParams {
    /// The job id.
    pub job_id: String,
}

/// `dli.simulate.cancel` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateCancelResult {
    /// The job status after cancellation.
    pub status: String,
}

// -- dli.timeline.* ----------------------------------------------------------

/// `dli.timeline.show` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineShowParams {
    /// The document id.
    pub document: String,
    /// The position to show the timeline at, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub at: Option<String>,
}

/// `dli.timeline.order` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineOrderParams {
    /// The document id.
    pub document: String,
}

/// `dli.timeline.knows` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineKnowsParams {
    /// The document id.
    pub document: String,
    /// The position to evaluate knowledge at, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub at: Option<String>,
}

/// `dli.timeline.episodes` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEpisodesParams {
    /// The document id.
    pub document: String,
}

/// `dli.timeline.phases` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelinePhasesParams {
    /// The document id.
    pub document: String,
    /// The episode to walk phases for.
    pub episode: String,
}

/// The shared result of the `dli.timeline.*` reports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineResult {
    /// The rendered findings for the requested report.
    pub findings: Vec<String>,
}

// -- dli.export.run ----------------------------------------------------------

/// `dli.export.run` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportRunParams {
    /// The document id.
    pub document: String,
    /// The export profile.
    pub profile: ExportProfile,
}

/// `dli.export.run` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportRunResult {
    /// The export manifest.
    pub manifest: ExportManifest,
}

// -- dli.attestation-sweep ---------------------------------------------------

/// `dli.attestation-sweep` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestationSweepParams {
    /// The attestation store to sweep.
    pub store: AttestationStore,
}

/// `dli.attestation-sweep` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestationSweepResult {
    /// The attestations now stale.
    pub stale: Vec<Attestation>,
}

// -- dli.payload-submit ------------------------------------------------------

/// `dli.payload-submit` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PayloadSubmitParams {
    /// The draft payload to submit.
    pub payload: DraftPayload,
}

/// `dli.payload-submit` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PayloadSubmitResult {
    /// Whether the payload was accepted.
    pub accepted: bool,
    /// Findings raised during acceptance.
    pub findings: Vec<Finding>,
}

// -- dli.version -------------------------------------------------------------

/// `dli.version` params (empty).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    /// Findings from the six-check vetting.
    pub findings: Vec<Finding>,
}

// -- dli.attest.missing ------------------------------------------------------

/// `dli.attest.missing` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestMissingParams {
    /// The document id.
    pub document: String,
}

/// `dli.attest.missing` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestMissingResult {
    /// Question ids that lack a current attestation.
    pub questions: Vec<String>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorStatusResult {
    /// The rendered workspace state.
    pub rendered: Vec<String>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuthorDiffResult {
    /// The rendered diff lines.
    pub rendered: Vec<String>,
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
    /// The event name.
    pub name: String,
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
    /// symbol is a ground constant.
    pub goal: String,
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
