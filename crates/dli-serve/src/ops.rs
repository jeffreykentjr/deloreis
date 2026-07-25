//! The closed operation set.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Every wire operation. The serde name is the operation's wire identifier; it
/// is the contract and must never drift from the fixtures. The batch `dli.*`
/// inventory tracks `docs/integration/fantasy-contract.md` §3.1; the `author.*`
/// tools are Door T (§3.2).
///
/// The wire identifier is exactly the serde encoding, so it round-trips:
///
/// ```
/// use dli_serve::Op;
/// let wire = serde_json::to_string(&Op::Version).expect("serialize op");
/// assert_eq!(wire, "\"dli.version\"");
/// let back: Op = serde_json::from_str(&wire).expect("deserialize op");
/// assert_eq!(back, Op::Version);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Op {
    /// Report the contract and grammar versions.
    #[serde(rename = "dli.version")]
    Version,
    /// Validate a document.
    #[serde(rename = "dli.check")]
    Check,
    /// Query a saturated world.
    #[serde(rename = "dli.query")]
    Query,
    /// Create a fork.
    #[serde(rename = "dli.fork.new")]
    ForkNew,
    /// List the open forks.
    #[serde(rename = "dli.fork.list")]
    ForkList,
    /// Derived-consequence diff of a fork.
    #[serde(rename = "dli.fork.diff")]
    ForkDiff,
    /// Assume an option or event in a fork.
    #[serde(rename = "dli.fork.suppose")]
    ForkSuppose,
    /// Add proposed declarations to a fork.
    #[serde(rename = "dli.fork.add")]
    ForkAdd,
    /// Retract an assumption from a fork.
    #[serde(rename = "dli.fork.suppress")]
    ForkSuppress,
    /// Restrict a fork's valuation scope.
    #[serde(rename = "dli.fork.restrict")]
    ForkRestrict,
    /// Ratification pre-flight: candidate valuation and conflict scan.
    #[serde(rename = "dli.fork.preflight")]
    ForkPreflight,
    /// Emit a ruling-request draft from a fork.
    #[serde(rename = "dli.fork.promote")]
    ForkPromote,
    /// Check a diegetic proof block in a practice.
    #[serde(rename = "dli.check-proof")]
    CheckProof,
    /// Fact-check a draft.
    #[serde(rename = "dli.check-draft")]
    CheckDraft,
    /// Start a simulation battery.
    #[serde(rename = "dli.simulate.start")]
    SimulateStart,
    /// Poll a simulation battery.
    #[serde(rename = "dli.simulate.status")]
    SimulateStatus,
    /// Cancel a simulation battery.
    #[serde(rename = "dli.simulate.cancel")]
    SimulateCancel,
    /// Show the timeline at a position.
    #[serde(rename = "dli.timeline.show")]
    TimelineShow,
    /// Partial-order report over the timeline.
    #[serde(rename = "dli.timeline.order")]
    TimelineOrder,
    /// Who-knows-what matrix at a position.
    #[serde(rename = "dli.timeline.knows")]
    TimelineKnows,
    /// Episode-order check and slack.
    #[serde(rename = "dli.timeline.episodes")]
    TimelineEpisodes,
    /// Phase-walk verdict for an episode.
    #[serde(rename = "dli.timeline.phases")]
    TimelinePhases,
    /// Run an export target and profile.
    #[serde(rename = "dli.export.run")]
    ExportRun,
    /// Vet a proposed name.
    #[serde(rename = "dli.name.check")]
    NameCheck,
    /// List questions missing attestations.
    #[serde(rename = "dli.attest.missing")]
    AttestMissing,
    /// Recompute attestation staleness.
    #[serde(rename = "dli.attestation-sweep")]
    AttestationSweep,
    /// Submit a draft payload.
    #[serde(rename = "dli.payload-submit")]
    PayloadSubmit,
    /// Open an authoring workspace.
    #[serde(rename = "author.open")]
    AuthorOpen,
    /// Report an authoring workspace's status.
    #[serde(rename = "author.status")]
    AuthorStatus,
    /// Render an authoring workspace's diff.
    #[serde(rename = "author.diff")]
    AuthorDiff,
    /// Discard an authoring workspace.
    #[serde(rename = "author.discard")]
    AuthorDiscard,
    /// Materialize an authoring workspace into a pack.
    #[serde(rename = "author.materialize")]
    AuthorMaterialize,
    /// Add an entity.
    #[serde(rename = "author.add_entity")]
    AuthorAddEntity,
    /// Add a fact.
    #[serde(rename = "author.add_fact")]
    AuthorAddFact,
    /// Add a relation.
    #[serde(rename = "author.add_relation")]
    AuthorAddRelation,
    /// Add an event.
    #[serde(rename = "author.add_event")]
    AuthorAddEvent,
    /// Add a precedence edge.
    #[serde(rename = "author.add_before")]
    AuthorAddBefore,
    /// Open a question.
    #[serde(rename = "author.open_question")]
    AuthorOpenQuestion,
    /// Add an option to a question.
    #[serde(rename = "author.add_option")]
    AuthorAddOption,
    /// Add a hypothesis.
    #[serde(rename = "author.add_hypothesis")]
    AuthorAddHypothesis,
    /// Suppose a set of hypotheses.
    #[serde(rename = "author.suppose")]
    AuthorSuppose,
    /// Query the staged world (base + staged declarations).
    #[serde(rename = "author.query")]
    AuthorQuery,
    /// Run the check battery over the staged world.
    #[serde(rename = "author.check")]
    AuthorCheck,
    /// Blame/explanation for a staged atom.
    #[serde(rename = "author.why")]
    AuthorWhy,
    /// The workspace's open findings, by severity.
    #[serde(rename = "author.findings")]
    AuthorFindings,
    /// The open authoring workspaces and their digests.
    #[serde(rename = "author.list_workspaces")]
    AuthorListWorkspaces,
}

