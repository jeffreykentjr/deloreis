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
    /// Check a resource ledger.
    // TODO(CON-1, wave 3): the surface-v1 pass renamed the proof-block keyword
    // `ledger` → `proof`; the wire op name `dli.check-ledger` is a frozen contract
    // shape and is deliberately left unchanged here. Rename it with the contract.
    #[serde(rename = "dli.check-ledger")]
    CheckLedger,
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
}

impl Op {
    /// Every operation, in wire order.
    pub const ALL: [Op; 41] = [
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
        Op::CheckLedger,
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
    ];
}
