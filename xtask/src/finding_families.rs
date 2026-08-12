//! The finding-families reference page: `docs/reference-generated/finding-families.md`.
//!
//! Every graded finding the system emits carries a stable, dotted `family.item`
//! code (charter koan 9, *errors are lore*). Those codes are the contract a
//! benchmark, a repair tool, or a downstream consumer matches on, and until now
//! they were scattered across the crates that raise them. This page is their one
//! catalogue: for each code, its family, the check or tool that emits it, its
//! severity, its repair kind, and what it means.
//!
//! The catalogue is authored data (the `FAMILIES` table). A coverage test parses
//! each owning Rust source, follows unambiguous immutable constant/local code
//! flow into finding constructors, rejects unsupported mutable or macro
//! emission, and asserts the authored code set matches actual emitter contexts.
//! Configurable `outline` emitters are also checked against the closed registry
//! that validates caller text rules.

use std::fmt::Write as _;

// The scrape helpers and coverage test are the only readers of the source-parity
// metadata (`sources`, `scrape`, and the `Scrape` payloads); they compile under
// `#[cfg(test)]`, so those readers — and these imports — are test-only.
#[cfg(test)]
use proc_macro2::{TokenStream, TokenTree};
#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use syn::parse::Parser as _;
#[cfg(test)]
use syn::punctuated::Punctuated;
#[cfg(test)]
use syn::visit::{self, Visit};
#[cfg(test)]
use syn::{
    Arm, Attribute, Expr, ExprCall, ExprLit, ExprMacro, FieldValue, ImplItem, ImplItemFn, Item,
    ItemConst, ItemFn, ItemMacro, Lit, Local, Member, Meta, Pat, Stmt, Token,
};

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
    /// Door-T source that also maps another registered family. Scrape every
    /// `code: "…"` field except the named prefixes, which that family owns.
    CodeFieldsExcept(&'static [&'static str]),
    /// A closed sub-code enum whose `as_str` returns bare tokens (no dotted
    /// prefix); the authored tokens are asserted present, one by one.
    BareTokens,
    /// A still-landing family scraped by `"prefix.item"` literal, covered
    /// no-phantom: every documented code must still be emitted, but a newer
    /// source code does not fail the gate (so an in-flight lane edit does not
    /// gate on this page).
    #[allow(dead_code)]
    PrefixedSnapshot(&'static str),
    /// The closed outline family: bare suffixes from the season battery plus
    /// registered dotted codes accepted from caller text-rule config.
    OutlineClosed,
    /// The craft family: every `KernelRule` used at a production emit site must
    /// resolve through `KernelRule::id()`, and every non-meta id must be emitted.
    /// Stable judge outcomes are collected from their constructors.
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
                code: "load.malformed-world-rule",
                severity: "error",
                repair: "—",
                meaning: "a bare top-level world rule is not valid Datalog and is refused instead of lowering no rule.",
            },
            Code {
                code: "load.unbound-head-variable",
                severity: "error",
                repair: "—",
                meaning: "a rule concludes about a variable that nothing in its premises names.",
            },
            Code {
                code: "load.unknown-temporal-reference",
                severity: "error",
                repair: "—",
                meaning: "a typed temporal clause names an era or event the merged world does not declare.",
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
            Code {
                code: "load.inert-declaration",
                severity: "error",
                repair: "—",
                meaning: "a declaration is accepted by the grammar but names nothing evaluable, so it would lower to no fact (DL-0029); surfaced rather than silently dropped.",
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
            Code {
                code: "continuity.fluent-contradiction",
                severity: "error",
                repair: "—",
                meaning: "a fluent slot holds two distinct values certainly at once over overlapping intervals (a state's `since`/`until` spans collide).",
            },
            Code {
                code: "continuity.fd-violation",
                severity: "error",
                repair: "—",
                meaning: "canon's own certain layer holds two distinct values for a subject on a predicate the author declared `functional`, breaking the functional dependency.",
            },
            Code {
                code: "continuity.identity-temporal-unsupported",
                severity: "error",
                repair: "—",
                meaning: "a `same-as` identity link names a declared era or event on either side — v1 refuses to unify time points (a defeasible merge over chronology indices would half-merge the timeline).",
            },
        ],
    },
    Family {
        key: "name",
        title: "name — the naming-oracle kernel vet",
        emitter: "the world-true half of the naming oracle, run by `dli.name.check` and the \
            language layer of `check`",
        owner: "dli-world :: language/derivation.rs (LangFinding)",
        severity_note: "Mixed: structural, register, uniqueness, morphology, and banned-token \
            violations are errors; aural clashes, set clashes, sayability, and confusables are \
            warnings. (Distinct from the Door-T `name.unvetted` finding below.)",
        sources: &["../crates/dli-world/src/language/derivation.rs"],
        scrape: Scrape::Prefixed("name"),
        codes: &[
            Code {
                code: "name.slot-sequence",
                severity: "error",
                repair: "—",
                meaning: "the name's slots match no name-kind pattern of its dialect.",
            },
            Code {
                code: "name.role-membership",
                severity: "error",
                repair: "—",
                meaning: "a name-part is not declared in the dialect under the role it fills.",
            },
            Code {
                code: "name.register-fit",
                severity: "error",
                repair: "—",
                meaning: "the name does not fit its declared dialect/register.",
            },
            Code {
                code: "name.phonotactics",
                severity: "error",
                repair: "—",
                meaning: "the name violates the dialect's phonotactics (vowels, endings, clusters, stress).",
            },
            Code {
                code: "name.uniqueness",
                severity: "error",
                repair: "—",
                meaning: "the name collides with an existing name in canon.",
            },
            Code {
                code: "name.fixed-token-reuse",
                severity: "error",
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
                severity: "error",
                repair: "—",
                meaning: "the name's morphology conflicts with its declared gender.",
            },
            Code {
                code: "name.sayability-cap",
                severity: "warn",
                repair: "—",
                meaning: "the name exceeds the configured sayability-tier caps.",
            },
            Code {
                code: "name.banned-in-name",
                severity: "error",
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
        emitter: "the attestation lookup a check consumes, surfaced by `dli.check` and `dli.attest.missing`",
        owner: "dli-world :: attest.rs (storage seam); dli-serve :: contract.rs + handlers.rs (document verdict)",
        severity_note: "Missing, stale, and negative judgments are errors for the consuming \
            check; a current affirmative judgment is a pass with no severity.",
        sources: &[
            "../crates/dli-world/src/attest.rs",
            "../crates/dli-serve/src/contract.rs",
            "../crates/dli-serve/src/handlers.rs",
        ],
        scrape: Scrape::Prefixed("attest"),
        codes: &[
            Code {
                code: "attest.missing",
                severity: "error",
                repair: "—",
                meaning: "the question has no attestation in the store; a sweep must pose it before a check can consume it.",
            },
            Code {
                code: "attest.stale",
                severity: "error",
                repair: "—",
                meaning: "the stored judgment no longer matches the current world fingerprint.",
            },
            Code {
                code: "attest.pass",
                severity: "—",
                repair: "—",
                meaning: "the current bounded model judgment affirms the configured proposition at grade attested.",
            },
            Code {
                code: "attest.fail",
                severity: "error",
                repair: "—",
                meaning: "the current bounded model judgment rejects the configured proposition at grade attested.",
            },
        ],
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
        owner: "dli-ledger :: ledger_check.rs (ProofFinding); dli-world :: payload.rs (proof-shadow replay)",
        severity_note: "Error: a proof finding means the spoken proof is invalid; the L-class in \
            the message (L1–L7) names which structural obligation broke.",
        sources: &[
            "../crates/dli-ledger/src/ledger_check.rs",
            "../crates/dli-world/src/payload.rs",
        ],
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
                code: "proof.unknown-role",
                severity: "error",
                repair: "—",
                meaning: "the proof header binds a role the practice does not declare.",
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
            Code {
                code: "proof.invalid-header-layout",
                severity: "error",
                repair: "—",
                meaning: "the direct proof AST header is not `<role> <actor> in <practice>`.",
            },
            Code {
                code: "proof.malformed-body",
                severity: "error",
                repair: "—",
                meaning: "captured proof clauses do not replay through the canonical parser.",
            },
            Code {
                code: "proof.invalid-close-layout",
                severity: "error",
                repair: "—",
                meaning: "the proof does not carry exactly one final close.",
            },
            Code {
                code: "proof.no-close",
                severity: "error",
                repair: "—",
                meaning: "a payload proof shadow carries an empty close atom.",
            },
            Code {
                code: "proof.unknown-verdict",
                severity: "error",
                repair: "—",
                meaning: "a payload proof shadow names an outcome outside the closed verdict set.",
            },
            Code {
                code: "proof.empty-ledger",
                severity: "error",
                repair: "—",
                meaning: "a payload proof shadow carries neither premises nor steps.",
            },
            Code {
                code: "proof.undeclared-shadow",
                severity: "error",
                repair: "—",
                meaning: "a payload proof shadow has no matching declared proof.",
            },
            Code {
                code: "proof.ambiguous-declaration",
                severity: "error",
                repair: "—",
                meaning: "a payload proof shadow resolves to more than one declaration.",
            },
            Code {
                code: "proof.shadow-mismatch",
                severity: "error",
                repair: "—",
                meaning: "a payload proof shadow differs from the declared proof body.",
            },
            Code {
                code: "proof.practice-missing",
                severity: "error",
                repair: "—",
                meaning: "the selected declared proof names no practice.",
            },
            Code {
                code: "proof.practice-unknown",
                severity: "error",
                repair: "—",
                meaning: "the selected declared proof names a practice that is not loaded.",
            },
            Code {
                code: "proof.speaker-absent",
                severity: "error",
                repair: "—",
                meaning: "the proof speaker is absent from the payload scene.",
            },
        ],
    },
    Family {
        key: "anchor",
        title: "anchor — payload quote integrity",
        emitter: "the fat-payload verifier, run by `check-draft` / `dli.check-draft`",
        owner: "dli-world :: payload.rs",
        severity_note: "All error: an extracted source anchor must be non-empty, verbatim, and on its claimed line.",
        sources: &["../crates/dli-world/src/payload.rs"],
        scrape: Scrape::Prefixed("anchor"),
        codes: ANCHOR_CODES,
    },
    Family {
        key: "factcheck",
        title: "factcheck — payload claim standing",
        emitter: "the claim-relative certain-layer checks inside `check-draft` and Door T",
        owner: "dli-world :: payload.rs + check/factcheck.rs; dli-serve :: author/reads.rs + author/tools.rs",
        severity_note: "All error: supported claims are silent; only contradicted and unsupported claims emit findings.",
        sources: &[
            "../crates/dli-world/src/payload.rs",
            "../crates/dli-serve/src/author/reads.rs",
            "../crates/dli-serve/src/author/tools.rs",
        ],
        scrape: Scrape::Prefixed("factcheck"),
        codes: FACTCHECK_CODES,
    },
    Family {
        key: "payload",
        title: "payload — structured-shadow integrity",
        emitter: "the fat-payload shape and provenance battery inside `check-draft`",
        owner: "dli-world :: payload.rs",
        severity_note: "All error: malformed, stale, incomplete, or referentially invalid structured shadow data fails closed.",
        sources: &["../crates/dli-world/src/payload.rs"],
        scrape: Scrape::Prefixed("payload"),
        codes: PAYLOAD_CODES,
    },
    Family {
        key: "phase",
        title: "phase — payload cycle observations",
        emitter: "the scene-phase replay inside `check-draft`",
        owner: "dli-world :: payload.rs",
        severity_note: "All error: cycle observations must resolve and form a contiguous walk.",
        sources: &["../crates/dli-world/src/payload.rs"],
        scrape: Scrape::Prefixed("phase"),
        codes: PHASE_CODES,
    },
    Family {
        key: "lexicon",
        title: "lexicon — deterministic banned-word replay",
        emitter: "the payload lexicon replay inside `check-draft`",
        owner: "dli-world :: payload.rs",
        severity_note: "All error: supplied hits must be well formed and exactly match the kernel scan.",
        sources: &["../crates/dli-world/src/payload.rs"],
        scrape: Scrape::Prefixed("lexicon"),
        codes: LEXICON_CODES,
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
        severity_note: "Error for the affected claim: lowering keeps checking the rest of the \
            world, but forged authority fails that claim closed.",
        sources: &[
            "../crates/dli-eval/src/world.rs",
            "../crates/dli-world/src/lib.rs",
        ],
        scrape: Scrape::Prefixed("citation"),
        codes: &[Code {
            code: "citation.unknown-decision",
            severity: "error",
            repair: "—",
            meaning: "a claim cites a decision the corpus never declares; the claim is failed closed.",
        }],
    },
    Family {
        key: "author",
        title: "author — Door-T authoring findings",
        emitter: "the `author.*` MCP tools (Door T)",
        owner: "dli-serve :: author/tools.rs + author/reads.rs (Finding)",
        severity_note: "Per-code severity gates `author.materialize`: an `error` blocks it, a \
            `warn` passes. These are the findings that carry structured machine repairs.",
        sources: &[
            "../crates/dli-serve/src/author/tools.rs",
            "../crates/dli-serve/src/author/reads.rs",
            "../crates/dli-serve/src/handlers.rs",
        ],
        scrape: Scrape::CodeFieldsExcept(&["factcheck."]),
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
                code: "event.era-missing",
                severity: "error",
                repair: "—",
                meaning: "an event has no era anchor; every event must occur during a declared era.",
            },
            Code {
                code: "event.era-kind",
                severity: "error",
                repair: "—",
                meaning: "an event's era anchor names an entity that is not an era.",
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
                meaning: "below contract 1.3.0 only: a citation is refused outright and nothing is staged. Scrubbed at 1.3.0 and above, where a citation is recorded and verified instead (DL-0054, DL-0058); every earlier wire keeps this refusal permanently.",
            },
            Code {
                code: "citation.unknown-decision",
                severity: "error",
                repair: "resolve-citation",
                meaning: "a cited decision id does not resolve against the decisions the corpus declares. The engine records the attribution a caller supplies but verifies that what it cites exists: the unknown decision's variable is left open, so the claim can never read certain, and materialize is barred until the id is repaired.",
            },
            Code {
                code: "workspace.base-missing",
                severity: "error",
                repair: "—",
                meaning: "the workspace's project document base is no longer loaded.",
            },
            Code {
                code: "world.uncompilable",
                severity: "error",
                repair: "—",
                meaning: "the staged declarations do not compile into a checkable world.",
            },
        ],
    },
    Family {
        key: "craft",
        title: "craft — the season-ledger join-rule battery",
        emitter: "`check_season_rules` / `check_episode` and judge-attestation replay in `dli-world::craft`",
        owner: "dli-world :: craft/season.rs + craft/kb.rs (WorldFinding, via KernelRule)",
        severity_note: "Mixed error/warn per rule: structural verdicts (accomplishment-earned, \
            yes-and-terminal, scene-name-clash) are error; band/threshold rules are warn until \
            calibrated; judge outcomes are warn. Rule severities are defaults that caller finding \
            policy may override. The set is exact against the stable emitters.",
        sources: &[
            "../crates/dli-world/src/craft/kb.rs",
            "../crates/dli-world/src/craft/season.rs",
        ],
        scrape: Scrape::CraftKernelRules,
        codes: CRAFT_CODES,
    },
    Family {
        key: "outline",
        title: "outline — the season/outline conformance battery",
        emitter: "the season-outline checks, run by `check-draft`'s craft battery",
        owner: "dli-world :: craft/season.rs + payload.rs + craft/kb.rs",
        severity_note: "Mixed error/warn defaults per rule; caller finding policy may override \
            them. The list is exact against both hardcoded emit sites and the closed registry \
            used to validate caller text-rule codes.",
        sources: &[
            "../crates/dli-world/src/craft/season.rs",
            "../crates/dli-world/src/payload.rs",
            "../crates/dli-world/src/craft/kb.rs",
        ],
        scrape: Scrape::OutlineClosed,
        codes: OUTLINE_CODES,
        // OUTLINE_CODES follows this table.
    },
];

