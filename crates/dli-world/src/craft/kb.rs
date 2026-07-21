//! The craft knowledge base as `principle` declarations (`CRAFT-3`).
//!
//! The consuming repo's 79-entry knowledge base
//! ([06-craft-and-payloads](../../../../docs/spec/06-craft-and-payloads.md) §5)
//! ports here as data — [`PRINCIPLES`], a static table of [`Principle`] with every
//! legacy field intact (`{id, family, statement, source, mission_filter,
//! tension_note, resolution, phases, checkable, check_ref}`). The port is faithful
//! to the *data*, not the doc's round number: the pinned corpus carries **107**
//! principles, not 79, and the table is the source of truth.
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

use std::collections::BTreeSet;

use dli_core::grade::Grade;
use schemars::JsonSchema;
use serde::Serialize;

use crate::attest::{Attestation, Question, QuestionKey};
use crate::check::WorldFinding;

/// A doctrine family — the axis a principle speaks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "mission_filter", rename_all = "kebab-case")]
pub enum MissionFilter {
    /// The principle applies to the mission unchanged.
    Pass,
    /// The principle applies with a caveat; the caveat is required.
    Adapted {
        /// Why the generic principle needs adapting to the mission.
        note: &'static str,
    },
    /// A generic principle conflicts with a pillar or with canon; the resolution
    /// (the mission-side ruling) is required (spec §5).
    Tension {
        /// The conflict between the received principle and the mission.
        note: &'static str,
        /// The mission-side ruling that resolves it — where the pillar wins.
        resolution: &'static str,
    },
}

impl MissionFilter {
    /// The tension/adaptation note, when the filter carries one.
    #[must_use]
    pub fn note(&self) -> Option<&'static str> {
        match self {
            Self::Pass => None,
            Self::Adapted { note } | Self::Tension { note, .. } => Some(note),
        }
    }

    /// The mission-side resolution, present exactly when the filter is a tension.
    #[must_use]
    pub fn resolution(&self) -> Option<&'static str> {
        match self {
            Self::Tension { resolution, .. } => Some(resolution),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "binding", rename_all = "kebab-case")]
pub enum Binding {
    /// A deterministic kernel rule (the reference cannot dangle — closed enum).
    Rule(KernelRule),
    /// A deterministic check that stays factory-side (spec §6), recorded not run.
    Factory {
        /// Which factory battery owns it.
        namespace: FactoryNs,
        /// The check symbol within that battery.
        symbol: &'static str,
    },
    /// A judge dimension: consumes a stored attestation of this question kind and
    /// reports grade [`Grade::Attested`].
    Attest(&'static str),
    /// Declaration-only: no check, surfaced by brief/phase queries.
    Declaration,
}

impl Binding {
    /// The legacy checkability trichotomy this binding realizes.
    #[must_use]
    pub fn checkable(self) -> Checkable {
        match self {
            Self::Rule(_) | Self::Factory { .. } => Checkable::Deterministic,
            Self::Attest(_) => Checkable::Judge,
            Self::Declaration => Checkable::None,
        }
    }

    /// Whether this binding is standing checkable-debt — `judge` or `none` (spec
    /// §5: the roadmap of rules not yet written).
    #[must_use]
    pub fn is_debt(self) -> bool {
        matches!(self, Self::Attest(_) | Self::Declaration)
    }

    /// The kernel rule this binding names, if it is a kernel binding.
    #[must_use]
    pub fn kernel_rule(self) -> Option<KernelRule> {
        match self {
            Self::Rule(rule) => Some(rule),
            _ => None,
        }
    }

