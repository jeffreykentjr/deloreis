//! World craft-config — the factory OUTPUT, loaded as data (design §1.2, §2.4, §4).
//!
//! Step 5 of the craft-separation DAG flips ownership: the compiled 107-principle
//! corpus is no longer a `dli-world` const. Instead the engine ships the generic
//! doctrine packs ([`doctrines`](super::load::doctrines)) plus this **mechanism**,
//! and a world supplies its craft config as data under `worlds/<world>/craft/*`.
//! [`apply`] composes the two: each enabled [`DoctrinePrinciple`] is folded with its
//! optional [`PrincipleOverlay`] (the world ruling that rides on it), and the
//! world's own [`WorldPrinciple`]s (the config-only rows, including the precedence
//! tier) are appended. The resulting [`AppliedCraft`] retains the runtime
//! [`Principle`] corpus, precedence policy, and external registries.
//!
//! The boundary is compiler-checked: a [`DoctrinePrinciple`] cannot carry a mission
//! filter, a citation, a binding symbol, or [`Family::Pillar`] — all of that lives
//! here, in world config. `dli-world` therefore ships **zero** world content; the
//! `TPtH` corpus is reconstructed by `apply(load_config("worlds/tpth/craft"),
//! doctrines())`, proven against the committed pre-separation fields in the
//! `craft_tpth_roundtrip` integration test.
//!
//! ## How `apply` reconstructs a principle
//!
//! For a shipped [`DoctrinePrinciple`] `dp` and its optional overlay `o`:
//!
//! - **no overlay** (a pure-generic row): the runtime principle is `dp` verbatim —
//!   [`MissionFilter::Pass`], family from [`DoctrineFamily`](super::kb::DoctrineFamily),
//!   and a binding derived
//!   from `dp`'s [`CheckableKind`] (`none` → [`Binding::Declaration`],
//!   `deterministic-kernel` → [`Binding::Rule`] over `dp.kernel_rule`).
//! - **with overlay**: `dp`'s world-agnostic body is the base, and the overlay
//!   supplies the world's bits — the [`mission_filter`](PrincipleOverlay::mission_filter),
//!   the runtime [`binding`](PrincipleOverlay::binding) (the concrete factory battery
//!   or judge dimension the shipped record could not name), the pillar-tier promotion
//!   ([`in_precedence_tier`](PrincipleOverlay::in_precedence_tier) → [`Family::Pillar`]),
//!   and — when the world's authored wording diverges from the shipped generic
//!   statement/source — a [`statement`](PrincipleOverlay::statement) /
//!   [`source`](PrincipleOverlay::source) override.
//!
//! A [`WorldPrinciple`] is config-only (no generic kernel exists) and converts
//! straight to a runtime [`Principle`].

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use super::config::{
    ArcShapeCatalog, DEFAULT_ARC_SHAPES, DEFAULT_FINDING_POLICY, DEFAULT_MATERIAL_CATALOG,
    DEFAULT_NARRATOR_POLICY, DEFAULT_PROVENANCE, Disposition, FindingPolicy, FindingRule,
    MaterialCatalog, MaterialCategory, NarratorModeRule, NarratorPolicy, ProvenanceFormats,
    TurnSlot,
};
use super::kb::{
    Binding, CheckableKind, DoctrinePack, DoctrinePrinciple, Family, MissionFilter, Phase,
    Principle, Scalar, is_outline_finding_code,
};
use super::outline::{MaterialCategoryId, NarratorMode};
use super::season::{self, EpisodePolicy, TextMatchRule, Thresholds, TokenPair};
use crate::check::Severity;

#[allow(clippy::trivially_copy_pass_by_ref)] // serde `skip_serializing_if` needs `fn(&T)`.
fn is_false(b: &bool) -> bool {
    !*b
}

// ---------------------------------------------------------------------------
// Factory records (design §1.2)
// ---------------------------------------------------------------------------

/// A world ruling that rides on a shipped [`DoctrinePrinciple`] by id — the config
/// half of a generic-plus-overlay principle (design §1.2).
///
/// It carries exactly what the shipped record cannot: the world's mission filter,
/// its provenance citations, the concrete runtime binding (the factory battery or
/// judge dimension the generic record only typed as a *kind*), the pillar-tier
/// promotion, and — when the world's authored wording diverges from the shipped
/// generic body — a statement/source override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrincipleOverlay {
    /// The shipped principle this overlay rides on; must resolve to an enabled pack.
    pub principle_id: Cow<'static, str>,
    /// How the mission filters the received principle (default [`MissionFilter::Pass`]).
    #[serde(default)]
    pub mission_filter: MissionFilter,
    /// The provenance citations (`D-nnnn` / canon anchors) that justify the ruling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<Cow<'static, str>>,
    /// The concrete runtime binding — the factory battery / judge dimension / kernel
    /// rule the composed principle checks by.
    pub binding: Binding,
    /// Promote this generic principle into the world's precedence tier (→
    /// [`Family::Pillar`]).
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_precedence_tier: bool,
    /// A world-specific statement that overrides the shipped generic wording (absent
    /// when the shipped statement already reads world-agnostic and unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement: Option<Cow<'static, str>>,
    /// A world-specific source/citation string that overrides the shipped source
    /// (absent when unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Cow<'static, str>>,
    /// Overrides for the shipped record's generic default parameters (metadata; the
    /// runtime [`Principle`] record carries no parameters).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub param_overrides: BTreeMap<Cow<'static, str>, Scalar>,
}

/// A config-only principle — a world row with no shipped generic kernel (design
/// §1.2). It converts straight to a runtime [`Principle`]; its
/// [`family`](Self::family) may be [`Family::Pillar`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldPrinciple {
    /// The unique kebab-case slug.
    pub id: Cow<'static, str>,
    /// The family (may be [`Family::Pillar`] — the world's precedence tier).
    pub family: Family,
    /// The principle, stated in the world's own terms (world content allowed).
    pub statement: Cow<'static, str>,
    /// The provenance — a canon anchor or `D-nnnn` ruling.
    pub source: Cow<'static, str>,
    /// How the mission filters it (default [`MissionFilter::Pass`]).
    #[serde(default)]
    pub mission_filter: MissionFilter,
    /// The production phases it bears on.
    pub phases: Cow<'static, [Phase]>,
    /// How its check binds.
    pub binding: Binding,
    /// The provenance citations that justify the row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<Cow<'static, str>>,
    /// Parameters consumed by this principle's binding.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<Cow<'static, str>, Scalar>,
    /// Exact kernel finding codes governed by this principle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finding_codes: Vec<Cow<'static, str>>,
}

impl WorldPrinciple {
    /// The runtime [`Principle`] this config row composes into.
    #[must_use]
    pub fn to_principle(&self) -> Principle {
        Principle {
            id: self.id.clone(),
            family: self.family,
            statement: self.statement.clone(),
            source: self.source.clone(),
            mission_filter: self.mission_filter.clone(),
            phases: self.phases.clone(),
            binding: self.binding.clone(),
            citations: self.citations.clone(),
            parameters: self.parameters.clone(),
            finding_codes: self.finding_codes.clone(),
        }
    }
}

/// The world's precedence tier — the generic pillar mechanism, world-populated.
/// The tier that wins a conflict plus the standing-rule text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrecedenceTier {
    /// The tier name (`pillar`).
    pub name: Cow<'static, str>,
    /// What the tier wins over (`all-other-families`).
    pub wins_over: Cow<'static, str>,
    /// The standing-rule statement (the `pillars-win-standing-rule` meta-rule).
    pub standing_rule: Cow<'static, str>,
}

/// The citation (+ optional severity override) a finding code carries — the loadable
/// form of [`super::config::FindingRule`] (design §3 item 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FindingRuleCfg {
    /// The dotted finding code this rule governs.
    pub code: Cow<'static, str>,
    /// The citations attached to the finding, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<Cow<'static, str>>,
    /// An optional severity override; `None` keeps the code's intrinsic severity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
}

/// The disposition a single narrator mode earns — loadable form of
/// [`super::config::NarratorModeRule`] (design §3 item 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NarratorModeRuleCfg {
    /// The narrator mode this rule governs.
    pub mode: NarratorMode,
    /// Its disposition.
    pub disposition: Disposition,
}

/// The narrator-framing policy — loadable form of [`super::config::NarratorPolicy`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NarratorPolicyCfg {
    /// Per-mode dispositions; modes absent here are [`Disposition::Allowed`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mode_rules: Vec<NarratorModeRuleCfg>,
    /// The disposition a direct-audience-address plan earns.
    #[serde(default)]
    pub direct_address: Disposition,
}

