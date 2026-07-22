//! Batteries: a declared practice, a bounded seeded generator, and the metrics
//! and invariants to grade over it.
//!
//! A [`Battery`] points the one generic simulator at one declared practice
//! (`docs/spec/04-practices.md` §5). It draws bounded scenes from a seed, applies
//! each declared resource's law with the *frozen* PRAC-1 semantics
//! ([`dli_ledger::resource`]), and reduces the run two ways at once: **metrics**
//! (Monte-Carlo aggregations, graded `tested`) and **invariants** (exact
//! bookkeeping predicates, graded `checked`). The trial loop and the resumable
//! state live in [`crate::checkpoint`]; this module owns the *spec* and the
//! per-trial *semantics* — [`Battery::generate_trial`] and
//! [`Battery::account_trial`] are public so a test can re-derive the exact same
//! trials and tally them independently.
//!
//! The generator is deliberately *law-agnostic*: it walks the practice's declared
//! resources and emits bounded, arbitrary-sign delta sequences — legal castings
//! and illegal ones alike. The practice's laws are what the `check_*` functions
//! enforce over that space; a conserved resource fed an unbalanced sequence is
//! exactly the "breakable conservation" an invariant sweep must catch.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ledger::practice::{PracticeDef, ResourceDecl};
use dli_ledger::resource::{
    Delta, ResourceKind, ResourceViolationCode, check_affine_bounded, check_conserved,
    check_graded_monotone, check_linear,
};

use crate::manifest::digest_json;
use crate::rng::{SplitMix64, stream_value};

/// The bounds a battery's generator draws within — the "bounded" in a bounded
/// seeded generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GenBounds {
    /// The most resource deltas a single trial may draw for one resource; each
    /// trial draws a count uniformly in `[0, max_steps]`.
    pub max_steps: u32,
    /// The largest magnitude of a single delta; each step is drawn uniformly in
    /// `[-max_magnitude, +max_magnitude]`.
    pub max_magnitude: u32,
}

/// A per-trial scalar the simulator can observe, for a metric aggregation.
///
/// Each observable maps a trial's [`TrialOutcome`] to an integer sample (a rate
/// observable yields `0`/`1`); [`Observable::observe`] returns [`None`] only when
/// the observable names a resource the practice does not declare, which
/// [`Battery::validate`] surfaces as a configuration finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "observe", rename_all = "kebab-case")]
pub enum Observable {
    /// `1` if the named resource's law was violated this trial, else `0`.
    Violated {
        /// The resource observed.
        resource: String,
    },
    /// `1` if any accounted resource was violated this trial, else `0`.
    AnyViolation,
    /// The number of accounted resources violated this trial.
    ViolationCount,
    /// The named resource's closing balance (net of its deltas) this trial.
    ClosingBalance {
        /// The resource observed.
        resource: String,
    },
    /// The units spent (summed magnitude of negative deltas) on the named
    /// resource this trial.
    Spent {
        /// The resource observed.
        resource: String,
    },
}

impl Observable {
    /// The resource this observable names, if any.
    #[must_use]
    pub fn resource(&self) -> Option<&str> {
        match self {
            Self::Violated { resource }
            | Self::ClosingBalance { resource }
            | Self::Spent { resource } => Some(resource),
            Self::AnyViolation | Self::ViolationCount => None,
        }
    }

    /// Observe this trial's outcome; [`None`] iff a named resource is undeclared.
    #[must_use]
    pub fn observe(&self, outcome: &TrialOutcome) -> Option<i64> {
        match self {
            Self::Violated { resource } => {
                outcome.resource(resource).map(|r| i64::from(r.violated()))
            }
            Self::AnyViolation => Some(i64::from(outcome.any_violation())),
            Self::ViolationCount => Some(outcome.violation_count()),
            Self::ClosingBalance { resource } => {
                outcome.resource(resource).map(|r| r.closing_balance)
            }
            Self::Spent { resource } => outcome.resource(resource).map(|r| r.spent),
        }
    }
}

/// A declared Monte-Carlo metric: a name and the per-trial observable it means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Metric {
    /// The metric's name, carried onto its estimate.
    pub name: String,
    /// The per-trial value it aggregates.
    pub observable: Observable,
}

/// An exact bookkeeping predicate over a trial's run state.
///
/// Every variant is an exact accounting check — never a statistical one — so a
/// sweep of it over the run's trials grades [`checked`](dli_core::Grade::Checked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "invariant", rename_all = "kebab-case")]
pub enum Invariant {
    /// The named resource's law is never violated in any trial.
    NeverViolated {
        /// The resource guarded.
        resource: String,
    },
    /// No accounted resource is violated in any trial.
    AllLawsHold,
    /// The named resource's closing balance never exceeds `bound`.
    BalanceAtMost {
        /// The resource guarded.
        resource: String,
        /// The inclusive upper bound.
        bound: i64,
    },
    /// The named resource's closing balance never falls below `bound`.
    BalanceAtLeast {
        /// The resource guarded.
        resource: String,
        /// The inclusive lower bound.
        bound: i64,
    },
}