macro_rules! error_code {
    ($code:literal, $meaning:literal) => {
        Code {
            code: $code,
            severity: "error",
            repair: "—",
            meaning: $meaning,
        }
    };
}

const ANCHOR_CODES: &[Code] = &[
    error_code!(
        "anchor.empty-quote",
        "an extracted item carries no quote to anchor."
    ),
    error_code!(
        "anchor.quote-missing",
        "the claimed quote does not occur in the draft."
    ),
    error_code!(
        "anchor.quote-misplaced",
        "the quote occurs in the draft, but not on its claimed line."
    ),
];

const FACTCHECK_CODES: &[Code] = &[
    error_code!(
        "factcheck.contradicted",
        "the claim conflicts with certain or explicitly rejected lore."
    ),
    error_code!(
        "factcheck.unsupported",
        "canon bears no certain support for the claim."
    ),
];

const PAYLOAD_CODES: &[Code] = &[
    error_code!(
        "payload.stale",
        "the payload source digest no longer matches the draft."
    ),
    error_code!(
        "payload.unparseable-claim",
        "a claim shadow is not a well-formed atom."
    ),
    error_code!(
        "payload.runtime-continuity-invalid",
        "a caller-owned runtime continuity fact is not a well-formed atom."
    ),
    error_code!(
        "payload.outline-episode-unknown",
        "the payload episode has no matching reviewed outline."
    ),
    error_code!(
        "payload.proof-unknown-scene",
        "a proof shadow references an undeclared scene."
    ),
    error_code!(
        "payload.scene-id-empty",
        "a payload scene carries no identifier."
    ),
    error_code!(
        "payload.scene-duplicate",
        "a payload scene identifier is declared more than once."
    ),
    error_code!(
        "payload.cast-duplicate",
        "a scene repeats an on-mic cast member."
    ),
    error_code!(
        "payload.presence-conflict",
        "a cast member is repeated or marked both on- and off-mic."
    ),
    error_code!(
        "payload.location-unknown",
        "a scene location is blank or undeclared."
    ),
    error_code!(
        "payload.delta-incomplete",
        "a scene delta lacks a required subject, fact, scene, or line."
    ),
    error_code!(
        "payload.delta-unknown-subject",
        "a scene delta names an undeclared subject."
    ),
    error_code!(
        "payload.delta-unknown-scene",
        "a scene delta references an undeclared scene."
    ),
    error_code!(
        "payload.delta-line-out-of-range",
        "a scene delta points beyond the end of the draft."
    ),
    error_code!(
        "payload.beat-incomplete",
        "a beat lacks a required id, scene, thread, or line."
    ),
    error_code!(
        "payload.beat-duplicate",
        "a beat identifier is declared more than once."
    ),
    error_code!(
        "payload.beat-unknown-scene",
        "a beat references an undeclared scene."
    ),
    error_code!(
        "payload.beat-unknown-performer",
        "a beat names an undeclared performer."
    ),
    error_code!(
        "payload.beat-assistance-without-performer",
        "an assisted beat names no performer."
    ),
    error_code!(
        "payload.promise-id-empty",
        "an opened or closed promise carries no identifier."
    ),
    error_code!(
        "payload.promise-duplicate",
        "a promise repeats within its opened or closed list."
    ),
    error_code!("payload.name-empty", "an introduced name is empty."),
    error_code!("payload.name-duplicate", "an introduced name is repeated."),
    error_code!(
        "payload.episode-empty",
        "payload metadata carries no episode identifier."
    ),
    error_code!(
        "payload.source-file-empty",
        "payload metadata carries no source filename."
    ),
    error_code!(
        "payload.schema-unsupported",
        "the extractor schema is not the supported payload version."
    ),
    error_code!(
        "payload.extracted-invalid",
        "the extraction date is not a valid calendar date."
    ),
    error_code!(
        "payload.source-word-count",
        "the declared source word count differs from the draft."
    ),
    error_code!(
        "payload.register-token-count",
        "the register-token count exceeds the draft token count."
    ),
    error_code!(
        "payload.extractor-missing",
        "a non-empty extracted field has no extractor provenance."
    ),
    error_code!(
        "payload.extractor-field-unknown",
        "the extractor manifest names a field outside payload/v1."
    ),
    error_code!(
        "payload.extractor-model-invalid",
        "an extractor record carries no model identifier."
    ),
    error_code!(
        "payload.extractor-prompt-invalid",
        "an extractor prompt digest is not a lowercase SHA-256 address."
    ),
    error_code!("payload.cast-empty", "a scene cast entry is empty."),
    error_code!(
        "payload.cast-unknown",
        "a scene names an undeclared cast member."
    ),
];

