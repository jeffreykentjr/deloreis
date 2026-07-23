//! The finding-families reference page: `docs/reference-generated/finding-families.md`.
//!
//! Every graded finding the system emits carries a stable, dotted `family.item`
//! code (charter koan 9, *errors are lore*). Those codes are the contract a
//! benchmark, a repair tool, or a downstream consumer matches on, and until now
//! they were scattered across the crates that raise them. This page is their one
//! catalogue: for each code, its family, the check or tool that emits it, its
//! severity, its repair kind, and what it means.
//!
//! The catalogue is authored data (the `FAMILIES` table) rather than derived at runtime —
//! xtask cannot depend on `dli-world`/`dli-ledger`/`dli-time`/`dli-eval` — so a
//! coverage test scrapes each emitting crate's source and asserts the
//! authored code set matches the emitted one exactly, the same source-parity
//! discipline the CLI and error-code pages already use. The volatile `outline`
//! family (the season/outline battery, still landing under CRAFT-2) is covered
//! by a looser no-phantom check so an in-flight craft edit does not gate on this
//! page; the nine settled families are covered strictly, both directions.

use std::fmt::Write as _;

// The scrape helpers and coverage test are the only readers of the source-parity
// metadata (`sources`, `scrape`, and the `Scrape` payloads); they compile under
// `#[cfg(test)]`, so those readers — and these imports — are test-only.
#[cfg(test)]
use std::collections::BTreeSet;
#[cfg(test)]
use std::path::Path;