    /// The judge question kind this binding consumes, if it is a judge binding.
    #[must_use]
    pub fn attest_kind(self) -> Option<&'static str> {
        match self {
            Self::Attest(kind) => Some(kind),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Principle {
    /// The unique kebab-case slug.
    pub id: &'static str,
    /// The doctrine family.
    pub family: Family,
    /// The principle, stated plainly.
    pub statement: &'static str,
    /// The citation(s) — every entry is sourced.
    pub source: &'static str,
    /// How the mission filters it (carrying its note/resolution).
    pub mission_filter: MissionFilter,
    /// The production phases it bears on.
    pub phases: &'static [Phase],
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
    pub fn tension_note(&self) -> Option<&'static str> {
        self.mission_filter.note()
    }

    /// The mission-side resolution, present exactly for a tension.
    #[must_use]
    pub fn resolution(&self) -> Option<&'static str> {
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

/// The ported knowledge base: every principle, in corpus order.
pub static PRINCIPLES: &[Principle] = &[
    Principle {
        id: "promise-progress-payoff",
        family: Family::Plot,
        statement: "The opening makes promises about what the story will deliver; the middle shows visible progress toward them; the ending pays them off. A reader's sense of satisfaction is the felt distance between a promise made and a promise kept.",
        source: "Sanderson, BYU Creative Writing Lectures (plot); Writing Excuses.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_ppp"),
    },
    Principle {
        id: "promise-is-a-contract",
        family: Family::Plot,
        statement: "The type of promise sets the genre contract: a promise of a mystery owes a solution, a promise of a relationship owes a resolution, a promise of a wound owes a healing. Track each promise so none is left dangling by accident.",
        source: "Sanderson, BYU Creative Writing Lectures; Kowal (MICE thread openings).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Revision],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "arc-promise-ledger",
        family: Family::Plot,
        statement: "Arc and packet review need a promise ledger that crosses levels: where the promise opens, where it pays off, and whether it is deliberately carried beyond the episode. Without that ledger, \"it feels promising\" is only taste language.",
        source: "Sanderson, BYU Creative Writing Lectures (promises/progress/payoff); Kowal (MICE accounting).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Revision],
        binding: Binding::Rule(KernelRule::CheckSeason),
    },
    Principle {
        id: "core-vs-umbrella-plot",
        family: Family::Plot,
        statement: "The umbrella plot is the visible situation the story hangs on; the core plot is the character arc and the relationship question that actually keeps a listener engaged. The umbrella can be quiet so long as the core burns.",
        source: "Sanderson, BYU Creative Writing Lectures (umbrella/core plot).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "big-p-little-p",
        family: Family::Plot,
        statement: "Big-P Plot answers 'what is this about' across the whole run; little-p plot is the beat-by-beat progression inside a scene. Every little-p beat should feed a Big-P promise so no scene is merely local color.",
        source: "Sanderson, BYU Creative Writing Lectures (Big-P vs little-p).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "causality-therefore-but",
        family: Family::Plot,
        statement: "Beats should connect with 'therefore' and 'but', never 'and then'. If two adjacent beats can be reordered without breaking the story, the causal spine is missing and the scene is a list.",
        source: "Stone & Parker's 'therefore/but' rule; McKee, Story (progressive causality), filtered.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_ppp"),
    },
    Principle {
        id: "tryfail-yes-but-no-and",
        family: Family::Plot,
        statement: "A try/fail cycle answers the character's attempt with 'yes-but' (they succeed and a new problem opens) or 'no-and' (they fail and it worsens). These cycles are the engine of the middle; 'yes-and' only ends a thread.",
        source: "Card/Kowal try-fail cycles; Writing Excuses.",
        mission_filter: MissionFilter::Adapted {
            note: "The received model assumes the obstacle is external and often physical (a fight, a chase). Our climaxes are intellectual, restorative, or defensive (allegory constraint 2; D-0052 no Freytag).",
        },
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "yes-and-is-terminal",
        family: Family::Plot,
        statement: "A 'yes-and' outcome (clean success that also opens good fortune) resolves a thread and should appear only at a thread's close. A 'yes-and' in the middle deflates tension by handing the character an unearned win.",
        source: "Card/Kowal try-fail cycles.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Revision],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "tryfail-count-three",
        family: Family::Plot,
        statement: "Roughly three attempts before a turn reads as earned: too few feels easy, too many feels padded. The count is a heuristic, not a rule; the test is whether each attempt teaches the character something the next one uses.",
        source: "Folk-tale rule of three; Writing Excuses (try-fail pacing).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "mice-quotient",
        family: Family::Plot,
        statement: "Every thread is one of four kinds: Milieu (a place entered and left), Inquiry (a question asked and answered), Character (an identity strained and resettled), or Event (an order disrupted and restored). Name each thread's kind so you know what it owes.",
        source: "Card, How to Write SF&F (1990); Kowal, MICE Quotient.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "mice-lifo-nesting",
        family: Family::Plot,
        statement: "Threads nest like code: the first opened is the last closed (LIFO). A thread opened inside another must close before its parent, or the structure reads as a dropped stitch and the ending feels wrong without the reader knowing why.",
        source: "Kowal, MICE Quotient nesting codes (HTML-tag analogy).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Revision],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "mice-milieu-as-hub",
        family: Family::Plot,
        statement: "A Milieu thread opens on entering a place and closes on leaving it. The hub-and- spoke structure is a chain of Milieu threads: the party arrives, works the region, and migrates.",
        source: "Card/Kowal (Milieu); canon/narrative/structure.md (hub-and-spoke, D-0055).",
        mission_filter: MissionFilter::Adapted {
            note: "The textbook Milieu thread closes by exiting back to where the traveller started (there-and-back-again). Our structure is one-directional migration through a root system, not a round trip.",
        },
        phases: &[Phase::Outline],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "mice-event-new-equilibrium",
        family: Family::Plot,
        statement: "An Event thread opens when an order is disrupted and closes when order returns. The satisfaction is the world knitted back together.",
        source: "Card/Kowal (Event thread).",
        mission_filter: MissionFilter::Adapted {
            note: "Ruled no real conflict (D-0152): \"we're kinda following that advice... it's just cycles within cycles.\" The Event thread's order-returns satisfaction holds — the order that returns is a new cycle's equilibrium, never the old extractive order turned back (D-0014, D-0022).",
        },
        phases: &[Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "scene-goal-conflict-outcome",
        family: Family::Plot,
        statement: "A scene of action runs goal, conflict, outcome: the viewpoint character wants something, meets resistance, and leaves in a changed position. In packet review, ledger the goal/conflict/disaster chain explicitly so a quiet scene still has mechanical movement rather than mood alone.",
        source: "Swain, Techniques of the Selling Writer (scene/sequel); Bickham.",
        mission_filter: MissionFilter::Adapted {
            note: "Swain's 'outcome' is usually a disaster that escalates external stakes, which pulls toward Freytag rising action (D-0052 rejected).",
        },
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "sequel-reaction-dilemma-decision",
        family: Family::Plot,
        statement: "Between scenes of action sit sequels: the character reacts, faces a dilemma, and decides. Sequels are where a meditative story does its real work, and they have a shape, so a quiet scene is never shapeless.",
        source: "Swain, Techniques of the Selling Writer (sequel structure).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "foreshadow-and-payoff",
        family: Family::Plot,
        statement: "A payoff lands only if the ground was seeded earlier. Plant the detail plainly and let it lie inert until the turn recolors it. In a fractal kishōtenketsu the cold- open koan is often the seed the episode's ten cashes in.",
        source: "Chekhov's gun; Sanderson (setup/payoff); canon/narrative/narrator-framing.md.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "kishotenketsu-ten-is-the-turn",
        family: Family::Plot,
        statement: "In ki-shō-ten-ketsu the third act (ten) is the crux: a turn that recontextualizes the first two acts so the listener re-reads everything just heard. The turn need not be a conflict; it is a shift in what the material means.",
        source: "Kishōtenketsu (classical East Asian four-act structure); canon/narrative/structure.md (D-0044, D-0055).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "climax-is-perspective-shift",
        family: Family::Plot,
        statement: "The high point of an arc is the moment the trouble is revealed to be other than it seemed, and its cause is shown to be human and systemic. The resolution is a proof spoken and a community resettled.",
        source: "canon/narrative/structure.md (episode-arc mechanics, D-0072); kishōtenketsu.",
        mission_filter: MissionFilter::Tension {
            note: "Shōnen and three-act habits make the climax a battle or a power-spike against a villain (D-0010 no demon king, D-0052 no Freytag).",
            resolution: "The climax is the ten: a perspective shift dissolved through empathy, pedagogy, and rigorous logic. Where the draft reaches for a confrontation to peak the arc, replace it with the click of understanding that reframes the whole.",
        },
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "subplot-is-the-relationship-question",
        family: Family::Plot,
        statement: "The most reliable engagement engine is a relationship question running under the umbrella plot: will these two trust each other, will the found family hold. It can carry episodes where the external situation is calm.",
        source: "Sanderson, BYU Creative Writing Lectures (the will-they/won't-they as core).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "three-sliders",
        family: Family::Character,
        statement: "A character's hold on the audience runs on three dials: proactivity (do they pursue a goal), competence (are they good at something), and sympathy (do we like or understand them). A character high on even one dial engages; high on none goes flat.",
        source: "Sanderson, BYU Creative Writing Lectures (the three sliders).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_sliders"),
    },
    Principle {
        id: "slider-ledger-per-major-character",
        family: Family::Character,
        statement: "For outline and packet review, track every major character's proactivity, competence, and sympathy as explicit numbers with compensation notes. The point is not arithmetic precision; it is forcing the reviewer to name which dial carries the audience's investment.",
        source: "Sanderson, BYU Creative Writing Lectures (three sliders and compensation).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Revision],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "slider-compensation",
        family: Family::Character,
        statement: "A deficit on one dial is bought back by a surplus on another: an unlikeable character reads if hyper-competent and driven (the Sherlock trick). When a character feels wrong, the fix is usually to raise a different dial, not to soften the flaw.",
        source: "Sanderson, BYU Creative Writing Lectures (compensation).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_sliders"),
    },
    Principle {
        id: "proactivity-as-investigation",
        family: Family::Character,
        statement: "Proactive characters pursue a goal and take action toward it, which is what makes an audience invest in the journey.",
        source: "Sanderson, BYU Creative Writing Lectures (proactivity).",
        mission_filter: MissionFilter::Adapted {
            note: "The default image of proactivity is a hero who drives the plot by force and confrontation, which pulls toward conquest (D-0052) and decapitation strikes (allegory constraint 1).",
        },
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Attest("judge_sliders"),
    },
    Principle {
        id: "competence-reads-cold",
        family: Family::Character,
        statement: "Competence engages an audience through a kind of wish-fulfillment: we like watching someone who is very good at a thing.",
        source: "Sanderson, BYU Creative Writing Lectures (competence).",
        mission_filter: MissionFilter::Tension {
            note: "Sanderson frames competence as wish-fulfillment, which invites the power-fantasy flex — the hero showing off. That collides with aesthetic sobriety, where power reads cold, clinical, and intellectual (D-0025, D-0065).",
            resolution: "Competence is a compelling engine and may be savored on-mic (D-0152) — but in Enfys it is never showy, arrogant, or noisy: quiet, hyper-focused labor, the steady breath under strain, the exact accounting. The constraint is hers, not a house ban on display; other characters may carry their skill louder. The scariest sound in the show is still a very calm voice under enormous strain.",
        },
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_sliders"),
    },
    Principle {
        id: "sympathy-through-jeopardy-and-care",
        family: Family::Character,
        statement: "Sympathy grows when a character is placed in undeserved difficulty, shows care for someone else, or is good at something we respect. It is earned in the first minutes and spent across the arc.",
        source: "Sanderson, BYU Creative Writing Lectures (likeability/sympathy).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_sliders"),
    },
    Principle {
        id: "want-versus-need",
        family: Family::Character,
        statement: "A character consciously wants one thing and unconsciously needs another; the arc closes the gap. The want drives the plot's surface, the need drives the ten.",
        source: "Truby, The Anatomy of Story (desire vs need), filtered through kishōtenketsu.",
        mission_filter: MissionFilter::Adapted {
            note: "Truby frames the closing of the gap as a moral self-revelation at a Western third-act crisis, tied to escalating conflict.",
        },
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "contrasting-temperament-cast",
        family: Family::Character,
        statement: "An ensemble reads clearest when built by contrasting temperament: sense against sensibility against dry irony. Contrast lets a listener tell characters apart by how they meet the same trouble.",
        source: "Austen/Brontë harvest, canon/premise/influences.md (D-0073).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::LoreDecision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "agency-never-victimhood",
        family: Family::Character,
        statement: "A character in a cruel world is never staged as a victim for spectacle. Even a child in an adult danger stays an agent: exact, stubborn, running the numbers, and probing the rules so as not to be cheated.",
        source: "Portis, True Grit (Mattie Ross); canon (apprentice, D-0066); D-0076 no trauma-drama.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "antagonism-is-systemic",
        family: Family::Character,
        statement: "The opposing force is a system, not a person. There is no single villain whose defeat ends the trouble; the pressure comes from quotas, ledgers, and tired functionaries doing their jobs.",
        source: "canon/decisions.md (D-0010 no demon king); allegory constraint 1 (D-0087).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::LoreDecision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "anti-purity-characters",
        family: Family::Character,
        statement: "The oppressed are not required to be morally pure to deserve rescue. They inform, betray, and scab to survive, and the story saves them anyway. Empathy is a baseline right, not a reward for good behavior.",
        source: "canon/premise/allegory.md (anti-purity politics, D-0087); D-0033/D-0034.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "strangeness-is-a-transaction",
        family: Family::Character,
        statement: "A character is weird only if others react on-mic: a plain question, a challenge, a silence with cost. Unreacted strangeness is set-dressing.",
        source: "D-0163 ('others react' — the reaction economy); RQ-020 §4.3 (D-0166).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("indirection_discipline"),
    },
    Principle {
        id: "the-multitude-is-distinct",
        family: Family::Character,
        statement: "The minds a vessel carries are individually clear, each in its own regional silhouette, mostly plainer than the blended surface that holds them. Strangeness is the blend, never fog.",
        source: "D-0165 (distinct-not-muddled); RQ-020 §1.2/§4.3 (D-0166).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("register_fidelity"),
    },
    Principle {
        id: "first-law-magic",
        family: Family::World,
        statement: "A reader's satisfaction from magic solving a problem is proportional to how well they understand its rules. Magic used to resolve a plot must first be shown and bounded, or the resolution reads as a cheat.",
        source: "Sanderson's First Law of Magic (2007 essay).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "second-law-limitations",
        family: Family::World,
        statement: "Limitations matter more than powers: what magic cannot do, and what it costs, generates more story than what it can do. Weaknesses are where drama lives.",
        source: "Sanderson's Second Law of Magic (2011 essay).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "magic-costs-the-body",
        family: Family::World,
        statement: "Magic always costs the body: calories, blood, neurological bandwidth. Proof-speech sounds like work — controlled breath starting to fray, a nosebleed on slate, an anchor's grip keeping a swaying body upright. There is no costless hand-waving.",
        source: "canon/premise/allegory.md (constraint 3, D-0087); canon/narrative/audio-design.md; Sanderson Second Law.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "third-law-expand-before-add",
        family: Family::World,
        statement: "Expand what you have before you add something new. A deeper use of an established rule beats a fresh power bolted on; sprawl of systems dilutes the ones the listener has learned to trust.",
        source: "Sanderson's Third Law of Magic (2013 essay).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "zeroth-law-err-awesome",
        family: Family::World,
        statement: "Err on the side of what is compelling: begin from an idea that grips you and let the rules bend to serve it, since the systems exist only to make a good story.",
        source: "Sanderson's Zeroth Law ('err on the side of awesome').",
        mission_filter: MissionFilter::Tension {
            note: "'Awesome' is easily read as spectacle — the bigger blast, the cooler feat. That collides head-on with aesthetic sobriety (D-0128, D-0025): power reads cold, and the show refuses the anime power-spike.",
            resolution: "The awe we chase is wonder-amid-ruin (D-0025): the green shoot in poisoned mud, the vast melancholic anatomy of time, a hard-won eucatastrophe. Reach for the breathtaking through beauty and dread, never through spectacle or a bigger fight.",
        },
        phases: &[Phase::LoreDecision, Phase::Generation],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "iceberg-worldbuilding",
        family: Family::World,
        statement: "Show a fraction of what you have built and imply the rest. The felt depth of a world comes from the sense that the visible detail rests on a much larger unseen mass. Withheld lore is load-bearing.",
        source: "Sanderson (iceberg principle); Hemingway (omission).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "world-as-plot-engine",
        family: Family::World,
        statement: "Build the world modularly so it generates arcs on demand: each region, faction, and rule is a socket a new spoke can plug into. A world designed this way answers 'what happens next' from its own structure.",
        source: "Wildbow (modular worldbuilding); Sanderson; canon/narrative/structure.md (hub-and-spoke).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "consistency-enables-solutions",
        family: Family::World,
        statement: "A consistent, pre-established rule lets the audience predict and follow a clever resolution, so the payoff feels fair. Retconning a rule to solve a problem breaks the contract even when nobody can name the rule aloud.",
        source: "Sanderson's First Law (corollary); canon continuity discipline.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "material-reality-first",
        family: Family::World,
        statement: "Identity and culture are forged by geography, trauma, and economic class, never by modern identity shorthand pasted over a world that ignores material reality. Characters navigate a cruel supply chain in thick cloaks, covered in toxic mud.",
        source: "canon/narrative/craft-pillars.md (material sovereignty, D-0128); D-0056, D-0058.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "diction-geography",
        family: Family::World,
        statement: "Speech mirrors the naming map: distinct syntax preferences and diction strata per region, occasional attested loan words as texture, never an invented language. Region is audible in shape before any accent is cast.",
        source: "D-0165 (diction geography); RQ-020 §1.0/§4.3 (D-0166); style/data/diction_map.yaml.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Style,
            symbol: "check_conlang_guard",
        },
    },
    Principle {
        id: "windowpane-vs-stained-glass",
        family: Family::Prose,
        statement: "Prose runs a spectrum from windowpane (invisible, the story seen straight through) to stained glass (the language itself part of the experience). Default to windowpane so the story leads.",
        source: "Orwell ('good prose is a windowpane'); Sanderson (windowpane/stained-glass); Writing Excuses.",
        mission_filter: MissionFilter::Tension {
            note: "A flat windowpane default would erase the elevated registers canon requires: the narrator's Le Guin/Clarke mythic dryness and the protagonist's Ishiguro/Wolfe/ McKillip elevation at proof-moments (D-0083, D-0122).",
            resolution: "Transparency is register-scoped, with the split ruled in D-0152: the narrator's QUOTES (koan cold-opens, outros) are one deliberate tone; MOST NARRATION is windowpane; at moments the narrator's register is allowed to reveal future knowledge. Dialogue-common and the apprentice run near windowpane. How Enfys performs the proofs is an open register question (Q-0018) — it needs its own register, maybe its own language; do not default it to either pole.",
        },
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "pyramid-of-abstraction",
        family: Family::Prose,
        statement: "Concrete beats abstract. Where a draft names an emotion or a concept, a specific sensory image usually carries it further. Climb to abstraction only for earned effect, then come back down to the ground.",
        source: "Sanderson (pyramid of abstraction); Le Guin (the concrete).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_abstraction_density",
        },
    },
    Principle {
        id: "concrete-detail-doctrine",
        family: Family::Prose,
        statement: "One exact, distilled detail outperforms a pile of generic ones. 'Salt-swollen fingers on a cord that will not seat' beats 'she felt anxious'. Strategic specificity is the difference between a scene heard and a scene skimmed.",
        source: "Le Guin, Steering the Craft; canon/narrative/structure.md (the opening, D-0113).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_abstraction_density",
        },
    },
    Principle {
        id: "opening-sensory-seasonal-grounding",
        family: Family::Prose,
        statement: "Opening action and dialogue must happen somewhere the audience can picture. Establish a legible physical place, at least one heard or smelled detail beyond the mere fact of speech, and the weather or season; choose details that act on the scene rather than inventorying a room.",
        source: "D-0217; canon/narrative/craft-pillars.md#Sensory and seasonal grounding; Le Guin, Steering the Craft (the concrete).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_opening_sensory_grounding",
        },
    },
    Principle {
        id: "said-asked-eighty-five",
        family: Family::Prose,
        statement: "Keep dialogue attribution near-invisible: roughly 85 percent of tags should be plain 'said' or 'asked'. The eye slides over 'said'; an ornate tag (murmured, hissed, breathed) is a small interruption, and in a voiced drama it dictates the actor's delivery.",
        source: "Sanderson/Writing Excuses (said-tag discipline); Leonard's 10 Rules.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_said_tag_ratio",
        },
    },
    Principle {
        id: "no-adverb-tags",
        family: Family::Prose,
        statement: "Prefer a plain tag plus a beat of action over an adverb-loaded tag. 'She said, turning the cord over' carries more than 'she said nervously'. The adverb tells what the line and the action should show.",
        source: "Leonard's 10 Rules of Writing; King, On Writing (adverbs).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_said_tag_ratio",
        },
    },
    Principle {
        id: "show-versus-tell-is-pacing",
        family: Family::Prose,
        statement: "Show-versus-tell is a pacing lever, not a commandment. Dramatize the moments that matter and summarize the connective tissue; a story that shows everything crawls, and a story that tells everything goes cold.",
        source: "Sanderson (show-vs-tell as pacing); Writing Excuses.",
        mission_filter: MissionFilter::Adapted {
            note: "The blanket 'show, don't tell' rule would forbid the omniscient narrator's mythic summary, which canon requires (Le Guin/Clarke, D-0083; the narrator frame, D-0035).",
        },
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "sentence-rhythm-and-sound",
        family: Family::Prose,
        statement: "The sound of the language is where it begins; the test of a sentence is whether it sounds right aloud. Pace and movement depend on rhythm, and rhythm is controlled by hearing the prose, so vary sentence length to make it move.",
        source: "Le Guin, Steering the Craft (sound and rhythm).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_sentence_rhythm",
        },
    },
    Principle {
        id: "read-everything-aloud",
        family: Family::Prose,
        statement: "Read every line aloud before it ships. The ear catches what the eye forgives: a clotted clause, a tongue-twister, a rhythm that stumbles. In an audio-first show the read-aloud pass is the primary proofing pass, not an optional one.",
        source: "Le Guin, Steering the Craft; Gill (SFWA, revise by reading dialog aloud); Wayland.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "no-filter-words",
        family: Family::Prose,
        statement: "Cut filter verbs (she saw, he felt, they heard) that put a pane of glass between the listener and the sense. Render the thing itself: not 'she heard the wind rise' but the wind rising.",
        source: "Le Guin, Steering the Craft (crowding/leaping); Palmer (filter words).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "subtext-over-announcement",
        family: Family::Prose,
        statement: "People talk about rations, weather, and repairs and mean love, fear, and defiance. Nobody narrates their own feelings or monologues a backstory. The meaning lives under the line.",
        source: "canon/narrative/tone-style.md (D-0082, D-0083); Gilroy (Andor), Mantel (Wolf Hall).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "distilled-imagery-not-purple",
        family: Family::Prose,
        statement: "Earn every metaphor; genre-apt, specific imagery beats a purple-but-empty flourish. A stacked pile of figuration reads as decoration, and in audio the listener cannot re-scan it, so the image must land the first time.",
        source: "Le Guin, Steering the Craft; style-defense figurative-cliché research.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "episode-is-complete-kishotenketsu",
        family: Family::Serial,
        statement: "Each installment carries its own complete shape so a listener is satisfied by the single episode before trusting the season. The self-contained shape is how serial trust is earned first.",
        source: "Platt & Truant, Write. Publish. Repeat.; canon/narrative/episode-format.md (D-0043, D-0044).",
        mission_filter: MissionFilter::Adapted {
            note: "Serial-craft sources assume a self-contained three-act episode inside a season arc. Our episode shape is kishōtenketsu, not three-act (D-0044, D-0052).",
        },
        phases: &[Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "open-loop-discipline",
        family: Family::Serial,
        statement: "Every installment opens at least one new psychological loop — a question, an unease, a thing withheld — that pulls the listener forward. A loop is not always a cliffhanger; it is any tension the mind wants closed.",
        source: "Wildbow, Thoughts on Writing Serials; Platt & Truant (open loops); Eyal (hooks).",
        mission_filter: MissionFilter::Adapted {
            note: "The reflex reading of an open loop is a peril cliffhanger, which collides with the unhurried register and with D-0104 (no ticking clock).",
        },
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "micro-pacing-every-installment",
        family: Family::Serial,
        statement: "Every installment must do at least one of three things: advance the plot, change a relationship, or reveal a secret. An installment that does none is filler, however pretty.",
        source: "Fox, Write to Market; Platt & Truant, Write. Publish. Repeat.",
        mission_filter: MissionFilter::Tension {
            note: "Fox's rule is often read as 'advance the plot every chapter', which pushes toward constant forward motion and against the courage to hold a quiet scene (Frieren; audio-design 'courage to hold silence').",
            resolution: "A quiet scene of weather or mending still qualifies if it changes a relationship or an understanding. The test is motion of feeling or knowledge, not noise. Whole episodes of weather and teaching are features, not filler (D-0051). The counting rule is ruled explicit (D-0152): there is always movement, and pure relationship-deepening beats, richness of understanding, and emotional growth COUNT as movement.",
        },
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "promise-payoff-new-promise",
        family: Family::Serial,
        statement: "The serial rhythm is a chain: each installment pays off an earlier promise and makes a fresh one, so the listener always carries an open expectation into the next drop. Close a loop, open a loop.",
        source: "Platt & Truant, Write. Publish. Repeat.; Sanderson (promise/payoff at serial scale).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "escalate-depth-not-pace",
        family: Family::Serial,
        statement: "Long serials keep readers by escalation of intensity and stakes across the run.",
        source: "Wildbow, Worm/Pact (escalation as autopilot); serial-fiction craft.",
        mission_filter: MissionFilter::Tension {
            note: "Wildbow's escalation-is-everything is the sharpest conflict with our meditative register and with D-0104 (no ticking clock). Constant escalation of pace and peril exhausts the very stillness the show is built on — a lesson Wildbow himself drew from Pact.",
            resolution: "Escalate depth and the stakes of understanding, not pace. What deepens across the run is the archaeology of the world, the cost of what the protagonist becomes, and the moral weight of each rescue. The tempo stays unhurried; the meaning compounds.",
        },
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "reader-breathing-room",
        family: Family::Serial,
        statement: "Give characters calm scenes to breathe between intensities. A serial that never lets up exhausts its audience and flattens its own peaks; the quiet scenes are what make the loud ones register.",
        source: "Wildbow, Thoughts on Writing Serials (the lesson of Pact's over-escalation).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "first-three-episodes-compression",
        family: Family::Serial,
        statement: "The opening installments carry the heaviest load: establish voice, promise, and the world's rules fast enough that a listener commits. Front-load the hooks and the distinct textures; a slow first drop loses the audience before the shape can pay off.",
        source: "Fox, Launch to Market; Platt & Truant; streaming pilot craft.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "multiple-pov-loop-management",
        family: Family::Serial,
        statement: "With more than one viewpoint, track a loop ledger per thread so no strand goes cold. Cutting away from a POV should leave its loop open; returning should advance it, or the listener stops caring about the neglected thread.",
        source: "Wildbow (multi-arc interludes); GRRM-style POV rotation, filtered.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "modular-world-generates-arcs",
        family: Family::Serial,
        statement: "A serial sustains hundreds of installments only if the world can spin up a fresh arc from its own parts. Each new region and enclosed hoard is a ready spoke; the structure supplies stories without the author inventing an escalating threat.",
        source: "Wildbow (world-as-engine); canon/narrative/structure.md (hub-and-spoke, D-0051).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::LoreDecision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "cold-open-is-a-tone-promise",
        family: Family::Serial,
        statement: "The cold open is a promise of tone: a koan-like microcosm of the episode, read at a centuries-out remove, whose meaning clicks into place only as the episode plays. It sets the contract the installment must honor.",
        source: "canon/narrative/narrator-framing.md (D-0037, D-0045); Dune epigraphs.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "sfx-as-character",
        family: Family::Audio,
        statement: "Sound effects are atmosphere and orientation, not a hidden visual channel. Script the sonic reality so the mix carries place, pressure, and punctuation, but do not ask it to teach subtle object mechanics the ear cannot decode.",
        source: "Wayland, Bombs Always Beep (theater for the mind); canon/narrative/audio-design.md; Episode 1 craft author feedback (2026-07-10).",
        mission_filter: MissionFilter::Adapted {
            note: "Audio-drama craft often treats SFX as a full scene partner. The current project mode is closer to audiobook-with-cast/foley, and many generated effects will be approximate rather than image-painting.",
        },
        phases: &[Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "audio-show-dont-tell",
        family: Family::Audio,
        statement: "Let sound carry obvious heard facts: the door opens, rain hits shutters, footsteps cross a room. Do not use sound as the primary carrier for rope, knot, card, proof, or causal information; the narrator or dialogue must state what changed and why it matters.",
        source: "Gill (SFWA, listeners fill the gap); Wayland; Urbina (Wolf 359).",
        mission_filter: MissionFilter::Adapted {
            note: "The generic rule is correct for simple audible facts but becomes dangerous when a scene depends on quiet rope, knot, card, or proof mechanics.",
        },
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "narrated-sound-to-sfx",
        family: Family::Audio,
        statement: "If the narrator describes an event whose point is that it is heard -- latch, rain, wind, cord, footsteps, ash, breath -- move it into SFX/foley/music unless the line is carrying causality or object meaning that sound alone cannot carry.",
        source: "Episode 1 script-format author feedback (2026-07-10); audio-show-dont-tell doctrine.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_narrated_audio_events",
        },
    },
    Principle {
        id: "sfx-cannot-carry-mechanics",
        family: Family::Audio,
        statement: "Rope, knot, card, ward, proof, text, recognition, private knowledge, and emotional interpretation are not legible as sound. A cue can add rasp, rain, breath, or a tactile vibe, but narrator or dialogue must plainly carry the state change: what moved, what failed, what proved false, what was discovered, and why it matters. Over-detailed or impossible SFX prompts are hard failures in audio scripts.",
        source: "Episode 1 craft author feedback (2026-07-10); magic-pedagogy SFX recovery rules.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_sfx_semantic_load",
        },
    },
    Principle {
        id: "no-em-dashes-in-scripts",
        family: Family::Prose,
        statement: "Generated scripts use no em dashes at all. The author has fully banned them; use sentence breaks, commas, semicolons, colons, or plain hyphens where rhythm needs a hinge.",
        source: "Episode 1 craft author feedback (2026-07-10); D-0153 em-dash rejection.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_no_em_dashes",
        },
    },
    Principle {
        id: "sixty-second-hook",
        family: Family::Audio,
        statement: "Earn attention inside the first minute: the smallest moment of boredom or confusion can lose an audio listener, who has less to hold them than a screen does. The opening image must grip before the frame explains itself.",
        source: "Urbina, Advice for Radio Drama Beginners; Wayland (the hook).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Rule(KernelRule::CheckOutline),
    },
    Principle {
        id: "three-four-voices-per-scene",
        family: Family::Audio,
        statement: "Keep a scene to three or four speaking voices. Past that the ear loses track of who is who, especially when two performers sit near the same pitch. Split a crowded scene or let some characters stay silent presences.",
        source: "Toscan, Writing Audio Drama; Gill (SFWA, cast-size confusion); Urbina (four-hander).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_cast_per_scene",
        },
    },
    Principle {
        id: "vocal-signature-per-character",
        family: Family::Audio,
        statement: "Every character needs a distinct vocal signature — vocabulary, cadence, register — so a listener can tell them apart before a name is spoken. Homogenized voices are the top narrative tell and this show's central risk.",
        source: "Toscan; Wayland; Kowal (puppetry-derived voice work); style-defense (voice_separation).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_vocal_signature",
        },
    },
    Principle {
        id: "voice-silhouette-principle",
        family: Family::Audio,
        statement: "Every speaking party is distinguishable by ear within one line, from sentence shape, vocabulary stratum, rhythm, and address form alone, before actor timbre helps. Design voices as at-a-glance silhouettes.",
        source: "D-0163 (the silhouette commission, verbatim); RQ-020 §0/§4.3 (D-0166).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Attest("silhouette"),
    },
    Principle {
        id: "one-sage-per-scene",
        family: Family::Audio,
        statement: "At most one voice in an exchange may run oblique. Two voices trading figures reads as authorial manner, not character.",
        source: "D-0163 (the zen-textbook ban); RQ-020 §0/§4.3 (D-0166).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Style,
            symbol: "check_stichomythia",
        },
    },
    Principle {
        id: "viewpoint-asks-the-listeners-question",
        family: Family::Audio,
        statement: "The viewpoint character asks the plain question the listener would ask, with a real question mark, and never carries the scene's wisdom line.",
        source: "D-0163 (Aalto asks the listener's question); RQ-020 §1.4/§4.3 (D-0166).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Style,
            symbol: "check_unmarked_question",
        },
    },
    Principle {
        id: "strategic-name-pepper",
        family: Family::Audio,
        statement: "Salt the dialogue with names so the listener always knows who is in the room and who is speaking. Too little breeds who-is-talking confusion; too much reads as unnatural. Address by name at entrances, turns, and after any ambiguity.",
        source: "Toscan, Writing Audio Drama (signposting); radio-drama convention.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_name_pepper",
        },
    },
    Principle {
        id: "scripted-silence",
        family: Family::Audio,
        statement: "Silence is a scored instrument: a held beat, wind and fire-crackle, boots in mud. The courage to hold silence instead of wall-to-wall scoring is a signature of the register, and a pause held a beat long carries more than a line.",
        source: "canon/narrative/audio-design.md (courage to hold silence, D-0023/D-0024); Werner.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_silence_scripting",
        },
    },
    Principle {
        id: "performance-direction-markers",
        family: Family::Audio,
        statement: "Human review scripts may stay readable, but dialogue-heavy drafts should include sparse bracketed performance direction where it matters, e.g. ENFYS [tired]: before the line, so the artifact is already production-aware.",
        source: "Episode 1 script-format author feedback (2026-07-10); hybrid-format practice.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_performance_direction_markers",
        },
    },
    Principle {
        id: "four-beat-labels-earn-their-turns",
        family: Family::Audio,
        statement: "Visible Four-Beat Turn labels are not structure by themselves. The deepen line is the actual complication the listener hears, and the turn is the actual recontextualization; do not label a later weak beat as deepen or turn when the scene has already moved.",
        source: "Episode 1 craft author feedback (2026-07-10); kishōtenketsu ten-is-the-turn doctrine.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_four_beat_turn_integrity",
        },
    },
    Principle {
        id: "first-meeting-audibility",
        family: Family::Audio,
        statement: "When two core characters meet for the first time, the listener must hear and keep hearing that relationship state. A lexical stranger cue does not excuse established-pair habits, assumed shared plans, or instant-family behavior later in the episode.",
        source: "D-0216; campaign-033 false-pass audit; Toscan signposting.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_first_meeting_clarity",
        },
    },
    Principle {
        id: "earned-arc1-relationship-phase",
        family: Family::Character,
        statement: "Arc 1 relationship behavior progresses no faster than the earned evidence: Episode 1 is a stranger emergency alliance, Episode 2 a functional truce, and Episode 3 may reach earned respect but not friendship, partnership, or found family. Preserve friction, separate motives and knowledge, negotiated next steps, and a credible choice to part.",
        source: "D-0216; canon/narrative/craft-pillars.md#Early-arc-legibility.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_first_meeting_clarity",
        },
    },
    Principle {
        id: "early-bounded-magic-genre-promise",
        family: Family::World,
        statement: "Before extended explanation in an early episode, Enfys's proof or Turning causes one bounded physical impossibility whose cause and effect are plain by ear. At the first meeting, power-caused overwhelm must temporarily disable a named function; fatigue, blood, or mood alone cannot carry the causal fact. Keep the event costly and clinical, without glow or spectacle.",
        source: "D-0216; D-0142; D-0155; campaign-033 false-pass audit.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_early_magic_genre_promise",
        },
    },
    Principle {
        id: "named-lore-backed-cold-open",
        family: Family::Audio,
        statement: "Every cold-open quotation is two or three sentences followed immediately by a separate spoken named-source attribution. Its proper nouns resolve to a ratified or provisional canon section or an explicit naming decision, and the episode contract records that provenance; no draft may invent an author, archive, recension, or source-place relationship. Reject compressed near-borrowings such as map-versus-territory formulations.",
        source: "D-0216; D-0220; canon/texts/in-world-texts.md; naming-oracle discipline.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_opening_epigraph_discipline",
        },
    },
    Principle {
        id: "early-aalto-enfys-care-hooks",
        family: Family::Audio,
        statement: "Episode 1 must quickly buy investment in Aalto and Enfys. Aalto needs audible sympathy, competence, or proactivity before the lore density rises, including a concrete want or pressure; Enfys needs an immediate hook of competence, cost, care, or decisive action when she enters. The first shared scene must also make clear why these two being bound together starts a story.",
        source: "Episode 1 craft author feedback (2026-07-10); Sanderson character sliders; pilot-scene craft.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_early_character_hooks",
        },
    },
    Principle {
        id: "dialogue-scenes-have-turns",
        family: Family::Audio,
        statement: "Good drama dialogue is motivated action: a line asks, resists, reframes, withholds, or changes the power in the room. Long exchanges need questions, contrast, reversals, grounding beats, and subtext with enough surface clarity to keep pace.",
        source: "Episode 1 craft author feedback (2026-07-10); film/TV scene craft; McKee scene turns; Mamet practical dialogue doctrine.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_dialogue_turn_motivation",
        },
    },
    Principle {
        id: "closing-punctuation-not-archival-quote",
        family: Family::Audio,
        statement: "The opening may cite a quoted archival source. The ending defaults to two to four narrator sentences followed by final music: image, consequence, and next motion, not a long quote, audience address, stated moral, whole-episode recap, or abrupt one-line cutoff.",
        source: "D-0216; narrator-frame discipline; campaign-033 false-pass audit.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_closing_quote_discipline",
        },
    },
    Principle {
        id: "audio-opening-pressure",
        family: Family::Audio,
        statement: "A first episode may use the Four-Beat Turn, but the first story minute still needs a Western-audio hook: a concrete pressure already happening, then enough narrator context to understand it. Do not open with a room inventory or a child muttering procedures before the listener knows what can be lost.",
        source: "D-0195; Sanderson (promises in the opening); audio-first clarity doctrine.",
        mission_filter: MissionFilter::Adapted {
            note: "The project's default structure prizes set/deepen/turn/settle rather than constant escalation, but a cold audio listener still needs immediate orientation and pressure before patient pattern work can feel earned.",
        },
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_audio_opening_pressure",
        },
    },
    Principle {
        id: "specific-family-context",
        family: Family::Audio,
        statement: "If family danger motivates the scene, make the family context incidentally specific. Aalto knows who is in the bed, so the listener should hear a name or relation early; vague labels like 'the sick kinsman' turn human stakes into fog.",
        source: "D-0195; Sanderson (sympathy through concrete stakes); Toscan signposting.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_specific_family_context",
        },
    },
    Principle {
        id: "lone-opening-dialogue-context",
        family: Family::Audio,
        statement: "Do not make Aalto carry the opening by talking to herself. If she is alone, the narrator must ground what the listener cannot see; if another person is present, let that person become an audible anchor when useful.",
        source: "D-0195; audio-drama listener-orientation practice.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_lone_opening_dialogue_context",
        },
    },
    Principle {
        id: "performable-voice-strangeness",
        family: Family::Audio,
        statement: "Enfys may sound strange, but the script cannot depend on unproducible voice layering. Let the actor or TTS simply perform a line; put her otherness in word choice, behavior, cost, and reactions, not in directions for multiple hidden voices or impossible acoustic variations.",
        source: "D-0195; ElevenLabs validation learning from campaign-016.",
        mission_filter: MissionFilter::Adapted {
            note: "The project wants Enfys to remain liminal and a little inhuman, but generated voices and listeners will not reliably resolve subtle internal voice shifts.",
        },
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_performable_voice_strangeness",
        },
    },
    Principle {
        id: "narrator-never-describes-voices",
        family: Family::Audio,
        statement: "The narrator never describes what a person's voice sounds like or how speech is delivered. No vocal qualities, accents or phonetics, delivery adverbs, vocal metaphors, or how-said paraphrases belong in Narrator records. Character dialogue may mention voices; the narrator may state that speech happened, describe breath or bodily cost, and use sound metaphors for objects, animals, or weather.",
        source: "D-0218; canon/narrative/narrator-framing.md#No voice-description narration.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_narrator_voice_descriptions",
        },
    },
    Principle {
        id: "no-as-you-know-bob",
        family: Family::Audio,
        statement: "No character tells another something both already know for the audience's benefit (the maid-and-butler / as-you-know-Bob leak). It is the loudest exposition tell and it shatters the sincerity the register depends on.",
        source: "Wayland; Toscan; classic BBC radio-drama guidance (maid-and-butler).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_as_you_know_bob",
        },
    },
    Principle {
        id: "hybrid-script-formatting",
        family: Family::Audio,
        statement: "Use a script format that carries sound as first-class: speaker cues, sound cues, and beats legible to a director and a foley artist. Format serves the production process and the listener, nothing else.",
        source: "Wayland, Bombs Always Beep (hybrid format); canon/narrative/episode-format.md.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "clarity-is-life-or-death",
        family: Family::Audio,
        statement: "In audio, clarity is survival: give the audience exactly enough to fill the gaps the way you intend. A plot beat that depends on the listener seeing something will not land, so route every load-bearing turn through sound and speech.",
        source: "Urbina, Advice for Radio Drama Beginners; Gill (SFWA).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "plot-points-per-episode",
        family: Family::Audio,
        statement: "A useful density heuristic: roughly ten plot points per episode, where a plot point is any item you would list when recapping the installment. It is a texture gauge, not a rule — a way to feel whether an episode is thin or overstuffed.",
        source: "Urbina, Wolf 359 (ten plot points per episode).",
        mission_filter: MissionFilter::Adapted {
            note: "Ten discrete plot points can read as briskness, which pulls against the meditative pace and the whole-episode-of-weather license (D-0051).",
        },
        phases: &[Phase::Outline],
        binding: Binding::Declaration,
    },
    Principle {
        id: "aphorism-as-acoustic-logo",
        family: Family::Audio,
        statement: "Because the show is heard, belief systems cannot lean on a visible symbol; a creed's repeated aphorism, in a fixed cadence over a signature texture, is its acoustic logo. Give each faction a phrase the listener knows before a name is said.",
        source: "canon/narrative/audio-design.md (aphorisms as acoustic identity, D-0090).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "music-as-learned-lexicon",
        family: Family::Audio,
        statement: "Music works two ways: fixed signatures that punctuate the open and close, and a recognizable mood lexicon the listener learns (the rigid order, the wild Wyrd, the warm hearth). Consistency turns the score into orientation.",
        source: "canon/narrative/audio-design.md (music as punctuation and mood, D-0039).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "final-musical-sting",
        family: Family::Audio,
        statement: "A full episode script should mark its closing musical sting or coda explicitly. The final cue is part of the episode shape, not an afterthought for production to infer from prose.",
        source: "Episode 1 script-format author feedback (2026-07-10); canon/narrative/audio-design.md (D-0039).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_final_music_sting",
        },
    },
    Principle {
        id: "narrator-brackets-the-frame",
        family: Family::Audio,
        statement: "The narrator owns the cold open and the outro and intrudes between them only when audio genuinely needs it (passage of time, the unshowable). The present action belongs to the cast and the foley.",
        source: "canon/narrative/narrator-framing.md (mid-episode discipline, D-0036).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Generation],
        binding: Binding::Declaration,
    },
    Principle {
        id: "narrator-sfx-load-boundary",
        family: Family::Audio,
        statement: "Current audio mode is audiobook-with-cast/foley: a load-bearing windowpane narrator carries orientation and causality, while SFX stays sparse support. Packet reviews must not let silence and foley become the hidden semantic channel.",
        source: "canon/decisions.md (D-0179 medium correction); D-0176 narrator amendment.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Factory {
            namespace: FactoryNs::Check,
            symbol: "check_sfx_semantic_load",
        },
    },
    Principle {
        id: "pillar-aesthetic-sobriety",
        family: Family::Pillar,
        statement: "Against shouty bravado, for melancholic competence: grounded, quiet confidence; emotion carried by a pause held a beat long, a steady breath, the hyper-focused labor of a knot. Power reads cold, clinical, intellectual.",
        source: "canon/narrative/craft-pillars.md (aesthetic sobriety, D-0128); D-0025, D-0065.",
        mission_filter: MissionFilter::Pass,
        phases: &[
            Phase::Outline,
            Phase::LoreDecision,
            Phase::Generation,
            Phase::Revision,
        ],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "pillar-rhizomatic-restoration",
        family: Family::Pillar,
        statement: "Against Freytag's conquest, for the kishōtenketsu ecology: horizontal, exploratory, community-driven structure on the hub-and-spoke model. An arc's climax is a localized puzzle dissolved through empathy, pedagogy, and rigorous logic.",
        source: "canon/narrative/craft-pillars.md (rhizomatic restoration, D-0128); D-0010, D-0052, D-0055.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "pillar-material-sovereignty",
        family: Family::Pillar,
        statement: "Against the gaze and the shallow trope, for grounded demography: identity forged by geography, trauma, and economic class; gender handled with pragmatic, period-true specificity and absolute respect; characters defined by autonomy and intellect.",
        source: "canon/narrative/craft-pillars.md (material sovereignty, D-0128); D-0056, D-0219.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::LoreDecision, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "pillar-earnest-sincerity",
        family: Family::Pillar,
        statement: "Against ironic detachment, for true subcreation: the world is a terrifying, breathtaking, authentic reality; the allegory is a hidden skeleton and the flesh is pure hard-magic fantasy. No meta winks; human connection is treated with un-ironic sincerity.",
        source: "canon/narrative/craft-pillars.md (earnest sincerity, D-0128); D-0025, D-0030, D-0085.",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Attest("judge_pillars"),
    },
    Principle {
        id: "pillars-win-standing-rule",
        family: Family::Pillar,
        statement: "Where any outside craft authority conflicts with a pillar or with canon, the pillar wins. The conflict is recorded as a tension with an explicit mission-side resolution; generic advice is adapted to the mission, never allowed to override it.",
        source: "canon/narrative/craft-pillars.md (D-0128, the standing rule).",
        mission_filter: MissionFilter::Pass,
        phases: &[
            Phase::Outline,
            Phase::LoreDecision,
            Phase::Generation,
            Phase::Revision,
        ],
        binding: Binding::Declaration,
    },
    Principle {
        id: "wonder-amid-ruin",
        family: Family::Pillar,
        statement: "The emotional target is recovery, escape, and consolation: a hard-won eucatastrophe, the green shoot in poisoned mud, never cheap despair and never spectacle. Awe comes through beauty and dread inside a broken world.",
        source: "canon/premise/influences.md (Sanderson framing, D-0025); Tolkien (eucatastrophe).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Outline, Phase::Generation, Phase::Revision],
        binding: Binding::Declaration,
    },
    Principle {
        id: "bar-raiser-taste-gap",
        family: Family::Pillar,
        statement: "A draft that clears every gate can still fall short of its own ambition; the gap between taste and execution closes only by naming the highest-leverage opportunities and doing the work. Good-enough is the enemy of the bar.",
        source: "Glass, 'The Gap' (This American Life); the bar-raising mandate (D-0128).",
        mission_filter: MissionFilter::Pass,
        phases: &[Phase::Revision],
        binding: Binding::Attest("raise_draft"),
    },
];

/// A borrow-generic handle over a principle corpus (design §2.1, `CRAFT-3`).
///
/// The six knowledge-base queries are methods reading `self.principles`, so the
/// same query surface serves the built-in [`PRINCIPLES`] table today
/// ([`Kb::builtin`]) and a loaded corpus once packs are extracted. The borrow is
/// `'a`, so every query hands out `&'a Principle` references into the underlying
/// slice — independent of the `&self` borrow.
///
/// The free functions ([`by_id`], [`why`], …) remain as thin shims over
/// [`Kb::builtin`]; prefer the methods.
#[derive(Debug, Clone, Copy)]
pub struct Kb<'a> {
    principles: &'a [Principle],
}

impl Kb<'static> {
    /// The built-in knowledge base — the ported [`PRINCIPLES`] corpus.
    #[must_use]
    pub fn builtin() -> Self {
        Self {
            principles: PRINCIPLES,
        }
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
            if !seen.insert(p.id) {
                errors.push(KbError::DuplicateId(p.id.to_owned()));
            }
            if p.phases.is_empty() {
                errors.push(KbError::NoPhase(p.id.to_owned()));
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
