//! The merged outline schema — the season-wide first-class objects (`CRAFT-1`).
//!
//! This is the faithful port of the consuming repo's `fantasy.craft.outline`
//! schema ([06-craft-and-payloads](../../../../docs/spec/06-craft-and-payloads.md)
//! §1): promises, MICE threads, try/fail beats, character sliders, scene work,
//! the audio plan, the material-assumptions ledger, and scenes, gathered under a
//! [`SeasonOutline`] of ordered [`EpisodeOutline`]s. The legacy `EpisodeOutline`
//! and `EpisodeState` unify here so the checker in [`season`](super::season) can
//! join across episodes — the move that makes the season-scope rules possible.
//!
//! **How it hangs off the [`Episode`](crate::episode::Episode) spine.** An
//! [`EpisodeOutline::id`] *references* the production-order episode of the same id
//! on the wave-2 spine; the outline never re-folds knowledge or facts (that is the
//! [`fold`](crate::episode::fold)'s job). The outline carries only the craft
//! surface — promises, beats, sliders, and the outline's own [`Scene`] rows
//! (speaking cast and what each scene advances), which are distinct from the
//! spine's fact/knowledge deltas.
//!
//! **Serde is the payload contract.** Every type deserializes from the same YAML
//! the consuming repo's `fantasy craft outline` command reads, so one document
//! feeds both the legacy oracle and this kernel — the basis of the `CRAFT-1`
//! parity check. String enums use the legacy wire spellings (`yes-but`,
//! `load-bearing`, `off-mic-state`, the `snake_case` material categories); every
//! struct is `deny_unknown_fields`, matching the legacy `extra="forbid"`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The kind of promise made to the listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PromiseKind {
    /// The season-spanning umbrella promise.
    Umbrella,
    /// A core dramatic promise.
    Core,
    /// A character-arc promise.
    Character,
    /// A mystery/open-question promise.
    Mystery,
    /// A relationship promise.
    Relationship,
    /// A tonal promise.
    Tone,
}

/// A MICE thread's kind (Milieu / Inquiry / Character / Event).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MiceKind {
    /// A milieu thread (a place entered and left).
    Milieu,
    /// An inquiry thread (a question raised and answered).
    Inquiry,
    /// A character thread (an identity strained and settled).
    Character,
    /// An event thread (an upset and its new order).
    Event,
}

/// A try/fail outcome, typed on the yes/no × but/and grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TryFailOutcome {
    /// Succeeds, but at a cost that complicates.
    YesBut,
    /// Fails, and the situation worsens.
    NoAnd,
    /// Fails, but a consolation opens a path.
    NoBut,
    /// Succeeds cleanly — the terminal outcome.
    YesAnd,
}

/// What a scene moves forward (micro-pacing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Advance {
    /// The scene advances the plot.
    Plot,
    /// The scene advances a relationship.
    Relationship,
    /// The scene advances a revelation.
    Revelation,
}

/// The scope over which a season/arc promise is deliberately carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CarryScope {
    /// Carried to a later episode.
    Episode,
    /// Carried across the arc.
    Arc,
    /// Carried across the act.
    Act,
    /// Carried across the whole series.
    Series,
}

/// The narrator framing of an audio packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NarratorMode {
    /// No narrator at all.
    None,
    /// A windowpane narrator — transparent orientation, no personality.
    Windowpane,
    /// A frame narrator.
    Frame,
    /// A tale-host narrator (a host-narrator ban is world policy).
    Host,
}

/// The listener's orientation load in a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AudioLoad {
    /// Low orientation load.
    Low,
    /// Medium orientation load.
    Medium,
    /// High orientation load.
    High,
}

/// The density of the SFX bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SfxLoad {
    /// No SFX.
    None,
    /// Sparse SFX support.
    Sparse,
    /// A moderate SFX bed.
    Moderate,
    /// A heavy SFX bed.
    Heavy,
}

