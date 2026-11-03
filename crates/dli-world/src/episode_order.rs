//! Episode-order adapter over the fold (`TIME-3`): the generic order/slack
//! engine lives in dli-time; this file adapts the episode fold to it.
//!
//! Production order is the listener's dimension (`docs/spec/03-time.md` §4–§5).
//! The ordering constraints — the full-team floor, no-deep-time-in-the-opening-act,
//! payoff-after-reveal — are checked over the season *in production order*, by the
//! generic linear-scan engine in [`dli_time::order_rules`]. That engine is
//! deliberately ignorant of the episode layer; this module is the seam that
//! projects a real [`Episode`] season and its [`fold`](crate::episode::fold) into the engine's inputs:
//!
//! - [`episodes_to_slots`] turns episodes into [`SlotView`]s, deriving the
//!   listener-ledger fields a rule reads from the fold (which eras the listener
//!   learns about, when the full team has assembled, which promises open/close).
//! - [`check_order`] runs the order rules over those slots.
//! - [`season_slack`] wires the slots and rules into the [`slack`] passes,
//!   reporting each episode's earliest/latest legal slot.
//! - [`order_integrity`] reports duplicate and gapped production slots — the
//!   integrity the fold itself does not enforce (carry-item R1-L2).
//! - [`precedes_with_standing`] / [`StandingOrder`] are re-exported here so the
//!   episode layer reads event precedence through its **standing-aware** lens:
//!   a merely-proposed `before`-fact never hardens to a certain order (R2-G).
//!
//! # The listener configuration `full-team`
//!
//! The one built-in listener configuration this adapter derives is `full-team`:
//! it holds from the production slot at which every speaking (on-mic) cast member
//! of the season has appeared at least once, and every slot after. That is the
//! ledger the `full-team-floor` rule reads. A season with no on-mic presence data
//! asserts no `full-team` configuration at all.

use std::collections::{BTreeMap, BTreeSet};

use dli_core::ann::{Ann, Theory};
use dli_core::valuation::Valuation;
use dli_time::chronology::{Chronology, TimePoint};
use dli_time::order_rules::{
    OrderFinding, OrderRule, OrderRuleBody, SlotRequirement, SlotView, check,
};
use dli_time::slack::{Precedence, SlackInput, SlackReport, slack};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::episode::{Episode, Knower, Presence};

pub use dli_time::order_rules::{StandingOrder, precedes_with_standing};

/// The name of the one listener configuration this adapter derives.
const FULL_TEAM: &str = "full-team";

/// Project a season into the order engine's [`SlotView`]s.
///
/// Each episode becomes one slot at its production order. The listener-ledger
/// fields are derived: `learns_eras` from the fold's listener knowledge records
/// (the era each learned fact is anchored in), the `full-team` configuration from
/// on-mic presence, and the reveal/payoff tags from each scene's opened and
/// closed loops. Narrative `act` is not a kernel property of an [`Episode`], so
/// it is supplied by the caller through `acts` (episode id → act); an episode
/// absent from `acts` has an unknown act, which makes any `act`-bound rule report
/// *indeterminate* rather than pass.
#[must_use]
pub fn episodes_to_slots(episodes: &[Episode], acts: &BTreeMap<String, u32>) -> Vec<SlotView> {
    let folded = crate::episode::fold(episodes);
    let assembling = full_team_assembling_slot(episodes);

    // The eras the listener learns about, keyed by the production slot of the
    // reveal — straight off the fold's knowledge ledger.
    let mut learns: BTreeMap<u32, BTreeSet<String>> = BTreeMap::new();
    for record in folded.knowledge_records() {
        if record.knower == Knower::Listener
            && let Some(era) = &record.story_anchor.era
        {
            learns
                .entry(record.production_order)
                .or_default()
                .insert(era.0.clone());
        }
    }

    episodes
        .iter()
        .map(|episode| {
            let mut view = SlotView::new(&episode.id, episode.production_order);
            if let Some(&act) = acts.get(&episode.id) {
                view.act = Some(act);
            }
            if assembling.is_some_and(|at| episode.production_order >= at) {
                view.configurations.insert(FULL_TEAM.to_owned());
            }
            if let Some(eras) = learns.get(&episode.production_order) {
                view.learns_eras.clone_from(eras);
            }
            for scene in &episode.scenes {
                for question in &scene.opens {
                    view.reveals.insert(question.0.clone());
                }
                for question in &scene.closes {
                    view.payoffs.insert(question.0.clone());
                }
            }
            view
        })
        .collect()
}