/// The reference-format patterns provenance guards accept — loadable form of
/// [`super::config::ProvenanceFormats`] (design §3 item 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceCfg {
    /// The canon-decision prefix (`D-`).
    pub canon_prefix: Cow<'static, str>,
    /// The exact digit count following the canon prefix.
    pub canon_digits: usize,
    /// Accepted research-URL scheme prefixes, tried in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub research_url_schemes: Vec<Cow<'static, str>>,
    /// The research-id prefix (`RR-`).
    pub research_id_prefix: Cow<'static, str>,
    /// The exact digit count following the research-id prefix.
    pub research_id_digits: usize,
    /// The research-doc path prefix.
    pub research_path_prefix: Cow<'static, str>,
    /// The digit count following the research-doc path prefix.
    pub research_path_digits: usize,
    /// The research-doc path suffix.
    pub research_path_suffix: Cow<'static, str>,
}

/// One material-planning category and whether a worldbuilding review requires it —
/// loadable form of [`super::config::MaterialCategory`] (design §3 item 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialCategoryCfg {
    /// The category key.
    pub category: MaterialCategoryId,
    /// Whether a material-worldbuilding review requires it.
    pub required: bool,
}

/// The world's outline scalars + turn slot — the loadable companion to
/// [`super::season::EpisodePolicy`]'s scalars and [`super::config::ArcShapeCatalog`]
/// (design §3 items 2, 8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutlineThresholdsCfg {
    /// The scene speaking-cast ceiling.
    pub scene_cast_ceiling: usize,
    /// The opening-hook ceiling in seconds.
    pub hook_seconds_ceiling: u32,
    /// The flat-slider ceiling.
    pub slider_flat_ceiling: u8,
    /// The low-slider floor.
    pub slider_low_floor: u8,
    /// Which kishōtenketsu slot the battle-scan reads as the turn.
    pub turn_slot: TurnSlot,
}

/// One adjacent-token pattern in a configured lexical rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TokenPairCfg {
    /// Allowed first tokens.
    pub leads: Vec<Cow<'static, str>>,
    /// Required second token.
    pub tail: Cow<'static, str>,
}

/// A caller-defined lexical finding over one outline slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextMatchRuleCfg {
    /// Full `outline.*` finding code.
    pub code: Cow<'static, str>,
    /// Finding message.
    pub message: Cow<'static, str>,
    /// Outline slot to scan.
    pub slot: TurnSlot,
    /// Single-token matches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub single: Vec<Cow<'static, str>>,
    /// Adjacent-token matches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<TokenPairCfg>,
}

/// A world's full craft config — the aggregate the engine loads as data and
/// [`apply`]s over the shipped doctrine packs (design §1.2, §2.4).
///
/// Only [`enabled_packs`](Self::enabled_packs), [`overlays`](Self::overlays), and
/// [`world_principles`](Self::world_principles) bear on the composed principle
/// corpus; the remaining fields are season/outline policy consumed by the
/// relevant check entry points (design §2.3).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CraftConfig {
    /// The doctrine packs this world turns on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(schema_with = "unique_pack_names_schema")]
    pub enabled_packs: Vec<Cow<'static, str>>,
    /// The precedence tier (the pillar mechanism, world-populated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precedence_tier: Option<PrecedenceTier>,
    /// The factory-battery namespaces this world runs prose checks through.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub factory_namespaces: Vec<super::kb::FactoryNs>,
    /// The world's judge registry (judge-dimension names).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub judge_kinds: Vec<Cow<'static, str>>,
    /// The overlays riding on shipped generic principles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<PrincipleOverlay>,
    /// The config-only principle rows (incl. the pillars).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub world_principles: Vec<WorldPrinciple>,
    /// Per-finding-code citations / severity overrides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finding_policy: Vec<FindingRuleCfg>,
    /// The narrator-framing policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrator_policy: Option<NarratorPolicyCfg>,
    /// The provenance reference-format patterns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProvenanceCfg>,
    /// The material-planning catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_catalog: Vec<MaterialCategoryCfg>,
    /// The calibrated season-ledger thresholds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_thresholds: Option<Thresholds>,
    /// The outline scalars + turn slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_thresholds: Option<OutlineThresholdsCfg>,
    /// Caller-owned lexical checks over outline slots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_rules: Vec<TextMatchRuleCfg>,
}

fn unique_pack_names_schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema = Vec::<Cow<'static, str>>::json_schema(generator);
    schema.insert("uniqueItems".to_owned(), true.into());
    schema
}

impl CraftConfig {
    /// The generic config: every shipped pack enabled, no overlays, no world
    /// principles, no world policy. [`apply`]ing it over [`doctrines`](super::load::doctrines)
    /// yields the world-agnostic doctrine corpus the engine defaults to
    /// ([`Kb::builtin`](super::kb::Kb::builtin)).
    #[must_use]
    pub fn generic(packs: &[DoctrinePack]) -> Self {
        Self {
            enabled_packs: packs.iter().map(|p| p.pack.clone()).collect(),
            ..Self::default()
        }
    }
}

// ---------------------------------------------------------------------------
// apply / validate (design §2.4, §4)
// ---------------------------------------------------------------------------

/// Derive the runtime binding of a shipped principle that carries no overlay: `none`
/// → [`Binding::Declaration`], `deterministic-kernel` → [`Binding::Rule`]. A
/// factory/judge kind with no overlay to name its symbol has no runnable check, so
/// it is surfaced as [`Binding::Declaration`] (declaration-only) under the generic
/// default.
fn derive_binding(dp: &DoctrinePrinciple) -> Binding {
    match dp.checkable_kind {
        CheckableKind::None | CheckableKind::DeterministicProse | CheckableKind::Judge => {
            Binding::Declaration
        }
        CheckableKind::DeterministicKernel => {
            dp.kernel_rule.map_or(Binding::Declaration, Binding::Rule)
        }
    }
}

/// Compose a shipped [`DoctrinePrinciple`] with its optional [`PrincipleOverlay`]
/// into the runtime [`Principle`] (see the module docs for the exact rule).
fn compose(dp: &DoctrinePrinciple, overlay: Option<&PrincipleOverlay>) -> Principle {
    match overlay {
        None => Principle {
            id: dp.id.clone(),
            family: dp.family(),
            statement: dp.statement.clone(),
            source: dp.source.clone(),
            mission_filter: MissionFilter::Pass,
            phases: dp.phases.clone(),
            binding: derive_binding(dp),
            citations: Vec::new(),
            parameters: dp.default_params.clone(),
            finding_codes: dp.finding_codes.clone(),
        },
        Some(o) => {
            let mut parameters = dp.default_params.clone();
            parameters.extend(o.param_overrides.clone());
            Principle {
                id: dp.id.clone(),
                family: if o.in_precedence_tier {
                    Family::Pillar
                } else {
                    dp.family()
                },
                statement: o.statement.clone().unwrap_or_else(|| dp.statement.clone()),
                source: o.source.clone().unwrap_or_else(|| dp.source.clone()),
                mission_filter: o.mission_filter.clone(),
                phases: dp.phases.clone(),
                binding: o.binding.clone(),
                citations: o.citations.clone(),
                parameters,
                finding_codes: dp.finding_codes.clone(),
            }
        }
    }
}

/// A fully applied caller craft configuration.
///
/// Registries and precedence remain available to callers instead of disappearing
/// after validation; [`principles`](Self::principles) is the selected runtime KB.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedCraft {
    principles: Vec<Principle>,
    /// The caller's precedence policy.
    pub precedence_tier: Option<PrecedenceTier>,
    /// Declared external factory namespaces.
    pub factory_namespaces: Vec<super::kb::FactoryNs>,
    /// Declared attestation judge kinds.
    pub judge_kinds: Vec<Cow<'static, str>>,
}

impl AppliedCraft {
    /// The selected runtime principle corpus.
    #[must_use]
    pub fn principles(&self) -> &[Principle] {
        &self.principles
    }

    /// Consume the applied config and return its principle corpus.
    #[must_use]
    pub fn into_principles(self) -> Vec<Principle> {
        self.principles
    }
}

impl std::ops::Deref for AppliedCraft {
    type Target = [Principle];

    fn deref(&self) -> &Self::Target {
        self.principles()
    }
}

/// Compose a world's craft config with the shipped doctrine packs into the runtime
/// craft configuration (design §2.4, §4).
///
/// Every enabled pack's principles are folded with their overlays (in pack order),
/// then the config's [`WorldPrinciple`]s are appended. Composition is order-stable
/// within each half; the round-trip golden compares corpora by id, since every
/// engine query is by id or by a family/binding filter (corpus order is not
/// semantically load-bearing).
#[must_use]
pub fn apply(cfg: &CraftConfig, packs: &[DoctrinePack]) -> AppliedCraft {
    let overlays: BTreeMap<&str, &PrincipleOverlay> = cfg
        .overlays
        .iter()
        .map(|o| (o.principle_id.as_ref(), o))
        .collect();
    let enabled: BTreeSet<&str> = cfg.enabled_packs.iter().map(Cow::as_ref).collect();

    let mut out = Vec::new();
    for pack in packs {
        if !enabled.contains(pack.pack.as_ref()) {
            continue;
        }
        for dp in &pack.principles {
            out.push(compose(dp, overlays.get(dp.id.as_ref()).copied()));
        }
    }
    for wp in &cfg.world_principles {
        out.push(wp.to_principle());
    }
    AppliedCraft {
        principles: out,
        precedence_tier: cfg.precedence_tier.clone(),
        factory_namespaces: cfg.factory_namespaces.clone(),
        judge_kinds: cfg.judge_kinds.clone(),
    }
}