impl Op {
    /// Every operation, in wire order.
    pub const ALL: [Op; 46] = [
        Op::Version,
        Op::Check,
        Op::Query,
        Op::ForkNew,
        Op::ForkList,
        Op::ForkDiff,
        Op::ForkSuppose,
        Op::ForkAdd,
        Op::ForkSuppress,
        Op::ForkRestrict,
        Op::ForkPreflight,
        Op::ForkPromote,
        Op::CheckProof,
        Op::CheckDraft,
        Op::SimulateStart,
        Op::SimulateStatus,
        Op::SimulateCancel,
        Op::TimelineShow,
        Op::TimelineOrder,
        Op::TimelineKnows,
        Op::TimelineEpisodes,
        Op::TimelinePhases,
        Op::ExportRun,
        Op::NameCheck,
        Op::AttestMissing,
        Op::AttestationSweep,
        Op::PayloadSubmit,
        Op::AuthorOpen,
        Op::AuthorStatus,
        Op::AuthorDiff,
        Op::AuthorDiscard,
        Op::AuthorMaterialize,
        Op::AuthorAddEntity,
        Op::AuthorAddFact,
        Op::AuthorAddRelation,
        Op::AuthorAddEvent,
        Op::AuthorAddBefore,
        Op::AuthorOpenQuestion,
        Op::AuthorAddOption,
        Op::AuthorAddHypothesis,
        Op::AuthorSuppose,
        Op::AuthorQuery,
        Op::AuthorCheck,
        Op::AuthorWhy,
        Op::AuthorFindings,
        Op::AuthorListWorkspaces,
    ];

    /// Whether this workspace mutation accepts digest/idempotency fences.
    #[must_use]
    pub const fn accepts_author_fences(self) -> bool {
        matches!(
            self,
            Op::AuthorDiscard
                | Op::AuthorAddEntity
                | Op::AuthorAddFact
                | Op::AuthorAddRelation
                | Op::AuthorAddEvent
                | Op::AuthorAddBefore
                | Op::AuthorOpenQuestion
                | Op::AuthorAddOption
                | Op::AuthorAddHypothesis
                | Op::AuthorSuppose
        )
    }

