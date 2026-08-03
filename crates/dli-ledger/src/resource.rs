//! The closed resource algebra.
//!
//! A practice spends resources drawn from a fixed, small algebra
//! (`docs/spec/04-practices.md` §2) — nothing else is a resource. This module
//! makes that algebra *real*: [`ResourceKind`] is the closed law tag, and the
//! semantics functions detect the violation each law forbids, as pure folds over
//! a sequence of signed [`Delta`]s.
//!
//! The four members of the algebra and the object-level discipline each imposes
//! on a run of deltas applied to a running balance that starts at zero (a
//! positive delta acquires/reveals/releases a unit; a negative delta spends one):
//!
//! | Surface | [`ResourceKind`] | Object-level law |
//! |---|---|---|
//! | `linear` | `Linear { conserved: false }` | the balance never goes negative — no unit is spent that was not first accounted |
//! | `linear conserved` | `Linear { conserved: true }` | additionally the multiset balances: the net of every delta is zero across the close, so no step mints or burns |
//! | `affine bounded <cap>` | [`ResourceKind::AffineBounded`] | the balance stays within `[0, cap]` — overspend past the cap is a failure, underspend is legal |
//! | `graded monotone <order>` | [`ResourceKind::GradedMonotone`] | every delta moves the grade in the one declared [`GradeDirection`]; nothing is spent, the grade only wears |
//!
//! These are the object language (charter koan 6 / spec §5): a debt token spent
//! twice inside a casting is an in-world error, entirely separate from the
//! annotation algebra that governs which authorial events support a fact. The
//! cap of an affine resource and the direction of a graded one are parameters
//! the caller supplies — the practice declaration records them (see
//! [`crate::practice::ResourceDecl`]); the kind tag alone names the law.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::ident::Ident;

// The closed-algebra tag `ResourceKind` and its `GradeDirection` moved to
// `dli-ast` (LS-9) so the front-end producer (`dli_syntax::structure_practice_body`)
// and this ledger reader share them without a dependency cycle. The *semantics*
// that give the tags meaning — the folds below — stay here. Re-exported so
// `dli_ledger::resource::{ResourceKind, GradeDirection}` paths keep resolving.
pub use dli_ast::practice::{GradeDirection, ResourceKind};

/// A single resource change.
//
// The sign is the object-level convention shared by the whole algebra: a
// positive amount adds units (acquire, reveal, release into the account), a
// negative amount removes them (spend, wear). The interpretation of the running
// total is what the resource's `ResourceKind` fixes. (This note stays out of the
// doc comment: the one-line description is embedded verbatim in the committed
// `check-draft` contract schema, and must not drift.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Delta {
    /// The resource affected.
    pub resource: Ident,
    /// The signed change amount.
    pub amount: i64,
}

/// A detected breach of a resource's law: which delta broke it, and how.
///
/// This is the object-level analogue of a load finding — a two-channel error
/// (charter koan 9): a stable machine [`code`](ResourceViolation::code) and a
/// domain-spoken [`message`](ResourceViolation::message). It is the raw signal
/// the ledger checker (PRAC-2) lifts into an `L6 resource-violation` verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceViolation {
    /// The index of the offending delta in the applied sequence.
    pub at: usize,
    /// The running balance (or, for a graded resource, the signed grade step)
    /// after the offending delta was applied.
    pub balance: i64,
    /// A stable, machine-readable class for the breach.
    pub code: ResourceViolationCode,
    /// A domain-spoken description.
    pub message: String,
}

/// The stable class of a [`ResourceViolation`] (charter koan 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceViolationCode {
    /// A linear or affine balance fell below zero — a unit was spent that was
    /// never accounted (over-spend below the floor).
    Overspent,
    /// An affine balance rose above its declared capacity — more was held than
    /// the cap allows.
    OverCapacity,
    /// A `linear conserved` multiset did not net to zero at the close — a step
    /// minted or burned the resource.
    NotConserved,
    /// A graded-monotone grade moved against its one declared direction.
    WrongDirection,
}

impl ResourceViolationCode {
    /// The stable machine string this code serializes as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overspent => "overspent",
            Self::OverCapacity => "over-capacity",
            Self::NotConserved => "not-conserved",
            Self::WrongDirection => "wrong-direction",
        }
    }
}

/// Check a `linear` resource: the running balance must never go negative.
///
/// A plain linear resource accounts for every unit — you cannot spend one you
/// have not first acquired. Underspend (a positive closing balance) is legal;
/// only dipping below zero is a breach. With `conserved`, the multiset must
/// additionally net to zero at the close: see [`check_conserved`].
///
/// ```
/// use dli_ledger::resource::{check_linear, Delta, ResourceViolationCode};
/// use dli_ast::ident::Ident;
///
/// let d = |n: i64| Delta { resource: Ident::synthetic("debt"), amount: n };
/// // Acquire two, spend two: fine.
/// assert!(check_linear(&[d(1), d(1), d(-1), d(-1)]).is_none());
/// // Spend before acquiring: over-spent at delta 0.
/// let v = check_linear(&[d(-1)]).unwrap();
/// assert_eq!(v.at, 0);
/// assert_eq!(v.code, ResourceViolationCode::Overspent);
/// ```
#[must_use]
pub fn check_linear(deltas: &[Delta]) -> Option<ResourceViolation> {
    floor_scan(deltas)
}

