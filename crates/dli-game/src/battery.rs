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

/// Running sufficient statistics for one deal-battery metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DealMetricAccum {
    /// Metric identity, aligned with the battery declaration.
    pub metric: String,
    /// Readings folded into this accumulator.
    pub trials: u64,
    /// Sum of observed values.
    pub sum: i64,
    /// Sum of squared observed values.
    pub sum_of_squares: i64,
}

/// Earliest witnessed breach of a deal-battery invariant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DealBreach {
    /// Zero-based reading index.
    pub trial: u64,
    /// Domain-spoken breach detail.
    pub detail: String,
}

/// Restart-resumable state for a [`DealBattery`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DealCheckpoint {
    /// Digest of the battery this checkpoint belongs to.
    pub battery_digest: String,
    /// Master seed.
    pub seed: u64,
    /// Requested reading count.
    pub trials_total: u64,
    /// Readings folded so far.
    pub trials_done: u64,
    /// Durable checkpoint cadence.
    pub every: u64,
    /// Boundaries crossed so far.
    pub lineage: Vec<u64>,
    /// Per-metric sufficient statistics.
    pub metrics: Vec<DealMetricAccum>,
    /// Earliest breach per invariant.
    pub breaches: Vec<Option<DealBreach>>,
}

impl DealCheckpoint {
    /// Create a zeroed checkpoint for a deal battery.
    #[must_use]
    pub fn initial(battery: &DealBattery, every: u64) -> Self {
        Self {
            battery_digest: battery.digest(),
            seed: battery.seed,
            trials_total: battery.trials,
            trials_done: 0,
            every: every.max(1),
            lineage: Vec::new(),
            metrics: battery
                .metrics
                .iter()
                .map(|metric| DealMetricAccum {
                    metric: metric.name.clone(),
                    trials: 0,
                    sum: 0,
                    sum_of_squares: 0,
                })
                .collect(),
            breaches: vec![None; battery.invariants.len()],
        }
    }

    /// Whether every requested reading has been folded.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.trials_done >= self.trials_total
    }

    /// Validate this serialized state by replaying it from the battery.
    pub fn validate_for(&self, battery: &DealBattery) -> Result<(), String> {
        if self.battery_digest != battery.digest()
            || self.seed != battery.seed
            || self.trials_total != battery.trials
        {
            return Err("deal checkpoint belongs to another battery".to_owned());
        }
        if self.every == 0 || self.trials_done > self.trials_total {
            return Err("deal checkpoint has an invalid cadence or trial count".to_owned());
        }
        if self.metrics.len() != battery.metrics.len()
            || self
                .metrics
                .iter()
                .zip(&battery.metrics)
                .any(|(accum, metric)| {
                    accum.metric != metric.name
                        || accum.trials != self.trials_done
                        || accum.sum_of_squares < 0
                })
            || self.breaches.len() != battery.invariants.len()
            || self
                .breaches
                .iter()
                .flatten()
                .any(|breach| breach.trial >= self.trials_done)
        {
            return Err("deal checkpoint accumulators do not match the battery".to_owned());
        }
        let expected_lineage = checkpoint_lineage(self.every, self.trials_done, self.trials_total);
        if self.lineage != expected_lineage {
            return Err("deal checkpoint lineage does not match its cadence".to_owned());
        }
        let mut replayed = Self::initial(battery, self.every);
        replayed.advance_to(battery, self.trials_done);
        replayed.lineage = expected_lineage;
        if replayed != *self {
            return Err("deal checkpoint does not replay from its ruleset and seed".to_owned());
        }
        Ok(())
    }

    /// Fold exactly one cadence boundary.
    pub fn advance_boundary(&mut self, battery: &DealBattery) -> Result<bool, String> {
        self.validate_for(battery)?;
        if self.is_complete() {
            return Ok(true);
        }
        let next = (self.trials_done / self.every + 1)
            .saturating_mul(self.every)
            .min(self.trials_total);
        self.advance_to(battery, next);
        self.lineage.push(self.trials_done);
        Ok(self.is_complete())
    }

    /// Build the honest shared run manifest represented by this state.
    #[must_use]
    pub fn finalize(&self, battery: &DealBattery) -> RunManifest {
        let metrics = self
            .metrics
            .iter()
            .map(|accum| {
                MetricEstimate::from_counts(
                    accum.metric.clone(),
                    accum.trials,
                    accum.sum,
                    accum.sum_of_squares,
                )
            })
            .collect();
        let invariants = battery
            .invariants
            .iter()
            .zip(&self.breaches)
            .map(|(invariant, breach)| {
                let outcome = match breach {
                    None => InvariantOutcome::Held,
                    Some(breach) => InvariantOutcome::Violated {
                        trial: breach.trial,
                        detail: breach.detail.clone(),
                    },
                };
                InvariantVerdict::new(invariant.name(), outcome)
            })
            .collect();
        RunManifest {
            battery_digest: battery.digest(),
            seed: self.seed,
            trials: self.trials_done,
            metrics,
            invariants,
            checkpoint_lineage: self.lineage.clone(),
        }
    }

    fn advance_to(&mut self, battery: &DealBattery, target: u64) {
        let bindings = GameBindings::new();
        while self.trials_done < target.min(self.trials_total) {
            let trial = self.trials_done;
            let reading = battery.deal(trial);
            for (accum, metric) in self.metrics.iter_mut().zip(&battery.metrics) {
                let value = metric
                    .observable
                    .observe(&battery.ruleset, &reading, &bindings);
                accum.trials += 1;
                accum.sum = accum.sum.saturating_add(value);
                accum.sum_of_squares = accum
                    .sum_of_squares
                    .saturating_add(value.saturating_mul(value));
            }
            for (breach, invariant) in self.breaches.iter_mut().zip(&battery.invariants) {
                if breach.is_none()
                    && let Some(detail) = invariant.breach(&battery.ruleset, &reading, trial)
                {
                    *breach = Some(DealBreach { trial, detail });
                }
            }
            self.trials_done += 1;
        }
    }
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

fn checkpoint_lineage(every: u64, done: u64, total: u64) -> Vec<u64> {
    let mut lineage = Vec::new();
    let mut boundary = every;
    while boundary <= done {
        lineage.push(boundary);
        let next = boundary.saturating_add(every);
        if next == boundary {
            break;
        }
        boundary = next;
    }
    if done == total && done > 0 && lineage.last() != Some(&done) {
        lineage.push(done);
    }
    lineage
}

/// Pick an index uniformly in `[0, len)` from a generator; `None` when empty.
fn pick(prng: &mut SplitMix64, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let bound = u64::try_from(len).unwrap_or(u64::MAX);
    usize::try_from(prng.below(bound)).ok()
}