/// A structural fault in a world craft config, against the shipped packs (design
/// §4's `validate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// An enabled pack name is not among the shipped packs.
    UnknownPack(String),
    /// An enabled pack name appears more than once.
    DuplicateEnabledPack(String),
    /// An overlay's `principle_id` does not resolve to an enabled pack.
    DanglingOverlay(String),
    /// A world principle shares an id with a shipped or earlier world principle.
    DuplicateId(String),
    /// More than one overlay targets the same shipped principle.
    DuplicateOverlay(String),
    /// An overlay carries a factory/attest binding whose symbol is empty.
    EmptyBindingSymbol(String),
    /// An overlay's concrete binding disagrees with the doctrine's checkable kind.
    OverlayBindingMismatch {
        /// The overlaid doctrine principle.
        principle: String,
        /// The binding shape required by the doctrine record.
        expected: String,
        /// The binding shape supplied by the overlay.
        actual: String,
    },
    /// A binding names a factory namespace absent from the caller registry.
    UndeclaredFactoryNamespace(String),
    /// A binding names a judge kind absent from the caller registry.
    UndeclaredJudgeKind(String),
    /// A registry identifier is empty or duplicated.
    InvalidRegistryEntry(String),
    /// An overlay overrides a parameter the doctrine principle does not declare.
    UnknownParameter {
        /// Principle carrying the invalid override.
        principle: String,
        /// Unknown parameter name.
        parameter: String,
    },
    /// An override changes the scalar type of a declared parameter.
    ParameterTypeMismatch {
        /// Principle carrying the invalid override.
        principle: String,
        /// Parameter whose scalar type changed.
        parameter: String,
    },
    /// A material-catalog category appears twice.
    DuplicateMaterialCategory(String),
    /// A material category identifier is empty or malformed.
    InvalidMaterialCategory(String),
    /// A configured lexical rule is malformed or duplicated.
    InvalidTextRule(String),
    /// A finding-policy row names no finding the craft engine can emit.
    UnknownFindingCode(String),
    /// A finding-policy code appears more than once.
    DuplicateFindingCode(String),
    /// A doctrine or world principle has an empty required field.
    EmptyPrincipleField {
        /// The principle carrying the empty field.
        principle: String,
        /// The required field.
        field: &'static str,
    },
    /// A doctrine or world principle names no finding the craft engine can emit.
    UnknownPrincipleFindingCode {
        /// The principle carrying the invalid code.
        principle: String,
        /// The unknown code.
        code: String,
    },
    /// A principle's finding code does not agree with its concrete kernel binding.
    FindingCodeBindingMismatch {
        /// The principle carrying the inconsistent code.
        principle: String,
        /// The inconsistent code.
        code: String,
        /// The kernel binding the code must agree with.
        binding: String,
    },
    /// A principle repeats one finding code.
    DuplicatePrincipleFindingCode {
        /// The principle carrying the duplicate.
        principle: String,
        /// The repeated code.
        code: String,
    },
    /// A world principle names [`Family::Pillar`] but the config declares no
    /// precedence tier.
    PillarWithoutTier(String),
    /// An overlay promotes a principle into precedence without a declared tier.
    PrecedenceOverlayWithoutTier(String),
    /// A precedence tier has empty or unsupported semantics.
    InvalidPrecedenceTier(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPack(p) => write!(f, "enabled pack `{p}` is not shipped"),
            Self::DuplicateEnabledPack(p) => write!(f, "enabled pack `{p}` appears twice"),
            Self::DanglingOverlay(id) => {
                write!(f, "overlay `{id}` resolves to no enabled pack")
            }
            Self::DuplicateId(id) => write!(f, "duplicate principle id `{id}`"),
            Self::DuplicateOverlay(id) => {
                write!(f, "principle `{id}` has more than one overlay")
            }
            Self::EmptyBindingSymbol(id) => {
                write!(f, "principle `{id}` binds an empty factory/judge symbol")
            }
            Self::OverlayBindingMismatch {
                principle,
                expected,
                actual,
            } => fmt_overlay_binding_mismatch(f, principle, expected, actual),
            Self::UndeclaredFactoryNamespace(namespace) => {
                write!(f, "factory namespace `{namespace}` is not registered")
            }
            Self::UndeclaredJudgeKind(kind) => {
                write!(f, "judge kind `{kind}` is not registered")
            }
            Self::InvalidRegistryEntry(entry) => {
                write!(f, "registry entry `{entry}` is empty or duplicated")
            }
            Self::UnknownParameter {
                principle,
                parameter,
            } => fmt_parameter_error(f, principle, parameter, "overrides undeclared"),
            Self::ParameterTypeMismatch {
                principle,
                parameter,
            } => fmt_parameter_error(f, principle, parameter, "changes the scalar type of"),
            Self::DuplicateMaterialCategory(c) => {
                write!(f, "material category `{c}` appears twice")
            }
            Self::InvalidMaterialCategory(category) => {
                write!(
                    f,
                    "material category `{category}` is not a valid identifier"
                )
            }
            Self::InvalidTextRule(rule) => write!(f, "invalid lexical rule: {rule}"),
            Self::UnknownFindingCode(code) => {
                write!(
                    f,
                    "finding policy code `{code}` is not emitted by the craft engine"
                )
            }
            Self::DuplicateFindingCode(code) => {
                write!(f, "finding policy code `{code}` appears twice")
            }
            Self::EmptyPrincipleField { principle, field } => {
                write!(f, "principle `{principle}` has an empty `{field}`")
            }
            Self::UnknownPrincipleFindingCode { principle, code } => {
                write!(
                    f,
                    "principle `{principle}` names finding code `{code}` which the craft engine \
                     cannot emit"
                )
            }
            Self::FindingCodeBindingMismatch {
                principle,
                code,
                binding,
            } => {
                write!(
                    f,
                    "principle `{principle}` maps `{code}` to incompatible kernel binding \
                     `{binding}`"
                )
            }
            Self::DuplicatePrincipleFindingCode { principle, code } => {
                write!(f, "principle `{principle}` repeats finding code `{code}`")
            }
            Self::PillarWithoutTier(id) => {
                write!(
                    f,
                    "world principle `{id}` is a pillar but no precedence tier is declared"
                )
            }
            Self::PrecedenceOverlayWithoutTier(id) => {
                write!(
                    f,
                    "overlay `{id}` enters the precedence tier but no tier is declared"
                )
            }
            Self::InvalidPrecedenceTier(message) => {
                write!(f, "invalid precedence tier: {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

fn fmt_overlay_binding_mismatch(
    f: &mut std::fmt::Formatter<'_>,
    principle: &str,
    expected: &str,
    actual: &str,
) -> std::fmt::Result {
    write!(
        f,
        "overlay `{principle}` uses `{actual}`, but its doctrine requires `{expected}`"
    )
}

fn fmt_parameter_error(
    f: &mut std::fmt::Formatter<'_>,
    principle: &str,
    parameter: &str,
    fault: &str,
) -> std::fmt::Result {
    write!(f, "overlay `{principle}` {fault} parameter `{parameter}`")
}

fn binding_symbol_empty(binding: &Binding) -> bool {
    match binding {
        Binding::Factory { symbol, .. } => symbol.is_empty(),
        Binding::Attest(kind) => kind.is_empty(),
        Binding::Rule(_) | Binding::Declaration => false,
    }
}

fn binding_shape(binding: &Binding) -> String {
    match binding {
        Binding::Rule(rule) => format!("rule:{}", rule.id()),
        Binding::Factory { .. } => "factory".to_owned(),
        Binding::Attest(_) => "attest".to_owned(),
        Binding::Declaration => "declaration".to_owned(),
    }
}

fn expected_binding_shape(principle: &DoctrinePrinciple) -> String {
    match principle.checkable_kind {
        CheckableKind::None => "declaration".to_owned(),
        CheckableKind::DeterministicKernel => principle.kernel_rule.map_or_else(
            || "declared kernel rule".to_owned(),
            |rule| format!("rule:{}", rule.id()),
        ),
        CheckableKind::DeterministicProse => "factory".to_owned(),
        CheckableKind::Judge => "attest".to_owned(),
    }
}

fn overlay_binding_matches(principle: &DoctrinePrinciple, binding: &Binding) -> bool {
    match (principle.checkable_kind, binding) {
        (CheckableKind::None, Binding::Declaration)
        | (CheckableKind::DeterministicProse, Binding::Factory { .. })
        | (CheckableKind::Judge, Binding::Attest(_)) => true,
        (CheckableKind::DeterministicKernel, Binding::Rule(actual)) => {
            principle.kernel_rule == Some(*actual)
        }
        _ => false,
    }
}

fn validate_binding(
    principle_id: &str,
    binding: &Binding,
    factories: &BTreeSet<&str>,
    judges: &BTreeSet<&str>,
    errors: &mut Vec<ConfigError>,
) {
    if binding_symbol_empty(binding) {
        errors.push(ConfigError::EmptyBindingSymbol(principle_id.to_owned()));
    }
    match binding {
        Binding::Factory { namespace, .. } if !factories.contains(namespace.prefix()) => {
            errors.push(ConfigError::UndeclaredFactoryNamespace(
                namespace.prefix().to_owned(),
            ));
        }
        Binding::Attest(kind) if !judges.contains(kind.as_ref()) => {
            errors.push(ConfigError::UndeclaredJudgeKind(kind.to_string()));
        }
        Binding::Rule(_) | Binding::Declaration | Binding::Factory { .. } | Binding::Attest(_) => {}
    }
}

fn validate_registries<'a>(
    cfg: &'a CraftConfig,
    errors: &mut Vec<ConfigError>,
) -> (BTreeSet<&'a str>, BTreeSet<&'a str>) {
    let mut factories = BTreeSet::new();
    for namespace in &cfg.factory_namespaces {
        let name = namespace.prefix();
        if name.is_empty() || !factories.insert(name) {
            errors.push(ConfigError::InvalidRegistryEntry(name.to_owned()));
        }
    }
    let mut judges = BTreeSet::new();
    for kind in &cfg.judge_kinds {
        if kind.is_empty() || !judges.insert(kind.as_ref()) {
            errors.push(ConfigError::InvalidRegistryEntry(kind.to_string()));
        }
    }
    if let Some(tier) = &cfg.precedence_tier {
        if tier.name.trim().is_empty() {
            errors.push(ConfigError::InvalidPrecedenceTier(
                "name must be non-empty".to_owned(),
            ));
        }
        if tier.wins_over != "all-other-families" {
            errors.push(ConfigError::InvalidPrecedenceTier(format!(
                "unsupported wins_over `{}`",
                tier.wins_over
            )));
        }
        if tier.standing_rule.trim().is_empty() {
            errors.push(ConfigError::InvalidPrecedenceTier(
                "standing_rule must be non-empty".to_owned(),
            ));
        }
    }
    (factories, judges)
}

fn validate_policy_surfaces(cfg: &CraftConfig, errors: &mut Vec<ConfigError>) -> BTreeSet<String> {
    let mut categories = BTreeSet::new();
    for category in &cfg.material_catalog {
        if !valid_identifier(category.category.as_str(), '_') {
            errors.push(ConfigError::InvalidMaterialCategory(
                category.category.to_string(),
            ));
        }
        if !categories.insert(category.category.clone()) {
            errors.push(ConfigError::DuplicateMaterialCategory(
                category.category.to_string(),
            ));
        }
    }

    let mut text_rule_codes = BTreeSet::new();
    for rule in &cfg.text_rules {
        let code = rule.code.as_ref();
        let item = code.strip_prefix("outline.").unwrap_or("");
        if !valid_identifier(item, '-') {
            errors.push(ConfigError::InvalidTextRule(format!(
                "code `{code}` must be an outline.* identifier"
            )));
        }
        if !is_outline_finding_code(code) {
            errors.push(ConfigError::InvalidTextRule(format!(
                "code `{code}` is not a registered outline finding"
            )));
        }
        if !text_rule_codes.insert(code.to_owned()) {
            errors.push(ConfigError::InvalidTextRule(format!(
                "code `{code}` appears twice"
            )));
        }
        if rule.message.trim().is_empty() {
            errors.push(ConfigError::InvalidTextRule(format!(
                "code `{code}` has an empty message"
            )));
        }
        if rule.single.is_empty() && rule.pairs.is_empty() {
            errors.push(ConfigError::InvalidTextRule(format!(
                "code `{code}` has no patterns"
            )));
        }
        let invalid_token = rule
            .single
            .iter()
            .chain(rule.pairs.iter().flat_map(|pair| pair.leads.iter()))
            .any(|token| token.is_empty() || token.chars().any(char::is_whitespace))
            || rule
                .pairs
                .iter()
                .any(|pair| pair.tail.is_empty() || pair.tail.chars().any(char::is_whitespace));
        if invalid_token {
            errors.push(ConfigError::InvalidTextRule(format!(
                "code `{code}` contains an empty or multi-word token"
            )));
        }
    }

    let mut finding_codes = BTreeSet::new();
    for rule in &cfg.finding_policy {
        let code = rule.code.as_ref();
        let known = is_outline_finding_code(code)
            || text_rule_codes.contains(code)
            || code
                .strip_prefix("craft.")
                .and_then(super::kb::KernelRule::from_id)
                .is_some_and(|rule| {
                    !matches!(
                        rule,
                        super::kb::KernelRule::CheckOutline | super::kb::KernelRule::CheckSeason
                    )
                });
        if !known {
            errors.push(ConfigError::UnknownFindingCode(code.to_owned()));
        }
        if !finding_codes.insert(code) {
            errors.push(ConfigError::DuplicateFindingCode(code.to_owned()));
        }
    }
    text_rule_codes
}

fn known_finding_code(code: &str, text_rule_codes: &BTreeSet<String>) -> bool {
    is_outline_finding_code(code)
        || text_rule_codes.contains(code)
        || code
            .strip_prefix("craft.")
            .and_then(super::kb::KernelRule::from_id)
            .is_some_and(|rule| {
                !matches!(
                    rule,
                    super::kb::KernelRule::CheckOutline | super::kb::KernelRule::CheckSeason
                )
            })
}

#[derive(Clone, Copy)]
struct PrincipleValidation<'a> {
    id: &'a str,
    statement: &'a str,
    source: &'a str,
    phases: &'a [Phase],
    binding: &'a Binding,
    finding_codes: &'a [Cow<'static, str>],
}

