//! The craft knowledge base as `principle` declarations (`CRAFT-3`).
//!
//! The consuming repo's knowledge base
//! ([06-craft-and-payloads](../../../../docs/spec/06-craft-and-payloads.md) §5)
//! ports here as data — [`Principle`], with every legacy field intact
//! (`{id, family, statement, source, mission_filter, tension_note, resolution,
//! phases, checkable, check_ref}`). Since the Phase-3 craft separation (Step 5) the
//! corpus is no longer a compiled table: the engine ships the world-agnostic
//! doctrine packs ([`DoctrinePrinciple`]) and a world supplies its craft config as
//! data, which [`super::worldcfg::apply`] composes into the runtime corpus. The
//! engine's default is the **generic** doctrine corpus ([`Kb::builtin`]); the `TPtH`
//! corpus (formerly the compiled 107-principle table) is reconstructed from
//! `worlds/tpth/craft/*` and proven field-for-field against a committed golden.
//!
//! **Invariants become compile rules, not test assertions.** The spec asks that a
//! `tension` require a `resolution` and that a dangling `check_ref` be a *compile*
//! error. This port makes both true at the type level, the strongest reading:
//!
//! - [`MissionFilter::Tension`] carries its `resolution` as a **required field**, so
//!   a tension without a resolution cannot be written (proven by the `compile_fail`
//!   doctest on [`MissionFilter`]). `Adapted` likewise requires its `note`.
//! - A deterministic kernel binding names a rule through the **closed**
//!   [`KernelRule`] enum, so a binding to a rule that does not exist is not a
//!   spelling the compiler accepts (proven by the `compile_fail` doctest on
//!   [`Binding`]). This is the legacy `check_ref_resolves` reference checker,
//!   lifted from a runtime test to the type system.
//!
//! **Bindings follow checkability** (spec §5):
//!
//! - `deterministic` over kernel objects → [`Binding::Rule`] (a [`KernelRule`]);
//! - `deterministic` over prose, which stays **factory-side** (spec §6) →
//!   [`Binding::Factory`] — recorded, never run here;
//! - `judge` → [`Binding::Attest`] — the check consumes a stored attestation and
//!   reports grade [`Grade::Attested`]
//!   ([07-verdicts-and-llm-boundary](../../../../docs/spec/07-verdicts-and-llm-boundary.md) §3);
//! - `none` → [`Binding::Declaration`] — surfaced by brief/phase queries, never a
//!   verdict.
//!
//! **Coverage is a query, kept honest** ([`checkable_debt`]): the principles bound
//! `judge` or `none` are the standing roadmap of rules not yet written. **Pillars
//! win** (D-0128): [`why`] renders the principles behind a finding pillar-first, so
//! a mission-specific pillar overrides generic advice.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use dli_core::grade::Grade;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attest::{Attestation, Question, QuestionKey};
use crate::check::WorldFinding;

/// A doctrine family — the axis a principle speaks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    /// Plot: promises, causality, try/fail, arc shape.
    Plot,
    /// Character: sliders, interiority, voice.
    Character,
    /// World: material grounding, magic pedagogy, allegory.
    World,
    /// Prose: sentence craft, abstraction, said-ratio.
    Prose,
    /// Serial: open loops, micro-pacing, installment discipline.
    Serial,
    /// Audio: narrator/SFX load, aural distinctness, performability.
    Audio,
    /// Pillar: a mission-specific principle that overrides generic advice (D-0128).
    Pillar,
}

impl Family {
    /// Whether this is the pillar family — the one that wins a conflict (D-0128).
    #[must_use]
    pub fn is_pillar(self) -> bool {
        matches!(self, Self::Pillar)
    }
}

/// A production phase a principle bears on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// The outline phase.
    Outline,
    /// The lore-decision phase.
    LoreDecision,
    /// The generation phase.
    Generation,
    /// The revision phase.
    Revision,
}

