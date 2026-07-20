//! Run manifests: the honest, replayable record a battery produces.
//!
//! A [`RunManifest`] is what a battery run *commits*: the digest of the battery
//! that produced it, the seed and trial count that make it replayable, and two
//! kinds of result that the kernel refuses to confuse —
//!
//! - **invariant verdicts**, graded [`Grade::Checked`]. An invariant is an exact
//!   bookkeeping predicate swept over every trial the run produced; the claim
//!   "this predicate held on all of them" re-derives *exactly* from the same seed
//!   and count, so it is `checked`, and a breach carries the concrete witnessing
//!   trial.
//! - **metric estimates**, graded [`Grade::Tested`]. A metric is a Monte-Carlo
//!   estimate of a population quantity; the sample re-derives, but the *claim* is
//!   statistical, so it is `tested` forever (`docs/spec/08-reasoning.md` T18) and
//!   carries its counts and interval.
//!
//! This distinction is the SIM-1 oracle. It is enforced structurally: the only
//! constructors of [`MetricEstimate`] and [`InvariantVerdict`] stamp the grade
//! from a type-level constant, and [`RunManifest::grades_are_honest`] is a total
//! predicate the runner asserts before returning. A metric can never report
//! `checked`; an invariant can never report `tested`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::Grade;

/// A `fnv1a64:<hex>` content digest of a value's canonical JSON.
///
/// Dependency-free and version-stable, following the world layer's staleness
/// fingerprint (`dli-world`'s `ContentHash`): enough to pin *which* battery a
/// manifest came from and to catch accidental drift, not a cryptographic commit.
#[must_use]
pub fn digest_json<T: Serialize>(value: &T) -> String {
    // serde_json over declaration-ordered structs and sorted maps is canonical;
    // an empty pre-image on the (unreachable) serialize error still yields a
    // stable, obviously-degenerate digest rather than a panic.
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("fnv1a64:{:016x}", fnv1a64(&bytes))
}

/// A 64-bit FNV-1a hash — small, dependency-free, version-stable.
fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// A Monte-Carlo estimate of one declared metric, graded [`Grade::Tested`].
///
/// The [`sum`](MetricEstimate::sum) and [`sum_of_squares`](MetricEstimate::sum_of_squares)
/// are the exact integer sufficient statistics — the portable, replayable core of
/// the estimate. The [`mean`](MetricEstimate::mean) and
/// [`half_width`](MetricEstimate::half_width) are derived conveniences (a 95%
/// normal-approximation interval); trust the counts, read the interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MetricEstimate {
    /// The name of the declared metric this estimates.
    pub metric: String,
    /// The number of trials that contributed an observation.
    pub trials: u64,
    /// The exact sum of the observed per-trial values.
    pub sum: i64,
    /// The exact sum of squares of the observed values (for the variance).
    pub sum_of_squares: i64,
    /// The point estimate: `sum / trials`, or `0.0` when no trial contributed.
    pub mean: f64,
    /// Half-width of the 95% normal-approximation interval; `0.0` below 2 trials.
    pub half_width: f64,
    /// Always [`Grade::Tested`] — a metric is a statistical estimate, forever.
    ///
    /// Private: the only writer is [`from_counts`](MetricEstimate::from_counts),
    /// which stamps the type-level constant, so no struct literal outside this
    /// module can forge a grade. Read it with [`grade`](MetricEstimate::grade).
    /// Deserialization re-checks it against the constant, so a forged wire value
    /// is rejected rather than admitted.
    #[serde(deserialize_with = "deserialize_metric_grade")]
    grade: Grade,
}

impl MetricEstimate {
    /// The grade every metric estimate carries, structurally: it is Monte-Carlo.
    pub const GRADE: Grade = Grade::Tested;

    /// The grade this estimate carries — always [`Tested`](Grade::Tested).
    #[must_use]
    pub const fn grade(&self) -> Grade {
        self.grade
    }

    /// Build an estimate from its exact sufficient statistics, stamping the
    /// [`Tested`](Grade::Tested) grade. This is the *only* constructor, so a
    /// metric can never be minted with any other grade.
    #[must_use]
    pub fn from_counts(metric: String, trials: u64, sum: i64, sum_of_squares: i64) -> Self {
        let (mean, half_width) = estimate(trials, sum, sum_of_squares);
        Self {
            metric,
            trials,
            sum,
            sum_of_squares,
            mean,
            half_width,
            grade: Self::GRADE,
        }
    }
}