const PHASE_CODES: &[Code] = &[
    error_code!(
        "phase.unknown-cycle",
        "a scene observes a cycle that is not declared."
    ),
    error_code!("phase.empty-cycle", "a declared cycle carries no spokes."),
    error_code!(
        "phase.unknown-spoke",
        "a scene observes a spoke the cycle does not carry."
    ),
    error_code!(
        "phase.illegal-walk",
        "successive scene observations jump outside a cycle's contiguous walk."
    ),
];

const LEXICON_CODES: &[Code] = &[
    error_code!(
        "lexicon.empty-hit",
        "the extracted banned-hit list contains an empty entry."
    ),
    error_code!(
        "lexicon.duplicate-hit",
        "the extracted banned-hit list repeats an entry."
    ),
    error_code!(
        "lexicon.banned-hit",
        "the deterministic scan found banned vocabulary in the draft."
    ),
    error_code!(
        "lexicon.scan-mismatch",
        "the extracted banned-hit list differs from the kernel scan."
    ),
];

/// The craft family's codes — the 13 season-rule [`KernelRule`](dli_world::craft::kb::KernelRule)
/// ids emitted by the season-ledger join battery (`CRAFT-2`) via `craft_warn`/`craft_error`,
/// plus the two stable judge-attestation outcomes.
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
    Code {
        code: "craft.judge-attested",
        severity: "warn",
        repair: "—",
        meaning: "a judge-bound principle has a current stored attestation; the message names the principle and answer.",
    },
    Code {
        code: "craft.attestation-missing",
        severity: "warn",
        repair: "—",
        meaning: "a judge-bound principle has no current stored attestation, so the kernel refuses to guess.",
    },
];