    /// A one-line, role-bearing description of what the op does and when to reach
    /// for it — the tool-**selection** text an agent reads when choosing among the
    /// tools (the MCP `tools/list` description; the per-field schema documents
    /// *how* to call it, this says *why*). Kept here, next to the wire names, so
    /// the description cannot drift from the op it names.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Op::Version => {
                "Report the contract and grammar versions and this build's wired capabilities."
            }
            Op::Check => {
                "Validate a loaded document: self-consistency findings over the compiled world."
            }
            Op::Query => "Query a saturated world for a goal atom's support, grade, and standing.",
            Op::ForkNew => "Open a fork: an exploratory condition context over an unchanged world.",
            Op::ForkList => "List the open forks over a document.",
            Op::ForkDiff => "Show a fork's derived-consequence diff against the base ledger.",
            Op::ForkSuppose => {
                "Assume an option or event true within a fork and see what its consequences become."
            }
            Op::ForkAdd => {
                "Add proposed declarations into a fork — fork-local exploration, never canon."
            }
            Op::ForkSuppress => "Retract an assumption from a fork.",
            Op::ForkRestrict => "Restrict a fork's valuation scope to a set of atoms.",
            Op::ForkPreflight => {
                "Ratification pre-flight: the candidate valuation a fork implies and the conflicts it would introduce."
            }
            Op::ForkPromote => {
                "Emit a ruling-request draft from a fork — the machine proposes, it never rules."
            }
            Op::CheckProof => "Check a diegetic proof block declared in a practice.",
            Op::CheckDraft => {
                "Fact-check a draft's claims against canon: supported / contradicted / unsupported."
            }
            Op::SimulateStart => "Start a simulation battery over a document.",
            Op::SimulateStatus => "Poll a running simulation battery for its trace and manifest.",
            Op::SimulateCancel => "Cancel a running simulation battery.",
            Op::TimelineShow => "Show the timeline at a position.",
            Op::TimelineOrder => "Report the partial order over the timeline.",
            Op::TimelineKnows => "Who-knows-what matrix at a timeline position.",
            Op::TimelineEpisodes => "Check episode ordering and report the slack in each slot.",
            Op::TimelinePhases => "Phase-walk verdict for an episode.",
            Op::ExportRun => "Run an export target under a profile.",
            Op::NameCheck => "Vet a proposed name through the naming oracle before it enters lore.",
            Op::AttestMissing => "List questions that lack a current attestation.",
            Op::AttestationSweep => "Recompute which stored attestations have gone stale.",
            Op::PayloadSubmit => "Submit a draft payload for acceptance checking.",
            Op::AuthorOpen => {
                "Open an authoring workspace over a scope or fork; returns its id and digest."
            }
            Op::AuthorStatus => {
                "Summarize a workspace: its staged canonical surface and current digest."
            }
            Op::AuthorDiff => {
                "Render the workspace's staged change as canonical .dli, with its findings and derived consequences."
            }
            Op::AuthorDiscard => "Drop a whole workspace, or one staged declaration.",
            Op::AuthorMaterialize => {
                "Return the workspace's .dli file bytes as data; refuses while any error-severity finding is open."
            }
            Op::AuthorAddEntity => {
                "Introduce a person, place, thing, or era into the workspace, with provenance."
            }
            Op::AuthorAddFact => {
                "Assert a keyed attribute value on an entity (entity.key = value), with provenance."
            }
            Op::AuthorAddRelation => {
                "Assert a binary relation between two entities, with provenance."
            }
            Op::AuthorAddEvent => {
                "Introduce a chronology event anchored to an era, with provenance."
            }
            Op::AuthorAddBefore => "Order one event before another, with provenance.",
            Op::AuthorOpenQuestion => "File an open question with its options as tracked debt.",
            Op::AuthorAddOption => "Add one option to a question already open in the workspace.",
            Op::AuthorAddHypothesis => "Mint a fork-local hypothesis variable for supposition.",
            Op::AuthorSuppose => {
                "Suppose staged hypotheses true within a fork and see the consequences move."
            }
            Op::AuthorQuery => {
                "Query the staged world (base + your in-progress work): what holds about an entity now."
            }
            Op::AuthorCheck => {
                "Run the full check battery over the staged world — verify your whole draft before materialize."
            }
            Op::AuthorWhy => {
                "Explain a staged claim's standing: why it is certain / possible / dead, the blocker, and its repair."
            }
            Op::AuthorFindings => {
                "List the workspace's open findings by severity — what materialize would refuse on, and how to fix each."
            }
            Op::AuthorListWorkspaces => {
                "List your open authoring workspaces and their digests, to recover context."
            }
        }
    }
}