/// How the mission filters a principle (D-0128).
///
/// The two off-`Pass` arms carry their required companion fields inline, which is
/// how the spec's "a `tension` requires a `resolution`" invariant becomes a
/// **compile** rule rather than a runtime check: the value cannot be built without
/// it.
///
/// ```compile_fail
/// use dli_world::craft::kb::MissionFilter;
/// // A tension without a resolution is not a value that exists.
/// let _bad = MissionFilter::Tension { note: "generic advice loses to the pillar" };
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mission_filter", rename_all = "kebab-case")]
pub enum MissionFilter {
    /// The principle applies to the mission unchanged.
    #[default]
    Pass,
    /// The principle applies with a caveat; the caveat is required.
    Adapted {
        /// Why the generic principle needs adapting to the mission.
        note: Cow<'static, str>,
    },
    /// A generic principle conflicts with a pillar or with canon; the resolution
    /// (the mission-side ruling) is required (spec §5).
    Tension {
        /// The conflict between the received principle and the mission.
        note: Cow<'static, str>,
        /// The mission-side ruling that resolves it — where the pillar wins.
        resolution: Cow<'static, str>,
    },
}

impl MissionFilter {
    /// The tension/adaptation note, when the filter carries one.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        match self {
            Self::Pass => None,
            Self::Adapted { note } | Self::Tension { note, .. } => Some(note.as_ref()),
        }
    }

    /// The mission-side resolution, present exactly when the filter is a tension.
    #[must_use]
    pub fn resolution(&self) -> Option<&str> {
        match self {
            Self::Tension { resolution, .. } => Some(resolution.as_ref()),
            _ => None,
        }
    }
}

/// The **closed** set of deterministic kernel rules a principle may bind.
///
/// A `checkable: deterministic` principle over kernel objects names its rule here,
/// so a reference to a rule that does not exist is a compile error — the legacy
/// `check_ref_resolves` test, lifted into the type system. The outline entry
/// points are the `CRAFT-1` batteries; the season rules are `CRAFT-2`'s join
/// engine ([`super::season`]), and the two layers share this one rule namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum KernelRule {
    /// The `CRAFT-1` per-episode structural battery ([`super::season::check_episode`]).
    CheckOutline,
    /// The `CRAFT-1` season-scope battery ([`super::season::check_season`]).
    CheckSeason,
    /// Accomplishment-earned: a major beat needs a prior same-thread failure.
    AccomplishmentEarned,
    /// The climax refinement: a climax-class beat needs two prior failures.
    AccomplishmentEarnedClimax,
    /// A thread that lands its unqualified win may not re-open.
    YesAndTerminal,
    /// Competence-consistency: an unearned major win by an under-competent hand.
    CompetenceConsistency,
    /// The arc-flat generalization of the flat-slider check, at season scope.
    CompetenceArcFlat,
    /// Kishōtenketsu fair-play: a turn's reframing fact must be on the ledger early.
    RevealFairPlay,
    /// A mystery promise untouched for K episodes is starved.
    MysteryStarved,
    /// Cold-open discipline: open a loop, cast ≤ 2, no deep-time in act 1.
    ColdOpenDiscipline,
    /// New named speakers per episode within a band and a season budget.
    CastIntroductionRate,
    /// Two names below the aural-distinctness floor may not share a scene.
    SceneNameClash,
    /// Promise open/close density banded by season position.
    PromiseDensity,
    /// A kishōtenketsu episode places its *ten* in-band, not as a clash resolution.
    ArcShape,
    /// No run of N high-orientation-load episodes.
    AudioLoadTrajectory,
}

impl KernelRule {
    /// Every kernel rule, for coverage and the reference checker.
    pub const ALL: [KernelRule; 15] = [
        Self::CheckOutline,
        Self::CheckSeason,
        Self::AccomplishmentEarned,
        Self::AccomplishmentEarnedClimax,
        Self::YesAndTerminal,
        Self::CompetenceConsistency,
        Self::CompetenceArcFlat,
        Self::RevealFairPlay,
        Self::MysteryStarved,
        Self::ColdOpenDiscipline,
        Self::CastIntroductionRate,
        Self::SceneNameClash,
        Self::PromiseDensity,
        Self::ArcShape,
        Self::AudioLoadTrajectory,
    ];