impl Invariant {
    /// The resource this invariant names, if any.
    #[must_use]
    pub fn resource(&self) -> Option<&str> {
        match self {
            Self::NeverViolated { resource }
            | Self::BalanceAtMost { resource, .. }
            | Self::BalanceAtLeast { resource, .. } => Some(resource),
            Self::AllLawsHold => None,
        }
    }

    /// Whether this trial *breaches* the invariant, with a domain-spoken reason.
    ///
    /// [`Some`] is a breach; [`None`] means the trial upholds it. An invariant
    /// naming an undeclared resource never breaches (the misconfiguration is a
    /// [`Battery::validate`] finding, not a false verdict).
    #[must_use]
    pub fn breach(&self, outcome: &TrialOutcome) -> Option<String> {
        match self {
            Self::NeverViolated { resource } => {
                let r = outcome.resource(resource)?;
                r.violation.map(|code| {
                    format!(
                        "resource '{resource}' violated its law ({}) at trial {}",
                        code.as_str(),
                        outcome.trial
                    )
                })
            }
            Self::AllLawsHold => outcome.resources.iter().find_map(|r| {
                r.violation.map(|code| {
                    format!(
                        "resource '{}' violated its law ({}) at trial {}",
                        r.resource,
                        code.as_str(),
                        outcome.trial
                    )
                })
            }),
            Self::BalanceAtMost { resource, bound } => {
                let r = outcome.resource(resource)?;
                (r.closing_balance > *bound).then(|| {
                    format!(
                        "resource '{resource}' closed at {} above the bound {bound} at trial {}",
                        r.closing_balance, outcome.trial
                    )
                })
            }
            Self::BalanceAtLeast { resource, bound } => {
                let r = outcome.resource(resource)?;
                (r.closing_balance < *bound).then(|| {
                    format!(
                        "resource '{resource}' closed at {} below the bound {bound} at trial {}",
                        r.closing_balance, outcome.trial
                    )
                })
            }
        }
    }
}

/// A named invariant to sweep over a battery run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvariantSpec {
    /// The invariant's name, carried onto its verdict.
    pub name: String,
    /// The exact predicate it means.
    pub predicate: Invariant,
}

/// A practice simulator battery: everything a run needs, as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Battery {
    /// The declared practice this battery exercises.
    pub practice: PracticeDef,
    /// The generator's bounds.
    pub bounds: GenBounds,
    /// The master seed every trial derives its sub-stream from.
    pub seed: u64,
    /// The number of trials to run.
    pub trials: u64,
    /// The Monte-Carlo metrics to estimate.
    pub metrics: Vec<Metric>,
    /// The exact invariants to sweep.
    pub invariants: Vec<InvariantSpec>,
    /// Concrete capacities for `affine bounded` resources whose declared cap is a
    /// symbolic expression (`body-limit(caster)`), keyed by **resource name**. A
    /// bare-integer cap in the practice declaration (`affine bounded 5`) resolves
    /// directly and needs no binding.
    ///
    /// This map is the engine's cap-binding intake: a binding here turns an
    /// otherwise-`Deferred` symbolic-capped law into an *enforced* (`checked`) one
    /// (`resolve_cap` consults it, `account_resource` applies the affine law, and
    /// the sweep reports real violations). An entry for
    /// a resource the practice does not declare as `affine bounded` is simply
    /// unused; a symbolic cap with no entry stays honestly `Deferred`. The CLI
    /// (`deloreis simulate`) is the intended writer — it should surface a flag
    /// (e.g. `--cap <resource>=<n>`) that populates this map; today it constructs
    /// the battery with an empty map, so symbolic caps defer until the flag lands.
    pub affine_caps: BTreeMap<String, i64>,
}

