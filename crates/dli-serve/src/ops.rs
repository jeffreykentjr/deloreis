//! The closed operation set.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Every wire operation. The serde name is the operation's wire identifier; it
/// is the contract and must never drift from the fixtures. The batch `dli.*`
/// inventory tracks `docs/integration/fantasy-contract.md` §3.1; the `author.*`
/// tools are the authoring seam (§3.2).
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
    /// Report the worker contract version and wired capabilities.
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
    /// Import a corpus into a document's lorestore (DM-3, contract 1.2.0).
    #[serde(rename = "dli.corpus.import")]
    CorpusImport,
    /// Export a document's folded fact objects at a head (DM-3, contract 1.2.0).
    #[serde(rename = "dli.corpus.export")]
    CorpusExport,
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
    /// Create or list persistent branches (LS-6, contract 0.6.1).
    #[serde(rename = "author.branch")]
    AuthorBranch,
    /// Merge a named branch into the current branch (LS-6, contract 0.6.1).
    #[serde(rename = "author.merge")]
    AuthorMerge,
    /// Add a dialect register (DM-4 wave 1, contract 1.3.0).
    #[serde(rename = "author.add_dialect")]
    AuthorAddDialect,
    /// Add a name-part morpheme (DM-4 wave 1, contract 1.3.0).
    #[serde(rename = "author.add_name_part")]
    AuthorAddNamePart,
    /// Add a composed name (DM-4 wave 1, contract 1.3.0).
    #[serde(rename = "author.add_name")]
    AuthorAddName,
    /// Add a glossary term (DM-4 wave 1, contract 1.3.0).
    #[serde(rename = "author.add_term")]
    AuthorAddTerm,
    /// Ban a word from the lore surface (DM-4 wave 1, contract 1.3.0).
    #[serde(rename = "author.add_banned")]
    AuthorAddBanned,
    /// Add a time-varying state fluent (DM-4 tier 2, contract 1.3.0).
    #[serde(rename = "author.add_state")]
    AuthorAddState,
    /// Request an attestation for an open question (DM-4 tier 2, contract 1.3.0).
    #[serde(rename = "author.add_attest")]
    AuthorAddAttest,
    /// Promote a provisional claim to ratified (contract 1.5.0).
    #[serde(rename = "author.set_standing")]
    AuthorSetStanding,
    /// Record a defeating decision for a claim (contract 1.5.0).
    #[serde(rename = "author.defeat")]
    AuthorDefeat,
    /// Declare a predicate single-valued (a functional dependency), contract
    /// 1.6.0.
    #[serde(rename = "author.declare_functional")]
    AuthorDeclareFunctional,
    /// Assert two entity ids name the same bearer — the identity-closure
    /// kernel's wire surface (WS-1, contract 1.7.0).
    #[serde(rename = "author.assert_same_as")]
    AuthorAssertSameAs,
    /// Declare one datalog derivation rule — the rule-authoring wire surface
    /// (WS-2, contract 1.7.0).
    #[serde(rename = "author.add_rule")]
    AuthorAddRule,
    /// Declare a predicate's fluent values persistent (inertial), contract
    /// 1.8.0.
    #[serde(rename = "author.declare_inertial")]
    AuthorDeclareInertial,
    /// Project the inertially-current fluent value(s) at a query point,
    /// contract 1.9.0.
    #[serde(rename = "author.as_of")]
    AuthorAsOf,
    /// Record a baseline ratified (supporting) decision on the workspace's
    /// branch, so later facts that cite it read certain, contract 1.9.0.
    #[serde(rename = "author.record_decision")]
    AuthorRecordDecision,
    /// List the distinct predicates present in a workspace — the predicate
    /// census for incremental canonicalization, contract 1.10.0.
    #[serde(rename = "author.predicates")]
    AuthorPredicates,
    /// Bounded coreference candidates near a surface form — the entity half of
    /// incremental canonicalization, introduced in contract 1.10.0 and
    /// corrected to declared entities in 1.11.0.
    #[serde(rename = "author.entity_candidates")]
    AuthorEntityCandidates,
    /// Assert two predicate names denote the same property, contract 1.12.0.
    /// This uses a dedicated AST/evaluator channel and never aliases through
    /// entity identity. No `.dli` syntax or rendering form exists for it.
    #[serde(rename = "author.assert_predicate_same_as")]
    AuthorAssertPredicateSameAs,
}