    /// The rule's stable id — the item under the `craft.` finding family, and the
    /// name a season-rule [`WorldFinding`] carries. The outline entry points keep
    /// their legacy symbol names.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::CheckOutline => "check_outline",
            Self::CheckSeason => "check_season",
            Self::AccomplishmentEarned => "accomplishment-earned",
            Self::AccomplishmentEarnedClimax => "accomplishment-earned-climax",
            Self::YesAndTerminal => "yes-and-terminal",
            Self::CompetenceConsistency => "competence-consistency",
            Self::CompetenceArcFlat => "competence-arc-flat",
            Self::RevealFairPlay => "reveal-fair-play",
            Self::MysteryStarved => "mystery-starved",
            Self::ColdOpenDiscipline => "cold-open-discipline",
            Self::CastIntroductionRate => "cast-introduction-rate",
            Self::SceneNameClash => "scene-name-clash",
            Self::PromiseDensity => "promise-density",
            Self::ArcShape => "arc-shape",
            Self::AudioLoadTrajectory => "audio-load-trajectory",
        }
    }

    /// Resolve a `craft.`/`outline.` finding item back to its kernel rule.
    #[must_use]
    pub fn from_id(id: &str) -> Option<KernelRule> {
        Self::ALL.into_iter().find(|rule| rule.id() == id)
    }
}

/// A factory-side check namespace (spec §6: prose checks stay factory-invoked).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FactoryNs {
    /// The `fantasy.craft.check` prose battery.
    Check,
    /// The `fantasy.style` register lint.
    Style,
    /// The `fantasy.craft.raise` bar-raiser pass.
    Raise,
}

impl FactoryNs {
    /// The legacy `namespace:` prefix.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Style => "style",
            Self::Raise => "raise",
        }
    }
}

/// How a principle's check binds — the ported `checkable` × `check_ref`, typed.
///
/// A kernel binding names a [`KernelRule`], a closed enum, so a dangling deterministic
/// reference is a compile error — the legacy `check_ref_resolves` reference checker,
/// in the type system:
///
/// ```compile_fail
/// use dli_world::craft::kb::{Binding, KernelRule};
/// // The kernel rules are a closed set; a rule that does not exist is not a variant.
/// let _bad = Binding::Rule(KernelRule::NoSuchRule);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "binding", content = "spec", rename_all = "kebab-case")]
pub enum Binding {
    /// A deterministic kernel rule (the reference cannot dangle — closed enum).
    Rule(KernelRule),
    /// A deterministic check that stays factory-side (spec §6), recorded not run.
    Factory {
        /// Which factory battery owns it.
        namespace: FactoryNs,
        /// The check symbol within that battery.
        symbol: Cow<'static, str>,
    },
    /// A judge dimension: consumes a stored attestation of this question kind and
    /// reports grade [`Grade::Attested`].
    Attest(Cow<'static, str>),
    /// Declaration-only: no check, surfaced by brief/phase queries.
    Declaration,
}

impl Binding {
    /// The legacy checkability trichotomy this binding realizes.
    #[must_use]
    pub fn checkable(&self) -> Checkable {
        match self {
            Self::Rule(_) | Self::Factory { .. } => Checkable::Deterministic,
            Self::Attest(_) => Checkable::Judge,
            Self::Declaration => Checkable::None,
        }
    }

    /// Whether this binding is standing checkable-debt — `judge` or `none` (spec
    /// §5: the roadmap of rules not yet written).
    #[must_use]
    pub fn is_debt(&self) -> bool {
        matches!(self, Self::Attest(_) | Self::Declaration)
    }

    /// The kernel rule this binding names, if it is a kernel binding.
    #[must_use]
    pub fn kernel_rule(&self) -> Option<KernelRule> {
        match self {
            Self::Rule(rule) => Some(*rule),
            _ => None,
        }
    }

    /// The judge question kind this binding consumes, if it is a judge binding.
    #[must_use]
    pub fn attest_kind(&self) -> Option<&str> {
        match self {
            Self::Attest(kind) => Some(kind.as_ref()),
            _ => None,
        }
    }
}

/// The legacy checkability trichotomy (`none | deterministic | judge`), derived
/// from a [`Binding`]. Kept as an explicit type because coverage and the
/// checkable-debt report speak in these terms (spec §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Checkable {
    /// Declaration-only.
    None,
    /// A deterministic check (kernel rule or factory battery).
    Deterministic,
    /// A judge dimension, consumed as an attestation.
    Judge,
}