impl Battery {
    /// A `fnv1a64` digest of this battery's *semantics* — practice name and
    /// resource laws, bounds, seed, trials, and the metric and invariant specs —
    /// with source spans excluded so the digest tracks meaning, not whitespace.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_json(&BatteryDescriptor::of(self))
    }

    /// Configuration findings: a metric or invariant naming a resource the
    /// practice does not declare, or a battery with no work to do. An empty list
    /// is a well-formed battery. Nothing is silently dropped (charter koan 4).
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let declared: std::collections::BTreeSet<&str> = self
            .practice
            .resources
            .iter()
            .map(|r| r.name.text.as_str())
            .collect();
        let mut findings = Vec::new();
        for metric in &self.metrics {
            if let Some(res) = metric.observable.resource()
                && !declared.contains(res)
            {
                findings.push(format!(
                    "metric '{}' observes resource '{res}', which the practice does not declare",
                    metric.name
                ));
            }
        }
        for inv in &self.invariants {
            if let Some(res) = inv.predicate.resource()
                && !declared.contains(res)
            {
                findings.push(format!(
                    "invariant '{}' guards resource '{res}', which the practice does not declare",
                    inv.name
                ));
            }
        }
        // A declared law whose cap never resolves is unenforceable: its sweep is
        // deferred rather than checked, so name it here rather than let a
        // per-resource invariant read as a silent held pass (charter koan 4).
        for resource in &self.practice.resources {
            if matches!(resource.kind, Some(ResourceKind::AffineBounded))
                && self.resolve_cap(resource).is_none()
            {
                findings.push(format!(
                    "resource '{}' declares an affine bound whose cap does not resolve to a \
                     number (a symbolic cap with no binding in affine_caps); its law is deferred, \
                     not enforced",
                    resource.name.text
                ));
            }
        }
        findings
    }

    /// Generate trial `index`'s scene: for each declared resource, a bounded
    /// sequence of deltas drawn from that trial's sub-stream.
    ///
    /// The sub-stream is [`stream_value(seed, index)`](stream_value), so this is
    /// a pure function of `(seed, index)` — trial `k` draws the identical scene
    /// whether reached straight through or after a resume.
    #[must_use]
    pub fn generate_trial(&self, index: u64) -> TrialState {
        let mut prng = SplitMix64::new(stream_value(self.seed, index));
        let resources = self
            .practice
            .resources
            .iter()
            .map(|decl| {
                let steps = prng.below(u64::from(self.bounds.max_steps) + 1);
                let deltas = (0..steps)
                    .map(|_| Delta {
                        resource: decl.name.clone(),
                        amount: prng.delta_in(self.bounds.max_magnitude),
                    })
                    .collect();
                ResourceDraw {
                    resource: decl.name.text.clone(),
                    deltas,
                }
            })
            .collect();
        TrialState { index, resources }
    }

    /// Account a generated trial: run each resource's deltas through its declared
    /// law with the frozen PRAC-1 `check_*` semantics, yielding the run state.
    #[must_use]
    pub fn account_trial(&self, state: &TrialState) -> TrialOutcome {
        let resources = self
            .practice
            .resources
            .iter()
            .zip(&state.resources)
            .map(|(decl, draw)| self.account_resource(decl, &draw.deltas))
            .collect();
        TrialOutcome {
            trial: state.index,
            resources,
        }
    }

    fn account_resource(&self, decl: &ResourceDecl, deltas: &[Delta]) -> ResourceOutcome {
        let closing_balance = deltas.iter().map(|d| d.amount).sum();
        let spent = deltas
            .iter()
            .filter(|d| d.amount < 0)
            .map(|d| -d.amount)
            .sum();
        let (violation, accounted) = match decl.kind {
            Some(ResourceKind::Linear { conserved: false }) => {
                (check_linear(deltas).map(|v| v.code), true)
            }
            Some(ResourceKind::Linear { conserved: true }) => {
                (check_conserved(deltas).map(|v| v.code), true)
            }
            Some(ResourceKind::AffineBounded) => match self.resolve_cap(decl) {
                Some(cap) => (check_affine_bounded(deltas, cap).map(|v| v.code), true),
                None => (None, false),
            },
            Some(ResourceKind::GradedMonotone) => (
                check_graded_monotone(deltas, decl.direction).map(|v| v.code),
                true,
            ),
            None => (None, false),
        };
        ResourceOutcome {
            resource: decl.name.text.clone(),
            closing_balance,
            spent,
            violation,
            accounted,
        }
    }

    /// Resolve an `affine bounded` resource's cap to a concrete integer: a
    /// bare-integer cap in the declaration parses directly; otherwise the
    /// battery's [`affine_caps`](Battery::affine_caps) binding is consulted.
    fn resolve_cap(&self, decl: &ResourceDecl) -> Option<i64> {
        if let Some(cap) = &decl.cap
            && let Ok(value) = cap.trim().parse::<i64>()
        {
            return Some(value);
        }
        self.affine_caps.get(decl.name.text.as_str()).copied()
    }

    /// Why a resource's law could not be enforced this run, or [`None`] if it was.
    ///
    /// This is exactly the `accounted = false` condition of
    /// `account_resource`: an `affine bounded` resource
    /// whose symbolic cap never resolved to a number (no bare-integer cap and no
    /// [`affine_caps`](Battery::affine_caps) binding), or a resource whose declared
    /// law is outside the closed algebra. A law in either state was never applied,
    /// so a sweep of it has nothing to check.
    fn unenforced_reason(&self, decl: &ResourceDecl) -> Option<String> {
        match decl.kind {
            Some(ResourceKind::AffineBounded) if self.resolve_cap(decl).is_none() => Some(format!(
                "resource '{}' declares an affine bound whose cap did not resolve to a number \
                 (a symbolic cap with no binding), so its law was never enforced",
                decl.name.text
            )),
            None => Some(format!(
                "resource '{}' declares no law in the closed algebra, so nothing was enforced",
                decl.name.text
            )),
            _ => None,
        }
    }

    /// If sweeping `predicate` would report a *held* pass over a law that was never
    /// actually enforced, the reason it must instead be reported **deferred**.
    ///
    /// Only a [`NeverViolated`](Invariant::NeverViolated) sweep can be deferred: it
    /// reads the per-trial `violation`, which an unaccounted resource never sets,
    /// so it would silently read as held. The balance invariants read a closing
    /// balance that is always computed, and [`AllLawsHold`](Invariant::AllLawsHold)
    /// ranges only over the laws that *were* enforced — both are genuinely checked.
    /// Returns [`None`] for a genuinely-checkable law.
    #[must_use]
    pub fn deferred_reason(&self, predicate: &Invariant) -> Option<String> {
        let Invariant::NeverViolated { resource } = predicate else {
            return None;
        };
        let decl = self
            .practice
            .resources
            .iter()
            .find(|r| &r.name.text == resource)?;
        self.unenforced_reason(decl)
    }
}