fn validate_principle(
    principle: PrincipleValidation<'_>,
    text_rule_codes: &BTreeSet<String>,
    errors: &mut Vec<ConfigError>,
) {
    for (field, empty) in [
        ("statement", principle.statement.trim().is_empty()),
        ("source", principle.source.trim().is_empty()),
        ("phases", principle.phases.is_empty()),
    ] {
        if empty {
            errors.push(ConfigError::EmptyPrincipleField {
                principle: principle.id.to_owned(),
                field,
            });
        }
    }

    let mut seen = BTreeSet::new();
    for code in principle.finding_codes {
        let code = code.as_ref();
        if !seen.insert(code) {
            errors.push(ConfigError::DuplicatePrincipleFindingCode {
                principle: principle.id.to_owned(),
                code: code.to_owned(),
            });
        }
        if !known_finding_code(code, text_rule_codes) {
            errors.push(ConfigError::UnknownPrincipleFindingCode {
                principle: principle.id.to_owned(),
                code: code.to_owned(),
            });
            continue;
        }
        let expected = match principle.binding {
            Binding::Rule(
                super::kb::KernelRule::CheckOutline | super::kb::KernelRule::CheckSeason,
            )
            | Binding::Factory { .. }
            | Binding::Attest(_)
            | Binding::Declaration => None,
            Binding::Rule(rule) => {
                let expected = format!("craft.{}", rule.id());
                (code != expected).then_some(expected)
            }
        };
        if let Some(expected) = expected {
            errors.push(ConfigError::FindingCodeBindingMismatch {
                principle: principle.id.to_owned(),
                code: code.to_owned(),
                binding: expected,
            });
        }
    }
}

fn validate_world_principles(
    cfg: &CraftConfig,
    seen: &mut BTreeSet<String>,
    factories: &BTreeSet<&str>,
    judges: &BTreeSet<&str>,
    text_rule_codes: &BTreeSet<String>,
    errors: &mut Vec<ConfigError>,
) {
    for principle in &cfg.world_principles {
        if !seen.insert(principle.id.to_string()) {
            errors.push(ConfigError::DuplicateId(principle.id.to_string()));
        }
        if principle.family == Family::Pillar && cfg.precedence_tier.is_none() {
            errors.push(ConfigError::PillarWithoutTier(principle.id.to_string()));
        }
        validate_binding(
            principle.id.as_ref(),
            &principle.binding,
            factories,
            judges,
            errors,
        );
        validate_principle(
            PrincipleValidation {
                id: principle.id.as_ref(),
                statement: principle.statement.as_ref(),
                source: principle.source.as_ref(),
                phases: principle.phases.as_ref(),
                binding: &principle.binding,
                finding_codes: &principle.finding_codes,
            },
            text_rule_codes,
            errors,
        );
    }
}

