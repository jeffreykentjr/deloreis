//! The episode spine and the production-order fold (EPI-1).
//!
//! Two times meet in an episode and the whole design turns on keeping them
//! apart ([`03-time`](../../../../docs/spec/03-time.md)): **production order** is
//! the sequence in which a listener meets the material — the listener's
//! dimension — and **story-time** is when things were so in the world. An
//! [`Episode`] therefore binds a [`production_order`](Episode::production_order)
//! and a [story anchor](Episode::story_anchor) *separately*, and a [`Scene`] may
//! carry its own [anchor](Scene::scene_anchor) to cross into story-time — a
//! flashback.
//!
//! [`fold`] runs the season in production order. It accumulates a knowledge
//! ledger per knower — the [`listener`](Knower::Listener) included, as a
//! distinguished production-axis knower — while each fact delta's story-time
//! placement comes from its scene anchor, falling back to the episode anchor. A
//! flashback episode thus writes its *reveals* at its production slot (that is
//! when the listener learns) and its *facts* at its story anchor (that is when
//! the world was so): two dimensions over one fold.
//!
//! This is the kernel spine only. The craft objects — promises with kinds,
//! beats with threads and magnitudes, sliders — hang on this spine in a later
//! wave (`06-craft-and-payloads.md` §1); the fold carries the raw knowledge,
//! fact-introduction, and open-loop events those rules join over.
//!
//! ```
//! use dli_world::episode::{
//!     Episode, EpisodeAnchor, FactDelta, FactRef, Knower, KnowledgeDelta, Scene, fold,
//! };
//!
//! // Two episodes handed to the fold out of production order.
//! let ep_two = Episode {
//!     id: "e2".to_owned(),
//!     production_order: 2,
//!     story_anchor: EpisodeAnchor::era("the-beleaguered-present"),
//!     scenes: vec![Scene {
//!         id: "s1".to_owned(),
//!         fact_deltas: vec![FactDelta::of("the-well-runs-dry")],
//!         knowledge_deltas: vec![KnowledgeDelta::listener_learns("the-well-runs-dry")],
//!         ..Scene::default()
//!     }],
//! };
//! let ep_one = Episode {
//!     id: "e1".to_owned(),
//!     production_order: 1,
//!     story_anchor: EpisodeAnchor::era("the-beleaguered-present"),
//!     scenes: vec![],
//! };
//!
//! let world = fold(&[ep_two, ep_one]);
//! let dry = FactRef("the-well-runs-dry".to_owned());
//!
//! // The listener does not learn it until they reach production slot 2.
//! assert!(world.knowledge_at(1, &Knower::Listener).is_empty());
//! assert!(world.knowledge_at(2, &Knower::Listener).contains(&dry));
//! // And the fact is placed in story-time where the episode is anchored.
//! assert!(world.story_placement(&dry).is_some());
//! ```

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::annot::Annotation;
use dli_time::wheel::WheelPos;

/// A reference to a cast entity (a character), by canonical name.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct EntityRef(pub String);

/// A reference to an era on the chronology spine, by name.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct EraRef(pub String);

/// A reference to a named event (a point on the timeline), by name.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct EventRef(pub String);

/// A reference to a fact the world may hold, by name.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct FactRef(pub String);

/// A reference to an open-loop question (a promise to the listener), by name.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct QuestionRef(pub String);

/// Where an episode or scene sits in story-time.
///
/// An anchor names an [`era`](Self::era) membership and, *within* that era, an
/// honestly-partial position pinned by [`after`](Self::after) /
/// [`before`](Self::before) event constraints (there is no offset — an
/// undeclared pair is unordered). It may also bind cyclic [`phases`](Self::phases)
/// — a tide, a moon-night, a season — one [`WheelPos`] per relevant wheel.
///
/// The [`Default`] (empty) anchor is *unanchored*: an unanchored episode follows
/// its predecessor in the story-present, the current behavior preserved. A
/// scene's own anchor, when present, overrides its episode's ([`Scene::scene_anchor`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EpisodeAnchor {
    /// The era this anchor sits in, if declared.
    pub era: Option<EraRef>,
    /// Events this anchor falls strictly after (intra-era precedence).
    pub after: Vec<EventRef>,
    /// Events this anchor falls strictly before (intra-era precedence).
    pub before: Vec<EventRef>,
    /// Cyclic phase coordinates at this story point, one per wheel.
    pub phases: Vec<WheelPos>,
}