/// Whether SFX merely supports or carries meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SfxSemanticLoad {
    /// SFX supports the scene without carrying meaning.
    Support,
    /// SFX carries semantic load (a burden to check against the narrator plan).
    LoadBearing,
}

/// A load-bearing task the narrator carries in a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NarratorTask {
    /// Orienting the listener in space and situation.
    Orientation,
    /// Making causality legible.
    Causality,
    /// Reporting off-mic state.
    OffMicState,
    /// Naming an object relationship.
    ObjectRelationship,
    /// Marking the passage of time.
    TimePassage,
    /// Holding a tonal frame.
    ToneFrame,
}

/// A material-planning category. Serialized in `snake_case` to match the legacy
/// ledger field names.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MaterialUnknown {
    /// The season's time pressure.
    SeasonTimePressure,
    /// The staple-food dependency.
    StapleDependency,
    /// The water dependency.
    WaterDependency,
    /// The fuel dependency.
    FuelDependency,
    /// The shelter dependency.
    ShelterDependency,
    /// The labor group involved.
    LaborGroup,
    /// The source of the material.
    Source,
    /// How the material is transported.
    Transport,
    /// How the material is stored.
    Storage,
    /// The failure point.
    FailurePoint,
    /// The institution governing access.
    Institution,
    /// The ritual obligation attached (never required, only optional).
    RitualObligation,
}

impl MaterialUnknown {
    /// Every material category, in ledger order.
    pub const ALL: [MaterialUnknown; 12] = [
        Self::SeasonTimePressure,
        Self::StapleDependency,
        Self::WaterDependency,
        Self::FuelDependency,
        Self::ShelterDependency,
        Self::LaborGroup,
        Self::Source,
        Self::Transport,
        Self::Storage,
        Self::FailurePoint,
        Self::Institution,
        Self::RitualObligation,
    ];

    // Which categories a material-worldbuilding review requires is world policy,
    // not schema: it lives in the world's material catalog
    // (`craft::config::MaterialCatalog`, default `DEFAULT_MATERIAL_CATALOG`), which
    // the checker loops. `ALL` above stays — it is the enum's own variant list.

    /// The `snake_case` ledger field name (the legacy wire spelling).
    #[must_use]
    pub fn field_name(self) -> &'static str {
        match self {
            Self::SeasonTimePressure => "season_time_pressure",
            Self::StapleDependency => "staple_dependency",
            Self::WaterDependency => "water_dependency",
            Self::FuelDependency => "fuel_dependency",
            Self::ShelterDependency => "shelter_dependency",
            Self::LaborGroup => "labor_group",
            Self::Source => "source",
            Self::Transport => "transport",
            Self::Storage => "storage",
            Self::FailurePoint => "failure_point",
            Self::Institution => "institution",
            Self::RitualObligation => "ritual_obligation",
        }
    }
}

/// One promise made to the listener, with its eventual payoff or its carry.
///
/// Exactly one of [`payoff_ref`](Self::payoff_ref) or
/// [`open_loop_carry`](Self::open_loop_carry) must be set — the legacy pydantic
/// `_payoff_or_carry` validator, ported to a check finding
/// ([`super::season`]) so malformed input is a fail-closed finding rather than a
/// deserialization panic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Promise {
    /// The promise identifier.
    pub id: String,
    /// The kind of promise.
    pub kind: PromiseKind,
    /// What is promised, in the listener's terms.
    pub statement: String,
    /// Where the promise is made — a scene id, or `"cold-open"`.
    pub made_in: String,
    /// The scene id where it pays off, if it pays off in this episode.
    #[serde(default)]
    pub payoff_ref: Option<String>,
    /// Whether the promise is deliberately carried to a later episode/season.
    #[serde(default)]
    pub open_loop_carry: bool,
}