/// The point estimate and interval half-width from exact integer statistics.
///
/// The floats are derived from the integers, so given the same counts they are
/// bit-identical within a run (the resume-equivalence contract) and across runs
/// on the same target.
#[allow(clippy::cast_precision_loss)]
fn estimate(trials: u64, sum: i64, sum_of_squares: i64) -> (f64, f64) {
    if trials == 0 {
        return (0.0, 0.0);
    }
    let n = trials as f64;
    let mean = sum as f64 / n;
    let half_width = if trials >= 2 {
        // sample variance = (Σx² − (Σx)²/n)/(n−1) = (Σx² − Σx·mean)/(n−1)
        let variance = (sum_of_squares as f64 - sum as f64 * mean) / (n - 1.0);
        // clamp float error below zero before the root; 1.96 ≈ z_{0.975}.
        1.96 * (variance.max(0.0) / n).sqrt()
    } else {
        0.0
    };
    (mean, half_width)
}

/// The outcome of sweeping one exact invariant over every trial of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum InvariantOutcome {
    /// The predicate held on every trial the run produced.
    Held,
    /// The predicate failed; the earliest witnessing trial and why.
    Violated {
        /// The lowest-indexed trial at which the invariant broke.
        trial: u64,
        /// A domain-spoken account of the breach.
        detail: String,
    },
}

/// The verdict of one declared invariant over a run, graded [`Grade::Checked`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvariantVerdict {
    /// The name of the declared invariant this settles.
    pub invariant: String,
    /// Held, or violated with the witnessing trial.
    pub outcome: InvariantOutcome,
    /// Always [`Grade::Checked`] — an invariant is an exact, re-derivable sweep.
    ///
    /// Private: the only writer is [`new`](InvariantVerdict::new), which stamps
    /// the type-level constant, so no struct literal outside this module can
    /// forge a grade. Read it with [`grade`](InvariantVerdict::grade).
    /// Deserialization re-checks it against the constant, so a forged wire value
    /// is rejected rather than admitted.
    #[serde(deserialize_with = "deserialize_invariant_grade")]
    grade: Grade,
}

impl InvariantVerdict {
    /// The grade every invariant verdict carries, structurally: it is an exact
    /// bookkeeping sweep.
    pub const GRADE: Grade = Grade::Checked;

    /// The grade this verdict carries — always [`Checked`](Grade::Checked).
    #[must_use]
    pub const fn grade(&self) -> Grade {
        self.grade
    }

    /// Build a verdict, stamping the [`Checked`](Grade::Checked) grade. This is
    /// the *only* constructor, so an invariant can never be minted `tested`.
    #[must_use]
    pub fn new(invariant: String, outcome: InvariantOutcome) -> Self {
        Self {
            invariant,
            outcome,
            grade: Self::GRADE,
        }
    }

    /// Whether this invariant held.
    #[must_use]
    pub const fn held(&self) -> bool {
        matches!(self.outcome, InvariantOutcome::Held)
    }
}

/// The committed record of a battery run.
///
/// Serialization is deterministic: declaration-ordered fields, `Vec`s in the
/// battery's own declaration order (never a hash map), so
/// [`canonical_bytes`](RunManifest::canonical_bytes) is byte-stable and two runs
/// of the same battery — straight through or resumed from a checkpoint — produce
/// identical bytes.
///
/// Deserialization is guarded: each grade field validates itself against its
/// type-level constant as it is read (see
/// [`MetricEstimate`]/[`InvariantVerdict`]), so a hostile record with a forged
/// grade (a metric stamped `checked`, an invariant stamped `tested`) is
/// *rejected* at `deserialize` time in release, not merely in a debug assertion.
/// Combined with the private grade fields — which block a struct-literal forge at
/// compile time — the SIM-1 oracle cannot be evaded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunManifest {
    /// The digest of the battery that produced this run.
    pub battery_digest: String,
    /// The master seed the run drew every choice from.
    pub seed: u64,
    /// The number of trials folded into this manifest.
    pub trials: u64,
    /// Per-metric Monte-Carlo estimates, in the battery's declaration order.
    pub metrics: Vec<MetricEstimate>,
    /// Per-invariant exact verdicts, in the battery's declaration order.
    pub invariants: Vec<InvariantVerdict>,
    /// The trial-count boundaries at which the run's state was assembled — its
    /// checkpoint lineage. `[trials]` for an uninterrupted run; the full cadence
    /// (e.g. `[k, 2k, …, trials]`) for a checkpointed one, reconstructed
    /// identically whether the run was interrupted and resumed or not.
    pub checkpoint_lineage: Vec<u64>,
}

