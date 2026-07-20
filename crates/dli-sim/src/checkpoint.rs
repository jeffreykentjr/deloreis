//! The resumable trial engine and its checkpoints.
//!
//! A battery run folds trials into a [`Checkpoint`] — the serializable running
//! state: per-metric sufficient statistics and per-invariant first-breach, plus
//! the seed, the totals, and the cadence and lineage of boundaries crossed. The
//! fold is *order-independent per trial*: trial `k`'s scene is a pure function of
//! `(seed, k)` ([`crate::rng`]), sums are commutative, and an invariant's witness
//! is the minimum breaching index. So a run stopped at a boundary, serialized,
//! and [`resume`]d continues the identical sub-stream and lands on a
//! byte-identical [`RunManifest`] (`docs/spec/10-serving.md` §5: jobs are
//! checkpointed and resumable).
//!
//! The manifest a run commits is *provenance-independent*: [`Battery::run`],
//! [`Battery::run_checkpointing`], and [`resume`] with the same cadence all
//! produce the same bytes. The checkpoint is a side channel that never changes
//! the result — it only lets a long run survive an interruption.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::battery::Battery;
use crate::manifest::{InvariantOutcome, InvariantVerdict, MetricEstimate, RunManifest};

/// The earliest breach of one invariant: the witnessing trial and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Breach {
    /// The lowest-indexed trial that broke the invariant.
    pub trial: u64,
    /// A domain-spoken account of the breach.
    pub detail: String,
}

/// The running sufficient statistics for one metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MetricAccum {
    /// The metric's name.
    pub metric: String,
    /// Trials that contributed an observation.
    pub trials: u64,
    /// Sum of observed values (saturating; the realistic bounds never reach it).
    pub sum: i64,
    /// Sum of squares of observed values (saturating).
    pub sum_of_squares: i64,
}

impl MetricAccum {
    fn zeroed(metric: String) -> Self {
        Self {
            metric,
            trials: 0,
            sum: 0,
            sum_of_squares: 0,
        }
    }

    fn observe(&mut self, value: i64) {
        self.trials += 1;
        self.sum = self.sum.saturating_add(value);
        self.sum_of_squares = self
            .sum_of_squares
            .saturating_add(value.saturating_mul(value));
    }

    fn finalize(&self) -> MetricEstimate {
        MetricEstimate::from_counts(
            self.metric.clone(),
            self.trials,
            self.sum,
            self.sum_of_squares,
        )
    }
}

/// The running state for one invariant: its earliest breach, if any yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvariantAccum {
    /// The invariant's name.
    pub invariant: String,
    /// The earliest breach seen so far, or [`None`] while it still holds.
    pub first_breach: Option<Breach>,
}

impl InvariantAccum {
    fn clear(invariant: String) -> Self {
        Self {
            invariant,
            first_breach: None,
        }
    }

    fn record(&mut self, trial: u64, detail: String) {
        // Keep the minimum breaching trial. Folds run in increasing index and
        // resume never re-folds an earlier trial, so "first seen" *is* the min.
        if self.first_breach.is_none() {
            self.first_breach = Some(Breach { trial, detail });
        }
    }

    fn finalize(&self) -> InvariantVerdict {
        let outcome = match &self.first_breach {
            None => InvariantOutcome::Held,
            Some(breach) => InvariantOutcome::Violated {
                trial: breach.trial,
                detail: breach.detail.clone(),
            },
        };
        InvariantVerdict::new(self.invariant.clone(), outcome)
    }
}

