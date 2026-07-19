//! Annotations: the paired support and defeater polynomials, and the standing
//! they *evaluate* to.
//!
//! An [`Ann`] is `(P⁺, P⁻)` — a claim's support and its defeater polynomial.
//! No atom stores a status; standing is derived from the annotation under a
//! valuation `ν` and a constraint theory `Θ`, always. [`Status`] is that
//! derivation. Grade rides the same support polynomial on an orthogonal
//! channel and is computed elsewhere.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::atom::Atom;
use crate::posbool::{Monomial, PosBool};
use crate::valuation::Valuation;

/// A small constraint theory over event variables: the side conditions the
/// support algebra cannot see on its own.
///
/// For now it carries option-exclusivity families — the options of one open
/// question cannot both be ruled true, so no possible monomial may rest on two
/// of them at once. Supersession links and the negative-constraint store live
/// outside this type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Theory {
    /// Mutually-exclusive families of event variables. At most one member of a
    /// family may hold in any admissible world.
    pub exclusive: Vec<BTreeSet<Atom>>,
}

impl Theory {
    /// The empty theory: no exclusivity constraints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a family whose members are mutually exclusive (e.g. the options
    /// of one question). Families of fewer than two atoms constrain nothing and
    /// are dropped.
    pub fn add_exclusive(&mut self, family: impl IntoIterator<Item = Atom>) {
        let family: BTreeSet<Atom> = family.into_iter().collect();
        if family.len() >= 2 {
            self.exclusive.push(family);
        }
    }

    /// Whether a conjunction respects exclusivity: it does not assert two
    /// positive literals drawn from the same exclusivity family.
    #[must_use]
    pub fn respects(&self, monomial: &Monomial) -> bool {
        let positives: BTreeSet<Atom> = monomial
            .lits()
            .filter(|lit| lit.rail == crate::atom::Rail::Pos)
            .map(|lit| lit.atom)
            .collect();
        !self
            .exclusive
            .iter()
            .any(|family| family.intersection(&positives).nth(1).is_some())
    }
}

/// A claim's three-valued standing under `(ν, Θ)`, derived — never stored.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Every monomial is `Θ`-unsatisfiable: the claim holds in no admissible
    /// world. (Whether this is rejection or mere supersession is a `C`
    /// question, outside the algebra.)
    Dead,
    /// Some monomial is `Θ`-satisfiable but none is resolved: the shadow layer.
    Possible,
    /// Some monomial holds on resolved events alone, with no fully-ruled
    /// defeater: assertable.
    Certain,
}

/// The annotation of an atom or rule: its support and defeater polynomials.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ann {
    /// `P⁺` — the support: the events under which the claim holds.
    pub support: PosBool,
    /// `P⁻` — the defeater polynomial: the events under which a default the
    /// claim rests on is overridden.
    pub defeaters: PosBool,
}

impl Ann {
    /// An annotation with the given support and no defeaters.
    #[must_use]
    pub fn supported(support: PosBool) -> Self {
        Self {
            support,
            defeaters: PosBool::FALSE,
        }
    }

    /// An unconditional annotation: support is identically true, no defeaters.
    #[must_use]
    pub fn unconditional() -> Self {
        Self::supported(PosBool::truth())
    }

    /// An annotation with explicit support and defeater polynomials.
    #[must_use]
    pub fn new(support: PosBool, defeaters: PosBool) -> Self {
        Self { support, defeaters }
    }

    /// The claim's standing under a valuation and constraint theory.
    ///
    /// Certainty needs a monomial supported by resolved events alone (open
    /// variables never carry it) *and* no fully-ruled defeater. Possibility
    /// needs a `Θ`-satisfiable monomial. Otherwise the claim is dead.
    ///
    /// ```
    /// use dli_core::ann::{Ann, Status, Theory};
    /// use dli_core::atom::Atom;
    /// use dli_core::posbool::PosBool;
    /// use dli_core::valuation::Valuation;
    /// let theory = Theory::new();
    /// let ann = Ann::supported(PosBool::pos(Atom(0))); // holds when event 0 is ruled
    /// // Event 0 still open: the claim is a shadow, not assertable.
    /// assert_eq!(ann.status(&Valuation::new(), &theory), Status::Possible);
    /// // Rule event 0 true and it becomes certain — no re-derivation needed.
    /// let mut nu = Valuation::new();
    /// nu.set(Atom(0), true);
    /// assert_eq!(ann.status(&nu, &theory), Status::Certain);
    /// ```
    #[must_use]
    pub fn status(&self, valuation: &Valuation, theory: &Theory) -> Status {
        let possible = self
            .support
            .monomials()
            .any(|m| monomial_possible(m, valuation, theory));
        if !possible {
            return Status::Dead;
        }
        let certain = self
            .support
            .monomials()
            .any(|m| monomial_certain(m, valuation, theory));
        let defeated = self
            .defeaters
            .monomials()
            .any(|m| monomial_certain(m, valuation, theory));
        if certain && !defeated {
            Status::Certain
        } else {
            Status::Possible
        }
    }

    /// Whether the claim is certain yet carries a still-live defeater —
    /// *"assertable, but a pending ruling could defeat it."* False unless the
    /// claim is [`Status::Certain`].
    #[must_use]
    pub fn at_risk(&self, valuation: &Valuation, theory: &Theory) -> bool {
        self.status(valuation, theory) == Status::Certain
            && self
                .defeaters
                .monomials()
                .any(|m| monomial_possible(m, valuation, theory))
    }
}

/// Whether a monomial can hold in some admissible world: not contradictory, no
/// literal resolved against it, and no two positive literals from one
/// exclusivity family.
fn monomial_possible(monomial: &Monomial, valuation: &Valuation, theory: &Theory) -> bool {
    !monomial.is_contradictory() && monomial.is_live_under(valuation) && theory.respects(monomial)
}

/// Whether a monomial holds on resolved events alone and respects exclusivity —
/// the per-monomial certainty test.
fn monomial_certain(monomial: &Monomial, valuation: &Valuation, theory: &Theory) -> bool {
    monomial.is_certain_under(valuation) && theory.respects(monomial)
}