impl RunManifest {
    /// The canonical serialization used for digesting and byte-comparison.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// A `fnv1a64` digest of the whole manifest — a one-line replay fingerprint.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_json(self)
    }

    /// The SIM-1 honesty oracle, as a total predicate: **every metric is
    /// [`tested`](Grade::Tested) and every invariant is [`checked`](Grade::Checked)**.
    ///
    /// The runner asserts this before returning a manifest, and
    /// [`verify`](RunManifest::verify) enforces it at the deserialize boundary,
    /// so a mis-graded result fails loudly rather than lying quietly (charter
    /// koan 4).
    ///
    /// ```
    /// use dli_sim::manifest::{MetricEstimate, InvariantVerdict, InvariantOutcome, RunManifest};
    /// use dli_core::Grade;
    ///
    /// let manifest = RunManifest {
    ///     battery_digest: "fnv1a64:0000000000000000".to_owned(),
    ///     seed: 1,
    ///     trials: 3,
    ///     metrics: vec![MetricEstimate::from_counts("rate".to_owned(), 3, 1, 1)],
    ///     invariants: vec![InvariantVerdict::new("holds".to_owned(), InvariantOutcome::Held)],
    ///     checkpoint_lineage: vec![3],
    /// };
    /// assert!(manifest.grades_are_honest());
    /// assert_eq!(manifest.metrics[0].grade(), Grade::Tested);
    /// assert_eq!(manifest.invariants[0].grade(), Grade::Checked);
    /// ```
    #[must_use]
    pub fn grades_are_honest(&self) -> bool {
        self.verify().is_ok()
    }

    /// Verify the SIM-1 honesty oracle, returning the first violation as an
    /// error. `Ok(())` means every metric is [`tested`](Grade::Tested) and every
    /// invariant is [`checked`](Grade::Checked).
    ///
    /// This is the honesty check as a `Result` at the manifest boundary: it is
    /// what `deserialize` runs (via `try_from`), so a forged manifest never
    /// materializes as a trusted value, even in release.
    ///
    /// # Errors
    /// Returns [`DishonestManifest`] naming the first metric that is not `tested`
    /// or invariant that is not `checked`.
    pub fn verify(&self) -> Result<(), DishonestManifest> {
        if let Some(metric) = self.metrics.iter().find(|m| m.grade != Grade::Tested) {
            return Err(DishonestManifest {
                reason: format!(
                    "metric '{}' is graded {} — a metric is a statistical estimate and must be tested",
                    metric.metric,
                    metric.grade.as_str()
                ),
            });
        }
        if let Some(invariant) = self.invariants.iter().find(|i| i.grade != Grade::Checked) {
            return Err(DishonestManifest {
                reason: format!(
                    "invariant '{}' is graded {} — an invariant is an exact sweep and must be checked",
                    invariant.invariant,
                    invariant.grade.as_str()
                ),
            });
        }
        Ok(())
    }
}

/// A manifest whose grades violate the SIM-1 oracle: a metric not `tested` or an
/// invariant not `checked`. Raised when such a manifest is deserialized or
/// [`verify`](RunManifest::verify)ed, so a forged record is rejected instead of
/// trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DishonestManifest {
    /// A domain-spoken account of the first dishonest grade found.
    pub reason: String,
}

impl std::fmt::Display for DishonestManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dishonest run manifest: {}", self.reason)
    }
}

impl std::error::Error for DishonestManifest {}