/// The serializable running state of a battery run.
///
/// Capture one every N trials and a crash loses at most N trials of work; hand
/// it to [`resume`] and the run finishes on the identical manifest it would have
/// produced uninterrupted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Checkpoint {
    /// The digest of the battery this state belongs to — a resume against a
    /// different battery is a detectable mismatch, never a silent divergence.
    pub battery_digest: String,
    /// The master seed.
    pub seed: u64,
    /// The battery's total trial count.
    pub trials_total: u64,
    /// Trials folded so far; the next trial to run is this index.
    pub trials_done: u64,
    /// The checkpoint cadence in trials, if this is a checkpointed run.
    pub every: Option<u64>,
    /// The boundary trial-counts crossed so far (the lineage under construction).
    pub lineage: Vec<u64>,
    /// Per-metric running statistics, aligned to the battery's metric order.
    pub metrics: Vec<MetricAccum>,
    /// Per-invariant running state, aligned to the battery's invariant order.
    pub invariants: Vec<InvariantAccum>,
}

impl Checkpoint {
    /// A zeroed checkpoint for `battery`, before any trial has run.
    #[must_use]
    pub fn initial(battery: &Battery, every: Option<u64>) -> Self {
        Self {
            battery_digest: battery.digest(),
            seed: battery.seed,
            trials_total: battery.trials,
            trials_done: 0,
            every,
            lineage: Vec::new(),
            metrics: battery
                .metrics
                .iter()
                .map(|m| MetricAccum::zeroed(m.name.clone()))
                .collect(),
            invariants: battery
                .invariants
                .iter()
                .map(|i| InvariantAccum::clear(i.name.clone()))
                .collect(),
        }
    }

    /// Whether every trial has been folded.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.trials_done >= self.trials_total
    }

    /// Whether this checkpoint belongs to `battery` (its digest matches).
    #[must_use]
    pub fn matches(&self, battery: &Battery) -> bool {
        self.battery_digest == battery.digest()
    }

    /// Fold trials `[trials_done, target)` into this checkpoint, in order.
    fn advance_to(&mut self, battery: &Battery, target: u64) {
        let target = target.min(self.trials_total);
        while self.trials_done < target {
            let index = self.trials_done;
            let outcome = battery.account_trial(&battery.generate_trial(index));
            for (accum, metric) in self.metrics.iter_mut().zip(&battery.metrics) {
                if let Some(value) = metric.observable.observe(&outcome) {
                    accum.observe(value);
                }
            }
            for (accum, spec) in self.invariants.iter_mut().zip(&battery.invariants) {
                if accum.first_breach.is_none()
                    && let Some(detail) = spec.predicate.breach(&outcome)
                {
                    accum.record(index, detail);
                }
            }
            self.trials_done += 1;
        }
    }

    /// Assemble the committed [`RunManifest`] from this state.
    ///
    /// Asserts the SIM-1 honesty oracle
    /// ([`grades_are_honest`](RunManifest::grades_are_honest)) before returning:
    /// every metric `tested`, every invariant `checked`.
    #[must_use]
    pub fn finalize(&self, battery: &Battery) -> RunManifest {
        let manifest = RunManifest {
            battery_digest: battery.digest(),
            seed: self.seed,
            trials: self.trials_done,
            metrics: self.metrics.iter().map(MetricAccum::finalize).collect(),
            invariants: self
                .invariants
                .iter()
                .map(InvariantAccum::finalize)
                .collect(),
            checkpoint_lineage: self.lineage.clone(),
        };
        // A hard assert, not a debug assert: the SIM-1 oracle must hold in
        // release too, so a manifest can never be committed with a forged grade.
        assert!(
            manifest.grades_are_honest(),
            "SIM-1 oracle: metrics must be tested and invariants checked"
        );
        manifest
    }
}