/// One knowledge-base principle: researched, cited, mission-filtered doctrine.
///
/// Every field is the legacy entry's field, ported intact; `checkable` and
/// `check_ref` are folded into the typed [`binding`](Self::binding), and
/// `tension_note`/`resolution` into the typed
/// [`mission_filter`](Self::mission_filter).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Principle {
    /// The unique kebab-case slug.
    pub id: Cow<'static, str>,
    /// The doctrine family.
    pub family: Family,
    /// The principle, stated plainly.
    pub statement: Cow<'static, str>,
    /// The citation(s) — every entry is sourced.
    pub source: Cow<'static, str>,
    /// How the mission filters it (carrying its note/resolution).
    pub mission_filter: MissionFilter,
    /// The production phases it bears on.
    pub phases: Cow<'static, [Phase]>,
    /// How its check binds.
    pub binding: Binding,
}

impl Principle {
    /// The checkability trichotomy this principle realizes.
    #[must_use]
    pub fn checkable(&self) -> Checkable {
        self.binding.checkable()
    }

    /// Whether this is a pillar principle (wins a conflict — D-0128).
    #[must_use]
    pub fn is_pillar(&self) -> bool {
        self.family.is_pillar()
    }

    /// The tension/adaptation note, when the mission filter carries one.
    #[must_use]
    pub fn tension_note(&self) -> Option<&str> {
        self.mission_filter.note()
    }

    /// The mission-side resolution, present exactly for a tension.
    #[must_use]
    pub fn resolution(&self) -> Option<&str> {
        self.mission_filter.resolution()
    }

    /// The bounded judge question this principle poses against `atom`/`subject`,
    /// present exactly when the principle is judge-bound. The atom is the item
    /// under judgment; the subject is the staleness anchor (see [`Question`]).
    #[must_use]
    pub fn judge_question(
        &self,
        atom: impl Into<String>,
        subject: impl Into<String>,
    ) -> Option<Question> {
        self.binding.attest_kind().map(|kind| Question {
            kind: kind.to_owned(),
            atom: atom.into(),
            subject: subject.into(),
        })
    }

    /// Render the principle in place — statement, then the tension and its
    /// resolution when present. This is the body of `deloreis craft why`.
    #[must_use]
    pub fn explain(&self) -> String {
        use std::fmt::Write as _;
        let mut out = format!("{} — {}\n{}", self.id, self.family_label(), self.statement);
        if let Some(note) = self.tension_note() {
            let _ = write!(out, "\n  tension: {note}");
        }
        if let Some(resolution) = self.resolution() {
            let _ = write!(out, "\n  resolution: {resolution}");
        }
        let _ = write!(out, "\n  source: {}", self.source);
        out
    }

    fn family_label(&self) -> &'static str {
        match self.family {
            Family::Plot => "plot",
            Family::Character => "character",
            Family::World => "world",
            Family::Prose => "prose",
            Family::Serial => "serial",
            Family::Audio => "audio",
            Family::Pillar => "pillar",
        }
    }
}

// ---------------------------------------------------------------------------
// The shipped generic doctrine library (Phase-3 craft separation, Step 4).
//
// These types are the *shipped* half of the DL-0018 seam: the world-agnostic
// craft doctrine dli ships (`craft-doctrines/*.toml`), authored from the KB
// classification. They are world-agnostic **by construction** — no mission
// filter, no citation, no binding symbol, and a [`DoctrineFamily`] that has no
// pillar variant — so world (TPtH) content physically cannot ride on them
// (design §1.1/§2.4). Since Step 5 these packs, composed with a world's craft
// config by `worldcfg::apply`, ARE the runtime corpus: the compiled TPtH table
// that was the source of truth in Step 4 is gone.
// ---------------------------------------------------------------------------