impl Op {
    /// Every operation, in wire order.
    ///
    /// **Append only.** The capability list is this array filtered by the
    /// negotiated version's gates, so its order *is* the advertised order:
    /// inserting an op mid-array would reshuffle every older version's
    /// capability list and move released handshake bytes (audit C4). New ops go
    /// on the end, where the version filter can remove them without disturbing
    /// what came before.
    pub const ALL: [Op; 68] = [
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
        Op::CorpusImport,
        Op::CorpusExport,
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
        Op::AuthorBranch,
        Op::AuthorMerge,
        Op::AuthorAddDialect,
        Op::AuthorAddNamePart,
        Op::AuthorAddName,
        Op::AuthorAddTerm,
        Op::AuthorAddBanned,
        Op::AuthorAddState,
        Op::AuthorAddAttest,
        Op::AuthorSetStanding,
        Op::AuthorDefeat,
        Op::AuthorDeclareFunctional,
        Op::AuthorAssertSameAs,
        Op::AuthorAddRule,
        Op::AuthorDeclareInertial,
        Op::AuthorAsOf,
        Op::AuthorRecordDecision,
        Op::AuthorPredicates,
        Op::AuthorEntityCandidates,
        Op::AuthorAssertPredicateSameAs,
    ];

    /// Whether this op was introduced in contract `1.3.0` (the DM-4 expert
    /// authoring surface: wave 1 plus tier 2). Advertised only from `1.3.0` up,
    /// so a `1.2.0`-or-earlier negotiation's capability list — and the golden
    /// fixture recording it — stays byte-identical and needs no re-pin
    /// (audit C4).
    #[must_use]
    pub const fn added_in_1_3_0(self) -> bool {
        matches!(
            self,
            Op::AuthorAddDialect
                | Op::AuthorAddNamePart
                | Op::AuthorAddName
                | Op::AuthorAddTerm
                | Op::AuthorAddBanned
                | Op::AuthorAddState
                | Op::AuthorAddAttest
        )
    }

    /// Whether this op was introduced in contract `1.5.0`.
    #[must_use]
    pub const fn added_in_1_5_0(self) -> bool {
        matches!(self, Op::AuthorSetStanding | Op::AuthorDefeat)
    }

    /// Whether this op was introduced in contract `1.6.0` (the `functional`
    /// single-valuedness declaration). Advertised only from `1.6.0` up, so a
    /// `1.5.0`-or-earlier negotiation's capability list — and the golden fixture
    /// recording it — stays byte-identical and needs no re-pin (audit C4).
    #[must_use]
    pub const fn added_in_1_6_0(self) -> bool {
        matches!(self, Op::AuthorDeclareFunctional)
    }

    /// Whether this op was introduced in contract `1.7.0` (the
    /// wire-surface-completion campaign). Advertised only from `1.7.0` up, so
    /// a `1.6.0`-or-earlier negotiation's capability list — and the golden
    /// fixture recording it — stays byte-identical and needs no re-pin
    /// (audit C4). `author.assert_same_as` (WS-1, the identity-closure
    /// kernel's wire surface) is the first op the campaign appends;
    /// `author.add_rule` (WS-2, rule authoring) is the second and last.
    #[must_use]
    pub const fn added_in_1_7_0(self) -> bool {
        matches!(self, Op::AuthorAssertSameAs | Op::AuthorAddRule)
    }

    /// Whether this op was introduced in contract `1.8.0` (the inertia
    /// declaration, `author.declare_inertial`). Advertised only from `1.8.0`
    /// up, so a `1.7.0`-or-earlier negotiation's capability list — and the
    /// golden fixture recording it — stays byte-identical and needs no
    /// re-pin (audit C4).
    #[must_use]
    pub const fn added_in_1_8_0(self) -> bool {
        matches!(self, Op::AuthorDeclareInertial)
    }

    /// Whether this op was introduced in contract `1.9.0` (the inertia-aware
    /// point read, `author.as_of`). Advertised only from `1.9.0` up, so a
    /// `1.8.0`-or-earlier negotiation's capability list — and the golden
    /// fixture recording it — stays byte-identical and needs no re-pin
    /// (audit C4).
    #[must_use]
    pub const fn added_in_1_9_0(self) -> bool {
        matches!(self, Op::AuthorAsOf | Op::AuthorRecordDecision)
    }

    /// Whether this op was introduced in contract `1.10.0` (the
    /// incremental-canonicalization reads: `author.predicates` and
    /// `author.entity_candidates`). Advertised only from `1.10.0` up, so a
    /// `1.9.0`-or-earlier negotiation's capability list — and the golden
    /// fixture recording it — stays byte-identical and needs no re-pin
    /// (audit C4).
    #[must_use]
    pub const fn added_in_1_10_0(self) -> bool {
        matches!(self, Op::AuthorPredicates | Op::AuthorEntityCandidates)
    }