impl EpisodeAnchor {
    /// An anchor placed only by era membership.
    #[must_use]
    pub fn era(name: impl Into<String>) -> Self {
        Self {
            era: Some(EraRef(name.into())),
            ..Self::default()
        }
    }

    /// Whether this anchor pins no story-time position at all (the migration
    /// default — follow the predecessor).
    #[must_use]
    pub fn is_unanchored(&self) -> bool {
        self.era.is_none()
            && self.after.is_empty()
            && self.before.is_empty()
            && self.phases.is_empty()
    }

    /// Add an `after` event constraint, builder-style.
    #[must_use]
    pub fn after_event(mut self, event: impl Into<String>) -> Self {
        self.after.push(EventRef(event.into()));
        self
    }

    /// Add a `before` event constraint, builder-style.
    #[must_use]
    pub fn before_event(mut self, event: impl Into<String>) -> Self {
        self.before.push(EventRef(event.into()));
        self
    }

    /// Bind a cyclic phase coordinate, builder-style.
    #[must_use]
    pub fn with_phase(mut self, phase: WheelPos) -> Self {
        self.phases.push(phase);
        self
    }
}

/// A knower whose knowledge the fold tracks.
///
/// The [`Listener`](Self::Listener) is distinguished: their ledger runs on the
/// production axis, which is exactly where the staging and reveal-placement
/// constraints attach. Cast knowers name a character.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum Knower {
    /// The audience — the distinguished production-axis knower.
    Listener,
    /// A character in the world.
    Cast(EntityRef),
}

impl Knower {
    /// A cast knower named by string.
    #[must_use]
    pub fn cast(name: impl Into<String>) -> Self {
        Self::Cast(EntityRef(name.into()))
    }
}

/// An entity's presence in a scene: on-mic (an active speaker) or merely present
/// but off-mic. The distinction is load-bearing for the cast-over-four and
/// audio-plan checks that hang on this spine later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Presence {
    /// An active speaker in the scene.
    OnMic(EntityRef),
    /// Present in the scene but not speaking.
    OffMic(EntityRef),
}

impl Presence {
    /// The entity, regardless of on- or off-mic.
    #[must_use]
    pub fn entity(&self) -> &EntityRef {
        match self {
            Self::OnMic(e) | Self::OffMic(e) => e,
        }
    }
}

/// Someone learns a fact. The [`knower`](Self::knower) may be the
/// [`listener`](Knower::Listener) — the distinguished production-axis knower — or
/// a cast character.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeDelta {
    /// Who learns.
    pub knower: Knower,
    /// The fact learned.
    pub fact: FactRef,
}

impl KnowledgeDelta {
    /// The listener learns a fact (a reveal, landing on the production axis).
    #[must_use]
    pub fn listener_learns(fact: impl Into<String>) -> Self {
        Self {
            knower: Knower::Listener,
            fact: FactRef(fact.into()),
        }
    }

    /// A named character learns a fact.
    #[must_use]
    pub fn cast_learns(who: impl Into<String>, fact: impl Into<String>) -> Self {
        Self {
            knower: Knower::cast(who),
            fact: FactRef(fact.into()),
        }
    }
}

/// A fact the episode introduces, carried with provenance. Its story-time
/// placement is resolved by the fold from the scene (or episode) anchor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FactDelta {
    /// The fact introduced.
    pub fact: FactRef,
    /// The provenance backing the claim (citations / hypothesis condition).
    pub provenance: Option<Annotation>,
}

impl FactDelta {
    /// A fact delta with no provenance yet attached.
    #[must_use]
    pub fn of(fact: impl Into<String>) -> Self {
        Self {
            fact: FactRef(fact.into()),
            provenance: None,
        }
    }
}

