//! The generic deal battery: seeded readings folded into an honest run manifest.
//!
//! This lifts the card-deal paradigm onto the shared honesty substrate — the
//! `SplitMix64` draws and the `RunManifest` — so a deck study inherits the same
//! grade-honesty oracle the practice simulator enforces: rates grade `tested`,
//! exact sweeps grade `checked`. It reuses `dli_sim::rng` and `dli_sim::manifest`
//! whole and reimplements nothing.
//!
//! A battery deals `trials` readings, each a pure function of `(seed, index)`, so a
//! run replays byte-for-byte.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_sim::manifest::{
    InvariantOutcome, InvariantVerdict, MetricEstimate, RunManifest, digest_json,
};
use dli_sim::rng::{SplitMix64, stream_value};

use crate::binding::GameBindings;
use crate::model::{Reading, Ruleset};

/// A per-reading scalar the battery observes for a Monte-Carlo metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "observe", rename_all = "kebab-case")]
pub enum ReadingObservable {
    /// `1` if the named card id appears in the reading, else `0`.
    CardAppears {
        /// The card id watched.
        card: String,
    },
    /// `1` if the reading's verdict has the named id, else `0`.
    VerdictIs {
        /// The verdict id watched.
        verdict_id: String,
    },
}

impl ReadingObservable {
    /// Observe a dealt reading under its ruleset (and the game bindings, for the
    /// verdict).
    fn observe(&self, ruleset: &Ruleset, reading: &Reading, bindings: &GameBindings) -> i64 {
        match self {
            Self::CardAppears { card } => i64::from(reading.holds(card)),
            Self::VerdictIs { verdict_id } => {
                i64::from(ruleset.verdict(reading, bindings).id == *verdict_id)
            }
        }
    }
}

/// A named Monte-Carlo metric over the deal battery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadingMetric {
    /// The metric name, carried onto its estimate.
    pub name: String,
    /// The per-reading value it aggregates.
    pub observable: ReadingObservable,
}

/// An exact predicate swept over every dealt reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "invariant", rename_all = "kebab-case")]
pub enum ReadingInvariant {
    /// The named card never appears in any reading.
    CardNeverAppears {
        /// The card id that must not appear.
        card: String,
    },
    /// Every reading conserves the deck: no card laid twice.
    DeckConserved,
    /// Every play in every reading was a legal move given the prefix laid.
    OnlyLegalPlays,
}

impl ReadingInvariant {
    /// The invariant's stable name.
    fn name(&self) -> String {
        match self {
            Self::CardNeverAppears { card } => format!("{card}-never-appears"),
            Self::DeckConserved => "deck-conserved".to_owned(),
            Self::OnlyLegalPlays => "only-legal-plays".to_owned(),
        }
    }

    /// The breach reason if this reading violates the invariant, else `None`.
    fn breach(&self, ruleset: &Ruleset, reading: &Reading, trial: u64) -> Option<String> {
        match self {
            Self::CardNeverAppears { card } => reading
                .holds(card)
                .then(|| format!("'{card}' appeared at trial {trial}")),
            Self::DeckConserved => breach_conservation(reading, trial),
            Self::OnlyLegalPlays => breach_legality(ruleset, reading, trial),
        }
    }
}

fn breach_conservation(reading: &Reading, trial: u64) -> Option<String> {
    let unique: std::collections::BTreeSet<&String> = reading.laid.iter().collect();
    (unique.len() != reading.laid.len()).then(|| format!("a card was laid twice at trial {trial}"))
}

fn breach_legality(ruleset: &Ruleset, reading: &Reading, trial: u64) -> Option<String> {
    let mut prefix = Reading {
        layout: reading.layout.clone(),
        cards_dealt: reading.cards_dealt,
        laid: Vec::new(),
    };
    for id in &reading.laid {
        if !ruleset.legal_moves(&prefix).contains(id) {
            return Some(format!("'{id}' was an illegal play at trial {trial}"));
        }
        prefix.laid.push(id.clone());
    }
    None
}