fn validate_enabled_packs<'a>(
    cfg: &'a CraftConfig,
    packs: &'a [DoctrinePack],
    errors: &mut Vec<ConfigError>,
) -> BTreeSet<&'a str> {
    let shipped: BTreeSet<&str> = packs.iter().map(|pack| pack.pack.as_ref()).collect();
    let mut enabled = BTreeSet::new();
    for pack in &cfg.enabled_packs {
        if !enabled.insert(pack.as_ref()) {
            errors.push(ConfigError::DuplicateEnabledPack(pack.to_string()));
        }
        if !shipped.contains(pack.as_ref()) {
            errors.push(ConfigError::UnknownPack(pack.to_string()));
        }
    }
    enabled
}

fn validate_overlays(
    cfg: &CraftConfig,
    enabled_principles: &BTreeMap<&str, &DoctrinePrinciple>,
    factories: &BTreeSet<&str>,
    judges: &BTreeSet<&str>,
    errors: &mut Vec<ConfigError>,
) {
    let mut overlaid = BTreeSet::new();
    for overlay in &cfg.overlays {
        if !enabled_principles.contains_key(overlay.principle_id.as_ref()) {
            errors.push(ConfigError::DanglingOverlay(
                overlay.principle_id.to_string(),
            ));
        }
        if !overlaid.insert(overlay.principle_id.as_ref()) {
            errors.push(ConfigError::DuplicateOverlay(
                overlay.principle_id.to_string(),
            ));
        }
        validate_binding(
            overlay.principle_id.as_ref(),
            &overlay.binding,
            factories,
            judges,
            errors,
        );
        if overlay.in_precedence_tier && cfg.precedence_tier.is_none() {
            errors.push(ConfigError::PrecedenceOverlayWithoutTier(
                overlay.principle_id.to_string(),
            ));
        }
        if let Some(principle) = enabled_principles.get(overlay.principle_id.as_ref()) {
            if !overlay_binding_matches(principle, &overlay.binding) {
                errors.push(ConfigError::OverlayBindingMismatch {
                    principle: overlay.principle_id.to_string(),
                    expected: expected_binding_shape(principle),
                    actual: binding_shape(&overlay.binding),
                });
            }
            for (parameter, value) in &overlay.param_overrides {
                match principle.default_params.get(parameter.as_ref()) {
                    None => errors.push(ConfigError::UnknownParameter {
                        principle: overlay.principle_id.to_string(),
                        parameter: parameter.to_string(),
                    }),
                    Some(default)
                        if std::mem::discriminant(default) != std::mem::discriminant(value) =>
                    {
                        errors.push(ConfigError::ParameterTypeMismatch {
                            principle: overlay.principle_id.to_string(),
                            parameter: parameter.to_string(),
                        });
                    }
                    Some(_) => {}
                }
            }
        }
        if overlay
            .statement
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            errors.push(ConfigError::EmptyPrincipleField {
                principle: overlay.principle_id.to_string(),
                field: "statement",
            });
        }
        if overlay
            .source
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            errors.push(ConfigError::EmptyPrincipleField {
                principle: overlay.principle_id.to_string(),
                field: "source",
            });
        }
    }
}

/// Validate a world craft config against the shipped packs (design §4): every
/// enabled pack ships, every overlay resolves to an enabled pack, binding symbols
/// are non-empty, ids and overlays are unique, the material catalog has unique
/// keys, finding-policy rows name emitted codes exactly once, and a pillar row
/// implies a precedence tier. Returns the faults (empty ⇒ clean).
#[must_use]
pub fn validate(cfg: &CraftConfig, packs: &[DoctrinePack]) -> Vec<ConfigError> {
    let mut errors = Vec::new();
    let text_rule_codes = validate_policy_surfaces(cfg, &mut errors);

    let enabled = validate_enabled_packs(cfg, packs, &mut errors);

    // Ids reachable through enabled packs.
    let enabled_principles: BTreeMap<&str, &DoctrinePrinciple> = packs
        .iter()
        .filter(|p| enabled.contains(p.pack.as_ref()))
        .flat_map(|p| p.principles.iter().map(|dp| (dp.id.as_ref(), dp)))
        .collect();
    let enabled_ids: BTreeSet<&str> = enabled_principles.keys().copied().collect();

    let (factories, judges) = validate_registries(cfg, &mut errors);

    let mut seen: BTreeSet<String> = enabled_ids.iter().map(|s| (*s).to_owned()).collect();
    validate_overlays(cfg, &enabled_principles, &factories, &judges, &mut errors);
    let overlays: BTreeMap<&str, &PrincipleOverlay> = cfg
        .overlays
        .iter()
        .map(|overlay| (overlay.principle_id.as_ref(), overlay))
        .collect();
    for principle in enabled_principles.values() {
        let composed = compose(principle, overlays.get(principle.id.as_ref()).copied());
        validate_principle(
            PrincipleValidation {
                id: composed.id.as_ref(),
                statement: composed.statement.as_ref(),
                source: composed.source.as_ref(),
                phases: composed.phases.as_ref(),
                binding: &composed.binding,
                finding_codes: &composed.finding_codes,
            },
            &text_rule_codes,
            &mut errors,
        );
    }
    validate_world_principles(
        cfg,
        &mut seen,
        &factories,
        &judges,
        &text_rule_codes,
        &mut errors,
    );

    errors
}

fn valid_identifier(value: &str, separator: char) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == separator)
}

// ---------------------------------------------------------------------------
// load_config (design §4)
// ---------------------------------------------------------------------------

/// A failure loading a world craft config from a directory.
#[derive(Debug)]
pub enum ConfigLoadError {
    /// Reading a config file failed.
    Io(std::io::Error),
    /// Parsing a config file failed (the offending file is named).
    Parse {
        /// The config file whose contents did not parse.
        file: String,
        /// The underlying TOML error.
        source: toml::de::Error,
    },
    /// A TOML file in the config directory is not part of the closed config set.
    UnknownFile(String),
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "reading world craft config: {err}"),
            Self::Parse { file, source } => write!(f, "parsing {file}: {source}"),
            Self::UnknownFile(file) => write!(f, "unknown craft config file `{file}`"),
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Parse { source, .. } => Some(source),
            Self::UnknownFile(_) => None,
        }
    }
}

impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Read one optional config file in `dir`, parsing it as `T` when present.
fn read_opt<T: for<'de> Deserialize<'de>>(
    dir: &Path,
    file: &str,
) -> Result<Option<T>, ConfigLoadError> {
    let path = dir.join(file);
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text)
            .map(Some)
            .map_err(|source| ConfigLoadError::Parse {
                file: file.to_owned(),
                source,
            }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(ConfigLoadError::Io(err)),
    }
}