/// A generic default parameter value carried by a shipped [`DoctrinePrinciple`] —
/// a world may override it in its craft config (design §1.1). Deliberately small:
/// the tunables in the corpus are counts, ratios, flags, and short labels
/// (e.g. `said_tag_ratio = 0.85`, `cast_per_scene = 4`, `tryfail_count = 3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Scalar {
    /// A boolean flag.
    Bool(bool),
    /// An integer count.
    Int(i64),
    /// A real-valued ratio or threshold.
    Float(f64),
    /// A short textual value.
    Text(Cow<'static, str>),
}

/// The KIND of check a shipped doctrine principle invites — not a bound symbol.
///
/// The runtime [`Binding`] names a concrete kernel rule, factory battery, or judge
/// dimension; the shipped record keeps only the *kind*, because the concrete symbol
/// (a factory battery name, a judge-dimension name) is world-supplied config, not
/// generic doctrine (design §1.1/§2.4). The one exception is the kernel rule, which
/// is an engine battery from a closed set and therefore ships in
/// [`DoctrinePrinciple::kernel_rule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CheckableKind {
    /// Declaration-only: no check, surfaced by brief/phase queries.
    None,
    /// A deterministic in-engine kernel rule; the rule is named in
    /// [`DoctrinePrinciple::kernel_rule`].
    DeterministicKernel,
    /// A deterministic prose check that stays factory-side (spec §6); its symbol is
    /// world config.
    DeterministicProse,
    /// A judge dimension, consumed as an attestation; the dimension name is world
    /// config.
    Judge,
}

/// The doctrine axis of a shipped principle — [`Family`] minus [`Family::Pillar`].
///
/// A pillar is a *world-chosen* precedence tier (D-0128 for one world), never a
/// doctrine axis, so the shipped record cannot name it: this enum simply has no
/// pillar variant, making `Family::Pillar` unrepresentable in the shipped library
/// **by construction** (design §1.1/§2.4). It composes into a runtime [`Family`] via
/// [`DoctrineFamily::family`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DoctrineFamily {
    /// Plot: promises, causality, try/fail, arc shape.
    Plot,
    /// Character: sliders, interiority, voice.
    Character,
    /// World: material grounding, magic pedagogy.
    World,
    /// Prose: sentence craft, abstraction, said-ratio.
    Prose,
    /// Serial: open loops, micro-pacing, installment discipline.
    Serial,
    /// Audio: narrator/SFX load, aural distinctness, performability.
    Audio,
}

impl DoctrineFamily {
    /// The runtime [`Family`] this doctrine axis composes into (never
    /// [`Family::Pillar`]).
    #[must_use]
    pub fn family(self) -> Family {
        match self {
            Self::Plot => Family::Plot,
            Self::Character => Family::Character,
            Self::World => Family::World,
            Self::Prose => Family::Prose,
            Self::Serial => Family::Serial,
            Self::Audio => Family::Audio,
        }
    }
}

/// A shipped, world-agnostic craft principle — the data form of the generic
/// doctrine library (design §1.1).
///
/// This is the type the `craft-doctrines/*.toml` packs deserialize into. It is
/// world-agnostic **by construction**: it has no `mission_filter` field, no
/// citation field, and no binding-symbol field, and its [`family`](Self::family) is
/// a [`DoctrineFamily`] (which has no pillar variant), so world content physically
/// cannot ride on it — the generic/config boundary is compiler-checked, not
/// review-checked (design §2.4). A CI lint additionally forbids world tokens in
/// [`statement`](Self::statement) and [`source`](Self::source).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DoctrinePrinciple {
    /// The stable pack-namespaced slug (shared with the runtime [`Principle`] id).
    pub id: Cow<'static, str>,
    /// The pack this principle ships in; stamped from the pack file on load.
    #[serde(default)]
    pub pack: Cow<'static, str>,
    /// The doctrine axis (never a pillar).
    pub family: DoctrineFamily,
    /// The principle, stated world-agnostically.
    pub statement: Cow<'static, str>,
    /// A real, attributed published framework — never a world citation.
    pub source: Cow<'static, str>,
    /// The production phases it bears on.
    pub phases: Cow<'static, [Phase]>,
    /// The KIND of check it invites (not a bound symbol).
    pub checkable_kind: CheckableKind,
    /// The engine kernel rule it binds, present only for
    /// [`CheckableKind::DeterministicKernel`] (from the closed [`KernelRule`] set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_rule: Option<KernelRule>,
    /// Generic default parameters a world may override.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub default_params: BTreeMap<Cow<'static, str>, Scalar>,
}