    /// Whether this op was introduced in contract 1.12.0 (predicate identity).
    /// It is appended at the end so every earlier capability list stays
    /// byte-identical.
    #[must_use]
    pub const fn added_in_1_12_0(self) -> bool {
        matches!(self, Op::AuthorAssertPredicateSameAs)
    }

    /// Whether this op was introduced in contract `1.2.0` (the DM-3 corpus
    /// exchange). Advertised only from `1.2.0` up, so a `1.1.0`-or-earlier
    /// negotiation's capability list — and the golden fixture recording it —
    /// stays byte-identical and needs no re-pin (audit C4).
    #[must_use]
    pub const fn added_in_1_2_0(self) -> bool {
        matches!(self, Op::CorpusImport | Op::CorpusExport)
    }

    /// Whether this op was introduced in contract `0.6.1` (the LS-6 branch/merge
    /// surface). Such ops are omitted from an older negotiation's capability list,
    /// keeping prior-version handshakes byte-identical.
    #[must_use]
    pub const fn added_in_0_6_1(self) -> bool {
        matches!(self, Op::AuthorBranch | Op::AuthorMerge)
    }

    /// The earliest accepted contract version this op is advertised at — the
    /// compat baseline `0.5.1` for everything present since the beginning, or
    /// the version named by whichever `added_in_*` gate above matches. Kept
    /// immediately next to those gates so a new gate and this match cannot drift
    /// apart; the unit test below asserts the agreement for every op.
    #[must_use]
    pub const fn introduced_in(self) -> &'static str {
        if self.added_in_1_12_0() {
            "1.12.0"
        } else if self.added_in_1_10_0() {
            "1.10.0"
        } else if self.added_in_1_9_0() {
            "1.9.0"
        } else if self.added_in_1_8_0() {
            "1.8.0"
        } else if self.added_in_1_7_0() {
            "1.7.0"
        } else if self.added_in_1_6_0() {
            "1.6.0"
        } else if self.added_in_1_5_0() {
            "1.5.0"
        } else if self.added_in_1_3_0() {
            "1.3.0"
        } else if self.added_in_1_2_0() {
            "1.2.0"
        } else if self.added_in_0_6_1() {
            "0.6.1"
        } else {
            "0.5.1"
        }
    }

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
                | Op::AuthorAddDialect
                | Op::AuthorAddNamePart
                | Op::AuthorAddName
                | Op::AuthorAddTerm
                | Op::AuthorAddBanned
                | Op::AuthorAddState
                | Op::AuthorAddAttest
                | Op::AuthorSetStanding
                | Op::AuthorDefeat
                | Op::AuthorDeclareFunctional
                | Op::AuthorAssertSameAs
                | Op::AuthorAddRule
                | Op::AuthorDeclareInertial
                | Op::AuthorRecordDecision
                | Op::AuthorAssertPredicateSameAs
        )
    }

    /// A one-line, role-bearing description of what the op does and when to reach
    /// for it — the tool-**selection** text an agent reads when choosing among the
    /// tools (the MCP `tools/list` description; the per-field schema documents
    /// *how* to call it, this says *why*). Kept here, next to the wire names, so
    /// the description cannot drift from the op it names.
    #[must_use]
    #[expect(clippy::too_many_lines)]
    pub const fn summary(self) -> &'static str {
        match self {
            Op::Version => {
                "Report the worker contract version and this build's wired capabilities."
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
            Op::SimulateStart => "Start a simulation battery over a document or game.",
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
            Op::AttestationSweep => "Validate and append an immutable attestation batch.",
            Op::PayloadSubmit => "Check and persist an accepted fat draft payload.",
            Op::CorpusImport => {
                "Import a corpus into a document's store — fact objects on any build, .dli documents on a syntax build."
            }
            Op::CorpusExport => {
                "Export a document's folded fact objects at a head — the live state, not the commit log."
            }
            Op::AuthorOpen => {
                "Open an authoring workspace over a scope or fork; returns its id and digest."
            }
            Op::AuthorStatus => {
                "Summarize a workspace: its staged canonical surface and current digest."
            }
            Op::AuthorDiff => {
                "Show the workspace's staged change as structured per-fact diff rows (before/after standing) with findings and derived consequences; the .dli rendering is empty unless a formatter is linked."
            }
            Op::AuthorDiscard => "Drop a whole workspace, or one staged declaration.",
            Op::AuthorMaterialize => {
                "Commit the workspace's staged fact objects to the lorestore — the accept decision — and return them as JSON (facts and root_hash), with .dli rendering behind rendered:true; an optional decision makes the accepted FACTS certain under it (not carriage claims like rules, same-as, or inertial declarations — those need record_decision plus a cited declaration); refuses while any error-severity finding is open."
            }
            Op::AuthorAddEntity => {
                "Introduce a person, place, thing, or era into the workspace, with provenance (events use author.add_event)."
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
            Op::AuthorBranch => {
                "Create a persistent branch, or list them — a divergent line of durable writes."
            }
            Op::AuthorMerge => {
                "Merge a named branch into the current one; returns the merge commit and a structural conflict report."
            }
            Op::AuthorAddDialect => {
                "Declare a dialect register — the phonology and name grammar a naming population draws on."
            }
            Op::AuthorAddNamePart => {
                "Declare a name-part morpheme and its role, the raw material composed names are built from."
            }
            Op::AuthorAddName => {
                "Compose a name from name-part morphemes and optionally bind it to the entity that bears it."
            }
            Op::AuthorAddTerm => "Introduce a glossary term — a word the lore defines and reuses.",
            Op::AuthorAddBanned => {
                "Ban a word from the lore surface, with the reason; the guardrail that refuses it later."
            }
            Op::AuthorAddState => {
                "Assert a time-varying state on an entity (a fluent that holds over an interval, not forever)."
            }
            Op::AuthorAddAttest => {
                "Request an attestation for an open question — the ledger entry a later sweep answers."
            }
            Op::AuthorSetStanding => {
                "Promote a provisional stored claim to ratified by citing a ratifying decision."
            }
            Op::AuthorDefeat => {
                "Defeat a stored claim with a decision while preserving its history and object."
            }
            Op::AuthorDeclareFunctional => {
                "Declare a predicate single-valued (a functional dependency), so two distinct certain values for one subject are a contradiction — even for a first-seen predicate canon has no precedent for."
            }
            Op::AuthorAssertSameAs => {
                "Assert two entity ids name the same bearer; the evaluable world merges them, the merge's standing carried by this claim's provenance."
            }
            Op::AuthorAddRule => {
                "Declare one datalog derivation rule (a typed head :- body clause); every fact it derives carries this rule's own provenance, defeasible like any other claim."
            }
            Op::AuthorDeclareInertial => {
                "Declare a predicate's fluent values persistent (inertial): a value asserted since a point holds forward until a later, distinct, live value defeats it."
            }
            Op::AuthorAsOf => {
                "Project the inertially-current fluent value(s) at a query point: what holds, and why, including a silent transition's derived boundary and defeater — the one inertia-aware read."
            }
            Op::AuthorRecordDecision => {
                "Record a baseline ratified decision on the workspace branch so facts that cite it read certain — the wire way to reach certainty and make contradiction checks (fd-violation, fluent, inertia) fire, replacing the fixture-authority hack; defeasible by author.defeat."
            }
            Op::AuthorPredicates => {
                "List the distinct predicates present in the staged world, each with its arity and whether you declared it functional or inertial — the predicate census to reuse an existing name before minting a new one."
            }
            Op::AuthorEntityCandidates => {
                "Find entity candidates whose id is closest to a surface form (bounded by a caller limit) — the coreference candidates to consider merging into before minting a new entity id. Contract 1.11.0 and later restrict the universe to declared entities."
            }
            Op::AuthorAssertPredicateSameAs => {
                "Assert two predicate names denote the same property; the evaluator records a defeasible symmetric-transitive identity claim and exposes its consequences through checks, queries, derivations, and the durable store."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `introduced_in` must agree with every `added_in_*` gate: whenever a gate
    /// says an op was added in version X, `introduced_in` must say X too. This
    /// is the drift guard the doc comment promises.
    #[test]
    fn introduced_in_agrees_with_every_added_in_gate() {
        for op in Op::ALL {
            if op.added_in_1_10_0() {
                assert_eq!(op.introduced_in(), "1.10.0", "{op:?}");
            }
            if op.added_in_1_12_0() {
                assert_eq!(op.introduced_in(), "1.12.0", "{op:?}");
            }
            if op.added_in_1_9_0() {
                assert_eq!(op.introduced_in(), "1.9.0", "{op:?}");
            }
            if op.added_in_1_8_0() {
                assert_eq!(op.introduced_in(), "1.8.0", "{op:?}");
            }
            if op.added_in_1_6_0() {
                assert_eq!(op.introduced_in(), "1.6.0", "{op:?}");
            }
            if op.added_in_1_5_0() {
                assert_eq!(op.introduced_in(), "1.5.0", "{op:?}");
            }
            if op.added_in_1_3_0() {
                assert_eq!(op.introduced_in(), "1.3.0", "{op:?}");
            }
            if op.added_in_1_2_0() {
                assert_eq!(op.introduced_in(), "1.2.0", "{op:?}");
            }
            if op.added_in_0_6_1() {
                assert_eq!(op.introduced_in(), "0.6.1", "{op:?}");
            }
        }
    }
}