/// A span-free projection of a battery, for a semantics-tracking digest.
#[derive(Serialize)]
struct BatteryDescriptor<'a> {
    practice: &'a str,
    resources: Vec<ResourceLaw<'a>>,
    bounds: GenBounds,
    seed: u64,
    trials: u64,
    metrics: &'a [Metric],
    invariants: &'a [InvariantSpec],
    affine_caps: &'a BTreeMap<String, i64>,
}

/// A span-free rendering of one resource's law.
#[derive(Serialize)]
struct ResourceLaw<'a> {
    name: &'a str,
    kind: &'a Option<ResourceKind>,
    cap: &'a Option<String>,
    direction: dli_ledger::resource::GradeDirection,
}

impl<'a> BatteryDescriptor<'a> {
    fn of(battery: &'a Battery) -> Self {
        Self {
            practice: &battery.practice.name.text,
            resources: battery
                .practice
                .resources
                .iter()
                .map(|r| ResourceLaw {
                    name: &r.name.text,
                    kind: &r.kind,
                    cap: &r.cap,
                    direction: r.direction,
                })
                .collect(),
            bounds: battery.bounds,
            seed: battery.seed,
            trials: battery.trials,
            metrics: &battery.metrics,
            invariants: &battery.invariants,
            affine_caps: &battery.affine_caps,
        }
    }
}

/// A generated trial: the deltas drawn for each declared resource, before laws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialState {
    /// The trial's index (its stream position).
    pub index: u64,
    /// The drawn deltas, one entry per declared resource, in declaration order.
    pub resources: Vec<ResourceDraw>,
}

/// The deltas drawn for one resource in a trial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDraw {
    /// The resource name.
    pub resource: String,
    /// The drawn deltas, in order.
    pub deltas: Vec<Delta>,
}

/// A trial's run state after accounting: the outcome per declared resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialOutcome {
    /// The trial's index.
    pub trial: u64,
    /// The per-resource outcomes, in declaration order.
    pub resources: Vec<ResourceOutcome>,
}

impl TrialOutcome {
    /// The outcome for a named resource, if the practice declares it.
    #[must_use]
    pub fn resource(&self, name: &str) -> Option<&ResourceOutcome> {
        self.resources.iter().find(|r| r.resource == name)
    }

    /// Whether any accounted resource was violated this trial.
    #[must_use]
    pub fn any_violation(&self) -> bool {
        self.resources.iter().any(ResourceOutcome::violated)
    }

    /// How many accounted resources were violated this trial.
    #[must_use]
    pub fn violation_count(&self) -> i64 {
        self.resources
            .iter()
            .filter(|r| r.violated())
            .count()
            .try_into()
            .unwrap_or(i64::MAX)
    }
}

/// One resource's accounting outcome in a trial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceOutcome {
    /// The resource name.
    pub resource: String,
    /// The net of the trial's deltas (the closing balance).
    pub closing_balance: i64,
    /// The units spent (summed magnitude of negative deltas).
    pub spent: i64,
    /// The law violation code, if the resource's law was broken.
    pub violation: Option<ResourceViolationCode>,
    /// Whether the resource's law could be applied (a resolvable cap and a law in
    /// the closed algebra); an unaccounted resource is never reported violated.
    pub accounted: bool,
}

impl ResourceOutcome {
    /// Whether this resource's law was violated.
    #[must_use]
    pub const fn violated(&self) -> bool {
        self.violation.is_some()
    }
}