impl DoctrinePrinciple {
    /// The runtime [`Family`] this shipped principle composes into.
    #[must_use]
    pub fn family(&self) -> Family {
        self.family.family()
    }
}

/// A shipped, togglable pack of world-agnostic [`DoctrinePrinciple`]s (design §1.1).
///
/// Packs are fine-grained on purpose; a world's craft config enables them
/// individually or by coarse group. The `save-the-cat` pack ships as a declared,
/// **empty** stub (the corpus has no beat-sheet principle to fabricate).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DoctrinePack {
    /// The pack name (e.g. `prose-craft`, `audio-drama`).
    pub pack: Cow<'static, str>,
    /// The principles this pack ships, in pack order (empty for a declared stub).
    #[serde(default)]
    pub principles: Vec<DoctrinePrinciple>,
}

/// The engine's default **generic** doctrine corpus — the shipped packs composed
/// with an empty world config (`CraftConfig::generic`), computed once (design §5,
/// Step 5). This carries **no** world (`TPtH`) content: when ownership flipped, the
/// compiled 107-principle `TPtH` table was removed and a world's corpus became
/// `apply(load_config(dir), doctrines())`. [`Kb::builtin`] reads this corpus.
static GENERIC_CORPUS: std::sync::LazyLock<Vec<Principle>> = std::sync::LazyLock::new(|| {
    let packs = super::load::doctrines();
    super::worldcfg::apply(&super::worldcfg::CraftConfig::generic(&packs), &packs)
});

/// A borrow-generic handle over a principle corpus (design §2.1, `CRAFT-3`).
///
/// The six knowledge-base queries are methods reading `self.principles`, so the
/// same query surface serves the engine's default generic corpus
/// ([`Kb::builtin`]) and any loaded world corpus ([`super::worldcfg::apply`]). The
/// borrow is `'a`, so every query hands out `&'a Principle` references into the
/// underlying slice — independent of the `&self` borrow.
///
/// The free functions ([`by_id`], [`why`], …) remain as thin shims over
/// [`Kb::builtin`]; prefer the methods.
#[derive(Debug, Clone, Copy)]
pub struct Kb<'a> {
    principles: &'a [Principle],
}

impl Kb<'static> {
    /// The built-in knowledge base — the engine's default **generic** doctrine
    /// corpus (`GENERIC_CORPUS`: the shipped packs composed with an empty world
    /// config). It carries no world content; a world's own corpus is
    /// [`apply`](super::worldcfg::apply)`(load_config(dir), doctrines())`.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            principles: GENERIC_CORPUS.as_slice(),
        }
    }

    /// The shipped generic doctrine set — every [`DoctrinePrinciple`] across the
    /// embedded `craft-doctrines/*.toml` packs, flattened in pack-declaration order
    /// (design §1.1/§4). This is the world-agnostic library dli ships; a world's
    /// craft config composes it into the runtime corpus via
    /// [`super::worldcfg::apply`], and the engine's own default
    /// ([`Kb::builtin`]) is these packs composed with an empty world config.
    #[must_use]
    pub fn doctrines() -> Vec<DoctrinePrinciple> {
        super::load::doctrines()
            .into_iter()
            .flat_map(|pack| pack.principles)
            .collect()
    }
}

impl<'a> Kb<'a> {
    /// A knowledge base over an arbitrary principle slice.
    #[must_use]
    pub fn new(principles: &'a [Principle]) -> Self {
        Self { principles }
    }