/// The closed outline-family codes. Severity follows the hardcoded emitter or
/// the default severity of the registered configurable rule.
const OUTLINE_CODES: &[Code] = &[
    Code {
        code: "outline.beat-unaccounted",
        severity: "error",
        repair: "—",
        meaning: "the outline declares a beat absent from the payload.",
    },
    Code {
        code: "outline.beat-undeclared",
        severity: "error",
        repair: "—",
        meaning: "the payload carries a beat absent from the outline.",
    },
    Code {
        code: "outline.cast-unaccounted",
        severity: "error",
        repair: "—",
        meaning: "the outline casts a speaker absent from the payload scene.",
    },
    Code {
        code: "outline.cast-undeclared",
        severity: "error",
        repair: "—",
        meaning: "the payload scene casts a speaker absent from the outline.",
    },
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
        code: "outline.promise-unaccounted",
        severity: "error",
        repair: "—",
        meaning: "the outline declares a promise absent from the payload.",
    },
    Code {
        code: "outline.promise-undeclared",
        severity: "error",
        repair: "—",
        meaning: "the payload carries a promise absent from the outline.",
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
        severity: "error or warn (narrator policy)",
        repair: "—",
        meaning: "the narrator directly addresses the audience where the frame forbids it.",
    },
    Code {
        code: "outline.narrator-host-frame",
        severity: "error or warn (narrator policy)",
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
        severity: "warn",
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
        severity: "warn",
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
    let dotted: usize = FAMILIES
        .iter()
        .filter(|family| family.key != "resource")
        .map(|family| family.codes.len())
        .sum();
    let resource = FAMILIES
        .iter()
        .find(|family| family.key == "resource")
        .map_or(0, |family| family.codes.len());
    let family_count = FAMILIES
        .iter()
        .filter(|family| family.key != "resource")
        .count();
    let mut out = format!(
        "---\nstatus: reference\nverdict: \"The finding-code catalogue: {dotted} stable \
         dotted `family.item` codes across {family_count} families, plus {resource} resource \
         sub-codes, each with its emitting check/tool, severity, repair kind, and meaning — \
         generated from the emitting crate sources and coverage-checked against them.\"\n---\n\n\
         <!-- GENERATED by `cargo run -p \
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
        "**{dotted} dotted codes** across **{family_count} families**, plus **{resource} \
         `proof.resource-violation` sub-codes**. A `family.item` code names its check battery in \
         the family and the specific violation in the item. Two codes can share a stem across \
         families — the naming vet's `name.register-fit` (a kernel error) and Door T's \
         `name.unvetted` (an authoring warn) are different findings from different surfaces.\n",
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
            let severity = if matches!(family.key, "craft" | "outline")
                && matches!(code.severity, "error" | "warn")
            {
                format!("{} (default)", code.severity)
            } else {
                code.severity.to_owned()
            };
            let repair = if code.repair == "—" {
                "—".to_owned()
            } else {
                format!("`{}`", code.repair)
            };
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                code.code, severity, repair, code.meaning
            );
        }
        out.push('\n');
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

/// Parsed production Rust relevant to finding-code coverage.
#[cfg(test)]
#[derive(Default)]
struct ProductionSource {
    emitted_codes: BTreeSet<String>,
    emitted_severities: BTreeMap<String, BTreeSet<String>>,
    code_fields: BTreeSet<String>,
    outline_calls: BTreeSet<String>,
    outline_registry: BTreeSet<String>,
    craft_rule_ids: BTreeMap<String, String>,
    craft_rule_uses: BTreeSet<String>,
    craft_rule_severities: BTreeMap<String, BTreeSet<String>>,
    resource_subcodes: BTreeSet<String>,
    unsupported: Vec<String>,
}

/// Recover the quoted `"prefix.item"` code literals production Rust emits.
#[cfg(test)]
fn scrape_prefixed(source: &ProductionSource, prefix: &str) -> BTreeSet<String> {
    let prefix = format!("{prefix}.");
    source
        .emitted_codes
        .iter()
        .filter(|code| code.starts_with(&prefix) && is_code(code))
        .cloned()
        .collect()
}

/// Recover the `code: "…"` struct-field literals production Rust writes.
#[cfg(test)]
fn scrape_code_fields(source: &ProductionSource) -> BTreeSet<String> {
    source.code_fields.clone()
}

/// Recover outline call-site suffixes, prefixed with `outline.`.
#[cfg(test)]
fn scrape_outline(source: &ProductionSource) -> BTreeSet<String> {
    let mut out = source.outline_calls.clone();
    out.extend(source.outline_registry.iter().cloned());
    out.extend(scrape_prefixed(source, "outline"));
    out
}

/// A well-formed dotted code: lowercase, dots, hyphens.
#[cfg(test)]
fn is_code(s: &str) -> bool {
    !s.is_empty()
        && s.contains('.')
        && s.split('.').all(|part| !part.is_empty())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'-')
}

#[cfg(test)]
fn is_finding_code_candidate(s: &str) -> bool {
    is_code(s)
        && FAMILIES
            .iter()
            .filter(|family| family.key != "resource")
            .any(|family| s.starts_with(&format!("{}.", family.key)))
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum CfgValue {
    True,
    False,
    Unknown,
}

#[cfg(test)]
fn cfg_value(meta: &Meta) -> CfgValue {
    match meta {
        Meta::Path(path) if path.is_ident("test") => CfgValue::False,
        Meta::Path(_) | Meta::NameValue(_) => CfgValue::Unknown,
        Meta::List(list) => {
            let Ok(nested) =
                Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
            else {
                return CfgValue::Unknown;
            };
            if list.path.is_ident("all") {
                nested.iter().fold(CfgValue::True, |left, right| {
                    match (left, cfg_value(right)) {
                        (CfgValue::False, _) | (_, CfgValue::False) => CfgValue::False,
                        (CfgValue::True, CfgValue::True) => CfgValue::True,
                        _ => CfgValue::Unknown,
                    }
                })
            } else if list.path.is_ident("any") {
                nested.iter().fold(CfgValue::False, |left, right| {
                    match (left, cfg_value(right)) {
                        (CfgValue::True, _) | (_, CfgValue::True) => CfgValue::True,
                        (CfgValue::False, CfgValue::False) => CfgValue::False,
                        _ => CfgValue::Unknown,
                    }
                })
            } else if list.path.is_ident("not") && nested.len() == 1 {
                match cfg_value(&nested[0]) {
                    CfgValue::True => CfgValue::False,
                    CfgValue::False => CfgValue::True,
                    CfgValue::Unknown => CfgValue::Unknown,
                }
            } else {
                CfgValue::Unknown
            }
        }
    }
}

#[cfg(test)]
fn is_test_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("cfg") {
            return attr
                .parse_args::<Meta>()
                .is_ok_and(|meta| cfg_value(&meta) == CfgValue::False);
        }
        if !attr.path().is_ident("cfg_attr") {
            return false;
        }
        let Meta::List(list) = &attr.meta else {
            return false;
        };
        let Ok(args) = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
        else {
            return false;
        };
        let Some(condition) = args.first() else {
            return false;
        };
        cfg_value(condition) == CfgValue::True
            && args.iter().skip(1).any(|nested| {
                let Meta::List(cfg) = nested else {
                    return false;
                };
                cfg.path.is_ident("cfg")
                    && syn::parse2::<Meta>(cfg.tokens.clone())
                        .is_ok_and(|meta| cfg_value(&meta) == CfgValue::False)
            })
    })
}

#[cfg(test)]
fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

#[cfg(test)]
fn literal_string(expr: &Expr, constants: &BTreeMap<String, String>) -> Option<String> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Some(value.value()),
        Expr::Macro(ExprMacro { mac, .. }) if mac.path.is_ident("concat") => {
            let args = Punctuated::<Expr, Token![,]>::parse_terminated
                .parse2(mac.tokens.clone())
                .ok()?;
            let mut joined = String::new();
            for arg in args {
                joined.push_str(&literal_string(&arg, constants)?);
            }
            Some(joined)
        }
        Expr::MethodCall(call)
            if call.args.is_empty()
                && matches!(
                    call.method.to_string().as_str(),
                    "to_owned" | "to_string" | "into"
                ) =>
        {
            literal_string(&call.receiver, constants)
        }
        Expr::Group(group) => literal_string(&group.expr, constants),
        Expr::Paren(paren) => literal_string(&paren.expr, constants),
        Expr::Reference(reference) => literal_string(&reference.expr, constants),
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .and_then(|segment| constants.get(&segment.ident.to_string()))
            .cloned(),
        _ => None,
    }
}

#[cfg(test)]
fn macro_literal_strings(tokens: TokenStream, constants: &BTreeMap<String, String>) -> Vec<String> {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let mut strings = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if matches!(&tokens[index], TokenTree::Ident(ident) if ident == "concat")
            && matches!(tokens.get(index + 1), Some(TokenTree::Punct(punct)) if punct.as_char() == '!')
            && let Some(TokenTree::Group(group)) = tokens.get(index + 2)
            && let Ok(args) = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(group.stream())
        {
            let joined = args
                .iter()
                .map(|arg| literal_string(arg, constants))
                .collect::<Option<Vec<_>>>();
            if let Some(parts) = joined {
                strings.push(parts.concat());
                index += 3;
                continue;
            }
        }
        match &tokens[index] {
            TokenTree::Group(group) => {
                strings.extend(macro_literal_strings(group.stream(), constants));
            }
            TokenTree::Literal(literal) => {
                if let Ok(Lit::Str(value)) = syn::parse_str::<Lit>(&literal.to_string()) {
                    strings.push(value.value());
                }
            }
            TokenTree::Ident(_) | TokenTree::Punct(_) => {}
        }
        index += 1;
    }
    strings
}