/// Check a `linear conserved` resource: never-negative *and* net-zero at close.
///
/// The never-negative discipline is checked first (a conserved unit cannot be
/// redistributed before it is revealed); then the closing balance must be zero,
/// or a step minted or burned the resource and the ledger does not check
/// (spec §2, §5).
///
/// ```
/// use dli_ledger::resource::{check_conserved, Delta, ResourceViolationCode};
/// use dli_ast::ident::Ident;
///
/// let d = |n: i64| Delta { resource: Ident::synthetic("debt"), amount: n };
/// // Reveal one, release it: balances.
/// assert!(check_conserved(&[d(1), d(-1)]).is_none());
/// // Reveal one and keep it: minted, not conserved.
/// let v = check_conserved(&[d(1)]).unwrap();
/// assert_eq!(v.code, ResourceViolationCode::NotConserved);
/// ```
#[must_use]
pub fn check_conserved(deltas: &[Delta]) -> Option<ResourceViolation> {
    if let Some(v) = floor_scan(deltas) {
        return Some(v);
    }
    let net: i64 = deltas.iter().map(|d| d.amount).sum();
    if net != 0 {
        return Some(ResourceViolation {
            at: deltas.len().saturating_sub(1),
            balance: net,
            code: ResourceViolationCode::NotConserved,
            message: format!(
                "the conserved multiset does not balance at the close: net {net} \
                 (a step minted or burned the resource)"
            ),
        });
    }
    None
}

/// Check an `affine bounded <cap>` resource: the balance stays within `[0, cap]`.
///
/// Overspend past the floor is [`Overspent`]; rising above the cap is
/// [`OverCapacity`]. Underspend is legal — an affine resource need not be spent
/// to the cap. A negative `cap` is treated as zero (no capacity).
///
/// [`Overspent`]: ResourceViolationCode::Overspent
/// [`OverCapacity`]: ResourceViolationCode::OverCapacity
///
/// ```
/// use dli_ledger::resource::{check_affine_bounded, Delta, ResourceViolationCode};
/// use dli_ast::ident::Ident;
///
/// let d = |n: i64| Delta { resource: Ident::synthetic("spread"), amount: n };
/// // Lay up to nine leaves: within the bound.
/// assert!(check_affine_bounded(&[d(9)], 9).is_none());
/// // A tenth leaf overspreads the affine bound.
/// let v = check_affine_bounded(&[d(9), d(1)], 9).unwrap();
/// assert_eq!(v.at, 1);
/// assert_eq!(v.code, ResourceViolationCode::OverCapacity);
/// ```
#[must_use]
pub fn check_affine_bounded(deltas: &[Delta], cap: i64) -> Option<ResourceViolation> {
    let cap = cap.max(0);
    let mut balance: i64 = 0;
    for (at, delta) in deltas.iter().enumerate() {
        balance += delta.amount;
        if balance < 0 {
            return Some(ResourceViolation {
                at,
                balance,
                code: ResourceViolationCode::Overspent,
                message: format!(
                    "the affine balance fell to {balance} below zero at step {at} \
                     — a unit was spent that was never held"
                ),
            });
        }
        if balance > cap {
            return Some(ResourceViolation {
                at,
                balance,
                code: ResourceViolationCode::OverCapacity,
                message: format!(
                    "the affine balance rose to {balance} above the cap of {cap} \
                     at step {at} — overspread past the declared capacity"
                ),
            });
        }
    }
    None
}

