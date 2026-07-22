//! The craft layer: the outline schema, its deterministic checker, the
//! season-ledger join rules, and the principle knowledge base.
//!
//! The consuming repo's outline checker is a pure structural prover over typed
//! data — the closest thing in that codebase to a kernel rule set — and its
//! natural home is the fabric's narrative layer
//! ([06-craft-and-payloads](../../../docs/spec/06-craft-and-payloads.md)). This
//! module is the faithful port: [`outline`] carries the merged season-wide
//! schema, and [`season`] carries the checker, with `fantasy.craft.outline` as
//! the **parity oracle** — every ported rule reproduces its verdict on the outline
//! fixture corpus, then the legacy checker retires (exactly as the continuity
//! checker does for the world checks).
//!
//! `CRAFT-1` ports the *legacy structural battery* (the always-on and the
//! packet-methodology and material-worldbuilding checks) — [`season::check_episode`]
//! and [`season::check_season`]. The season-wide *join* rules the merge newly makes
//! possible — accomplishment-earned, competence-consistency, reveal pacing, and the
//! marketability/clarity family — are `CRAFT-2` ([`season::check_season_rules`] over a
//! [`season::CraftLedger`]), which consumes those objects in the same linear-scan
//! shape as the story-time order-rules. The 79-entry principle knowledge base is
//! `CRAFT-3` ([`kb`]); its findings share the [`kb::KernelRule`] namespace with the
//! `CRAFT-2` rules, so every craft finding's governing principle is recovered by
//! [`kb::why`].
//!
//! # The port census — legacy code → dotted family code
//!
//! Every legacy `Violation.code` ports 1:1 into the `outline.` family (the family
//! names the check battery; the item is the legacy flat code, unchanged). All 42:
//!
//! | legacy code | dli-world code | severity | check |
//! |---|---|---|---|
//! | `promise-made-in-unknown` | `outline.promise-made-in-unknown` | error | promises |
//! | `dangling-payoff` | `outline.dangling-payoff` | error | promises |
//! | `no-character-slider-ledger` | `outline.no-character-slider-ledger` | warn¹ | sliders |
//! | `character-slider-duplicate` | `outline.character-slider-duplicate` | error | sliders |
//! | `character-slider-scene-unknown` | `outline.character-slider-scene-unknown` | error | sliders |
//! | `character-slider-flat` | `outline.character-slider-flat` | warn | sliders |
//! | `slider-compensation-missing` | `outline.slider-compensation-missing` | warn | sliders |
//! | `major-character-slider-missing` | `outline.major-character-slider-missing` | warn | sliders |
//! | `mice-beat-unknown` | `outline.mice-beat-unknown` | error | MICE |
//! | `mice-closes-before-open` | `outline.mice-closes-before-open` | error | MICE |
//! | `mice-interleaved` | `outline.mice-interleaved` | error | MICE |
//! | `no-scene-work-ledger` | `outline.no-scene-work-ledger` | warn¹ | scene-work |
//! | `scene-work-duplicate` | `outline.scene-work-duplicate` | error | scene-work |
//! | `scene-work-unknown-scene` | `outline.scene-work-unknown-scene` | error | scene-work |
//! | `scene-work-missing` | `outline.scene-work-missing` | warn¹ | scene-work |
//! | `scene-work-premature-resolution` | `outline.scene-work-premature-resolution` | warn | scene-work |
//! | `scene-work-chain-unsettled` | `outline.scene-work-chain-unsettled` | warn | scene-work |
//! | `no-tryfail-cycle` | `outline.no-tryfail-cycle` | warn | try/fail |
//! | `premature-resolution` | `outline.premature-resolution` | warn | try/fail |
//! | `cast-over-four` | `outline.cast-over-four` | warn² | scenes |
//! | `scene-idles` | `outline.scene-idles` | warn | scenes |
//! | `no-audio-plan` | `outline.no-audio-plan` | warn¹ | audio |
//! | `narrator-direct-address` | `outline.narrator-direct-address` | error | audio (D-0176) |
//! | `narrator-host-frame` | `outline.narrator-host-frame` | error | audio (D-0176) |
//! | `narrator-needed-for-orientation` | `outline.narrator-needed-for-orientation` | warn | audio |
//! | `no-narrator-sfx-burden` | `outline.no-narrator-sfx-burden` | warn | audio |
//! | `sfx-not-sparse` | `outline.sfx-not-sparse` | warn | audio (D-0179) |
//! | `sfx-load-bearing` | `outline.sfx-load-bearing` | warn | audio |
//! | `narrator-tasks-missing` | `outline.narrator-tasks-missing` | warn | audio |
//! | `no-material-assumption-ledger` | `outline.no-material-assumption-ledger` | warn³ | material |
//! | `material-assumption-incomplete` | `outline.material-assumption-incomplete` | warn³ | material |
//! | `unsupported-material-assumption` | `outline.unsupported-material-assumption` | warn | material |
//! | `no-loop-opened` | `outline.no-loop-opened` | warn | loops |
//! | `no-loop-closed` | `outline.no-loop-closed` | warn | loops |
//! | `ten-reads-as-battle` | `outline.ten-reads-as-battle` | warn² | ten (D-0052) |
//! | `hook-late` | `outline.hook-late` | warn² | hook |
//! | `no-season-promise-ledger` | `outline.no-season-promise-ledger` | warn¹ | season promises |
//! | `season-promise-duplicate` | `outline.season-promise-duplicate` | error | season promises |
//! | `season-promise-open-unknown` | `outline.season-promise-open-unknown` | error | season promises |
//! | `season-promise-payoff-unknown` | `outline.season-promise-payoff-unknown` | error | season promises |
//! | `loop-closed-unopened` | `outline.loop-closed-unopened` | error | season loops |
//! | `loop-never-closed` | `outline.loop-never-closed` | warn | season loops |
//!
//! ¹ gated by [`CheckOptions::require_methodology`](season::CheckOptions).
//! ² a [`policy`](season::policy) threshold-family finding — warn until calibrated
//!   (spec §4); the legacy severity was already warn, so parity holds.
//! ³ gated by [`CheckOptions::material_worldbuilding`](season::CheckOptions).
//!
//! ## Constructor validators → fail-closed findings
//!
//! The legacy pydantic *constructor* validators had no `Violation` code — they
//! raised `ValueError` at construction, so an invalid outline never reached the
//! checker. Serde does not run them, so they port to check findings under the
//! same family, keeping the kernel's rule that malformed input is a fail-closed
//! finding, never a panic. On well-formed outlines every one is silent, so the
//! oracle's verdict is unchanged:
//!
//! | legacy pydantic validator | dli-world code | severity |
//! |---|---|---|
//! | `Promise._payoff_or_carry` | `outline.promise-payoff-xor-carry` | error |
//! | `ArcPromise._payoff_or_carry` | `outline.arc-promise-payoff-xor-carry` | error |
//! | `MaterialAssumptionLedger._assumption_is_nonblank` | `outline.material-assumption-blank` | error |
//! | `MaterialAssumptionLedger._answers_are_not_also_unknown` | `outline.material-answer-and-unknown` | error |
//! | `MaterialAssumptionLedger._canon_refs_are_decisions` (unique) | `outline.material-canon-ref-duplicate` | error |
//! | `MaterialAssumptionLedger._canon_refs_are_decisions` (format) | `outline.material-canon-ref-malformed` | error |
//! | `MaterialAssumptionLedger._research_refs_are_labelled` (unique) | `outline.material-research-ref-duplicate` | error |
//! | `MaterialAssumptionLedger._research_refs_are_labelled` (format) | `outline.material-research-ref-malformed` | error |
//!
//! The one field constraint deliberately **not** ported as a check is the slider
//! `0..=10` bound: it is pure input validation, and every slider check compares
//! only against the [`policy`](season::policy) thresholds, which sit well inside
//! `u8`.