    /// The principle slice this handle reads.
    #[must_use]
    pub fn principles(&self) -> &'a [Principle] {
        self.principles
    }

    /// Look a principle up by its id.
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<&'a Principle> {
        self.principles.iter().find(|p| p.id == id)
    }

    /// Every principle that binds a given kernel rule, in corpus order.
    #[must_use]
    pub fn principles_binding(&self, rule: KernelRule) -> Vec<&'a Principle> {
        self.principles
            .iter()
            .filter(|p| p.binding.kernel_rule() == Some(rule))
            .collect()
    }

    /// The standing checkable-debt report: principles bound `judge` or `none` — the
    /// roadmap of rules not yet written, kept honest rather than hidden (spec §5).
    #[must_use]
    pub fn checkable_debt(&self) -> Vec<&'a Principle> {
        self.principles
            .iter()
            .filter(|p| p.binding.is_debt())
            .collect()
    }

    /// Tally the knowledge base by binding class.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        let mut c = Coverage {
            total: self.principles.len(),
            kernel_rule: 0,
            factory: 0,
            judge: 0,
            declaration: 0,
        };
        for p in self.principles {
            match p.binding {
                Binding::Rule(_) => c.kernel_rule += 1,
                Binding::Factory { .. } => c.factory += 1,
                Binding::Attest(_) => c.judge += 1,
                Binding::Declaration => c.declaration += 1,
            }
        }
        c
    }

    /// The principles behind a finding, **pillars first** (D-0128 — the pillar
    /// wins).
    ///
    /// A `craft.<rule>` finding resolves to the principles binding that season rule;
    /// an `outline.<item>` finding resolves to the principles behind the `CRAFT-1`
    /// batteries. Within the result, pillar-family principles sort first, so the
    /// head of the list is the doctrine that governs when advice conflicts. Empty
    /// when no principle claims the code.
    #[must_use]
    pub fn why(&self, finding_code: &str) -> Vec<&'a Principle> {
        let item = finding_code
            .split_once('.')
            .map_or(finding_code, |(_, item)| item);
        let mut hits: Vec<&'a Principle> = if finding_code.starts_with("outline.") {
            self.principles
                .iter()
                .filter(|p| {
                    matches!(
                        p.binding.kernel_rule(),
                        Some(KernelRule::CheckOutline | KernelRule::CheckSeason)
                    )
                })
                .collect()
        } else if let Some(rule) = KernelRule::from_id(item) {
            self.principles_binding(rule)
        } else {
            Vec::new()
        };
        // Pillars win: stable-sort the pillar family to the front (D-0128).
        hits.sort_by_key(|p| !p.is_pillar());
        hits
    }

    /// Audit the knowledge base for the integrity faults the types do not preclude:
    /// duplicate ids and phase-less principles. Empty on the ported corpus; a
    /// nonempty result is a data fault to fix at the source.
    #[must_use]
    pub fn audit(&self) -> Vec<KbError> {
        let mut errors = Vec::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for p in self.principles {
            if !seen.insert(p.id.as_ref()) {
                errors.push(KbError::DuplicateId(p.id.to_string()));
            }
            if p.phases.is_empty() {
                errors.push(KbError::NoPhase(p.id.to_string()));
            }
        }
        errors
    }
}

/// Look a principle up by its id. Thin shim over [`Kb::builtin`]`.by_id`.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Principle> {
    Kb::builtin().by_id(id)
}

/// Every principle that binds a given kernel rule, in corpus order. Thin shim over
/// [`Kb::builtin`]`.principles_binding`.
#[must_use]
pub fn principles_binding(rule: KernelRule) -> Vec<&'static Principle> {
    Kb::builtin().principles_binding(rule)
}

/// The standing checkable-debt report: principles bound `judge` or `none` — the
/// roadmap of rules not yet written, kept honest rather than hidden (spec §5). Thin
/// shim over [`Kb::builtin`]`.checkable_debt`.
#[must_use]
pub fn checkable_debt() -> Vec<&'static Principle> {
    Kb::builtin().checkable_debt()
}

/// A coverage tally over the knowledge base — how each principle's check binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Coverage {
    /// Total principles.
    pub total: usize,
    /// Bound to an in-kernel deterministic rule.
    pub kernel_rule: usize,
    /// Bound to a factory-side deterministic check (spec §6).
    pub factory: usize,
    /// Bound to a judge dimension (attestation-consuming).
    pub judge: usize,
    /// Declaration-only.
    pub declaration: usize,
}

impl Coverage {
    /// Deterministic checks: kernel rules plus factory batteries.
    #[must_use]
    pub fn deterministic(&self) -> usize {
        self.kernel_rule + self.factory
    }