/// A scene: its cast and presence, its location, the facts and knowledge it
/// moves, and the open loops it opens or closes. A scene with its own
/// [`scene_anchor`](Self::scene_anchor) overrides the episode's story anchor — a
/// flashback.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    /// The scene identifier, unique within its episode.
    pub id: String,
    /// The cast present in the scene.
    pub cast: Vec<EntityRef>,
    /// Who is on-mic versus present but off-mic.
    pub presence: Vec<Presence>,
    /// Where the scene takes place, if named.
    pub location: Option<String>,
    /// A story-time override; `None` inherits the episode's anchor.
    pub scene_anchor: Option<EpisodeAnchor>,
    /// Who learns what fact in this scene.
    pub knowledge_deltas: Vec<KnowledgeDelta>,
    /// Facts this scene introduces, carried with provenance.
    pub fact_deltas: Vec<FactDelta>,
    /// Open loops (questions) opened here.
    pub opens: Vec<QuestionRef>,
    /// Open loops (questions) closed here.
    pub closes: Vec<QuestionRef>,
}

/// An episode: a production slot, a story anchor, and its scenes.
///
/// [`production_order`](Self::production_order) is the listener's dimension;
/// [`story_anchor`](Self::story_anchor) is the world's. The two are bound
/// separately and never collapse.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Episode {
    /// The episode identifier.
    pub id: String,
    /// Where the episode sits in production order — when the listener meets it.
    pub production_order: u32,
    /// Where the episode sits in story-time — when the world was so.
    pub story_anchor: EpisodeAnchor,
    /// The episode's scenes, in reading order.
    pub scenes: Vec<Scene>,
}

/// One knowledge event, resolved onto both axes by the fold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeRecord {
    /// Who learned.
    pub knower: Knower,
    /// The fact learned.
    pub fact: FactRef,
    /// The production slot at which the learning happened (the listener's when).
    pub production_order: u32,
    /// The story-time anchor in effect where the learning happened.
    pub story_anchor: EpisodeAnchor,
}

/// One fact-introduction, resolved onto its story-time placement by the fold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Placement {
    /// The fact introduced.
    pub fact: FactRef,
    /// Where in story-time the fact was so (scene anchor, else episode anchor).
    pub anchor: EpisodeAnchor,
    /// The production slot that introduced it (the listener's when).
    pub production_order: u32,
}

/// The introduced-versus-resolved shape of the open-loop ledger at a cursor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenLoopLedger {
    /// Questions opened and not yet closed, in the order they were opened.
    pub open: Vec<QuestionRef>,
    /// Questions opened and later closed, in the order they were closed.
    pub closed: Vec<QuestionRef>,
}

/// One open or close of a loop, tagged with its production slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
struct LoopEvent {
    production_order: u32,
    question: QuestionRef,
    opening: bool,
}

/// The result of folding a season in production order.
///
/// The fold is production-order-driven throughout: knowledge accrues by
/// production slot (the listener's dimension), while each fact's story placement
/// is projected separately from its anchor (the world's dimension). Read
/// production-axis knowledge with [`knowledge_at`](Self::knowledge_at), story
/// placement with [`story_placement`](Self::story_placement), and the running
/// open-loop shape with [`open_loops_at`](Self::open_loops_at).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EpisodeFold {
    knowledge: Vec<KnowledgeRecord>,
    placements: Vec<Placement>,
    loops: Vec<LoopEvent>,
}

impl EpisodeFold {
    /// What `knower` knows once the listener has reached production slot
    /// `production_pos`.
    ///
    /// Knowledge is inertial: every delta at a slot `<= production_pos` for this
    /// knower is included. The listener's own ledger is exactly this read with
    /// [`Knower::Listener`].
    #[must_use]
    pub fn knowledge_at(&self, production_pos: u32, knower: &Knower) -> BTreeSet<FactRef> {
        self.knowledge
            .iter()
            .filter(|r| r.production_order <= production_pos && &r.knower == knower)
            .map(|r| r.fact.clone())
            .collect()
    }

