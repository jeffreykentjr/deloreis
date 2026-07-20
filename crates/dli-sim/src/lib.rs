//! Deloreis Lore Interchange practice simulator (`SIM-1`).
//!
//! Batteries over a declared practice with declared metrics and invariants,
//! checkpointing, and run manifests (`docs/spec/04-practices.md` §5,
//! `docs/spec/08-reasoning.md` T18, `docs/spec/10-serving.md` §5). One generic
//! simulator points at any declared practice — the same machine as the workbench
//! and the ledger checker, pointed at batteries instead of one casting
//! (charter koan 10).
//!
//! # The honesty distinction (the SIM-1 oracle)
//!
//! A run reports two kinds of result, and the kernel refuses to confuse them:
//!
//! - An **invariant** is an exact bookkeeping predicate. A run sweeps it over
//!   every trial and the claim "it held on all of them" re-derives *exactly* from
//!   the same seed and count — so an invariant verdict grades
//!   [`Checked`](dli_core::Grade::Checked), and a breach carries its witnessing
//!   trial. This is the "legality = sweep" half of spec T18.
//! - A **metric** is a Monte-Carlo estimate of a population quantity. The sample
//!   re-derives, but the *claim* is statistical, so a metric estimate grades
//!   [`Tested`](dli_core::Grade::Tested) — forever — and carries its counts and
//!   interval. This is the "metrics = MC, `tested` forever" half of spec T18.
//!
//! The distinction is enforced structurally: [`MetricEstimate`] and
//! [`InvariantVerdict`] stamp their grade from a type-level constant, and
//! [`RunManifest::grades_are_honest`] is the total predicate the runner asserts.
//! A metric can never report `checked`.
//!
//! # Determinism
//!
//! Seeds are inputs. Every random choice comes from a `u64` seed through a tiny
//! owned [`splitmix64`](rng) — no wall-clock, no `rand`. Trial `k`'s scene is a
//! pure function of `(seed, k)`, so a run replays byte-for-byte and a
//! [`checkpoint`]ed run [`resume`]s onto a byte-identical [`RunManifest`].
//!
//! # Example
//!
//! ```
//! use std::collections::BTreeMap;
//! use dli_sim::battery::{Battery, GenBounds, Invariant, InvariantSpec, Metric, Observable};
//! use dli_syntax::parse_document;
//! use dli_ledger::practice::compile_practices;
//!
//! // A practice with a conserved resource — its multiset must net to zero.
//! let (doc, _) = parse_document(
//!     "scope magic\n\
//!      practice spoken-proof proposed {\n\
//!        resource debt : linear conserved\n\
//!        cost epistemic\n\
//!      }\n",
//! );
//! let battery = Battery {
//!     practice: compile_practices(&doc).remove(0),
//!     bounds: GenBounds { max_steps: 4, max_magnitude: 3 },
//!     seed: 20260720,
//!     trials: 200,
//!     metrics: vec![Metric {
//!         name: "conservation-breach-rate".to_owned(),
//!         observable: Observable::Violated { resource: "debt".to_owned() },
//!     }],
//!     invariants: vec![InvariantSpec {
//!         name: "debt-always-conserved".to_owned(),
//!         predicate: Invariant::NeverViolated { resource: "debt".to_owned() },
//!     }],
//!     affine_caps: BTreeMap::new(),
//! };
//!
//! let manifest = battery.run();
//! assert!(manifest.grades_are_honest());
//!
//! // The metric is a Monte-Carlo estimate, graded `tested`, with counts.
//! use dli_core::Grade;
//! assert_eq!(manifest.metrics[0].grade(), Grade::Tested);
//! assert_eq!(manifest.metrics[0].trials, 200);
//!
//! // The invariant is an exact sweep, graded `checked` — and an unconstrained
//! // generator breaks conservation, so it is caught with a witnessing trial.
//! assert_eq!(manifest.invariants[0].grade(), Grade::Checked);
//! assert!(!manifest.invariants[0].held());
//! ```

pub mod battery;
pub mod checkpoint;
pub mod manifest;
pub mod rng;
pub mod turning;

pub use battery::{
    Battery, GenBounds, Invariant, InvariantSpec, Metric, Observable, ResourceDraw,
    ResourceOutcome, TrialOutcome, TrialState,
};
pub use checkpoint::{Breach, Checkpoint, InvariantAccum, MetricAccum, resume};
pub use manifest::{InvariantOutcome, InvariantVerdict, MetricEstimate, RunManifest, digest_json};
pub use rng::{SplitMix64, stream_value};