    /// Standing checkable-debt: judge plus declaration-only (spec §5).
    #[must_use]
    pub fn debt(&self) -> usize {
        self.judge + self.declaration
    }
}

/// Tally the knowledge base by binding class. Thin shim over
/// [`Kb::builtin`]`.coverage`.
#[must_use]
pub fn coverage() -> Coverage {
    Kb::builtin().coverage()
}

/// The principles behind a finding, **pillars first** (D-0128 — the pillar wins).
///
/// A `craft.<rule>` finding resolves to the principles binding that season rule; an
/// `outline.<item>` finding resolves to the principles behind the `CRAFT-1`
/// batteries. Within the result, pillar-family principles sort first, so the head
/// of the list is the doctrine that governs when advice conflicts. Empty when no
/// principle claims the code.
///
/// Thin shim over [`Kb::builtin`]`.why`.
///
/// ```
/// use dli_world::craft::kb::why;
/// // Every finding the KB explains renders its governing principle first.
/// let ps = why("craft.accomplishment-earned");
/// assert!(ps.iter().any(|p| p.id == "tryfail-yes-but-no-and") || ps.is_empty());
/// ```
#[must_use]
pub fn why(finding_code: &str) -> Vec<&'static Principle> {
    Kb::builtin().why(finding_code)
}

/// Consume a stored attestation for a judge-bound principle, reporting a finding
/// graded [`Grade::Attested`] — the judgment was made once, in the factory, by a
/// model; the kernel only surfaces it (spec §5, 07-verdicts-and-llm-boundary §3).
/// `None` for a principle that is not judge-bound (there is no attestation to
/// consume).
///
/// A present attestation yields an `attested` finding carrying the model's answer;
/// an absent one yields a deterministic `craft.attestation-missing` — the kernel
/// never guesses a judgment ([`crate::attest::require_attestation`]).
#[must_use]
pub fn evaluate_judge(
    principle: &Principle,
    store: &crate::attest::AttestationStore,
    atom: impl Into<String>,
    subject: impl Into<String>,
) -> Option<WorldFinding> {
    let question = principle.judge_question(atom, subject)?;
    let key = question.key();
    Some(match crate::attest::require_attestation(store, &key) {
        Ok(attestation) => attested_finding(principle, attestation),
        Err(_) => missing_finding(principle, &key),
    })
}

/// The `attested`-graded finding a judge-bound principle produces from a stored
/// attestation — the ORACLE: a judge-bound principle reports [`Grade::Attested`].
#[must_use]
pub fn attested_finding(principle: &Principle, attestation: &Attestation) -> WorldFinding {
    WorldFinding::warn(
        format!("craft.{}", principle.id),
        format!(
            "{}: judge dimension attested `{}` ({}) — grade attested, the kernel \
             surfaces the factory's judgment, it does not re-run it",
            principle.id, attestation.answer, principle.statement
        ),
    )
    .with_grade(Grade::Attested)
}

/// The `craft.attestation-missing` finding for a judge principle whose attestation
/// the store lacks — a deterministic (checked) absence, never a guessed verdict.
#[must_use]
pub fn missing_finding(principle: &Principle, key: &QuestionKey) -> WorldFinding {
    WorldFinding::warn(
        "craft.attestation-missing".to_owned(),
        format!(
            "{}: no attestation for '{}' in the store; a sweep must pose the judge \
             question before this dimension can report — the kernel will not guess",
            principle.id, key.0
        ),
    )
}

/// A knowledge-base integrity fault the type system cannot rule out on its own —
/// the reference checker's runtime complement (the compile-time half is
/// [`KernelRule`]'s closedness and [`MissionFilter`]'s required fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub enum KbError {
    /// Two principles share an id.
    DuplicateId(String),
    /// A principle carries no phase — it would be invisible to every phase query.
    NoPhase(String),
}

/// Audit the knowledge base for the integrity faults the types do not preclude:
/// duplicate ids and phase-less principles. Empty on the ported corpus (asserted
/// in the tests); a nonempty result is a data fault to fix at the source. Thin shim
/// over [`Kb::builtin`]`.audit`.
#[must_use]
pub fn audit() -> Vec<KbError> {
    Kb::builtin().audit()
}