/// The one-file wrappers `load_config` reads (each file holds a named list/record).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PacksFile {
    #[serde(default)]
    packs: Vec<Cow<'static, str>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistriesFile {
    #[serde(default)]
    factory_namespaces: Vec<super::kb::FactoryNs>,
    #[serde(default)]
    judge_kinds: Vec<Cow<'static, str>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OverlaysFile {
    #[serde(default)]
    overlays: Vec<PrincipleOverlay>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldPrinciplesFile {
    #[serde(default)]
    world_principles: Vec<WorldPrinciple>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FindingPolicyFile {
    #[serde(default)]
    rules: Vec<FindingRuleCfg>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialCatalogFile {
    #[serde(default)]
    categories: Vec<MaterialCategoryCfg>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeatLexiconsFile {
    #[serde(default)]
    rules: Vec<TextMatchRuleCfg>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdsFile {
    #[serde(default)]
    season: Option<Thresholds>,
    #[serde(default)]
    outline: Option<OutlineThresholdsCfg>,
}

/// The closed set of files understood by [`load_config`].
pub const CONFIG_FILES: &[&str] = &[
    "enabled-packs.toml",
    "registries.toml",
    "overlays.toml",
    "world-principles.toml",
    "precedence.toml",
    "finding-policy.toml",
    "narrator-policy.toml",
    "provenance.toml",
    "material-catalog.toml",
    "beat-lexicons.toml",
    "thresholds.toml",
];

fn reject_unknown_config_files(dir: &Path) -> Result<(), ConfigLoadError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !CONFIG_FILES.contains(&name.as_ref()) {
            return Err(ConfigLoadError::UnknownFile(name.into_owned()));
        }
    }
    Ok(())
}

/// Load a world craft config from `dir` (`worlds/<world>/craft/`) — serde over the
/// `*.toml` files (design §4). Missing files default empty, so a partial config
/// (e.g. only `enabled-packs.toml` + `overlays.toml`) loads.
///
/// # Errors
/// Returns [`ConfigLoadError`] when a present file cannot be read or parsed.
pub fn load_config(dir: &Path) -> Result<CraftConfig, ConfigLoadError> {
    reject_unknown_config_files(dir)?;
    let packs: Option<PacksFile> = read_opt(dir, "enabled-packs.toml")?;
    let registries: Option<RegistriesFile> = read_opt(dir, "registries.toml")?;
    let overlays: Option<OverlaysFile> = read_opt(dir, "overlays.toml")?;
    let world: Option<WorldPrinciplesFile> = read_opt(dir, "world-principles.toml")?;
    let precedence: Option<PrecedenceTier> = read_opt(dir, "precedence.toml")?;
    let finding: Option<FindingPolicyFile> = read_opt(dir, "finding-policy.toml")?;
    let narrator: Option<NarratorPolicyCfg> = read_opt(dir, "narrator-policy.toml")?;
    let provenance: Option<ProvenanceCfg> = read_opt(dir, "provenance.toml")?;
    let material: Option<MaterialCatalogFile> = read_opt(dir, "material-catalog.toml")?;
    let lexicons: Option<BeatLexiconsFile> = read_opt(dir, "beat-lexicons.toml")?;
    let thresholds: Option<ThresholdsFile> = read_opt(dir, "thresholds.toml")?;

    let registries = registries.unwrap_or(RegistriesFile {
        factory_namespaces: Vec::new(),
        judge_kinds: Vec::new(),
    });
    let thresholds = thresholds.unwrap_or(ThresholdsFile {
        season: None,
        outline: None,
    });

    Ok(CraftConfig {
        enabled_packs: packs.map(|p| p.packs).unwrap_or_default(),
        precedence_tier: precedence,
        factory_namespaces: registries.factory_namespaces,
        judge_kinds: registries.judge_kinds,
        overlays: overlays.map(|o| o.overlays).unwrap_or_default(),
        world_principles: world.map(|w| w.world_principles).unwrap_or_default(),
        finding_policy: finding.map(|f| f.rules).unwrap_or_default(),
        narrator_policy: narrator,
        provenance,
        material_catalog: material.map(|m| m.categories).unwrap_or_default(),
        season_thresholds: thresholds.season,
        outline_thresholds: thresholds.outline,
        text_rules: lexicons.map(|file| file.rules).unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// build_episode_policy — the CraftConfig→EpisodePolicy bridge (design §2.3)
// ---------------------------------------------------------------------------

/// Build an owned [`FindingPolicy`] from the config's finding rules.
fn build_finding_policy(rules: &[FindingRuleCfg]) -> FindingPolicy<'static> {
    if rules.is_empty() {
        return DEFAULT_FINDING_POLICY;
    }
    let static_rules = rules
        .iter()
        .map(|r| FindingRule {
            code: Cow::Owned(r.code.to_string()),
            citations: Cow::Owned(
                r.citations
                    .iter()
                    .map(|citation| Cow::Owned(citation.to_string()))
                    .collect(),
            ),
            severity: r.severity,
        })
        .collect();
    FindingPolicy {
        rules: Cow::Owned(static_rules),
    }
}

/// Build an owned [`NarratorPolicy`] from caller config.
fn build_narrator_policy(cfg: Option<&NarratorPolicyCfg>) -> NarratorPolicy<'static> {
    let Some(np) = cfg else {
        return DEFAULT_NARRATOR_POLICY;
    };
    let mode_rules = Cow::Owned(
        np.mode_rules
            .iter()
            .map(|r| NarratorModeRule {
                mode: r.mode,
                disposition: r.disposition,
            })
            .collect(),
    );
    NarratorPolicy {
        mode_rules,
        direct_address: np.direct_address,
    }
}

/// Build owned provenance formats from caller config.
fn build_provenance(cfg: Option<&ProvenanceCfg>) -> ProvenanceFormats<'static> {
    let Some(prov) = cfg else {
        return DEFAULT_PROVENANCE;
    };
    ProvenanceFormats {
        canon_prefix: Cow::Owned(prov.canon_prefix.to_string()),
        canon_digits: prov.canon_digits,
        research_url_schemes: Cow::Owned(
            prov.research_url_schemes
                .iter()
                .map(|scheme| Cow::Owned(scheme.to_string()))
                .collect(),
        ),
        research_id_prefix: Cow::Owned(prov.research_id_prefix.to_string()),
        research_id_digits: prov.research_id_digits,
        research_path_prefix: Cow::Owned(prov.research_path_prefix.to_string()),
        research_path_digits: prov.research_path_digits,
        research_path_suffix: Cow::Owned(prov.research_path_suffix.to_string()),
    }
}

/// Build an owned [`MaterialCatalog`] from caller config.
fn build_material_catalog(cats: &[MaterialCategoryCfg]) -> MaterialCatalog<'static> {
    if cats.is_empty() {
        return DEFAULT_MATERIAL_CATALOG;
    }
    let entries = Cow::Owned(
        cats.iter()
            .map(|c| MaterialCategory {
                category: c.category.clone(),
                required: c.required,
            })
            .collect(),
    );
    MaterialCatalog { entries }
}

/// Build an [`ArcShapeCatalog`] from the config's outline thresholds. Falls back to
/// [`DEFAULT_ARC_SHAPES`] when the config carries no outline thresholds.
fn build_arc_shapes(cfg: Option<&OutlineThresholdsCfg>) -> ArcShapeCatalog {
    cfg.map_or(DEFAULT_ARC_SHAPES, |ot| ArcShapeCatalog {
        turn_slot: ot.turn_slot,
    })
}

fn build_text_rules(rules: &[TextMatchRuleCfg]) -> Cow<'static, [TextMatchRule]> {
    Cow::Owned(
        rules
            .iter()
            .map(|rule| TextMatchRule {
                code: rule.code.to_string(),
                message: rule.message.to_string(),
                slot: rule.slot,
                single: rule.single.iter().map(ToString::to_string).collect(),
                pairs: rule
                    .pairs
                    .iter()
                    .map(|pair| TokenPair {
                        leads: pair.leads.iter().map(ToString::to_string).collect(),
                        tail: pair.tail.to_string(),
                    })
                    .collect(),
            })
            .collect(),
    )
}