/// A MICE thread with the scenes where it opens and closes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MiceThread {
    /// The thread identifier.
    pub id: String,
    /// The thread's kind.
    pub kind: MiceKind,
    /// The scene id where the thread opens.
    pub open_beat: String,
    /// The scene id where the thread closes.
    pub close_beat: String,
}

/// One attempt in a try/fail cycle, typed by outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TryFailBeat {
    /// The beat identifier.
    pub id: String,
    /// What was attempted.
    pub attempt: String,
    /// How the attempt turned out.
    pub outcome: TryFailOutcome,
}

/// Per-major-character proactivity/competence/sympathy accounting.
///
/// The three dials are `0..=10` in the legacy schema; the port keeps them as
/// `u8` and does not reject an out-of-range value at deserialize time (the legacy
/// pydantic field bound is input validation, and every check compares only
/// against the [`policy`](super::season::policy) thresholds, which are well
/// inside the byte range).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterSlider {
    /// The character these dials describe.
    pub character: String,
    /// How much the character drives events.
    pub proactivity: u8,
    /// How capable the character is.
    pub competence: u8,
    /// How much the audience is drawn to the character.
    pub sympathy: u8,
    /// A note naming which dial carries investment when another runs low.
    #[serde(default)]
    pub compensation: String,
    /// The scenes this ledger row draws on.
    #[serde(default)]
    pub scene_refs: Vec<String>,
}

/// Scene/sequel mechanics: goal, conflict, disaster, and optional sequel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneWork {
    /// The scene this work describes.
    pub scene_id: String,
    /// The viewpoint character, if fixed.
    #[serde(default)]
    pub viewpoint: Option<String>,
    /// The scene goal.
    pub goal: String,
    /// The conflict opposing the goal.
    pub conflict: String,
    /// The disaster the scene turns on.
    pub disaster: String,
    /// How the scene resolves.
    pub outcome: TryFailOutcome,
    /// The sequel's reaction beat, if present.
    #[serde(default)]
    pub sequel_reaction: Option<String>,
    /// The sequel's dilemma beat, if present.
    #[serde(default)]
    pub sequel_dilemma: Option<String>,
    /// The sequel's decision beat, if present.
    #[serde(default)]
    pub sequel_decision: Option<String>,
}

/// The narrator/SFX load boundary for an audio packet.
///
/// Every field carries the legacy default (a windowpane narrator, medium
/// orientation, sparse supporting SFX), so an empty `audio_plan: {}` maps to the
/// same object the legacy model builds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct AudioPlan {
    /// The narrator framing.
    pub narrator_mode: NarratorMode,
    /// The orientation load placed on the listener.
    pub orientation_load: AudioLoad,
    /// The density of the SFX bed.
    pub sfx_load: SfxLoad,
    /// Whether SFX merely supports or carries meaning.
    pub sfx_semantic_load: SfxSemanticLoad,
    /// The load-bearing tasks the narrator carries.
    pub narrator_tasks: Vec<NarratorTask>,
    /// Whether the narrator addresses the audience directly (a policy-forbidden
    /// framing).
    pub direct_address: bool,
}

impl Default for AudioPlan {
    fn default() -> Self {
        Self {
            narrator_mode: NarratorMode::Windowpane,
            orientation_load: AudioLoad::Medium,
            sfx_load: SfxLoad::Sparse,
            sfx_semantic_load: SfxSemanticLoad::Support,
            narrator_tasks: Vec::new(),
            direct_address: false,
        }
    }
}