/// Deserialize a metric's grade, rejecting any value but
/// [`Tested`](Grade::Tested). A metric is a statistical estimate forever, so a
/// wire record that restamps it `checked` is a forgery and fails to deserialize.
fn deserialize_metric_grade<'de, D>(deserializer: D) -> Result<Grade, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let grade = Grade::deserialize(deserializer)?;
    if grade == MetricEstimate::GRADE {
        Ok(grade)
    } else {
        Err(serde::de::Error::custom(format!(
            "a metric estimate is always graded {}, never {}",
            MetricEstimate::GRADE.as_str(),
            grade.as_str()
        )))
    }
}

/// Deserialize an invariant's grade, rejecting any value but
/// [`Checked`](Grade::Checked). An invariant is an exact re-derivable sweep, so a
/// wire record that restamps it `tested` is a forgery and fails to deserialize.
fn deserialize_invariant_grade<'de, D>(deserializer: D) -> Result<Grade, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let grade = Grade::deserialize(deserializer)?;
    if grade == InvariantVerdict::GRADE {
        Ok(grade)
    } else {
        Err(serde::de::Error::custom(format!(
            "an invariant verdict is always graded {}, never {}",
            InvariantVerdict::GRADE.as_str(),
            grade.as_str()
        )))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn metric_is_always_tested_never_checked() {
        let m = MetricEstimate::from_counts("r".to_owned(), 10, 4, 4);
        assert_eq!(m.grade, Grade::Tested);
        assert_ne!(m.grade, Grade::Checked);
    }

    #[test]
    fn invariant_is_always_checked() {
        let v = InvariantVerdict::new("i".to_owned(), InvariantOutcome::Held);
        assert_eq!(v.grade, Grade::Checked);
        let v = InvariantVerdict::new(
            "i".to_owned(),
            InvariantOutcome::Violated {
                trial: 2,
                detail: "broke".to_owned(),
            },
        );
        assert_eq!(v.grade, Grade::Checked);
        assert!(!v.held());
    }

    #[test]
    fn estimate_matches_hand_computation() {
        // values 1,1,1,1 over 4 trials: mean 1, variance 0, half-width 0.
        let m = MetricEstimate::from_counts("r".to_owned(), 4, 4, 4);
        assert!((m.mean - 1.0).abs() < 1e-12);
        assert!(m.half_width.abs() < 1e-12);
        // no trials: degenerate but total.
        let z = MetricEstimate::from_counts("r".to_owned(), 0, 0, 0);
        assert!(z.mean.abs() < 1e-12);
        assert!(z.half_width.abs() < 1e-12);
    }

    #[test]
    fn digest_is_stable_and_content_sensitive() {
        let a = MetricEstimate::from_counts("r".to_owned(), 3, 1, 1);
        let b = MetricEstimate::from_counts("r".to_owned(), 3, 1, 1);
        let c = MetricEstimate::from_counts("r".to_owned(), 3, 2, 2);
        assert_eq!(digest_json(&a), digest_json(&b));
        assert_ne!(digest_json(&a), digest_json(&c));
    }

    fn honest_manifest() -> RunManifest {
        RunManifest {
            battery_digest: "fnv1a64:0000000000000000".to_owned(),
            seed: 1,
            trials: 3,
            metrics: vec![MetricEstimate::from_counts("rate".to_owned(), 3, 1, 1)],
            invariants: vec![InvariantVerdict::new(
                "holds".to_owned(),
                InvariantOutcome::Held,
            )],
            checkpoint_lineage: vec![3],
        }
    }

    #[test]
    fn honest_manifest_verifies_and_round_trips() {
        let manifest = honest_manifest();
        assert!(manifest.verify().is_ok());
        let json = serde_json::to_string(&manifest).unwrap();
        let back: RunManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, back);
    }

    #[test]
    fn a_forged_deserialized_manifest_is_rejected() {
        // Struct-literal forgery is a compile error now (the grade fields are
        // private), so the hostile vector is a tampered wire record. A metric
        // restamped `checked` — the grade a metric must never carry — has to be
        // rejected at deserialize time, in release, not merely in a debug assert.
        let json = serde_json::to_string(&honest_manifest()).unwrap();
        let forged = json.replacen("\"grade\":\"tested\"", "\"grade\":\"checked\"", 1);
        assert_ne!(
            forged, json,
            "the tamper must actually restamp the metric grade"
        );

        let result = serde_json::from_str::<RunManifest>(&forged);
        assert!(
            result.is_err(),
            "a forged manifest (metric stamped checked) must be rejected: {result:?}"
        );
    }
}