/// Build an [`EpisodePolicy`] from a loaded [`CraftConfig`] — the bridge that
/// makes world-authored craft config actually feed the checker (design §2.3,
/// audit findings F7B-1/F7B-2/F8-1).
///
/// World-specific policy fields (finding policy, narrator policy, provenance,
/// material catalog, thresholds, arc shapes) are taken from the config when
/// present, falling back to the compiled defaults from [`super::config`] when
/// absent. Battle vocabulary and similar linguistic constants always use the
/// static defaults — they are generic linguistic patterns, not world content.
///
/// The returned policy owns loaded strings and tables. Its `'static` parameter
/// means it borrows no caller data; repeated library loads do not leak memory.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use dli_world::craft::worldcfg::{load_config, build_episode_policy};
///
/// let cfg = load_config(Path::new("worlds/tpth/craft")).unwrap();
/// let policy = build_episode_policy(&cfg);
/// // policy is now usable with check_episode(ep, opts, &policy)
/// ```
#[must_use]
pub fn build_episode_policy(cfg: &CraftConfig) -> EpisodePolicy<'static> {
    let finding_policy = build_finding_policy(&cfg.finding_policy);
    let narrator_policy = build_narrator_policy(cfg.narrator_policy.as_ref());
    let provenance = build_provenance(cfg.provenance.as_ref());
    let material_catalog = build_material_catalog(&cfg.material_catalog);
    let arc_shapes = build_arc_shapes(cfg.outline_thresholds.as_ref());
    let text_rules = build_text_rules(&cfg.text_rules);
    let packs = super::load::doctrines();
    let mut enabled_findings: BTreeSet<String> = apply(cfg, &packs)
        .into_principles()
        .into_iter()
        .flat_map(|principle| principle.finding_codes)
        .map(Cow::into_owned)
        .collect();
    enabled_findings.extend(cfg.text_rules.iter().map(|rule| rule.code.to_string()));

    // Outline scalars: use config when present, otherwise fall back to the
    // compiled policy leaf tables.
    let (scene_cast_ceiling, hook_seconds_ceiling, slider_flat_ceiling, slider_low_floor) =
        match &cfg.outline_thresholds {
            Some(ot) => (
                ot.scene_cast_ceiling,
                ot.hook_seconds_ceiling,
                ot.slider_flat_ceiling,
                ot.slider_low_floor,
            ),
            None => (
                season::policy::SCENE_CAST_CEILING,
                season::policy::HOOK_SECONDS_CEILING,
                season::policy::SLIDER_FLAT_CEILING,
                season::policy::SLIDER_LOW_FLOOR,
            ),
        };

    EpisodePolicy {
        scene_cast_ceiling,
        hook_seconds_ceiling,
        slider_flat_ceiling,
        slider_low_floor,
        text_rules,
        enabled_findings: Some(enabled_findings),
        finding_policy,
        narrator_policy,
        provenance,
        material_catalog,
        arc_shapes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn tpth_craft_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../worlds/tpth/craft")
            .canonicalize()
            .expect("worlds/tpth/craft exists at the repo root")
    }

    #[test]
    fn build_episode_policy_from_tpth_config() {
        let cfg = load_config(&tpth_craft_dir()).expect("TPtH craft config loads");
        let policy = build_episode_policy(&cfg);

        // The finding policy rules should match the config's rules count and codes.
        assert_eq!(
            policy.finding_policy.rules.len(),
            cfg.finding_policy.len(),
            "finding policy rule count matches config"
        );
        for (got, want) in policy.finding_policy.rules.iter().zip(&cfg.finding_policy) {
            assert_eq!(got.code, want.code.as_ref(), "finding rule code matches");
            let got_cites: Vec<&str> = got.citations.iter().map(Cow::as_ref).collect();
            let want_cites: Vec<&str> = want.citations.iter().map(Cow::as_ref).collect();
            assert_eq!(got_cites, want_cites, "citations for {} match", got.code);
            assert_eq!(
                got.severity, want.severity,
                "severity for {} matches",
                got.code
            );
        }

        // Narrator policy matches config.
        let np = cfg
            .narrator_policy
            .as_ref()
            .expect("TPtH config carries narrator_policy");
        assert_eq!(
            policy.narrator_policy.mode_rules.len(),
            np.mode_rules.len(),
            "narrator mode rule count matches"
        );
        assert_eq!(
            policy.narrator_policy.direct_address, np.direct_address,
            "narrator direct_address matches"
        );

        // Provenance matches config.
        let prov = cfg
            .provenance
            .as_ref()
            .expect("TPtH config carries provenance");
        assert_eq!(policy.provenance.canon_prefix, prov.canon_prefix.as_ref());
        assert_eq!(policy.provenance.canon_digits, prov.canon_digits);

        // Material catalog matches config.
        assert_eq!(
            policy.material_catalog.entries.len(),
            cfg.material_catalog.len(),
            "material catalog entry count matches"
        );
        for (got, want) in policy
            .material_catalog
            .entries
            .iter()
            .zip(&cfg.material_catalog)
        {
            assert_eq!(got.category, want.category);
            assert_eq!(got.required, want.required);
        }

        // Outline scalars from config.
        let ot = cfg
            .outline_thresholds
            .as_ref()
            .expect("TPtH config carries outline_thresholds");
        assert_eq!(policy.scene_cast_ceiling, ot.scene_cast_ceiling);
        assert_eq!(policy.hook_seconds_ceiling, ot.hook_seconds_ceiling);
        assert_eq!(policy.slider_flat_ceiling, ot.slider_flat_ceiling);
        assert_eq!(policy.slider_low_floor, ot.slider_low_floor);

        // Arc shapes from config.
        assert_eq!(policy.arc_shapes.turn_slot, ot.turn_slot);

        assert_eq!(policy.text_rules.len(), cfg.text_rules.len());
    }

    #[test]
    fn build_episode_policy_empty_config_uses_defaults() {
        let cfg = CraftConfig::default();
        let policy = build_episode_policy(&cfg);

        // All fields fall back to compiled defaults.
        assert_eq!(
            policy.finding_policy.rules.len(),
            super::super::config::DEFAULT_FINDING_POLICY.rules.len(),
            "empty config falls back to DEFAULT_FINDING_POLICY"
        );
        assert_eq!(
            policy.scene_cast_ceiling,
            season::policy::SCENE_CAST_CEILING
        );
        assert_eq!(
            policy.hook_seconds_ceiling,
            season::policy::HOOK_SECONDS_CEILING
        );
        assert_eq!(
            policy.slider_flat_ceiling,
            season::policy::SLIDER_FLAT_CEILING
        );
        assert_eq!(policy.slider_low_floor, season::policy::SLIDER_LOW_FLOOR);
        assert_eq!(policy.narrator_policy.direct_address, Disposition::Allowed);
        assert_eq!(
            policy
                .narrator_policy
                .mode(super::super::outline::NarratorMode::Host),
            Disposition::Allowed
        );
        assert!(policy.provenance.is_canon_ref("caller-canon-42"));
        assert!(policy.provenance.is_research_ref("library shelf A"));
        assert!(!policy.provenance.is_canon_ref("  "));
        assert!(!policy.provenance.is_research_ref(""));
        assert!(policy.material_catalog.entries.is_empty());
    }

    #[test]
    fn configured_provenance_rejects_malformed_references_with_generic_messages() {
        let cfg = CraftConfig {
            enabled_packs: vec![Cow::Borrowed("worldbuilding")],
            provenance: Some(ProvenanceCfg {
                canon_prefix: Cow::Borrowed("CANON-"),
                canon_digits: 3,
                research_url_schemes: vec![Cow::Borrowed("https://")],
                research_id_prefix: Cow::Borrowed("SOURCE-"),
                research_id_digits: 2,
                research_path_prefix: Cow::Borrowed("research/SOURCE-"),
                research_path_digits: 2,
                research_path_suffix: Cow::Borrowed(".md"),
            }),
            ..CraftConfig::default()
        };
        let policy = build_episode_policy(&cfg);
        let episode = super::super::outline::EpisodeOutline {
            id: "e1".to_owned(),
            material_assumptions: Some(super::super::outline::MaterialAssumptionLedger {
                canon_refs: vec!["D-0001".to_owned()],
                research_refs: vec!["RR-001".to_owned()],
                ..super::super::outline::MaterialAssumptionLedger::default()
            }),
            ..super::super::outline::EpisodeOutline::default()
        };

        let findings = season::check_episode(&episode, season::CheckOptions::default(), &policy);
        let messages = findings
            .iter()
            .map(|finding| (finding.code.as_str(), finding.message.as_str()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            messages.get("outline.material-canon-ref-malformed"),
            Some(&"e1: canon_ref \"D-0001\" must match the configured canon reference format")
        );
        assert_eq!(
            messages.get("outline.material-research-ref-malformed"),
            Some(&"e1: research_ref \"RR-001\" must match a configured research reference format")
        );
    }

    #[test]
    fn finding_policy_codes_are_closed_and_unique() {
        let packs = super::super::load::doctrines();
        let cfg = CraftConfig {
            text_rules: vec![TextMatchRuleCfg {
                code: Cow::Borrowed("outline.caller-invented"),
                message: Cow::Borrowed("invented"),
                slot: TurnSlot::Ten,
                single: vec![Cow::Borrowed("invented")],
                pairs: Vec::new(),
            }],
            finding_policy: vec![
                FindingRuleCfg {
                    code: Cow::Borrowed("outline.no-such-rule"),
                    citations: vec![],
                    severity: None,
                },
                FindingRuleCfg {
                    code: Cow::Borrowed("craft.check_outline"),
                    citations: vec![],
                    severity: None,
                },
                FindingRuleCfg {
                    code: Cow::Borrowed("craft.accomplishment-earned"),
                    citations: vec![],
                    severity: None,
                },
                FindingRuleCfg {
                    code: Cow::Borrowed("craft.accomplishment-earned"),
                    citations: vec![],
                    severity: None,
                },
            ],
            ..CraftConfig::default()
        };

        let errors = validate(&cfg, &packs);
        assert!(errors.contains(&ConfigError::UnknownFindingCode(
            "outline.no-such-rule".to_owned()
        )));
        assert!(errors.contains(&ConfigError::InvalidTextRule(
            "code `outline.caller-invented` is not a registered outline finding".to_owned()
        )));
        assert!(errors.contains(&ConfigError::UnknownFindingCode(
            "craft.check_outline".to_owned()
        )));
        assert!(errors.contains(&ConfigError::DuplicateFindingCode(
            "craft.accomplishment-earned".to_owned()
        )));
    }

    #[test]
    fn world_principles_validate_required_fields_codes_and_rule_consistency() {
        let packs = super::super::load::doctrines();
        let cfg = CraftConfig {
            world_principles: vec![WorldPrinciple {
                id: Cow::Borrowed("invalid-principle"),
                family: Family::World,
                statement: Cow::Borrowed(" "),
                source: Cow::Borrowed(""),
                mission_filter: MissionFilter::Pass,
                phases: Cow::Borrowed(&[]),
                binding: Binding::Rule(super::super::kb::KernelRule::AccomplishmentEarned),
                citations: Vec::new(),
                parameters: BTreeMap::new(),
                finding_codes: vec![
                    Cow::Borrowed("outline.no-such-rule"),
                    Cow::Borrowed("craft.arc-shape"),
                    Cow::Borrowed("craft.arc-shape"),
                ],
            }],
            ..CraftConfig::default()
        };

        let errors = validate(&cfg, &packs);
        for field in ["statement", "source", "phases"] {
            assert!(errors.contains(&ConfigError::EmptyPrincipleField {
                principle: "invalid-principle".to_owned(),
                field,
            }));
        }
        assert!(errors.contains(&ConfigError::UnknownPrincipleFindingCode {
            principle: "invalid-principle".to_owned(),
            code: "outline.no-such-rule".to_owned(),
        }));
        assert!(errors.contains(&ConfigError::FindingCodeBindingMismatch {
            principle: "invalid-principle".to_owned(),
            code: "craft.arc-shape".to_owned(),
            binding: "craft.accomplishment-earned".to_owned(),
        }));
        assert!(
            errors.contains(&ConfigError::DuplicatePrincipleFindingCode {
                principle: "invalid-principle".to_owned(),
                code: "craft.arc-shape".to_owned(),
            })
        );
    }

    #[test]
    fn duplicate_overlays_are_rejected_instead_of_last_wins() {
        let packs = super::super::load::doctrines();
        let principle_id = packs[0].principles[0].id.clone();
        let overlay = PrincipleOverlay {
            principle_id: principle_id.clone(),
            mission_filter: MissionFilter::Pass,
            citations: Vec::new(),
            binding: Binding::Declaration,
            in_precedence_tier: false,
            statement: None,
            source: None,
            param_overrides: BTreeMap::new(),
        };
        let cfg = CraftConfig {
            enabled_packs: vec![packs[0].pack.clone()],
            overlays: vec![overlay.clone(), overlay],
            ..CraftConfig::default()
        };

        assert!(
            validate(&cfg, &packs)
                .contains(&ConfigError::DuplicateOverlay(principle_id.to_string()))
        );
    }

    #[test]
    fn duplicate_pack_selection_is_rejected() {
        let packs = super::super::load::doctrines();
        let pack = packs[0].pack.clone();
        let cfg = CraftConfig {
            enabled_packs: vec![pack.clone(), pack.clone()],
            ..CraftConfig::default()
        };

        assert!(
            validate(&cfg, &packs).contains(&ConfigError::DuplicateEnabledPack(pack.to_string()))
        );
        let schema =
            serde_json::to_value(schemars::schema_for!(CraftConfig)).expect("serialize schema");
        assert_eq!(
            schema["properties"]["enabled_packs"]["uniqueItems"],
            serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn overlays_cannot_change_the_doctrine_binding() {
        let principle = DoctrinePrinciple {
            id: Cow::Borrowed("earned-triumph"),
            pack: Cow::Borrowed("test-pack"),
            family: super::super::kb::DoctrineFamily::Plot,
            statement: Cow::Borrowed("A triumph must be earned."),
            source: Cow::Borrowed("Test doctrine."),
            phases: Cow::Borrowed(&[Phase::Outline]),
            checkable_kind: CheckableKind::DeterministicKernel,
            kernel_rule: Some(super::super::kb::KernelRule::AccomplishmentEarned),
            default_params: BTreeMap::new(),
            finding_codes: vec![Cow::Borrowed("craft.accomplishment-earned")],
        };
        let packs = vec![DoctrinePack {
            pack: Cow::Borrowed("test-pack"),
            principles: vec![principle.clone()],
        }];
        let cfg = CraftConfig {
            enabled_packs: vec![principle.pack.clone()],
            overlays: vec![PrincipleOverlay {
                principle_id: principle.id.clone(),
                mission_filter: MissionFilter::Pass,
                citations: Vec::new(),
                binding: Binding::Rule(super::super::kb::KernelRule::ArcShape),
                in_precedence_tier: false,
                statement: None,
                source: None,
                param_overrides: BTreeMap::new(),
            }],
            ..CraftConfig::default()
        };

        let errors = validate(&cfg, &packs);
        assert!(errors.contains(&ConfigError::OverlayBindingMismatch {
            principle: principle.id.to_string(),
            expected: "rule:accomplishment-earned".to_owned(),
            actual: "rule:arc-shape".to_owned(),
        }));
        assert!(errors.contains(&ConfigError::FindingCodeBindingMismatch {
            principle: principle.id.to_string(),
            code: "craft.accomplishment-earned".to_owned(),
            binding: "craft.arc-shape".to_owned(),
        }));
    }

    #[test]
    fn overlays_preserve_citations_and_merge_declared_parameters() {
        let packs = super::super::load::doctrines();
        let principle = packs
            .iter()
            .flat_map(|pack| &pack.principles)
            .find(|principle| principle.id == "said-asked-eighty-five")
            .expect("parameterized doctrine principle");
        let cfg = CraftConfig {
            enabled_packs: vec![principle.pack.clone()],
            factory_namespaces: vec![super::super::kb::FactoryNs::new("editorial")],
            overlays: vec![PrincipleOverlay {
                principle_id: principle.id.clone(),
                mission_filter: MissionFilter::Pass,
                citations: vec![Cow::Borrowed("PROJECT-42")],
                binding: Binding::Factory {
                    namespace: super::super::kb::FactoryNs::new("editorial"),
                    symbol: Cow::Borrowed("said_ratio"),
                },
                in_precedence_tier: false,
                statement: None,
                source: None,
                param_overrides: BTreeMap::from([(
                    Cow::Borrowed("said_tag_ratio"),
                    Scalar::Float(0.9),
                )]),
            }],
            ..CraftConfig::default()
        };

        assert!(validate(&cfg, &packs).is_empty());
        let composed = apply(&cfg, &packs);
        let got = composed
            .iter()
            .find(|row| row.id == principle.id)
            .expect("composed principle");
        assert_eq!(got.citations, vec![Cow::Borrowed("PROJECT-42")]);
        assert_eq!(
            got.parameters.get("said_tag_ratio"),
            Some(&Scalar::Float(0.9))
        );
    }

    #[test]
    fn registries_parameters_and_precedence_fail_closed() {
        let packs = super::super::load::doctrines();
        let principle = packs
            .iter()
            .flat_map(|pack| &pack.principles)
            .find(|principle| principle.id == "said-asked-eighty-five")
            .expect("parameterized doctrine principle");
        let cfg = CraftConfig {
            enabled_packs: vec![principle.pack.clone()],
            overlays: vec![PrincipleOverlay {
                principle_id: principle.id.clone(),
                mission_filter: MissionFilter::Pass,
                citations: Vec::new(),
                binding: Binding::Factory {
                    namespace: super::super::kb::FactoryNs::new("editorial"),
                    symbol: Cow::Borrowed("said_ratio"),
                },
                in_precedence_tier: true,
                statement: None,
                source: None,
                param_overrides: BTreeMap::from([(Cow::Borrowed("not_declared"), Scalar::Int(1))]),
            }],
            ..CraftConfig::default()
        };
        let errors = validate(&cfg, &packs);
        assert!(errors.contains(&ConfigError::UndeclaredFactoryNamespace(
            "editorial".to_owned()
        )));
        assert!(errors.contains(&ConfigError::PrecedenceOverlayWithoutTier(
            principle.id.to_string()
        )));
        assert!(errors.contains(&ConfigError::UnknownParameter {
            principle: principle.id.to_string(),
            parameter: "not_declared".to_owned(),
        }));
    }

    #[test]
    fn material_catalog_accepts_caller_defined_categories() {
        let root =
            std::env::temp_dir().join(format!("deloreis-material-category-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create material config directory");
        std::fs::write(
            root.join("material-catalog.toml"),
            "[[categories]]\ncategory = \"mana_source\"\nrequired = true\n",
        )
        .expect("write custom material category");

        let cfg = load_config(&root).expect("custom material category loads");
        assert_eq!(cfg.material_catalog[0].category.as_str(), "mana_source");
        assert!(validate(&cfg, &super::super::load::doctrines()).is_empty());
        std::fs::remove_dir_all(root).expect("remove material config directory");
    }

    #[test]
    fn empty_config_enables_no_doctrine_or_project_lexicon() {
        let policy = build_episode_policy(&CraftConfig::default());
        let episode = super::super::outline::EpisodeOutline {
            id: "e1".to_owned(),
            ten: "A final battle ends the war.".to_owned(),
            hook_by_seconds: u32::MAX,
            ..super::super::outline::EpisodeOutline::default()
        };
        assert!(
            season::check_episode(&episode, season::CheckOptions::default(), &policy).is_empty()
        );
    }

    #[test]
    fn config_files_and_fields_are_closed() {
        let root = std::env::temp_dir().join(format!(
            "deloreis-craft-config-closed-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create closed-config test directory");

        std::fs::write(root.join("finding-polciy.toml"), "rules = []\n")
            .expect("write unknown config file");
        assert!(matches!(
            load_config(&root),
            Err(ConfigLoadError::UnknownFile(file)) if file == "finding-polciy.toml"
        ));
        std::fs::remove_file(root.join("finding-polciy.toml")).expect("remove unknown config file");

        std::fs::write(root.join("finding-policy.yaml"), "rules: []\n")
            .expect("write unknown non-TOML config file");
        assert!(matches!(
            load_config(&root),
            Err(ConfigLoadError::UnknownFile(file)) if file == "finding-policy.yaml"
        ));
        std::fs::remove_file(root.join("finding-policy.yaml"))
            .expect("remove unknown non-TOML config file");

        std::fs::write(root.join("enabled-packs.toml"), "pakcs = []\n")
            .expect("write config with unknown field");
        assert!(matches!(
            load_config(&root),
            Err(ConfigLoadError::Parse { file, .. }) if file == "enabled-packs.toml"
        ));
        std::fs::remove_dir_all(root).expect("remove closed-config test directory");
    }
}