/// Optional, provenance-labelled material planning assumptions.
///
/// This records planning questions and working assumptions; it is **not** a
/// world-model fact surface. `research_refs` never make an answer canon, and any
/// unresolved category belongs in [`unknowns`](Self::unknowns) rather than
/// receiving a guessed value — the boundary the checker enforces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialAssumptionLedger {
    /// The season's time pressure, if assumed.
    #[serde(default)]
    pub season_time_pressure: Option<String>,
    /// The staple-food dependency, if assumed.
    #[serde(default)]
    pub staple_dependency: Option<String>,
    /// The water dependency, if assumed.
    #[serde(default)]
    pub water_dependency: Option<String>,
    /// The fuel dependency, if assumed.
    #[serde(default)]
    pub fuel_dependency: Option<String>,
    /// The shelter dependency, if assumed.
    #[serde(default)]
    pub shelter_dependency: Option<String>,
    /// The labor group, if assumed.
    #[serde(default)]
    pub labor_group: Option<String>,
    /// The source of the material, if assumed.
    #[serde(default)]
    pub source: Option<String>,
    /// How the material is transported, if assumed.
    #[serde(default)]
    pub transport: Option<String>,
    /// How the material is stored, if assumed.
    #[serde(default)]
    pub storage: Option<String>,
    /// The failure point, if assumed.
    #[serde(default)]
    pub failure_point: Option<String>,
    /// The governing institution, if assumed.
    #[serde(default)]
    pub institution: Option<String>,
    /// The ritual obligation, if assumed.
    #[serde(default)]
    pub ritual_obligation: Option<String>,
    /// Decision-ledger references (`D-nnnn`) supporting planning provenance.
    #[serde(default)]
    pub canon_refs: Vec<String>,
    /// Research references (`RR-nnn`, a research doc path, or an URL).
    #[serde(default)]
    pub research_refs: Vec<String>,
    /// Categories deliberately left unknown.
    #[serde(default)]
    pub unknowns: Vec<MaterialUnknown>,
}

impl MaterialAssumptionLedger {
    /// The working answer recorded for a category, if any.
    #[must_use]
    pub fn field(&self, category: MaterialUnknown) -> Option<&str> {
        let value = match category {
            MaterialUnknown::SeasonTimePressure => &self.season_time_pressure,
            MaterialUnknown::StapleDependency => &self.staple_dependency,
            MaterialUnknown::WaterDependency => &self.water_dependency,
            MaterialUnknown::FuelDependency => &self.fuel_dependency,
            MaterialUnknown::ShelterDependency => &self.shelter_dependency,
            MaterialUnknown::LaborGroup => &self.labor_group,
            MaterialUnknown::Source => &self.source,
            MaterialUnknown::Transport => &self.transport,
            MaterialUnknown::Storage => &self.storage,
            MaterialUnknown::FailurePoint => &self.failure_point,
            MaterialUnknown::Institution => &self.institution,
            MaterialUnknown::RitualObligation => &self.ritual_obligation,
        };
        value.as_deref()
    }

    /// Material categories carrying a working answer rather than an unknown.
    #[must_use]
    pub fn asserted_fields(&self) -> Vec<MaterialUnknown> {
        MaterialUnknown::ALL
            .into_iter()
            .filter(|category| self.field(*category).is_some())
            .collect()
    }
}

/// One scene: its speaking cast and what it moves.
///
/// This is the outline's craft view of a scene, distinct from the
/// [`Scene`](crate::episode::Scene) on the fold spine: it carries the speaking
/// cast and the [`Advance`]s the scene makes, not the fact/knowledge deltas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    /// The scene identifier, unique within its episode.
    pub id: String,
    /// A one-line summary.
    pub summary: String,
    /// The speaking cast (checked against the cast-over-four ceiling).
    #[serde(default)]
    pub speaking_cast: Vec<String>,
    /// What the scene advances (checked for micro-pacing).
    #[serde(default)]
    pub advances: Vec<Advance>,
    /// Whether a scripted silence/held beat is present (audio).
    #[serde(default)]
    pub silence_beat: bool,
}