/// The first production slot at which the full on-mic ensemble has assembled, or
/// `None` when the season records no on-mic presence.
fn full_team_assembling_slot(episodes: &[Episode]) -> Option<u32> {
    let roster: BTreeSet<&str> = episodes
        .iter()
        .flat_map(|e| &e.scenes)
        .flat_map(|s| &s.presence)
        .filter_map(on_mic)
        .collect();
    if roster.is_empty() {
        return None;
    }
    let mut ordered: Vec<&Episode> = episodes.iter().collect();
    ordered.sort_by_key(|e| e.production_order);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for episode in ordered {
        for entity in episode
            .scenes
            .iter()
            .flat_map(|s| &s.presence)
            .filter_map(on_mic)
        {
            seen.insert(entity);
        }
        if seen.len() == roster.len() {
            return Some(episode.production_order);
        }
    }
    None
}

/// The entity name of an on-mic presence, or `None` for off-mic.
fn on_mic(presence: &Presence) -> Option<&str> {
    match presence {
        Presence::OnMic(entity) => Some(entity.0.as_str()),
        Presence::OffMic(_) => None,
    }
}

/// Check a season against its `over order` rules, projecting through the fold.
///
/// A thin composition of [`episodes_to_slots`] and [`dli_time::order_rules::check`].
/// The `acts` map supplies narrative acts (see [`episodes_to_slots`]).
#[must_use]
pub fn check_order(
    episodes: &[Episode],
    rules: &[OrderRule],
    acts: &BTreeMap<String, u32>,
) -> Vec<OrderFinding> {
    let slots = episodes_to_slots(episodes, acts);
    check(&slots, rules)
}

/// Build the [`SlackInput`] for a season from its episodes and order rules.
///
/// Guarded `slot >= N` / `slot <= N` rules become per-episode floors/ceilings on
/// exactly the episodes their guard selects. Reveal→payoff structure becomes
/// production-order precedences (the episode that first reveals a tag must
/// precede any episode that pays it off); a promise set up and paid off in one
/// episode adds no self-edge. Every derived precedence is certain — it is a hard
/// structural constraint, not a proposed ordering.
#[must_use]
pub fn slack_input(
    episodes: &[Episode],
    rules: &[OrderRule],
    acts: &BTreeMap<String, u32>,
) -> SlackInput {
    let slots = episodes_to_slots(episodes, acts);
    let mut floors: BTreeMap<String, u32> = BTreeMap::new();
    let mut ceilings: BTreeMap<String, u32> = BTreeMap::new();
    for rule in rules {
        let OrderRuleBody::Guarded { guard, requirement } = &rule.body else {
            continue;
        };
        for slot in &slots {
            if !guard.matches(slot) {
                continue;
            }
            match requirement {
                SlotRequirement::SlotAtLeast(n) => {
                    floors
                        .entry(slot.episode.clone())
                        .and_modify(|f| *f = (*f).max(*n))
                        .or_insert(*n);
                }
                SlotRequirement::SlotAtMost(n) => {
                    ceilings
                        .entry(slot.episode.clone())
                        .and_modify(|c| *c = (*c).min(*n))
                        .or_insert(*n);
                }
                SlotRequirement::ActAtLeast(_) => {}
            }
        }
    }
    SlackInput {
        episodes: episodes.iter().map(|e| e.id.clone()).collect(),
        precedences: reveal_payoff_precedences(&slots),
        floors,
        ceilings,
    }
}

/// Compute each episode's earliest/latest legal production slot.
///
/// Wires [`slack_input`] into the [`slack`] forward/backward passes. Only certain
/// precedences bind the band (the reveal→payoff edges are certain by
/// construction); proposed orderings would be set aside — see the standing note
/// on [`slack`].
#[must_use]
pub fn season_slack(
    episodes: &[Episode],
    rules: &[OrderRule],
    acts: &BTreeMap<String, u32>,
    valuation: &Valuation,
    theory: &Theory,
) -> SlackReport {
    let input = slack_input(episodes, rules, acts);
    slack(&input, valuation, theory)
}

/// The reveal→payoff production-order precedences implied by the slot ledger: the
/// episode that first reveals a tag must precede any episode paying it off.
fn reveal_payoff_precedences(slots: &[SlotView]) -> Vec<Precedence> {
    let mut earliest_reveal: BTreeMap<&str, (u32, &str)> = BTreeMap::new();
    for slot in slots {
        for tag in &slot.reveals {
            earliest_reveal
                .entry(tag.as_str())
                .and_modify(|cur| {
                    if slot.slot < cur.0 {
                        *cur = (slot.slot, slot.episode.as_str());
                    }
                })
                .or_insert((slot.slot, slot.episode.as_str()));
        }
    }
    let mut precedences: Vec<Precedence> = Vec::new();
    for slot in slots {
        for tag in &slot.payoffs {
            if let Some(&(_, reveal_episode)) = earliest_reveal.get(tag.as_str())
                && reveal_episode != slot.episode
            {
                let precedence = Precedence::certain(reveal_episode, &slot.episode);
                if !precedences.contains(&precedence) {
                    precedences.push(precedence);
                }
            }
        }
    }
    precedences
}

