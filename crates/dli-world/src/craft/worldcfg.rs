//! World craft-config — the factory OUTPUT, loaded as data (design §1.2, §2.4, §4).
//!
//! Step 5 of the craft-separation DAG flips ownership: the compiled 107-principle
//! corpus is no longer a `dli-world` const. Instead the engine ships the generic
//! doctrine packs ([`doctrines`](super::load::doctrines)) plus this **mechanism**,
//! and a world supplies its craft config as data under `worlds/<world>/craft/*`.
//! [`apply`] composes the two: each enabled [`DoctrinePrinciple`] is folded with its
//! optional [`PrincipleOverlay`] (the world ruling that rides on it), and the
//! world's own [`WorldPrinciple`]s (the config-only rows, including the precedence
//! tier) are appended, yielding the runtime [`Principle`] corpus.
//!
//! The boundary is compiler-checked: a [`DoctrinePrinciple`] cannot carry a mission
//! filter, a citation, a binding symbol, or [`Family::Pillar`] — all of that lives
//! here, in world config. `dli-world` therefore ships **zero** world content; the
//! `TPtH` corpus is reconstructed by `apply(load_config("worlds/tpth/craft"),
//! doctrines())`, proven field-for-field against a committed golden snapshot in the
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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::config::{Disposition, TurnSlot};
use super::kb::{
    Binding, CheckableKind, DoctrinePack, DoctrinePrinciple, Family, MissionFilter, Phase,
    Principle, Scalar,
};
use super::outline::{MaterialUnknown, NarratorMode};
use super::season::Thresholds;
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
        }
    }
}

/// The world's precedence tier — the generic pillar mechanism, world-populated.
/// The tier that wins a conflict plus the standing-rule text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
pub struct NarratorModeRuleCfg {
    /// The narrator mode this rule governs.
    pub mode: NarratorMode,
    /// Its disposition.
    pub disposition: Disposition,
}

/// The narrator-framing policy — loadable form of [`super::config::NarratorPolicy`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
pub struct MaterialCategoryCfg {
    /// The category key.
    pub category: MaterialUnknown,
    /// Whether a material-worldbuilding review requires it.
    pub required: bool,
}

/// The world's outline scalars + turn slot — the loadable companion to
/// [`super::season::EpisodePolicy`]'s scalars and [`super::config::ArcShapeCatalog`]
/// (design §3 items 2, 8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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

/// A world's full craft config — the aggregate the engine loads as data and
/// [`apply`]s over the shipped doctrine packs (design §1.2, §2.4).
///
/// Only [`enabled_packs`](Self::enabled_packs), [`overlays`](Self::overlays), and
/// [`world_principles`](Self::world_principles) bear on the composed principle
/// corpus; the remaining fields are the season/outline **policy** a later CLI/serve
/// wiring step reads (design §2.3) — authored here as data so the world config is
/// complete.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CraftConfig {
    /// The doctrine packs this world turns on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
        },
        Some(o) => Principle {
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
        },
    }
}

/// Compose a world's craft config with the shipped doctrine packs into the runtime
/// [`Principle`] corpus (design §2.4, §4).
///
/// Every enabled pack's principles are folded with their overlays (in pack order),
/// then the config's [`WorldPrinciple`]s are appended. Composition is order-stable
/// within each half; the round-trip golden compares corpora by id, since every
/// engine query is by id or by a family/binding filter (corpus order is not
/// semantically load-bearing).
#[must_use]
pub fn apply(cfg: &CraftConfig, packs: &[DoctrinePack]) -> Vec<Principle> {
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
    out
}