/// A single-episode outline in the fractal-kishōtenketsu shape.
///
/// [`id`](Self::id) references the production-order [`Episode`](crate::episode::Episode)
/// of the same identifier on the fold spine; the outline adds the craft surface
/// on top without re-folding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EpisodeOutline {
    /// The episode identifier (references the spine [`Episode`](crate::episode::Episode)).
    pub id: String,
    /// The episode title.
    pub title: String,
    /// When the hook lands, in seconds (over the ceiling warns `hook-late`).
    #[serde(default = "default_hook_seconds")]
    pub hook_by_seconds: u32,
    /// The *ki* — the opening image.
    #[serde(default)]
    pub ki: String,
    /// The *shō* — the development.
    #[serde(default)]
    pub sho: String,
    /// The *ten* — the turn (scanned for battle vocabulary).
    #[serde(default)]
    pub ten: String,
    /// The *ketsu* — the resolution.
    #[serde(default)]
    pub ketsu: String,
    /// The promise ledger for this episode.
    #[serde(default)]
    pub promises: Vec<Promise>,
    /// The MICE threads active in this episode.
    #[serde(default)]
    pub mice_threads: Vec<MiceThread>,
    /// The try/fail beats driving the middle.
    #[serde(default)]
    pub tryfail_beats: Vec<TryFailBeat>,
    /// The major characters this episode carries.
    #[serde(default)]
    pub major_characters: Vec<String>,
    /// The per-major-character slider ledger.
    #[serde(default)]
    pub character_sliders: Vec<CharacterSlider>,
    /// The scene goal/conflict/disaster ledger.
    #[serde(default)]
    pub scene_work: Vec<SceneWork>,
    /// The audio plan, when the episode declares one.
    #[serde(default)]
    pub audio_plan: Option<AudioPlan>,
    /// The material-assumptions ledger, when the episode declares one.
    #[serde(default)]
    pub material_assumptions: Option<MaterialAssumptionLedger>,
    /// The scenes in reading order.
    #[serde(default)]
    pub scenes: Vec<Scene>,
    /// Psychological loops opened this episode.
    #[serde(default)]
    pub loops_opened: Vec<String>,
    /// Psychological loops closed this episode.
    #[serde(default)]
    pub loops_closed: Vec<String>,
    /// Whether this episode sits on a season boundary (waives the close-a-loop
    /// and settle-the-chain requirements).
    #[serde(default)]
    pub season_boundary: bool,
}

impl EpisodeOutline {
    /// The scene ids in reading order.
    #[must_use]
    pub fn scene_ids(&self) -> Vec<&str> {
        self.scenes.iter().map(|s| s.id.as_str()).collect()
    }
}

/// A season/arc-scope promise with an episode/scene origin and payoff/carry.
///
/// Exactly one of [`payoff_ref`](Self::payoff_ref) or
/// [`carry_scope`](Self::carry_scope) must be set — the legacy pydantic
/// validator, ported to a check finding in [`super::season`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArcPromise {
    /// The promise identifier.
    pub id: String,
    /// The kind of promise.
    pub kind: PromiseKind,
    /// What is promised.
    pub statement: String,
    /// Where it opens — an episode id, scene id, `"cold-open"`, or `"arc-open"`.
    pub opened_in: String,
    /// The episode or scene id where it pays off, if it pays off.
    #[serde(default)]
    pub payoff_ref: Option<String>,
    /// The scope over which it is deliberately carried, if carried.
    #[serde(default)]
    pub carry_scope: Option<CarryScope>,
}

/// A season: an ordered list of episodes plus season-scope promise accounting.
///
/// This is the season-wide ledger the [`season`](super::season) checker joins
/// over. A document with an `episodes` key deserializes here; a document without
/// one is a bare [`EpisodeOutline`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeasonOutline {
    /// The season identifier.
    pub id: String,
    /// The season title.
    pub title: String,
    /// The arc/season promise ledger.
    #[serde(default)]
    pub promises: Vec<ArcPromise>,
    /// The episodes in production order.
    #[serde(default)]
    pub episodes: Vec<EpisodeOutline>,
}

/// The legacy default hook deadline: 60 seconds.
fn default_hook_seconds() -> u32 {
    60
}