// --- production-order integrity (carry-item R1-L2) ---------------------------

/// What an [`OrderIntegrityFinding`] reports about the production-order sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OrderIntegrityKind {
    /// Two or more episodes claim the same production slot — the fold would keep
    /// input order between them, silently, rather than flag the collision.
    DuplicateProductionOrder,
    /// A production slot between the first and last is empty — a gap in the
    /// sequence.
    GapInProductionOrder,
}

/// A production-order integrity defect: a duplicated or missing slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrderIntegrityFinding {
    /// The shape of the defect.
    pub kind: OrderIntegrityKind,
    /// The slot the defect concerns.
    pub slot: u32,
    /// The episodes at the slot (the colliding ids for a duplicate; empty for a
    /// gap).
    pub episodes: Vec<String>,
    /// A domain-spoken description.
    pub message: String,
}

/// Report duplicate and gapped production slots across a season.
///
/// The episode fold accepts duplicate production orders silently, keeping input
/// order between colliding episodes ([`fold`](crate::episode::fold)); this is the
/// integrity check that names them. Duplicates come first (ascending by slot),
/// then gaps (each empty slot between the first and last, ascending).
///
/// ```
/// use dli_world::episode::Episode;
/// use dli_world::episode_order::{order_integrity, OrderIntegrityKind};
///
/// let episodes = vec![
///     Episode { id: "a".to_owned(), production_order: 1, ..Episode::default() },
///     Episode { id: "b".to_owned(), production_order: 1, ..Episode::default() }, // collides
///     Episode { id: "c".to_owned(), production_order: 4, ..Episode::default() }, // gaps 2,3
/// ];
/// let findings = order_integrity(&episodes);
/// assert_eq!(findings[0].kind, OrderIntegrityKind::DuplicateProductionOrder);
/// assert_eq!(findings[0].episodes, vec!["a".to_owned(), "b".to_owned()]);
/// // Slots 2 and 3 are reported as gaps.
/// let gaps: Vec<u32> = findings
///     .iter()
///     .filter(|f| f.kind == OrderIntegrityKind::GapInProductionOrder)
///     .map(|f| f.slot)
///     .collect();
/// assert_eq!(gaps, vec![2, 3]);
/// ```
#[must_use]
pub fn order_integrity(episodes: &[Episode]) -> Vec<OrderIntegrityFinding> {
    let mut by_slot: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for episode in episodes {
        by_slot
            .entry(episode.production_order)
            .or_default()
            .push(episode.id.clone());
    }

    let mut findings = Vec::new();
    for (&slot, ids) in &by_slot {
        if ids.len() > 1 {
            findings.push(OrderIntegrityFinding {
                kind: OrderIntegrityKind::DuplicateProductionOrder,
                slot,
                episodes: ids.clone(),
                message: format!(
                    "production slot {slot} is claimed by {} episodes ({}); the fold keeps input \
                     order between them silently",
                    ids.len(),
                    ids.join(", ")
                ),
            });
        }
    }

    if let (Some(&min), Some(&max)) = (by_slot.keys().next(), by_slot.keys().next_back()) {
        for slot in min..=max {
            if !by_slot.contains_key(&slot) {
                findings.push(OrderIntegrityFinding {
                    kind: OrderIntegrityKind::GapInProductionOrder,
                    slot,
                    episodes: Vec::new(),
                    message: format!(
                        "production slot {slot} is empty — a gap in the sequence {min}..={max}"
                    ),
                });
            }
        }
    }

    findings
}

// --- standing-aware precedence at the episode layer (carry-item R2-G) --------

/// The standing-aware order between two events on the world chronology.
///
/// This is the episode-layer entry point to [`precedes_with_standing`]: it reads
/// precedence through the ratification standing of the `before`-facts on the
/// path, so a *merely-proposed* ordering yields [`StandingOrder::PossiblyBefore`]
/// rather than a hard [`StandingOrder::CertainlyBefore`]. Use it, not the
/// standing-blind [`Chronology::precedes`], wherever an unratified edge must not
/// be allowed to settle an ordering. `anns[i]` declares `chronology.edges[i]`
/// (the loader's parallel table); a missing entry is treated as a certain,
/// unconditional order.
#[must_use]
pub fn event_precedence(
    chronology: &Chronology,
    anns: &[Ann],
    valuation: &Valuation,
    theory: &Theory,
    earlier: TimePoint,
    later: TimePoint,
) -> StandingOrder {
    precedes_with_standing(chronology, anns, valuation, theory, earlier, later)
}