pub mod config;
pub mod kb;
pub mod load;
pub mod outline;
pub mod season;
pub mod worldcfg;

pub use kb::{
    Binding, Checkable, Coverage, FactoryNs, Family, Kb, KbError, KernelRule, MissionFilter, Phase,
    Principle, audit, by_id, checkable_debt, coverage, evaluate_judge, principles_binding, why,
};
pub use load::{LoadError, PrinciplePack, parse_pack, render_pack};
pub use outline::{
    Advance, ArcPromise, AudioLoad, AudioPlan, CarryScope, CharacterSlider, EpisodeOutline,
    MaterialAssumptionLedger, MaterialUnknown, MiceKind, MiceThread, NarratorMode, NarratorTask,
    Promise, PromiseKind, Scene, SceneWork, SeasonOutline, SfxLoad, SfxSemanticLoad, TryFailBeat,
    TryFailOutcome,
};
pub use season::{
    ArcShape, BeatKind, CheckOptions, CraftLedger, DEFAULT_EPISODE_POLICY, DEFAULT_THRESHOLDS,
    EpisodeMeta, EpisodePolicy, LedgerBeat, LedgerPromise, LedgerScene, Magnitude, Reveal,
    SliderState, Thresholds, Turn, check_episode, check_season, check_season_rules, worst_severity,
};

// Step 4 (craft separation): the shipped generic doctrine library.
pub use kb::{CheckableKind, DoctrineFamily, DoctrinePack, DoctrinePrinciple, Scalar};
pub use load::doctrines;

// Step 5 (craft separation): the world craft-config loaded as data, composed with
// the shipped packs into the runtime corpus.
pub use worldcfg::{
    ConfigError, ConfigLoadError, CraftConfig, FindingRuleCfg, MaterialCategoryCfg,
    NarratorModeRuleCfg, NarratorPolicyCfg, OutlineThresholdsCfg, PrecedenceTier, PrincipleOverlay,
    ProvenanceCfg, WorldPrinciple, apply, build_episode_policy, load_config, validate,
};