    /// Where in story-time a fact was introduced — its scene anchor, falling
    /// back to the episode anchor. `None` if no delta introduced the fact.
    ///
    /// This is the world's dimension, independent of the production slot at
    /// which the listener learned it: a flashback's fact lands here at its early
    /// story anchor even though the reveal landed late on the listener ledger.
    #[must_use]
    pub fn story_placement(&self, fact: &FactRef) -> Option<&EpisodeAnchor> {
        self.placements
            .iter()
            .find(|p| &p.fact == fact)
            .map(|p| &p.anchor)
    }

    /// The open-loop ledger as it stands once the listener has reached
    /// production slot `production_pos`: which questions are open, which resolved.
    #[must_use]
    pub fn open_loops_at(&self, production_pos: u32) -> OpenLoopLedger {
        let mut open: Vec<QuestionRef> = Vec::new();
        let mut closed: Vec<QuestionRef> = Vec::new();
        for event in self
            .loops
            .iter()
            .filter(|e| e.production_order <= production_pos)
        {
            if event.opening {
                if !open.contains(&event.question) {
                    open.push(event.question.clone());
                }
            } else {
                open.retain(|q| q != &event.question);
                if !closed.contains(&event.question) {
                    closed.push(event.question.clone());
                }
            }
        }
        OpenLoopLedger { open, closed }
    }

    /// The final open-loop ledger, once the whole season has played.
    #[must_use]
    pub fn open_loops(&self) -> OpenLoopLedger {
        let max = self.loops.iter().map(|e| e.production_order).max();
        self.open_loops_at(max.unwrap_or(0))
    }

    /// Every knowledge event, in production order — the raw ledger later waves
    /// (order-rules, craft) join over.
    #[must_use]
    pub fn knowledge_records(&self) -> &[KnowledgeRecord] {
        &self.knowledge
    }

    /// Every fact-introduction with its resolved story placement, in production
    /// order.
    #[must_use]
    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }
}

/// Fold a season in production order.
///
/// The input need not be sorted: episodes are processed by ascending
/// [`production_order`](Episode::production_order) (ties keep input order), which
/// is the listener's dimension. Within each episode, a scene's own anchor
/// overrides the episode anchor, so a flashback scene's facts and knowledge are
/// placed at the earlier story-time even though the listener meets them at the
/// episode's production slot.
///
/// The fold carries the kernel spine only — knowledge, fact-introductions, and
/// open-loop events. Craft objects (promises, beats, sliders) hang on these in a
/// later wave; no craft rule runs here.
#[must_use]
pub fn fold(episodes: &[Episode]) -> EpisodeFold {
    let mut order: Vec<usize> = (0..episodes.len()).collect();
    order.sort_by_key(|&i| episodes[i].production_order);

    let mut knowledge: Vec<KnowledgeRecord> = Vec::new();
    let mut placements: Vec<Placement> = Vec::new();
    let mut loops: Vec<LoopEvent> = Vec::new();

    for &i in &order {
        let episode = &episodes[i];
        let pos = episode.production_order;
        for scene in &episode.scenes {
            // A flashback scene carries its own anchor; otherwise the fact is so
            // when the episode is so.
            let anchor = scene.scene_anchor.as_ref().unwrap_or(&episode.story_anchor);
            for delta in &scene.knowledge_deltas {
                knowledge.push(KnowledgeRecord {
                    knower: delta.knower.clone(),
                    fact: delta.fact.clone(),
                    production_order: pos,
                    story_anchor: anchor.clone(),
                });
            }
            for delta in &scene.fact_deltas {
                placements.push(Placement {
                    fact: delta.fact.clone(),
                    anchor: anchor.clone(),
                    production_order: pos,
                });
            }
            for question in &scene.opens {
                loops.push(LoopEvent {
                    production_order: pos,
                    question: question.clone(),
                    opening: true,
                });
            }
            for question in &scene.closes {
                loops.push(LoopEvent {
                    production_order: pos,
                    question: question.clone(),
                    opening: false,
                });
            }
        }
    }

    EpisodeFold {
        knowledge,
        placements,
        loops,
    }
}