/// A deal battery: a ruleset, a layout, and the metrics and invariants to grade
/// over seeded readings. It reuses the honesty substrate whole, so a study inherits
/// the grade-honesty oracle — rates grade `tested`, exact sweeps grade `checked`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DealBattery {
    /// The ruleset dealt from.
    pub ruleset: Ruleset,
    /// The layout name dealt.
    pub layout: String,
    /// The master seed every reading derives its sub-stream from.
    pub seed: u64,
    /// The number of readings to deal.
    pub trials: u64,
    /// The Monte-Carlo metrics to estimate.
    pub metrics: Vec<ReadingMetric>,
    /// The exact invariants to sweep.
    pub invariants: Vec<ReadingInvariant>,
}

impl DealBattery {
    /// Deal reading `index`: play a legal reading from that reading's sub-stream,
    /// `stream_value(seed, index)`. Each play picks uniformly among the legal moves,
    /// so the reading is a pure function of `(seed, index)`.
    ///
    /// A layout the deck cannot deal yields an empty reading.
    #[must_use]
    pub fn deal(&self, index: u64) -> Reading {
        let Some(layout) = self.ruleset.layout(&self.layout) else {
            return Reading {
                layout: self.layout.clone(),
                cards_dealt: 0,
                laid: Vec::new(),
            };
        };
        let mut prng = SplitMix64::new(stream_value(self.seed, index));
        let mut reading = Reading::open(layout);
        while !self.ruleset.is_terminal(&reading) {
            let moves = self.ruleset.legal_moves(&reading);
            let Some(choice) = pick(&mut prng, moves.len()) else {
                break;
            };
            reading.laid.push(moves[choice].clone());
        }
        reading
    }

    /// Run the battery, folding every dealt reading into an honest `RunManifest`:
    /// each metric a `tested` estimate with exact counts, each invariant a `checked`
    /// sweep with its earliest witnessing trial. The returned manifest satisfies
    /// `RunManifest::grades_are_honest`.
    #[must_use]
    pub fn run(&self, bindings: &GameBindings) -> RunManifest {
        let mut metric_stats = vec![(0i64, 0i64); self.metrics.len()];
        let mut breaches: Vec<Option<(u64, String)>> = vec![None; self.invariants.len()];

        for index in 0..self.trials {
            let reading = self.deal(index);
            for (slot, metric) in self.metrics.iter().enumerate() {
                let value = metric.observable.observe(&self.ruleset, &reading, bindings);
                metric_stats[slot].0 += value;
                metric_stats[slot].1 += value * value;
            }
            for (slot, invariant) in self.invariants.iter().enumerate() {
                if breaches[slot].is_none()
                    && let Some(detail) = invariant.breach(&self.ruleset, &reading, index)
                {
                    breaches[slot] = Some((index, detail));
                }
            }
        }

        let metrics = self
            .metrics
            .iter()
            .zip(metric_stats)
            .map(|(metric, (sum, sum_sq))| {
                MetricEstimate::from_counts(metric.name.clone(), self.trials, sum, sum_sq)
            })
            .collect();
        let invariants = self
            .invariants
            .iter()
            .zip(breaches)
            .map(|(invariant, breach)| {
                let outcome = match breach {
                    None => InvariantOutcome::Held,
                    Some((trial, detail)) => InvariantOutcome::Violated { trial, detail },
                };
                InvariantVerdict::new(invariant.name(), outcome)
            })
            .collect();

        RunManifest {
            battery_digest: self.digest(),
            seed: self.seed,
            trials: self.trials,
            metrics,
            invariants,
            checkpoint_lineage: if self.trials > 0 {
                vec![self.trials]
            } else {
                Vec::new()
            },
        }
    }

    /// A content digest of the battery's semantics — for pinning which battery a
    /// manifest came from.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_json(self)
    }
}

/// Pick an index uniformly in `[0, len)` from a generator; `None` when empty.
fn pick(prng: &mut SplitMix64, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let bound = u64::try_from(len).unwrap_or(u64::MAX);
    usize::try_from(prng.below(bound)).ok()
}