/// A structural fault in a world craft config, against the shipped packs (design
/// §4's `validate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// An enabled pack name is not among the shipped packs.
    UnknownPack(String),
    /// An overlay's `principle_id` does not resolve to an enabled pack.
    DanglingOverlay(String),
    /// Two overlays (or a world principle) share an id with a shipped principle or
    /// with each other.
    DuplicateId(String),
    /// An overlay carries a factory/attest binding whose symbol is empty.
    EmptyBindingSymbol(String),
    /// A material-catalog category appears twice.
    DuplicateMaterialCategory(String),
    /// A world principle names [`Family::Pillar`] but the config declares no
    /// precedence tier.
    PillarWithoutTier(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPack(p) => write!(f, "enabled pack `{p}` is not shipped"),
            Self::DanglingOverlay(id) => {
                write!(f, "overlay `{id}` resolves to no enabled pack")
            }
            Self::DuplicateId(id) => write!(f, "duplicate principle id `{id}`"),
            Self::EmptyBindingSymbol(id) => {
                write!(f, "overlay `{id}` binds an empty factory/judge symbol")
            }
            Self::DuplicateMaterialCategory(c) => {
                write!(f, "material category `{c}` appears twice")
            }
            Self::PillarWithoutTier(id) => {
                write!(
                    f,
                    "world principle `{id}` is a pillar but no precedence tier is declared"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

fn binding_symbol_empty(binding: &Binding) -> bool {
    match binding {
        Binding::Factory { symbol, .. } => symbol.is_empty(),
        Binding::Attest(kind) => kind.is_empty(),
        Binding::Rule(_) | Binding::Declaration => false,
    }
}

/// Validate a world craft config against the shipped packs (design §4): every
/// enabled pack ships, every overlay resolves to an enabled pack, binding symbols
/// are non-empty, ids are unique across shipped/overlay/world rows, the material
/// catalog has unique keys, and a pillar row implies a precedence tier. Returns the
/// faults (empty ⇒ clean).
#[must_use]
pub fn validate(cfg: &CraftConfig, packs: &[DoctrinePack]) -> Vec<ConfigError> {
    let mut errors = Vec::new();

    let shipped_packs: BTreeSet<&str> = packs.iter().map(|p| p.pack.as_ref()).collect();
    let enabled: BTreeSet<&str> = cfg.enabled_packs.iter().map(Cow::as_ref).collect();
    for pack in &cfg.enabled_packs {
        if !shipped_packs.contains(pack.as_ref()) {
            errors.push(ConfigError::UnknownPack(pack.to_string()));
        }
    }

    // Ids reachable through enabled packs.
    let enabled_ids: BTreeSet<&str> = packs
        .iter()
        .filter(|p| enabled.contains(p.pack.as_ref()))
        .flat_map(|p| p.principles.iter().map(|dp| dp.id.as_ref()))
        .collect();

    let mut seen: BTreeSet<String> = enabled_ids.iter().map(|s| (*s).to_owned()).collect();
    for o in &cfg.overlays {
        if !enabled_ids.contains(o.principle_id.as_ref()) {
            errors.push(ConfigError::DanglingOverlay(o.principle_id.to_string()));
        }
        if binding_symbol_empty(&o.binding) {
            errors.push(ConfigError::EmptyBindingSymbol(o.principle_id.to_string()));
        }
    }
    for wp in &cfg.world_principles {
        if !seen.insert(wp.id.to_string()) {
            errors.push(ConfigError::DuplicateId(wp.id.to_string()));
        }
        if wp.family == Family::Pillar && cfg.precedence_tier.is_none() {
            errors.push(ConfigError::PillarWithoutTier(wp.id.to_string()));
        }
    }

    let mut cats: BTreeSet<MaterialUnknown> = BTreeSet::new();
    for c in &cfg.material_catalog {
        if !cats.insert(c.category) {
            errors.push(ConfigError::DuplicateMaterialCategory(format!(
                "{:?}",
                c.category
            )));
        }
    }

    errors
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
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "reading world craft config: {err}"),
            Self::Parse { file, source } => write!(f, "parsing {file}: {source}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Parse { source, .. } => Some(source),
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
struct PacksFile {
    #[serde(default)]
    packs: Vec<Cow<'static, str>>,
}
#[derive(Deserialize)]
struct RegistriesFile {
    #[serde(default)]
    factory_namespaces: Vec<super::kb::FactoryNs>,
    #[serde(default)]
    judge_kinds: Vec<Cow<'static, str>>,
}
#[derive(Deserialize)]
struct OverlaysFile {
    #[serde(default)]
    overlays: Vec<PrincipleOverlay>,
}
#[derive(Deserialize)]
struct WorldPrinciplesFile {
    #[serde(default)]
    world_principles: Vec<WorldPrinciple>,
}
#[derive(Deserialize)]
struct FindingPolicyFile {
    #[serde(default)]
    rules: Vec<FindingRuleCfg>,
}
#[derive(Deserialize)]
struct MaterialCatalogFile {
    #[serde(default)]
    categories: Vec<MaterialCategoryCfg>,
}
#[derive(Deserialize)]
struct ThresholdsFile {
    #[serde(default)]
    season: Option<Thresholds>,
    #[serde(default)]
    outline: Option<OutlineThresholdsCfg>,
}

/// Load a world craft config from `dir` (`worlds/<world>/craft/`) — serde over the
/// `*.toml` files (design §4). Missing files default empty, so a partial config
/// (e.g. only `enabled-packs.toml` + `overlays.toml`) loads.
///
/// # Errors
/// Returns [`ConfigLoadError`] when a present file cannot be read or parsed.
pub fn load_config(dir: &Path) -> Result<CraftConfig, ConfigLoadError> {
    let packs: Option<PacksFile> = read_opt(dir, "enabled-packs.toml")?;
    let registries: Option<RegistriesFile> = read_opt(dir, "registries.toml")?;
    let overlays: Option<OverlaysFile> = read_opt(dir, "overlays.toml")?;
    let world: Option<WorldPrinciplesFile> = read_opt(dir, "world-principles.toml")?;
    let precedence: Option<PrecedenceTier> = read_opt(dir, "precedence.toml")?;
    let finding: Option<FindingPolicyFile> = read_opt(dir, "finding-policy.toml")?;
    let narrator: Option<NarratorPolicyCfg> = read_opt(dir, "narrator-policy.toml")?;
    let provenance: Option<ProvenanceCfg> = read_opt(dir, "provenance.toml")?;
    let material: Option<MaterialCatalogFile> = read_opt(dir, "material-catalog.toml")?;
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
    })
}