#[cfg(test)]
fn stmt_expr(statement: &Stmt) -> Option<&Expr> {
    match statement {
        Stmt::Expr(expr, _) => Some(expr),
        _ => None,
    }
}

#[cfg(test)]
fn expression_strings(
    expr: &Expr,
    constants: &BTreeMap<String, String>,
    locals: &BTreeMap<String, Expr>,
    visiting: &mut BTreeSet<String>,
) -> BTreeSet<String> {
    if let Some(value) = literal_string(expr, constants) {
        return BTreeSet::from([value]);
    }
    match expr {
        Expr::Path(path) => {
            let Some(name) = path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
            else {
                return BTreeSet::new();
            };
            let Some(value) = locals.get(&name) else {
                return BTreeSet::new();
            };
            if !visiting.insert(name.clone()) {
                return BTreeSet::new();
            }
            let values = expression_strings(value, constants, locals, visiting);
            visiting.remove(&name);
            values
        }
        Expr::Match(expression) => expression
            .arms
            .iter()
            .filter(|arm| !is_test_only(&arm.attrs))
            .flat_map(|arm| expression_strings(&arm.body, constants, locals, visiting))
            .collect(),
        Expr::If(expression) => {
            let mut values = expression
                .then_branch
                .stmts
                .last()
                .and_then(stmt_expr)
                .map_or_else(BTreeSet::new, |tail| {
                    expression_strings(tail, constants, locals, visiting)
                });
            if let Some((_, otherwise)) = &expression.else_branch {
                values.extend(expression_strings(otherwise, constants, locals, visiting));
            }
            values
        }
        Expr::Block(expression) => expression
            .block
            .stmts
            .last()
            .and_then(stmt_expr)
            .map_or_else(BTreeSet::new, |tail| {
                expression_strings(tail, constants, locals, visiting)
            }),
        Expr::Call(expression) => expression
            .args
            .iter()
            .flat_map(|arg| expression_strings(arg, constants, locals, visiting))
            .collect(),
        Expr::Try(expression) => expression_strings(&expression.expr, constants, locals, visiting),
        Expr::MethodCall(expression) => {
            expression_strings(&expression.receiver, constants, locals, visiting)
        }
        Expr::Macro(expression) => {
            if let Ok(args) = Punctuated::<Expr, Token![,]>::parse_terminated
                .parse2(expression.mac.tokens.clone())
            {
                args.iter()
                    .flat_map(|arg| expression_strings(arg, constants, locals, visiting))
                    .collect()
            } else {
                macro_literal_strings(expression.mac.tokens.clone(), constants)
                    .into_iter()
                    .collect()
            }
        }
        Expr::Group(expression) => {
            expression_strings(&expression.expr, constants, locals, visiting)
        }
        Expr::Paren(expression) => {
            expression_strings(&expression.expr, constants, locals, visiting)
        }
        Expr::Reference(expression) => {
            expression_strings(&expression.expr, constants, locals, visiting)
        }
        _ => BTreeSet::new(),
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ImplContext {
    #[default]
    Other,
    KernelRule,
    ResourceViolationCode,
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
enum Collection {
    #[default]
    None,
    OutlineRegistry,
}

#[cfg(test)]
struct ProductionVisitor {
    source: ProductionSource,
    constants: BTreeMap<String, String>,
    locals: BTreeMap<String, Expr>,
    impl_context: ImplContext,
    collection: Collection,
}

#[cfg(test)]
impl ProductionVisitor {
    fn record_string(&mut self, value: String) {
        match self.collection {
            Collection::OutlineRegistry if value.starts_with("outline.") && is_code(&value) => {
                self.source.outline_registry.insert(value);
            }
            Collection::None | Collection::OutlineRegistry => {}
        }
    }

    fn values(&self, expression: &Expr) -> BTreeSet<String> {
        expression_strings(
            expression,
            &self.constants,
            &self.locals,
            &mut BTreeSet::new(),
        )
    }

    fn record_codes(&mut self, values: impl IntoIterator<Item = String>) {
        for value in values {
            if is_code(&value) {
                self.source.emitted_codes.insert(value);
            }
        }
    }

    fn record_severities(&mut self, values: impl IntoIterator<Item = String>, severity: &str) {
        for value in values {
            if is_code(&value) {
                self.source
                    .emitted_severities
                    .entry(value)
                    .or_default()
                    .insert(severity.to_owned());
            }
        }
    }
}

#[cfg(test)]
impl<'ast> Visit<'ast> for ProductionVisitor {
    fn visit_item(&mut self, item: &'ast Item) {
        if !is_test_only(item_attrs(item)) {
            visit::visit_item(self, item);
        }
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if is_test_only(&item.attrs) {
            return;
        }
        let prior = self.impl_context;
        self.impl_context = ImplContext::Other;
        if let syn::Type::Path(path) = item.self_ty.as_ref()
            && let Some(name) = path.path.segments.last()
        {
            if name.ident == "KernelRule" {
                self.impl_context = ImplContext::KernelRule;
            } else if name.ident == "ResourceViolationCode" {
                self.impl_context = ImplContext::ResourceViolationCode;
            }
        }
        visit::visit_item_impl(self, item);
        self.impl_context = prior;
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        let attrs: &[Attribute] = match item {
            ImplItem::Const(item) => &item.attrs,
            ImplItem::Fn(item) => &item.attrs,
            ImplItem::Type(item) => &item.attrs,
            ImplItem::Macro(item) => &item.attrs,
            _ => &[],
        };
        if !is_test_only(attrs) {
            visit::visit_impl_item(self, item);
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        if is_test_only(&item.attrs) {
            return;
        }
        let prior_locals = std::mem::take(&mut self.locals);
        let name = item.sig.ident.to_string();
        if self.impl_context == ImplContext::KernelRule && name == "id" {
            if let Some(expression) = tail_match(item) {
                for arm in &expression.arms {
                    let values = self.values(&arm.body);
                    let mut variants = Vec::new();
                    pattern_variants(&arm.pat, &mut variants);
                    for variant in variants {
                        for value in &values {
                            self.source
                                .craft_rule_ids
                                .insert(variant.clone(), value.clone());
                        }
                    }
                }
            }
        } else if self.impl_context == ImplContext::ResourceViolationCode && name == "as_str" {
            if let Some(expression) = item.block.stmts.last().and_then(stmt_expr) {
                self.source.resource_subcodes.extend(
                    self.values(expression)
                        .into_iter()
                        .filter(|value| is_bare(value)),
                );
            }
        } else if name == "code" {
            if let Some(expression) = item.block.stmts.last().and_then(stmt_expr) {
                let values = self.values(expression);
                self.record_codes(values);
            }
        } else {
            visit::visit_impl_item_fn(self, item);
        }
        self.locals = prior_locals;
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        if is_test_only(&item.attrs) {
            return;
        }
        let prior_locals = std::mem::take(&mut self.locals);
        if item.sig.ident == "code" {
            if let Some(expression) = item.block.stmts.last().and_then(stmt_expr) {
                let values = self.values(expression);
                self.record_codes(values);
            }
        } else {
            visit::visit_item_fn(self, item);
        }
        self.locals = prior_locals;
    }

    fn visit_item_macro(&mut self, item: &'ast ItemMacro) {
        if !is_test_only(&item.attrs) && item.ident.is_some() {
            self.source.unsupported.push(
                "production macro definitions are unsupported in finding-owner sources".to_owned(),
            );
        }
    }

    fn visit_item_const(&mut self, item: &'ast ItemConst) {
        if is_test_only(&item.attrs) {
            return;
        }
        let prior = self.collection;
        if item.ident == "OUTLINE_FINDING_CODES" {
            self.collection = Collection::OutlineRegistry;
        }
        visit::visit_item_const(self, item);
        self.collection = prior;
    }

    fn visit_arm(&mut self, arm: &'ast Arm) {
        if !is_test_only(&arm.attrs) {
            visit::visit_arm(self, arm);
        }
    }

    fn visit_local(&mut self, local: &'ast Local) {
        if is_test_only(&local.attrs) {
            return;
        }
        if let Pat::Ident(ident) = &local.pat
            && let Some(init) = &local.init
        {
            self.locals
                .insert(ident.ident.to_string(), (*init.expr).clone());
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        if is_test_only(&assignment.attrs) {
            return;
        }
        let assigned = match assignment.left.as_ref() {
            Expr::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        };
        let prior_values = assigned
            .as_ref()
            .and_then(|name| self.locals.get(name))
            .map(|expression| self.values(expression))
            .unwrap_or_default();
        let new_values = self.values(&assignment.right);
        if prior_values
            .iter()
            .chain(&new_values)
            .any(|value| is_code(value))
        {
            self.source.unsupported.push(format!(
                "mutable finding-code assignment to `{}` is unsupported",
                assigned.as_deref().unwrap_or("<expression>")
            ));
        }
        if let Some(name) = assigned {
            self.locals.remove(&name);
        }
        visit::visit_expr(self, &assignment.right);
    }

    fn visit_expr_macro(&mut self, expr: &'ast ExprMacro) {
        if is_test_only(&expr.attrs) {
            return;
        }
        let macro_name = expr
            .mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string());
        let macro_values: BTreeSet<String> = if let Ok(args) =
            Punctuated::<Expr, Token![,]>::parse_terminated.parse2(expr.mac.tokens.clone())
        {
            args.iter().flat_map(|arg| self.values(arg)).collect()
        } else {
            macro_literal_strings(expr.mac.tokens.clone(), &self.constants)
                .into_iter()
                .collect()
        };
        if !matches!(macro_name.as_deref(), Some("vec" | "concat" | "format"))
            && macro_values
                .iter()
                .any(|value| is_finding_code_candidate(value))
        {
            self.source.unsupported.push(format!(
                "macro invocation `{}` carries a finding code",
                macro_name.as_deref().unwrap_or("<unknown>")
            ));
        }
        if !matches!(self.collection, Collection::None)
            && let Some(value) = literal_string(&Expr::Macro(expr.clone()), &self.constants)
        {
            self.record_string(value);
            return;
        }
        if let Ok(args) =
            Punctuated::<Expr, Token![,]>::parse_terminated.parse2(expr.mac.tokens.clone())
        {
            for arg in &args {
                self.visit_expr(arg);
            }
            return;
        }
        if !matches!(self.collection, Collection::None) {
            for value in macro_literal_strings(expr.mac.tokens.clone(), &self.constants) {
                self.record_string(value);
            }
        }
        visit::visit_expr_macro(self, expr);
    }

    fn visit_expr_lit(&mut self, expr: &'ast ExprLit) {
        if !is_test_only(&expr.attrs) {
            if !matches!(self.collection, Collection::None)
                && let Lit::Str(value) = &expr.lit
            {
                self.record_string(value.value());
            }
            visit::visit_expr_lit(self, expr);
        }
    }

    fn visit_field_value(&mut self, field: &'ast FieldValue) {
        if is_test_only(&field.attrs) {
            return;
        }
        if matches!(&field.member, Member::Named(name) if name == "code") {
            for code in self.values(&field.expr) {
                if is_code(&code) {
                    self.source.code_fields.insert(code.clone());
                    self.source.emitted_codes.insert(code);
                }
            }
        }
        visit::visit_field_value(self, field);
    }

    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        if is_test_only(&expression.attrs) {
            return;
        }
        let is_wire_finding = expression
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Finding");
        if is_wire_finding {
            let code = expression.fields.iter().find_map(|field| {
                matches!(&field.member, Member::Named(name) if name == "code")
                    .then(|| self.values(&field.expr))
            });
            let severity = expression.fields.iter().find_map(|field| {
                if !matches!(&field.member, Member::Named(name) if name == "severity") {
                    return None;
                }
                let Expr::Path(path) = &field.expr else {
                    return None;
                };
                path.path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string().to_lowercase())
            });
            if let (Some(codes), Some(severity)) = (code, severity) {
                self.record_severities(codes, &severity);
            }
        }
        visit::visit_expr_struct(self, expression);
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if is_test_only(&call.attrs) {
            return;
        }
        let function = match call.func.as_ref() {
            Expr::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        };
        if matches!(function.as_deref(), Some("craft_warn" | "craft_error"))
            && let Some(Expr::Path(rule)) = call.args.first()
            && let Some(variant) = rule.path.segments.last()
        {
            let variant = variant.ident.to_string();
            self.source.craft_rule_uses.insert(variant.clone());
            self.source
                .craft_rule_severities
                .entry(variant)
                .or_default()
                .insert(if function.as_deref() == Some("craft_error") {
                    "error".to_owned()
                } else {
                    "warn".to_owned()
                });
        }
        let is_finding_emitter = matches!(
            function.as_deref(),
            Some("warn" | "error" | "report_duplicates" | "push_disposition")
        );
        if is_finding_emitter {
            for arg in &call.args {
                let values = self.values(arg);
                if !values.iter().any(|value| is_bare(value) || is_code(value)) {
                    continue;
                }
                for token in values {
                    let code = if is_bare(&token) {
                        let code = format!("outline.{token}");
                        self.source.outline_calls.insert(code.clone());
                        Some(code)
                    } else if is_code(&token) {
                        self.source.emitted_codes.insert(token.clone());
                        Some(token)
                    } else {
                        None
                    };
                    if let (Some(code), Some(severity)) = (
                        code,
                        match function.as_deref() {
                            Some("error" | "report_duplicates") => Some("error"),
                            Some("warn") => Some("warn"),
                            Some("push_disposition") => Some("error or warn (narrator policy)"),
                            _ => None,
                        },
                    ) {
                        self.record_severities([code], severity);
                    }
                }
                break;
            }
        }
        visit::visit_expr_call(self, call);
    }
}

#[cfg(test)]
#[derive(Default)]
struct ConstVisitor {
    expressions: BTreeMap<String, Expr>,
    duplicates: BTreeSet<String>,
}

#[cfg(test)]
impl<'ast> Visit<'ast> for ConstVisitor {
    fn visit_item(&mut self, item: &'ast Item) {
        if !is_test_only(item_attrs(item)) {
            visit::visit_item(self, item);
        }
    }

    fn visit_item_const(&mut self, item: &'ast ItemConst) {
        if !is_test_only(&item.attrs) {
            let name = item.ident.to_string();
            if self
                .expressions
                .insert(name.clone(), (*item.expr).clone())
                .is_some()
            {
                self.duplicates.insert(name);
            }
            visit::visit_item_const(self, item);
        }
    }
}

#[cfg(test)]
fn production_source(source: &str) -> ProductionSource {
    let file = syn::parse_file(source).expect("finding-family source must parse as Rust");
    let mut const_visitor = ConstVisitor::default();
    const_visitor.visit_file(&file);
    let mut constants = BTreeMap::new();
    loop {
        let before = constants.len();
        for (name, expr) in &const_visitor.expressions {
            if let Some(value) = literal_string(expr, &constants) {
                constants.insert(name.clone(), value);
            }
        }
        if constants.len() == before {
            break;
        }
    }
    let mut visitor = ProductionVisitor {
        source: ProductionSource::default(),
        constants,
        locals: BTreeMap::new(),
        impl_context: ImplContext::Other,
        collection: Collection::None,
    };
    visitor.source.unsupported.extend(
        const_visitor
            .duplicates
            .into_iter()
            .map(|name| format!("duplicate scoped constant name `{name}` is unsupported")),
    );
    visitor.visit_file(&file);
    visitor.source
}

#[cfg(test)]
fn pattern_variants(pattern: &Pat, out: &mut Vec<String>) {
    match pattern {
        Pat::Struct(pattern) => {
            if let Some(segment) = pattern.path.segments.last() {
                out.push(segment.ident.to_string());
            }
        }
        Pat::TupleStruct(pattern) => {
            if let Some(segment) = pattern.path.segments.last() {
                out.push(segment.ident.to_string());
            }
        }
        Pat::Path(pattern) => {
            if let Some(segment) = pattern.path.segments.last() {
                out.push(segment.ident.to_string());
            }
        }
        Pat::Or(pattern) => {
            for case in &pattern.cases {
                pattern_variants(case, out);
            }
        }
        Pat::Wild(_) => out.push("*".to_owned()),
        _ => {}
    }
}

#[cfg(test)]
fn tail_match(method: &ImplItemFn) -> Option<&syn::ExprMatch> {
    match method.block.stmts.last()? {
        Stmt::Expr(Expr::Match(expression), _) => Some(expression),
        _ => None,
    }
}

#[cfg(test)]
fn lang_finding_severities(source: &str) -> BTreeMap<String, String> {
    let file = syn::parse_file(source).expect("language derivation source must parse as Rust");
    let mut codes_by_variant = BTreeMap::new();
    let mut severity_by_variant = BTreeMap::new();
    let mut default_severity = None;

    for item in &file.items {
        let Item::Impl(item) = item else {
            continue;
        };
        let syn::Type::Path(self_type) = item.self_ty.as_ref() else {
            continue;
        };
        if self_type
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != "LangFinding")
        {
            continue;
        }
        for item in &item.items {
            let ImplItem::Fn(method) = item else {
                continue;
            };
            let Some(expression) = tail_match(method) else {
                continue;
            };
            if method.sig.ident == "code" {
                for arm in &expression.arms {
                    let Some(code) = literal_string(&arm.body, &BTreeMap::new()) else {
                        continue;
                    };
                    let mut variants = Vec::new();
                    pattern_variants(&arm.pat, &mut variants);
                    for variant in variants {
                        codes_by_variant.insert(variant, code.clone());
                    }
                }
            } else if method.sig.ident == "severity" {
                for arm in &expression.arms {
                    let Expr::Path(severity) = arm.body.as_ref() else {
                        continue;
                    };
                    let Some(severity) = severity.path.segments.last() else {
                        continue;
                    };
                    let severity = severity.ident.to_string().to_lowercase();
                    let mut variants = Vec::new();
                    pattern_variants(&arm.pat, &mut variants);
                    for variant in variants {
                        if variant == "*" {
                            default_severity = Some(severity.clone());
                        } else {
                            severity_by_variant.insert(variant, severity.clone());
                        }
                    }
                }
            }
        }
    }

    codes_by_variant
        .into_iter()
        .filter_map(|(variant, code)| {
            severity_by_variant
                .get(&variant)
                .or(default_severity.as_ref())
                .cloned()
                .map(|severity| (code, severity))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The union of a family's emitted codes across its source files, by scrape
    /// kind. `BareTokens` families are covered by their own test.
    fn emitted_codes(family: &Family) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut craft_rule_ids = BTreeMap::new();
        let mut craft_rule_uses = BTreeSet::new();
        for src in family.sources {
            let source = read_source(src);
            let source = production_source(&source);
            assert!(
                source.unsupported.is_empty(),
                "finding source {src} uses unsupported emission constructs: {:?}",
                source.unsupported
            );
            match family.scrape {
                Scrape::Prefixed(p) | Scrape::PrefixedSnapshot(p) => {
                    out.extend(scrape_prefixed(&source, p));
                }
                Scrape::CodeFieldsExcept(excluded) => {
                    let mut codes = scrape_code_fields(&source);
                    codes.retain(|code| !excluded.iter().any(|prefix| code.starts_with(prefix)));
                    out.extend(codes);
                }
                Scrape::OutlineClosed => {
                    out.extend(scrape_outline(&source));
                }
                Scrape::CraftKernelRules => {
                    out.extend(scrape_prefixed(&source, "craft"));
                    craft_rule_ids.extend(source.craft_rule_ids);
                    craft_rule_uses.extend(source.craft_rule_uses);
                }
                Scrape::BareTokens => {}
            }
        }
        if matches!(family.scrape, Scrape::CraftKernelRules) {
            for variant in &craft_rule_uses {
                let id = craft_rule_ids
                    .get(variant)
                    .unwrap_or_else(|| panic!("emitted KernelRule::{variant} has no id() mapping"));
                out.insert(format!("craft.{id}"));
            }
            for (variant, id) in &craft_rule_ids {
                if !matches!(id.as_str(), "check_outline" | "check_season") {
                    assert!(
                        craft_rule_uses.contains(variant),
                        "KernelRule::{variant} (`{id}`) has no production craft emitter"
                    );
                }
            }
        }
        out
    }

    fn emitted_severities(family: &Family) -> BTreeMap<String, BTreeSet<String>> {
        let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut craft_rule_ids = BTreeMap::new();
        let mut craft_rule_severities: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for src in family.sources {
            let source = production_source(&read_source(src));
            assert!(
                source.unsupported.is_empty(),
                "finding source {src} uses unsupported emission constructs: {:?}",
                source.unsupported
            );
            for (code, severities) in source.emitted_severities {
                out.entry(code).or_default().extend(severities);
            }
            craft_rule_ids.extend(source.craft_rule_ids);
            for (variant, severities) in source.craft_rule_severities {
                craft_rule_severities
                    .entry(variant)
                    .or_default()
                    .extend(severities);
            }
        }
        if matches!(family.scrape, Scrape::CraftKernelRules) {
            for (variant, severities) in craft_rule_severities {
                let id = craft_rule_ids
                    .get(&variant)
                    .unwrap_or_else(|| panic!("emitted KernelRule::{variant} has no id() mapping"));
                out.entry(format!("craft.{id}"))
                    .or_default()
                    .extend(severities);
            }
        }
        out
    }

    /// Every finding family is coverage-checked against the source that emits it.
    /// Settled families (`Prefixed`, `CodeFieldsExcept`) are checked strictly — the
    /// documented set must equal the emitted set, so a new code that no one
    /// documents fails the gate. Only explicitly snapshot families remain
    /// no-phantom while their owning lane is in flight.
    #[test]
    fn families_are_covered_against_source() {
        for family in FAMILIES {
            let authored: BTreeSet<String> =
                family.codes.iter().map(|c| c.code.to_owned()).collect();
            match family.scrape {
                Scrape::Prefixed(_)
                | Scrape::CodeFieldsExcept(_)
                | Scrape::OutlineClosed
                | Scrape::CraftKernelRules => {
                    let emitted = emitted_codes(family);
                    assert_eq!(
                        authored, emitted,
                        "finding-family `{}` drifted from source \
                         (left: documented, right: emitted). Regenerate the page \
                         and update FAMILIES in xtask/src/finding_families.rs.",
                        family.key
                    );
                }
                Scrape::PrefixedSnapshot(_) => {
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

    /// The `resource` sub-codes are the exact production return values of the
    /// closed enum's `as_str` method.
    #[test]
    fn resource_subcodes_present() {
        let family = FAMILIES
            .iter()
            .find(|f| f.key == "resource")
            .expect("resource family present");
        let source = read_source(family.sources[0]);
        let source = production_source(&source);
        let authored: BTreeSet<String> = family
            .codes
            .iter()
            .map(|code| code.code.to_owned())
            .collect();
        assert_eq!(
            authored, source.resource_subcodes,
            "resource sub-code catalogue drifted from ResourceViolationCode::as_str"
        );
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
        assert!(
            first.contains("204 stable dotted `family.item` codes across 15 families, plus 4 resource sub-codes"),
            "headline must distinguish dotted codes from bare resource sub-codes"
        );
    }

    #[test]
    fn production_scanner_is_rust_aware() {
        let source = production_source(
            r#"
                // "catalogue.line-comment"
                /* "catalogue.block-comment" */
                #[cfg(all(test, unix))]
                #[allow(dead_code)]
                mod tests {
                    const TEST_ONLY: &str = "catalogue.test-only";
                }

                #[cfg(test)]
                #[allow(dead_code)]
                fn helper() {
                    error("catalogue.helper", "test only");
                }

                #[cfg(not(test))]
                fn non_test() {
                    error("catalogue.non-test", "production");
                }

                #[cfg(any(test, unix))]
                fn conditional_production() {
                    error("catalogue.conditional", "production on unix");
                }

                #[cfg_attr(not(test), cfg(test))]
                fn cfg_attr_test_only() {
                    error("catalogue.cfg-attr-test-only", "test only");
                }

                fn constructed() {
                    error(concat!("catalogue.", "constructed"), "production");
                }

                fn variable_routed(flag: bool) {
                    let routed = match flag {
                        true => Some("catalogue.variable-a"),
                        false => Some("catalogue.variable-b"),
                    };
                    error(routed?, "production");
                }

                const DEAD: &str = "catalogue.dead";

                fn after_test_module() {
                    error("catalogue.after", "production");
                }

                enum Probe { Test, AttrTest, Production }
                impl Probe {
                    fn code(&self) -> &'static str {
                        match self {
                            #[cfg(test)]
                            Self::Test => "catalogue.arm-test-only",
                            #[cfg_attr(not(test), cfg(test))]
                            Self::AttrTest => "catalogue.arm-cfg-attr-test-only",
                            Self::Production => "catalogue.arm-production",
                        }
                    }
                }
            "#,
        );
        assert_eq!(
            scrape_prefixed(&source, "catalogue"),
            BTreeSet::from([
                "catalogue.after".to_owned(),
                "catalogue.arm-production".to_owned(),
                "catalogue.conditional".to_owned(),
                "catalogue.constructed".to_owned(),
                "catalogue.non-test".to_owned(),
                "catalogue.variable-a".to_owned(),
                "catalogue.variable-b".to_owned(),
            ])
        );
    }

    #[test]
    fn production_scanner_recovers_constructed_code_fields() {
        let source = production_source(
            r#"
                struct Finding { code: &'static str }
                const INDIRECT: &str = concat!("author.", "indirect");
                fn finding() -> Finding {
                    Finding { code: INDIRECT }
                }
            "#,
        );
        assert_eq!(
            scrape_code_fields(&source),
            BTreeSet::from(["author.indirect".to_owned()])
        );
    }

    #[test]
    fn duplicate_scoped_constants_fail_closed() {
        let source = production_source(
            r#"
                mod emitted {
                    const CODE: &str = "catalogue.actual";
                    fn finding() {
                        error(CODE, "production");
                    }
                }
                mod dead {
                    const CODE: &str = "catalogue.dead";
                }
            "#,
        );
        assert!(
            source
                .unsupported
                .iter()
                .any(|message| message.contains("duplicate scoped constant name `CODE`")),
            "ambiguous bare constant resolution must fail closed"
        );
    }

    #[test]
    fn mutable_code_assignment_fails_closed() {
        let source = production_source(
            r#"
                fn finding() {
                    let mut code = "catalogue.initial";
                    code = "catalogue.actual";
                    error(code, "production");
                }
            "#,
        );
        assert!(
            source
                .unsupported
                .iter()
                .any(|message| message.contains("mutable finding-code assignment")),
            "mutable code flow must not be reported as exact immutable coverage"
        );
        assert!(
            scrape_prefixed(&source, "catalogue").is_empty(),
            "invalidated mutable flow must not retain a stale initializer"
        );
    }

    #[test]
    fn expression_flow_collects_every_match_branch() {
        let expression: Expr = syn::parse_str(
            r#"match flag {
                true => Some("author.branch-a"),
                false => Some("author.branch-b"),
            }"#,
        )
        .expect("match expression parses");
        assert_eq!(
            expression_strings(
                &expression,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &mut BTreeSet::new()
            ),
            BTreeSet::from(["author.branch-a".to_owned(), "author.branch-b".to_owned()])
        );
    }

    #[test]
    fn macro_wrapped_emitters_are_rejected_not_missed() {
        let source = production_source(
            r#"
                macro_rules! emit {
                    ($code:expr) => { WorldFinding::error($code, "message") };
                }
                fn finding() {
                    emit!("author.macro-routed");
                }
            "#,
        );
        assert!(
            !source.unsupported.is_empty(),
            "macro-defined finding emission must require explicit scanner support"
        );
        assert!(
            scrape_prefixed(&source, "author").is_empty(),
            "unsupported macro emission must not masquerade as covered"
        );
    }

    #[test]
    fn name_severities_match_the_emitter() {
        let family = FAMILIES
            .iter()
            .find(|family| family.key == "name")
            .expect("name family present");
        let source = read_source(family.sources[0]);
        let emitted = lang_finding_severities(&source);
        let authored: BTreeMap<String, String> = family
            .codes
            .iter()
            .map(|code| (code.code.to_owned(), code.severity.to_owned()))
            .collect();
        assert_eq!(
            authored, emitted,
            "name finding severities drifted from LangFinding::severity"
        );
    }

    #[test]
    fn direct_severities_match_the_emitters() {
        let runtime_checked = BTreeMap::from([
            ("outline.beat-unaccounted", "error"),
            ("outline.beat-undeclared", "error"),
            ("outline.promise-unaccounted", "error"),
            ("outline.promise-undeclared", "error"),
            ("outline.ten-reads-as-battle", "warn"),
        ]);
        for family in FAMILIES.iter().filter(|family| {
            matches!(
                family.key,
                "continuity"
                    | "anchor"
                    | "factcheck"
                    | "payload"
                    | "phase"
                    | "lexicon"
                    | "author"
                    | "craft"
                    | "outline"
            )
        }) {
            let authored: BTreeMap<&str, &str> = family
                .codes
                .iter()
                .map(|code| (code.code, code.severity))
                .collect();
            let emitted = emitted_severities(family);
            assert!(
                !emitted.is_empty(),
                "finding-family `{}` exposed no source-derived severities",
                family.key
            );
            let missing = authored
                .keys()
                .filter(|code| {
                    !emitted.contains_key(**code) && !runtime_checked.contains_key(**code)
                })
                .copied()
                .collect::<BTreeSet<_>>();
            assert!(
                missing.is_empty(),
                "finding-family `{}` has codes with no source-derived severity: {missing:?}",
                family.key
            );
            for (code, expected) in authored {
                if let Some(severities) = emitted.get(code) {
                    assert_eq!(
                        *severities,
                        BTreeSet::from([expected.to_owned()]),
                        "finding `{code}` severity drifted from its production constructor"
                    );
                } else if let Some(runtime_severity) = runtime_checked.get(code) {
                    assert_eq!(
                        expected, *runtime_severity,
                        "finding `{code}` drifted from its runtime severity assertion"
                    );
                }
            }
        }
    }

    #[test]
    fn uniform_family_severity_contracts_are_closed() {
        for family in FAMILIES.iter().filter(|family| {
            matches!(
                family.key,
                "load" | "attest" | "practice" | "proof" | "citation"
            )
        }) {
            for code in family.codes {
                let expected = if code.code == "attest.pass" {
                    "—"
                } else {
                    "error"
                };
                assert_eq!(
                    code.severity, expected,
                    "finding `{}` drifted from the `{}` family severity contract",
                    code.code, family.key
                );
            }
        }
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
                    code.severity == "error"
                        || code.severity == "warn"
                        || code.severity == "error or warn (narrator policy)"
                        || (code.severity == "—"
                            && code
                                .code
                                .rsplit_once('.')
                                .is_some_and(|(_, suffix)| suffix == "pass")),
                    "code {} has an unexpected severity {}",
                    code.code,
                    code.severity
                );
            }
        }
    }
}