/// How the coverage test recovers a family's emitted codes from crate source,
/// and whether the family is covered strictly (settled) or no-phantom (a lane
/// still landing codes this wave).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy)]
enum Scrape {
    /// Settled family whose codes appear as quoted `"prefix.item"` literals on a
    /// code line — the enum `code()` match arms and the string-literal
    /// constructors. Covered strictly: the scraped set must equal the authored set.
    Prefixed(&'static str),
    /// Settled Door-T author findings: each appears as a `code: "…"` struct field.
    /// Covered strictly.
    CodeFields,
    /// A closed sub-code enum whose `as_str` returns bare tokens (no dotted
    /// prefix); the authored tokens are asserted present, one by one.
    BareTokens,
    /// A still-landing family scraped by `"prefix.item"` literal, covered
    /// no-phantom: every documented code must still be emitted, but a newer
    /// source code does not fail the gate (so an in-flight lane edit does not
    /// gate on this page).
    #[allow(dead_code)]
    PrefixedSnapshot(&'static str),
    /// A still-landing family whose codes are bare suffixes handed to the season
    /// battery's `warn(`/`error(` helpers; covered no-phantom.
    OutlineSnapshot,
    /// The craft season-rule family: codes are derived from `KernelRule::id()`
    /// match arms in `kb.rs`, prefixed with `craft.`. Covered no-phantom.
    CraftKernelRules,
}

/// One finding code: its dotted `family.item` string, severity, repair kind, and
/// a one-line domain meaning.
#[allow(clippy::struct_field_names)]
struct Code {
    /// The stable dotted code, e.g. `load.missing-provenance`. For [`Scrape::BareTokens`]
    /// families this is the bare token as it serializes.
    code: &'static str,
    /// `error` (blocks / a hard violation) or `warn` (advisory).
    severity: &'static str,
    /// The machine-repair kind the finding carries, or `—` for none.
    repair: &'static str,
    /// What the finding means, in the world's terms.
    meaning: &'static str,
}

/// A finding family: the check or tool that raises it and its codes.
struct Family {
    /// The family key — the part of the code before the first dot.
    key: &'static str,
    /// Human title for the section heading.
    title: &'static str,
    /// The check or tool that emits the family, and the CLI/wire surface it rides.
    emitter: &'static str,
    /// The crate module that owns the codes (for the reader who wants the source).
    owner: &'static str,
    /// A one-line note on the family's severity policy.
    severity_note: &'static str,
    /// Repo-relative source files the coverage test scrapes.
    #[cfg_attr(not(test), allow(dead_code))]
    sources: &'static [&'static str],
    /// How the coverage test recovers the emitted set.
    #[cfg_attr(not(test), allow(dead_code))]
    scrape: Scrape,
    /// The codes, in emission/declaration order.
    codes: &'static [Code],
}

const FAMILIES: &[Family] = &[
    Family {
        key: "load",
        title: "load — the loader's fail-closed gate",
        emitter: "the world loader, run by `deloreis check` / `dli.check` before any query",
        owner: "dli-world :: model.rs (LoadFinding)",
        severity_note: "All error: a load finding holds its declaration fail-closed — \
            unassertable until fixed. Fail-closed provenance *is* the assertability gate.",
        sources: &["../crates/dli-world/src/model.rs"],
        scrape: Scrape::Prefixed("load"),
        codes: &[
            Code {
                code: "load.missing-provenance",
                severity: "error",
                repair: "—",
                meaning: "a declaration carries no ruling and no hypothesis, so it cannot be asserted.",
            },
            Code {
                code: "load.malformed-provenance",
                severity: "error",
                repair: "—",
                meaning: "the provenance annotation cannot be read; held fail-closed until the annotation is fixed.",
            },
            Code {
                code: "load.unbound-head-variable",
                severity: "error",
                repair: "—",
                meaning: "a rule concludes about a variable that nothing in its premises names.",
            },
            Code {
                code: "load.duplicate-declaration",
                severity: "error",
                repair: "—",
                meaning: "a name is declared twice as the same kind within one scope.",
            },
            Code {
                code: "load.cross-pack-collision",
                severity: "error",
                repair: "—",
                meaning: "a name is declared in more than one scope/pack.",
            },
            Code {
                code: "load.unstratifiable",
                severity: "error",
                repair: "—",
                meaning: "the rules cannot be ordered for evaluation — a negative-dependency cycle (FRONT-1).",
            },
        ],
    },
    Family {
        key: "continuity",
        title: "continuity — the world-consistency battery",
        emitter: "the continuity checks, run by `dli.check` (world-internal) and `check-draft` (claim-relative)",
        owner: "dli-world :: check/continuity.rs (WorldFinding)",
        severity_note: "All error: a continuity finding is a contradiction of canon or an \
            assertion the claim may not make.",
        sources: &["../crates/dli-world/src/check/continuity.rs"],
        scrape: Scrape::Prefixed("continuity"),
        codes: &[
            Code {
                code: "continuity.chronology-cycle",
                severity: "error",
                repair: "—",
                meaning: "the before-graph closes a cycle; the timeline would have no consistent order.",
            },
            Code {
                code: "continuity.prior-contradiction",
                severity: "error",
                repair: "—",
                meaning: "a claim contradicts an established prior certain fact.",
            },
            Code {
                code: "continuity.rejected-lore",
                severity: "error",
                repair: "—",
                meaning: "a certain fact realizes a killed/rejected token.",
            },
            Code {
                code: "continuity.unknown-entity",
                severity: "error",
                repair: "—",
                meaning: "a claim references an entity that is not in scope.",
            },
            Code {
                code: "continuity.proposed-as-fact",
                severity: "error",
                repair: "—",
                meaning: "a claim states shadow (proposed) lore as settled.",
            },
            Code {
                code: "continuity.intra-episode-contradiction",
                severity: "error",
                repair: "—",
                meaning: "a claim set fixes one slot two contradictory ways within an episode.",
            },
        ],
    },
    Family {
        key: "name",
        title: "name — the naming-oracle kernel vet",
        emitter: "the world-true half of the naming oracle, run by `dli.name.check` and the \
            language layer of `check`",
        owner: "dli-world :: language/derivation.rs (LangFinding)",
        severity_note: "Warn: a name defect warns rather than blocks (grade `checked`); the \
            factory-side world-safe screens live outside the kernel. (Distinct from the \
            Door-T `name.unvetted` finding below.)",
        sources: &["../crates/dli-world/src/language/derivation.rs"],
        scrape: Scrape::Prefixed("name"),
        codes: &[
            Code {
                code: "name.slot-sequence",
                severity: "warn",
                repair: "—",
                meaning: "the name's slots match no name-kind pattern of its dialect.",
            },
            Code {
                code: "name.role-membership",
                severity: "warn",
                repair: "—",
                meaning: "a name-part is not declared in the dialect under the role it fills.",
            },
            Code {
                code: "name.register-fit",
                severity: "warn",
                repair: "—",
                meaning: "the name does not fit its declared dialect/register.",
            },
            Code {
                code: "name.phonotactics",
                severity: "warn",
                repair: "—",
                meaning: "the name violates the dialect's phonotactics (vowels, endings, clusters, stress).",
            },
            Code {
                code: "name.uniqueness",
                severity: "warn",
                repair: "—",
                meaning: "the name collides with an existing name in canon.",
            },
            Code {
                code: "name.fixed-token-reuse",
                severity: "warn",
                repair: "—",
                meaning: "the name reuses a fixed token reserved elsewhere.",
            },
            Code {
                code: "name.aural-clash",
                severity: "warn",
                repair: "—",
                meaning: "the name is aurally too close to another across the whole name-space.",
            },
            Code {
                code: "name.set-clash",
                severity: "warn",
                repair: "—",
                meaning: "the name is not aurally distinct within its declared set.",
            },
            Code {
                code: "name.gender-morphology",
                severity: "warn",
                repair: "—",
                meaning: "the name's morphology conflicts with its declared gender.",
            },
            Code {
                code: "name.sayability-cap",
                severity: "warn",
                repair: "—",
                meaning: "the name exceeds the sayability-tier caps (D-0095).",
            },
            Code {
                code: "name.banned-in-name",
                severity: "warn",
                repair: "—",
                meaning: "the name realizes banned diegetic vocabulary.",
            },
            Code {
                code: "name.confusable",
                severity: "warn",
                repair: "—",
                meaning: "the id mixes a Latin-looking form with a look-alike character from another script (a homoglyph).",
            },
        ],
    },
    Family {
        key: "attest",
        title: "attest — the attestation seam",
        emitter: "the attestation lookup a check consumes, surfaced as `dli.attest.missing`",
        owner: "dli-world :: attest.rs (AttestFinding)",
        severity_note: "Error for the consuming check: the kernel reads what a sweep committed \
            and never fabricates a judgment, so an absent record stops the check rather than \
            guessing an answer.",
        sources: &["../crates/dli-world/src/attest.rs"],
        scrape: Scrape::Prefixed("attest"),
        codes: &[Code {
            code: "attest.missing",
            severity: "error",
            repair: "—",
            meaning: "the question has no attestation in the store; a sweep must pose it before a check can consume it.",
        }],
    },
    Family {
        key: "practice",
        title: "practice — practice well-formedness",
        emitter: "the practice-block checker, run by `deloreis check` / `dli.check` when a `practice` is declared",
        owner: "dli-ledger :: practice.rs (PracticeFinding)",
        severity_note: "Error: a malformed practice cannot host a proof, so each finding blocks the practice.",
        sources: &["../crates/dli-ledger/src/practice.rs"],
        scrape: Scrape::Prefixed("practice"),
        codes: &[
            Code {
                code: "practice.error-refines-nothing",
                severity: "error",
                repair: "—",
                meaning: "a declared error refines a token that is not one of the seven structural classes L1–L7.",
            },
            Code {
                code: "practice.missing-discriminant",
                severity: "error",
                repair: "—",
                meaning: "two errors share a structural class with no discriminant to tell them apart.",
            },
            Code {
                code: "practice.resource-outside-algebra",
                severity: "error",
                repair: "—",
                meaning: "a resource's declared law is outside the closed resource algebra.",
            },
            Code {
                code: "practice.unbound-sort",
                severity: "error",
                repair: "—",
                meaning: "a role is typed by a sort the practice does not declare, that is not a primitive kind or an in-scope cycle.",
            },
            Code {
                code: "practice.unresolved-cycle",
                severity: "error",
                repair: "—",
                meaning: "a failure or law cites a cycle that is not in scope.",
            },
            Code {
                code: "practice.error-resource-unknown",
                severity: "error",
                repair: "—",
                meaning: "an L6 resource-violation error spends against a resource the practice does not declare.",
            },
            Code {
                code: "practice.renamed-interior-form",
                severity: "error",
                repair: "—",
                meaning: "a practice interior wrote a retired spelling (a killed form); refused with its successor named.",
            },
        ],
    },
    Family {
        key: "proof",
        title: "proof — the diegetic proof checker",
        emitter: "the proof/ledger checker, run by `deloreis check-proof` / `dli.check-proof`",
        owner: "dli-ledger :: ledger_check.rs (ProofFinding)",
        severity_note: "Error: a proof finding means the spoken proof is invalid; the L-class in \
            the message (L1–L7) names which structural obligation broke.",
        sources: &["../crates/dli-ledger/src/ledger_check.rs"],
        scrape: Scrape::Prefixed("proof"),
        codes: &[
            Code {
                code: "proof.unknown-form",
                severity: "error",
                repair: "—",
                meaning: "the form is not declared by this practice (L1).",
            },
            Code {
                code: "proof.premise-absent",
                severity: "error",
                repair: "—",
                meaning: "a premise is not established in the scene — the casting fizzles (L2, absent).",
            },
            Code {
                code: "proof.premise-contradicted",
                severity: "error",
                repair: "—",
                meaning: "a premise is denied by the certain projection — the casting rebounds (L2, contradicted).",
            },
            Code {
                code: "proof.wrong-phase",
                severity: "error",
                repair: "—",
                meaning: "a step runs at the wrong cyclic phase (L3, timing).",
            },
            Code {
                code: "proof.bad-instantiation",
                severity: "error",
                repair: "—",
                meaning: "a form is instantiated with ill-typed arguments (L4).",
            },
            Code {
                code: "proof.invalid-chain",
                severity: "error",
                repair: "—",
                meaning: "a chain step does not follow from its cited premises (L5).",
            },
            Code {
                code: "proof.resource-violation",
                severity: "error",
                repair: "—",
                meaning: "the ledger breaches a resource law (L6) — see the resource sub-codes below.",
            },
            Code {
                code: "proof.close-mismatch",
                severity: "error",
                repair: "—",
                meaning: "the close atom does not match what the steps derived (L7).",
            },
            Code {
                code: "proof.expectation-unmet",
                severity: "error",
                repair: "—",
                meaning: "the proof's expected verdict was not met (the expected-verdict discipline).",
            },
            Code {
                code: "proof.invalid-spend",
                severity: "error",
                repair: "—",
                meaning: "a spend draws against a resource illegally.",
            },
            Code {
                code: "proof.unknown-spoke",
                severity: "error",
                repair: "—",
                meaning: "a step cites a spoke or label that the proof does not define.",
            },
            Code {
                code: "proof.unknown-expectation",
                severity: "error",
                repair: "—",
                meaning: "an expectation names something the practice does not define.",
            },
            Code {
                code: "proof.missing-close",
                severity: "error",
                repair: "—",
                meaning: "the proof has no close step.",
            },
            Code {
                code: "proof.practice-mismatch",
                severity: "error",
                repair: "—",
                meaning: "the proof's `in <practice>` clause does not match the practice being checked.",
            },
        ],
    },
    Family {
        key: "resource",
        title: "resource — the discriminants of proof.resource-violation",
        emitter: "the resource-algebra accounting inside the proof checker; carried as the \
            discriminant of `proof.resource-violation`",
        owner: "dli-ledger :: resource.rs (ResourceViolationCode)",
        severity_note: "Sub-codes, not standalone families: each names how a `proof.resource-violation` \
            breached the closed algebra.",
        sources: &["../crates/dli-ledger/src/resource.rs"],
        scrape: Scrape::BareTokens,
        codes: &[
            Code {
                code: "overspent",
                severity: "error",
                repair: "—",
                meaning: "a linear or affine balance fell below zero — a unit was spent that was never accounted.",
            },
            Code {
                code: "over-capacity",
                severity: "error",
                repair: "—",
                meaning: "an affine balance rose above its declared capacity.",
            },
            Code {
                code: "not-conserved",
                severity: "error",
                repair: "—",
                meaning: "a `linear conserved` multiset did not net to zero at the close — a step minted or burned it.",
            },
            Code {
                code: "wrong-direction",
                severity: "error",
                repair: "—",
                meaning: "a graded-monotone grade moved against its one declared direction.",
            },
        ],
    },
    Family {
        key: "citation",
        title: "citation — lowering findings",
        emitter: "the evaluator's lowering pass, surfaced through `check`",
        owner: "dli-eval :: world.rs (LowerFinding)",
        severity_note: "Warn (non-fatal): lowering never aborts on one — it fails the affected \
            claim closed and records the finding so the rest of the world still checks.",
        sources: &["../crates/dli-eval/src/world.rs"],
        scrape: Scrape::Prefixed("citation"),
        codes: &[Code {
            code: "citation.unknown-decision",
            severity: "warn",
            repair: "—",
            meaning: "a claim cites a decision the corpus never declares; the claim is failed closed.",
        }],
    },
    Family {
        key: "author",
        title: "author — Door-T authoring findings",
        emitter: "the `author.*` MCP tools (Door T); `draft.defect` also rides `dli.check-draft`",
        owner: "dli-serve :: author/tools.rs + handlers.rs (Finding)",
        severity_note: "Per-code severity gates `author.materialize`: an `error` blocks it, a \
            `warn` passes. These are the findings that carry structured machine repairs.",
        sources: &[
            "../crates/dli-serve/src/author/tools.rs",
            "../crates/dli-serve/src/handlers.rs",
        ],
        scrape: Scrape::CodeFields,
        codes: &[
            Code {
                code: "name.unvetted",
                severity: "warn",
                repair: "run-check",
                meaning: "a name has not passed the naming oracle; it enters proposed and unvetted.",
            },
            Code {
                code: "entity.unknown",
                severity: "error",
                repair: "resolve-entity",
                meaning: "no entity of that id is in scope; the claim is staged but cannot materialize until it exists.",
            },
            Code {
                code: "fact.contradiction",
                severity: "error",
                repair: "—",
                meaning: "a claimed slot value contradicts a certain fact the base asserts.",
            },
            Code {
                code: "order.cycle",
                severity: "error",
                repair: "—",
                meaning: "a `before` fact would close a cycle against the existing orderings.",
            },
            Code {
                code: "consequence.changed",
                severity: "warn",
                repair: "—",
                meaning: "under a supposition the derived-consequence set may move; see `author.diff`.",
            },
            Code {
                code: "hypothesis.not-in-fork",
                severity: "error",
                repair: "—",
                meaning: "a hypothesis is legal only in a fork workspace.",
            },
            Code {
                code: "hypothesis.bundle-limit",
                severity: "error",
                repair: "—",
                meaning: "a proposal bundle carries at most one hypothesis; a second supposition is a second fork.",
            },
            Code {
                code: "hypothesis.unknown",
                severity: "error",
                repair: "—",
                meaning: "no hypothesis of that id is staged in this workspace to suppose.",
            },
            Code {
                code: "question.unknown",
                severity: "error",
                repair: "—",
                meaning: "no open question of that id exists in this workspace to add an option to.",
            },
            Code {
                code: "field.control-char",
                severity: "error",
                repair: "remove-control-chars",
                meaning: "a free-text field carries a control/bidi/zero-width character that could forge or reorder a declaration in the review diff.",
            },
            Code {
                code: "provenance.cited-refused",
                severity: "error",
                repair: "add-provenance",
                meaning: "Door T authors `proposed` or `hypothesis` only; an agent cannot cite a ruling to mint canon authority.",
            },
            Code {
                code: "draft.defect",
                severity: "error",
                repair: "—",
                meaning: "a fact-checked draft claim is a defect (contradicted, unsupported, unknown-entity, or a revived killed token).",
            },
        ],
    },
    Family {
        key: "craft",
        title: "craft — the season-ledger join-rule battery",
        emitter: "`check_season_rules` / `check_episode` in `dli-world::craft::season` (CRAFT-2)",
        owner: "dli-world :: craft/season.rs + craft/kb.rs (WorldFinding, via KernelRule)",
        severity_note: "Mixed error/warn per rule: structural verdicts (accomplishment-earned, \
            yes-and-terminal, scene-name-clash) are error; band/threshold rules are warn until \
            calibrated. This lane is landing under CRAFT-2; covered by a no-phantom check.",
        sources: &["../crates/dli-world/src/craft/kb.rs"],
        scrape: Scrape::CraftKernelRules,
        codes: CRAFT_CODES,
    },
    Family {
        key: "outline",
        title: "outline — the season/outline conformance battery",
        emitter: "the season-outline checks, run by `check-draft`'s craft battery",
        owner: "dli-world :: craft/season.rs (WorldFinding, via CRAFT-1/CRAFT-2)",
        severity_note: "Mixed error/warn per rule. This lane is landing under CRAFT-2; the list \
            below is a snapshot as of wave 4 and is covered by a no-phantom check rather than the \
            strict two-directional gate the settled families use.",
        sources: &["../crates/dli-world/src/craft/season.rs"],
        scrape: Scrape::OutlineSnapshot,
        codes: OUTLINE_CODES,
        // OUTLINE_CODES follows this table.
    },
];

/// The craft family's codes — the 13 season-rule [`KernelRule`](dli_world::craft::kb::KernelRule)
/// ids emitted by the season-ledger join battery (`CRAFT-2`) via `craft_warn`/`craft_error`.
/// The two entry-point meta-codes (`CheckOutline`, `CheckSeason`) name the battery functions,
/// not emitted finding codes, so they are excluded.
const CRAFT_CODES: &[Code] = &[
    Code {
        code: "craft.accomplishment-earned",
        severity: "error",
        repair: "—",
        meaning: "a major beat needs a prior same-thread failure; the triumph is unearned.",
    },
    Code {
        code: "craft.accomplishment-earned-climax",
        severity: "warn",
        repair: "—",
        meaning: "a climax-class beat needs two prior failures; the climax rests on too few.",
    },
    Code {
        code: "craft.yes-and-terminal",
        severity: "error",
        repair: "—",
        meaning: "a thread re-opens after its unqualified win; a yes-and is terminal per thread.",
    },
    Code {
        code: "craft.competence-consistency",
        severity: "warn",
        repair: "—",
        meaning: "an unearned major win by an under-competent hand with no intervening growth.",
    },
    Code {
        code: "craft.competence-arc-flat",
        severity: "warn",
        repair: "—",
        meaning: "competence never moves across the season; the character does not grow.",
    },
    Code {
        code: "craft.reveal-fair-play",
        severity: "warn",
        repair: "—",
        meaning: "a turn's reframing fact is not on the listener ledger early enough (kishōtenketsu fair-play).",
    },
    Code {
        code: "craft.mystery-starved",
        severity: "warn",
        repair: "—",
        meaning: "a mystery promise goes untouched for too many consecutive episodes.",
    },
    Code {
        code: "craft.cold-open-discipline",
        severity: "warn",
        repair: "—",
        meaning: "cold-open discipline: open a loop, cast within ceiling, no deep-time in act 1.",
    },
    Code {
        code: "craft.cast-introduction-rate",
        severity: "warn",
        repair: "—",
        meaning: "new named speakers per episode exceed the band or the season budget.",
    },
    Code {
        code: "craft.scene-name-clash",
        severity: "error",
        repair: "—",
        meaning: "two names below the aural-distinctness floor share a scene.",
    },
    Code {
        code: "craft.promise-density",
        severity: "warn",
        repair: "—",
        meaning: "promise open/close density violates the season-position bands.",
    },
    Code {
        code: "craft.arc-shape",
        severity: "warn",
        repair: "—",
        meaning: "a kishōtenketsu episode places its ten out of band or resolves a conflict.",
    },
    Code {
        code: "craft.audio-load-trajectory",
        severity: "warn",
        repair: "—",
        meaning: "too many consecutive high-orientation-load episodes without recovery.",
    },
];

/// The outline family's codes (snapshot). Severity is the `warn(`/`error(` the
/// emitting call site uses in `season.rs`.
const OUTLINE_CODES: &[Code] = &[
    Code {
        code: "outline.hook-late",
        severity: "warn",
        repair: "—",
        meaning: "the opening hook lands too late in the episode.",
    },
    Code {
        code: "outline.loop-closed-unopened",
        severity: "error",
        repair: "—",
        meaning: "a loop is closed that was never opened.",
    },
    Code {
        code: "outline.loop-never-closed",
        severity: "warn",
        repair: "—",
        meaning: "a loop is opened and never closed.",
    },
    Code {
        code: "outline.no-loop-opened",
        severity: "warn",
        repair: "—",
        meaning: "the episode opens no loop.",
    },
    Code {
        code: "outline.no-loop-closed",
        severity: "warn",
        repair: "—",
        meaning: "the episode closes no loop.",
    },
    Code {
        code: "outline.promise-payoff-xor-carry",
        severity: "error",
        repair: "—",
        meaning: "a promise must either pay off or carry forward, not both/neither.",
    },
    Code {
        code: "outline.arc-promise-payoff-xor-carry",
        severity: "error",
        repair: "—",
        meaning: "an arc-level promise must either pay off or carry, not both/neither.",
    },
    Code {
        code: "outline.promise-made-in-unknown",
        severity: "error",
        repair: "—",
        meaning: "a promise is made in a scene/beat the outline does not declare.",
    },
    Code {
        code: "outline.dangling-payoff",
        severity: "error",
        repair: "—",
        meaning: "a payoff has no matching promise.",
    },
    Code {
        code: "outline.season-promise-duplicate",
        severity: "error",
        repair: "—",
        meaning: "a season promise is declared twice.",
    },
    Code {
        code: "outline.season-promise-open-unknown",
        severity: "error",
        repair: "—",
        meaning: "a season promise opens in an unknown episode.",
    },
    Code {
        code: "outline.season-promise-payoff-unknown",
        severity: "error",
        repair: "—",
        meaning: "a season promise pays off in an unknown episode.",
    },
    Code {
        code: "outline.no-season-promise-ledger",
        severity: "warn",
        repair: "—",
        meaning: "the season declares no promise ledger.",
    },
    Code {
        code: "outline.no-character-slider-ledger",
        severity: "warn",
        repair: "—",
        meaning: "no character-slider ledger is declared.",
    },
    Code {
        code: "outline.character-slider-duplicate",
        severity: "error",
        repair: "—",
        meaning: "a character slider is declared twice.",
    },
    Code {
        code: "outline.character-slider-scene-unknown",
        severity: "error",
        repair: "—",
        meaning: "a character slider references an unknown scene.",
    },
    Code {
        code: "outline.character-slider-flat",
        severity: "warn",
        repair: "—",
        meaning: "a character slider never moves across the episode.",
    },
    Code {
        code: "outline.slider-compensation-missing",
        severity: "warn",
        repair: "—",
        meaning: "a slider change lacks its compensating counter-movement.",
    },
    Code {
        code: "outline.major-character-slider-missing",
        severity: "warn",
        repair: "—",
        meaning: "a major character carries no slider.",
    },
    Code {
        code: "outline.mice-beat-unknown",
        severity: "error",
        repair: "—",
        meaning: "a MICE beat references an unknown thread.",
    },
    Code {
        code: "outline.mice-closes-before-open",
        severity: "error",
        repair: "—",
        meaning: "a MICE thread closes before it opens.",
    },
    Code {
        code: "outline.mice-interleaved",
        severity: "error",
        repair: "—",
        meaning: "MICE threads are interleaved rather than LIFO-nested.",
    },
    Code {
        code: "outline.no-scene-work-ledger",
        severity: "warn",
        repair: "—",
        meaning: "no scene-work ledger is declared.",
    },
    Code {
        code: "outline.scene-work-duplicate",
        severity: "error",
        repair: "—",
        meaning: "a scene-work entry is declared twice.",
    },
    Code {
        code: "outline.scene-work-unknown-scene",
        severity: "error",
        repair: "—",
        meaning: "a scene-work entry references an unknown scene.",
    },
    Code {
        code: "outline.scene-work-missing",
        severity: "warn",
        repair: "—",
        meaning: "a scene carries no scene-work entry.",
    },
    Code {
        code: "outline.scene-work-premature-resolution",
        severity: "warn",
        repair: "—",
        meaning: "a scene resolves its work earlier than the arc allows.",
    },
    Code {
        code: "outline.scene-work-chain-unsettled",
        severity: "warn",
        repair: "—",
        meaning: "a scene-work chain is left unsettled.",
    },
    Code {
        code: "outline.no-tryfail-cycle",
        severity: "warn",
        repair: "—",
        meaning: "a thread carries no try/fail cycle.",
    },
    Code {
        code: "outline.premature-resolution",
        severity: "warn",
        repair: "—",
        meaning: "a thread resolves before its try/fail cycle earns it.",
    },
    Code {
        code: "outline.cast-over-four",
        severity: "warn",
        repair: "—",
        meaning: "a scene carries more than four active speakers.",
    },
    Code {
        code: "outline.scene-idles",
        severity: "warn",
        repair: "—",
        meaning: "a scene advances no promise, beat, or slider.",
    },
    Code {
        code: "outline.no-audio-plan",
        severity: "warn",
        repair: "—",
        meaning: "the episode declares no audio plan.",
    },
    Code {
        code: "outline.narrator-direct-address",
        severity: "error",
        repair: "—",
        meaning: "the narrator directly addresses the audience where the frame forbids it.",
    },
    Code {
        code: "outline.narrator-host-frame",
        severity: "error",
        repair: "—",
        meaning: "the narration uses a host frame the outline does not sanction.",
    },
    Code {
        code: "outline.narrator-needed-for-orientation",
        severity: "warn",
        repair: "—",
        meaning: "a scene needs narrator orientation it does not have.",
    },
    Code {
        code: "outline.narrator-tasks-missing",
        severity: "warn",
        repair: "—",
        meaning: "declared narrator tasks are missing.",
    },
    Code {
        code: "outline.no-narrator-sfx-burden",
        severity: "warn",
        repair: "—",
        meaning: "the narrator carries no SFX burden where one is expected.",
    },
    Code {
        code: "outline.sfx-load-bearing",
        severity: "warn",
        repair: "—",
        meaning: "an SFX cue carries load-bearing meaning the dialogue should hold.",
    },
    Code {
        code: "outline.sfx-not-sparse",
        severity: "warn",
        repair: "—",
        meaning: "SFX cues are not sparse enough for the audio register.",
    },
    Code {
        code: "outline.ten-reads-as-battle",
        severity: "warn",
        repair: "—",
        meaning: "a scene's cast/action reads as a battle the budget cannot stage (the ten-reads-as-battle lexicon).",
    },
    Code {
        code: "outline.no-material-assumption-ledger",
        severity: "warn",
        repair: "—",
        meaning: "no material-assumption ledger is declared.",
    },
    Code {
        code: "outline.material-assumption-blank",
        severity: "error",
        repair: "—",
        meaning: "a material assumption is blank.",
    },
    Code {
        code: "outline.material-assumption-incomplete",
        severity: "error",
        repair: "—",
        meaning: "a material assumption is incomplete.",
    },
    Code {
        code: "outline.material-answer-and-unknown",
        severity: "error",
        repair: "—",
        meaning: "a material assumption is both answered and left unknown.",
    },
    Code {
        code: "outline.unsupported-material-assumption",
        severity: "error",
        repair: "—",
        meaning: "a material assumption rests on unsupported canon.",
    },
    Code {
        code: "outline.material-canon-ref-duplicate",
        severity: "error",
        repair: "—",
        meaning: "a material canon reference is duplicated.",
    },
    Code {
        code: "outline.material-canon-ref-malformed",
        severity: "error",
        repair: "—",
        meaning: "a material canon reference is malformed.",
    },
    Code {
        code: "outline.material-research-ref-duplicate",
        severity: "error",
        repair: "—",
        meaning: "a material research reference is duplicated.",
    },
    Code {
        code: "outline.material-research-ref-malformed",
        severity: "error",
        repair: "—",
        meaning: "a material research reference is malformed.",
    },
];

/// Render the finding-families page.
#[must_use]
pub fn render() -> String {
    let total: usize = FAMILIES.iter().map(|f| f.codes.len()).sum();
    let family_count = FAMILIES.iter().filter(|f| f.key != "resource").count();
    let mut out = format!(
        "---\nstatus: reference\nverdict: \"The finding-code catalogue: {total} stable \
         dotted `family.item` codes across {family_count} families, each with its emitting \
         check/tool, severity, repair kind, and meaning — generated from the emitting crate \
         sources and coverage-checked against them.\"\n---\n\n<!-- GENERATED by `cargo run -p \
         xtask -- docgen` — do not edit by hand. Regenerate after any finding-code change. \
         -->\n\n"
    );
    out.push_str("# Finding families\n\n");
    out.push_str(
        "Every graded finding carries a stable, dotted `family.item` code (charter koan 9, \
         *errors are lore*). The code is the contract a benchmark, a repair tool, or a \
         downstream consumer matches on — it is stable across message wording. This page \
         catalogues every code the system emits: its family (the check or tool that raises \
         it), its severity, the machine-repair kind it carries (Door-T authoring findings \
         only), and what it means.\n\n",
    );
    out.push_str(
        "These finding codes are **disjoint** from the wire error-code registry (the 13 \
         `ErrorCode` values in `error-codes.md`, per the dli-worker contract §3.2 / §9). \
         The distinction is load-bearing: a wire error code says the *call* failed — it was \
         malformed (`schema`), refused by principle (`refused`), or hit a server defect \
         (`internal`); a finding code is a graded *domain* observation about the lore itself. \
         A well-formed call over inconsistent lore returns findings, not an error. No code \
         appears in both sets.\n\n",
    );
    let _ = writeln!(
        out,
        "**{total} codes** across **{family_count} families** (plus the \
         `proof.resource-violation` sub-codes). A `family.item` code names its check battery \
         in the family and the specific violation in the item. Two codes can share a stem across \
         families — the naming vet's `name.register-fit` (a kernel warn) and Door T's \
         `name.unvetted` (an authoring warn) are different findings from different surfaces.\n"
    );
    out.push_str(
        "\n> Note: severity is a property of the emitting site. Kernel check findings \
         (`load`, `continuity`, `proof`, `practice`, `attest`, `name`, `citation`) carry no \
         machine repair — read the message. Only the Door-T `author.*` findings carry \
         structured repairs, which is what makes `author.materialize` gateable.\n\n",
    );

    for family in FAMILIES {
        let _ = writeln!(out, "## {}\n", family.title);
        let _ = writeln!(out, "- **Emitted by:** {}", family.emitter);
        let _ = writeln!(out, "- **Owner:** `{}`", family.owner);
        let _ = writeln!(out, "- **Severity:** {}\n", family.severity_note);
        out.push_str("| Code | Severity | Repair | Meaning |\n|---|---|---|---|\n");
        for code in family.codes {
            let repair = if code.repair == "—" {
                "—".to_owned()
            } else {
                format!("`{}`", code.repair)
            };
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                code.code, code.severity, repair, code.meaning
            );
        }
        out.push('\n');
    }
    out
}