impl Battery {
    /// Run every trial straight through and commit the manifest.
    ///
    /// The lineage is the single final boundary `[trials]` — or empty for a
    /// zero-trial battery, which crosses no boundary. For the same seed and
    /// trials this is byte-identical to
    /// [`run_checkpointing(trials)`](Battery::run_checkpointing) and to a resume
    /// that reconstructs the same cadence, at every trial count including zero.
    ///
    /// ```
    /// use dli_sim::battery::{Battery, GenBounds, InvariantSpec, Invariant};
    /// use std::collections::BTreeMap;
    /// use dli_syntax::parse_document;
    /// use dli_ledger::practice::compile_practices;
    ///
    /// let (doc, _) = parse_document(
    ///     "scope magic\n\
    ///      practice p proposed {\n\
    ///        resource debt : linear conserved\n\
    ///        cost epistemic\n\
    ///      }\n",
    /// );
    /// let battery = Battery {
    ///     practice: compile_practices(&doc).remove(0),
    ///     bounds: GenBounds { max_steps: 4, max_magnitude: 3 },
    ///     seed: 1,
    ///     trials: 64,
    ///     metrics: Vec::new(),
    ///     invariants: vec![InvariantSpec {
    ///         name: "conserved".to_owned(),
    ///         predicate: Invariant::NeverViolated { resource: "debt".to_owned() },
    ///     }],
    ///     affine_caps: BTreeMap::new(),
    /// };
    /// let manifest = battery.run();
    /// assert!(manifest.grades_are_honest());
    /// // Same seed, same bytes.
    /// assert_eq!(manifest.canonical_bytes(), battery.run().canonical_bytes());
    /// ```
    #[must_use]
    pub fn run(&self) -> RunManifest {
        let mut cp = Checkpoint::initial(self, None);
        cp.advance_to(self, self.trials);
        // Record the single final boundary — but a zero-trial run crosses none,
        // exactly as `run_checkpointing(0)` and a resume of the initial
        // checkpoint record it (both leave the lineage empty). Committing `[0]`
        // here would break the run == run_checkpointing byte-identity contract at
        // the trials=0 boundary.
        if self.trials > 0 {
            cp.lineage = vec![self.trials];
        }
        cp.finalize(self)
    }

    /// Run to completion, capturing a checkpoint at every `every` trials.
    ///
    /// Returns the committed manifest and the checkpoints taken at each boundary
    /// (the last of which is the completed state). `every` is clamped to at least
    /// `1`. The manifest's lineage is the boundary cadence `[every, 2·every, …,
    /// trials]`.
    #[must_use]
    pub fn run_checkpointing(&self, every: u64) -> (RunManifest, Vec<Checkpoint>) {
        let every = every.max(1);
        let mut cp = Checkpoint::initial(self, Some(every));
        let mut snapshots = Vec::new();
        let mut boundary = every;
        while cp.trials_done < self.trials {
            cp.advance_to(self, boundary);
            cp.lineage.push(cp.trials_done);
            snapshots.push(cp.clone());
            boundary = boundary.saturating_add(every);
        }
        // A zero-trial battery crosses no boundary; still commit a manifest.
        let manifest = cp.finalize(self);
        (manifest, snapshots)
    }
}

/// Continue a checkpointed run to completion, committing the identical manifest
/// an uninterrupted run of the same cadence would have produced.
///
/// The checkpoint carries its cadence and the boundaries already crossed; resume
/// picks up at the next boundary and appends the rest, so the final lineage — and
/// every metric and invariant — matches straight-through byte-for-byte.
///
/// # Panics
/// Panics if the checkpoint does not belong to `battery` (its digest mismatches):
/// resuming foreign state would silently corrupt the run, so it fails loudly.
#[must_use]
pub fn resume(battery: &Battery, checkpoint: &Checkpoint) -> RunManifest {
    assert!(
        checkpoint.matches(battery),
        "checkpoint belongs to a different battery: {} vs {}",
        checkpoint.battery_digest,
        battery.digest()
    );
    let mut cp = checkpoint.clone();
    let every = cp.every.unwrap_or_else(|| battery.trials.max(1));
    // The next boundary is the smallest multiple of `every` strictly above what
    // has run — identical to what an uninterrupted run would target next.
    let mut boundary = (cp.trials_done / every + 1).saturating_mul(every);
    while cp.trials_done < battery.trials {
        cp.advance_to(battery, boundary);
        cp.lineage.push(cp.trials_done);
        boundary = boundary.saturating_add(every);
    }
    cp.finalize(battery)
}
