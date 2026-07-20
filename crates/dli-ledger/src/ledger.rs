//! The ledger-checking seam (PRAC-2).
//!
//! A **ledger** is one casting written out for the checker: its premises, its
//! steps, its spends, and its close, run `in <practice>`
//! (`docs/spec/04-practices.md` §5). [`check_ledger`] will replay the recorded
//! [`Delta`]s against each named practice's resource law and match the close
//! against its declared expectation.
//!
//! Only the seam types are frozen here; the replay itself lands in PRAC-2
//! (wave 3), which consumes the [`PracticeDef`]
//! and the resource semantics this crate already exposes. When it lands it will
//! project the world as of the scene (certain ∩ known-by-speaker ∩ present) to
//! select premises; that world handle is deliberately absent from the stub so
//! this crate stays free of the evaluator until the checker needs it.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::ident::Ident;
use dli_core::grade::Grade;

use crate::practice::PracticeDef;
use crate::resource::{
    Delta, ResourceKind, check_affine_bounded, check_conserved, check_graded_monotone, check_linear,
};

/// One ledger entry: a practice invocation and the deltas it applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerEntry {
    /// The practice invoked.
    pub practice: Ident,
    /// The resource changes it applied.
    pub deltas: Vec<Delta>,
}

/// A single detected law violation.
//
// The one-line description is embedded verbatim in the committed `check-ledger`
// contract schema; keep it byte-identical so `xtask schema --check` stays green.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerViolation {
    /// The index of the offending entry.
    pub entry: usize,
    /// A human-readable description.
    pub message: String,
    /// The grade at which the violation is asserted.
    pub grade: Grade,
}

/// The result of checking a ledger: the violations found, if any.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerReport {
    /// Every law violation detected, in entry order.
    pub violations: Vec<LedgerViolation>,
}

/// Replay a ledger against a set of practices and report law violations.
///
/// For each entry, look up its practice by name, group the recorded deltas by
/// resource, and run each resource's full delta sequence through its
/// [`ResourceKind`] law using the semantics in [`crate::resource`], lifting every
/// [`ResourceViolation`] into a [`LedgerViolation`] graded [`Grade::Checked`] (a
/// replay re-derives exactly). An entry naming an unknown practice, or a delta on
/// a resource the practice does not declare, has no law to run and is passed over.
///
/// This is the resource-law replay over *recorded* deltas — the object-level
/// bookkeeping that needs no world handle, so a battery (SIM-1) can drive it
/// per-trial. The scene-aware proof checker that projects the premise pool and
/// grades a whole casting is [`crate::ledger_check::check_proof`].
///
/// An `affine bounded <cap>` resource whose cap is *symbolic* (e.g.
/// `body-limit(caster)`) cannot be bounded without a scene; its affine check is
/// deferred here (never guessed) and belongs to `check_proof`, which resolves the
/// cap from the [`SceneProjection`](crate::premise::SceneProjection). An integer
/// cap (`affine bounded 9`) is checked directly.
///
/// [`ResourceKind`]: crate::resource::ResourceKind
/// [`ResourceViolation`]: crate::resource::ResourceViolation
#[must_use]
pub fn check_ledger(practices: &[PracticeDef], entries: &[LedgerEntry]) -> LedgerReport {
    let mut violations = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let Some(practice) = practices
            .iter()
            .find(|p| p.name.text == entry.practice.text)
        else {
            continue;
        };
        // Group by resource, preserving each resource's delta order.
        let mut by_resource: BTreeMap<&str, Vec<Delta>> = BTreeMap::new();
        for delta in &entry.deltas {
            by_resource
                .entry(delta.resource.text.as_str())
                .or_default()
                .push(delta.clone());
        }
        for (name, deltas) in &by_resource {
            let Some(decl) = practice.resources.iter().find(|r| &r.name.text == name) else {
                continue;
            };
            let Some(kind) = &decl.kind else { continue };
            let violation = match kind {
                ResourceKind::Linear { conserved: false } => check_linear(deltas),
                ResourceKind::Linear { conserved: true } => check_conserved(deltas),
                ResourceKind::GradedMonotone => check_graded_monotone(deltas, decl.direction),
                ResourceKind::AffineBounded => match cap_int(decl.cap.as_deref()) {
                    Some(cap) => check_affine_bounded(deltas, cap),
                    None => None,
                },
            };
            if let Some(v) = violation {
                violations.push(LedgerViolation {
                    entry: index,
                    message: v.message,
                    grade: Grade::Checked,
                });
            }
        }
    }
    LedgerReport { violations }
}

/// Extract an integer affine cap from a declared cap phrase — `9` or `9 leaves`
/// resolve; a symbolic `body-limit(caster)` does not.
fn cap_int(cap: Option<&str>) -> Option<i64> {
    cap?.split_whitespace().find_map(|w| w.parse::<i64>().ok())
}