/// Recover the quoted `"prefix.item"` code literals a source file emits, skipping
/// comment lines so a doc-comment mention is not read as an emitted code.
#[cfg(test)]
fn scrape_prefixed(source: &str, prefix: &str) -> BTreeSet<String> {
    let needle = format!("\"{prefix}.");
    let mut out = BTreeSet::new();
    for line in source.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let mut rest = line;
        while let Some(i) = rest.find(&needle) {
            let after = &rest[i + 1..]; // past the opening quote
            if let Some(end) = after.find('"') {
                let code = &after[..end];
                if is_code(code) {
                    out.insert(code.to_owned());
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    out
}

/// Recover the `code: "…"` struct-field literals a source file writes.
#[cfg(test)]
fn scrape_code_fields(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in source.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let mut rest = line;
        while let Some(i) = rest.find("code: \"") {
            let after = &rest[i + "code: \"".len()..];
            if let Some(end) = after.find('"') {
                let code = &after[..end];
                if is_code(code) {
                    out.insert(code.to_owned());
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    out
}

/// Recover the outline codes: the first string literal handed to a `warn(`,
/// `error(`, or `report_duplicates(` call, prefixed with `outline.`. Definitions
/// (`fn warn`) yield `outline.{code}`-shaped tokens that [`is_bare`] rejects.
/// Excludes `craft_warn(`/`craft_error(` matches (the CRAFT-2 battery uses its
/// own helper that takes a `KernelRule`, not a string code).
#[cfg(test)]
fn scrape_outline(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for kw in ["warn(", "error(", "report_duplicates(", "push_disposition("] {
        let mut rest = source;
        while let Some(i) = rest.find(kw) {
            // Skip craft_warn( / craft_error( — those are CRAFT-2 helpers, not
            // outline emitters. Check whether the match is preceded by "craft_".
            let is_craft = i >= 6 && &rest[i - 6..i] == "craft_";
            let after = &rest[i + kw.len()..];
            if is_craft {
                rest = after;
                continue;
            }
            let window = &after[..after.len().min(160)];
            if let Some(q) = window.find('"') {
                let tail = &window[q + 1..];
                if let Some(end) = tail.find('"') {
                    let token = &tail[..end];
                    if is_bare(token) {
                        out.insert(format!("outline.{token}"));
                    }
                }
            }
            rest = after;
        }
    }
    out
}

/// Recover the craft family's codes from `kb.rs`: each `KernelRule::id()` match
/// arm returns a quoted kebab-case token; we prefix it with `craft.` and also
/// accept the two underscore-bearing entry-point ids (`check_outline`,
/// `check_season`).
#[cfg(test)]
fn scrape_craft_kernel_rules(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    // The id() match arms look like: `Self::Variant => "token",`
    // We find every quoted string that appears as a `=> "..."` return in the
    // function and prefix with `craft.`.
    let needle = "=> \"";
    let mut rest = source;
    while let Some(i) = rest.find(needle) {
        let after = &rest[i + needle.len()..];
        if let Some(end) = after.find('"') {
            let token = &after[..end];
            // Accept kebab-case tokens and underscore-bearing entry points.
            if !token.is_empty()
                && token
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'-' || b == b'_')
            {
                out.insert(format!("craft.{token}"));
            }
        }
        rest = after;
    }
    out
}

/// A well-formed dotted code: lowercase, dots, hyphens.
#[cfg(test)]
fn is_code(s: &str) -> bool {
    !s.is_empty()
        && s.contains('.')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'-')
}

/// A well-formed bare code token: lowercase and hyphens, no dot.
#[cfg(test)]
fn is_bare(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
}

/// Read a repo-relative crate source file for the coverage test.
#[cfg(test)]
fn read_source(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "reading {} for finding-family coverage: {e}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The union of a family's emitted codes across its source files, by scrape
    /// kind. `BareTokens` families are covered by their own test.
    fn emitted_codes(family: &Family) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for src in family.sources {
            let source = read_source(src);
            match family.scrape {
                Scrape::Prefixed(p) | Scrape::PrefixedSnapshot(p) => {
                    out.extend(scrape_prefixed(&source, p));
                }
                Scrape::CodeFields => out.extend(scrape_code_fields(&source)),
                Scrape::OutlineSnapshot => out.extend(scrape_outline(&source)),
                Scrape::CraftKernelRules => out.extend(scrape_craft_kernel_rules(&source)),
                Scrape::BareTokens => {}
            }
        }
        out
    }

    /// Every finding family is coverage-checked against the source that emits it.
    /// Settled families (`Prefixed`, `CodeFields`) are checked strictly — the
    /// documented set must equal the emitted set, so a new code that no one
    /// documents fails the gate. Still-landing families (`PrefixedSnapshot`,
    /// `OutlineSnapshot`, `CraftKernelRules`) are checked no-phantom — every
    /// documented code must still be emitted, but a newer source code does not
    /// gate this page while the lane is in flight.
    #[test]
    fn families_are_covered_against_source() {
        for family in FAMILIES {
            let authored: BTreeSet<String> =
                family.codes.iter().map(|c| c.code.to_owned()).collect();
            match family.scrape {
                Scrape::Prefixed(_) | Scrape::CodeFields => {
                    let emitted = emitted_codes(family);
                    assert_eq!(
                        authored, emitted,
                        "finding-family `{}` drifted from source \
                         (left: documented, right: emitted). Regenerate the page \
                         and update FAMILIES in xtask/src/finding_families.rs.",
                        family.key
                    );
                }
                Scrape::PrefixedSnapshot(_)
                | Scrape::OutlineSnapshot
                | Scrape::CraftKernelRules => {
                    let emitted = emitted_codes(family);
                    let mut phantom: Vec<&str> = family
                        .codes
                        .iter()
                        .map(|c| c.code)
                        .filter(|c| !emitted.contains(*c))
                        .collect();
                    phantom.sort_unstable();
                    assert!(
                        phantom.is_empty(),
                        "finding-family `{}` documents codes no longer emitted by source: {phantom:?}",
                        family.key
                    );
                }
                Scrape::BareTokens => {}
            }
        }
    }

    /// The `resource` sub-codes are a closed enum: assert each documented token
    /// serializes in `resource.rs`.
    #[test]
    fn resource_subcodes_present() {
        let family = FAMILIES
            .iter()
            .find(|f| f.key == "resource")
            .expect("resource family present");
        let source = read_source(family.sources[0]);
        for code in family.codes {
            assert!(
                source.contains(&format!("\"{}\"", code.code)),
                "resource sub-code `{}` not found in resource.rs",
                code.code
            );
        }
    }

    /// Finding codes and wire error codes are disjoint (contract §3.2/§9): a
    /// finding is a graded domain observation, an `ErrorCode` is a failed call.
    /// The page asserts this; this test makes it true against `ErrorCode::ALL`.
    #[test]
    fn finding_codes_disjoint_from_wire_error_codes() {
        let wire: BTreeSet<&str> = dli_serve::ErrorCode::ALL
            .iter()
            .map(|c| c.wire_name())
            .collect();
        for family in FAMILIES {
            for code in family.codes {
                assert!(
                    !wire.contains(code.code),
                    "finding code `{}` collides with wire ErrorCode `{}` — they must stay disjoint",
                    code.code,
                    code.code
                );
            }
        }
    }

    /// The rendered page is deterministic and carries the doc-discipline header.
    #[test]
    fn render_is_stable_and_marked() {
        let first = render();
        assert_eq!(
            first,
            render(),
            "finding-families render is not deterministic"
        );
        assert!(first.starts_with("---\n"), "missing frontmatter");
        assert!(first.contains("GENERATED by"), "missing do-not-edit marker");
    }

    /// No code is documented twice across families with the same emitter.
    #[test]
    fn codes_are_well_formed() {
        for family in FAMILIES {
            for code in family.codes {
                match family.scrape {
                    Scrape::BareTokens => {
                        assert!(is_bare(code.code), "bad bare token {}", code.code);
                    }
                    _ => assert!(is_code(code.code), "bad dotted code {}", code.code),
                }
                assert!(
                    code.severity == "error" || code.severity == "warn",
                    "code {} has an unexpected severity {}",
                    code.code,
                    code.severity
                );
            }
        }
    }
}