/// Check a `graded monotone <order>` resource: every step moves the one way.
///
/// Nothing is spent; the grade wears. A [`GradeDirection::Decays`] grade admits
/// only non-positive deltas (it may only slip down its order); a
/// [`GradeDirection::Grows`] grade admits only non-negative deltas. A step
/// against the grain is [`WrongDirection`]. A zero delta is inert and legal in
/// either direction.
///
/// [`WrongDirection`]: ResourceViolationCode::WrongDirection
///
/// ```
/// use dli_ledger::resource::{check_graded_monotone, Delta, GradeDirection, ResourceViolationCode};
/// use dli_ast::ident::Ident;
///
/// let d = |n: i64| Delta { resource: Ident::synthetic("acquaintance"), amount: n };
/// // Speaking a name wears it down twice: monotone decay.
/// assert!(check_graded_monotone(&[d(-1), d(-1)], GradeDirection::Decays).is_none());
/// // A worn name cannot un-wear: moving up is against the grain.
/// let v = check_graded_monotone(&[d(-1), d(1)], GradeDirection::Decays).unwrap();
/// assert_eq!(v.at, 1);
/// assert_eq!(v.code, ResourceViolationCode::WrongDirection);
/// ```
#[must_use]
pub fn check_graded_monotone(
    deltas: &[Delta],
    direction: GradeDirection,
) -> Option<ResourceViolation> {
    for (at, delta) in deltas.iter().enumerate() {
        let against = match direction {
            GradeDirection::Decays => delta.amount > 0,
            GradeDirection::Grows => delta.amount < 0,
        };
        if against {
            let way = match direction {
                GradeDirection::Decays => "wears down",
                GradeDirection::Grows => "ripens up",
            };
            return Some(ResourceViolation {
                at,
                balance: delta.amount,
                code: ResourceViolationCode::WrongDirection,
                message: format!(
                    "the graded resource only {way} its order, but step {at} \
                     moved it {} — against the declared direction",
                    if delta.amount > 0 { "up" } else { "down" }
                ),
            });
        }
    }
    None
}

/// The shared never-negative fold used by the linear and conserved laws.
fn floor_scan(deltas: &[Delta]) -> Option<ResourceViolation> {
    let mut balance: i64 = 0;
    for (at, delta) in deltas.iter().enumerate() {
        balance += delta.amount;
        if balance < 0 {
            return Some(ResourceViolation {
                at,
                balance,
                code: ResourceViolationCode::Overspent,
                message: format!(
                    "the balance fell to {balance} below zero at step {at} \
                     — a unit was spent that was never accounted"
                ),
            });
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn d(n: i64) -> Delta {
        Delta {
            resource: Ident::synthetic("r"),
            amount: n,
        }
    }

    #[test]
    fn linear_allows_underspend_and_balanced_flows() {
        assert!(check_linear(&[]).is_none());
        assert!(check_linear(&[d(3), d(-1)]).is_none()); // underspend is legal
        assert!(check_linear(&[d(1), d(-1), d(1), d(-1)]).is_none());
    }

    #[test]
    fn linear_flags_the_first_dip_below_zero() {
        let v = check_linear(&[d(1), d(-1), d(-1)]).unwrap();
        assert_eq!(v.at, 2);
        assert_eq!(v.balance, -1);
        assert_eq!(v.code, ResourceViolationCode::Overspent);
    }

    #[test]
    fn conserved_requires_net_zero_on_top_of_the_floor() {
        assert!(check_conserved(&[d(1), d(1), d(-1), d(-1)]).is_none());
        // Balanced flow that never dips but ends non-zero: minted.
        let v = check_conserved(&[d(2), d(-1)]).unwrap();
        assert_eq!(v.code, ResourceViolationCode::NotConserved);
        assert_eq!(v.balance, 1);
        // A dip below zero is caught as over-spend before conservation is judged.
        let v = check_conserved(&[d(-1), d(1)]).unwrap();
        assert_eq!(v.code, ResourceViolationCode::Overspent);
    }

    #[test]
    fn affine_bounds_both_ends() {
        assert!(check_affine_bounded(&[d(1), d(1), d(1)], 9).is_none());
        assert!(check_affine_bounded(&[d(9), d(-9)], 9).is_none());
        // Below the floor.
        let v = check_affine_bounded(&[d(-1)], 9).unwrap();
        assert_eq!(v.code, ResourceViolationCode::Overspent);
        // Above the cap.
        let v = check_affine_bounded(&[d(10)], 9).unwrap();
        assert_eq!(v.code, ResourceViolationCode::OverCapacity);
        assert_eq!(v.balance, 10);
        // A negative cap collapses to zero capacity.
        assert!(check_affine_bounded(&[d(1)], -3).is_some());
    }

    #[test]
    fn graded_monotone_admits_only_its_direction() {
        assert!(check_graded_monotone(&[d(-1), d(-1), d(0)], GradeDirection::Decays).is_none());
        assert!(check_graded_monotone(&[d(1), d(2)], GradeDirection::Grows).is_none());
        let v = check_graded_monotone(&[d(1)], GradeDirection::Decays).unwrap();
        assert_eq!(v.code, ResourceViolationCode::WrongDirection);
        let v = check_graded_monotone(&[d(0), d(-1)], GradeDirection::Grows).unwrap();
        assert_eq!(v.at, 1);
        assert_eq!(v.code, ResourceViolationCode::WrongDirection);
    }

    #[test]
    fn kind_phrases_are_stable() {
        assert_eq!(ResourceKind::Linear { conserved: false }.phrase(), "linear");
        assert_eq!(
            ResourceKind::Linear { conserved: true }.phrase(),
            "linear conserved"
        );
        assert_eq!(ResourceKind::AffineBounded.phrase(), "affine bounded");
        assert_eq!(ResourceKind::GradedMonotone.phrase(), "graded monotone");
    }
}
