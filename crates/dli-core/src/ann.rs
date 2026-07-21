//! Annotations: the paired support and defeater polynomials, and the standing
//! they *evaluate* to.
//!
//! An [`Ann`] is `(P⁺, P⁻)` — a claim's support and its defeater polynomial.
//! No atom stores a status; standing is derived from the annotation under a
//! valuation `ν` and a constraint theory `Θ`, always. [`Status`] is that
//! derivation. Grade rides the same support polynomial on an orthogonal
//! channel and is computed elsewhere.
//!
//! **The defeater polynomial is conditioned per support monomial.** A negated
//! premise poisons the *specific* derivation path it rode in on, not the atom as
//! a whole (`docs/spec/02-semantics.md §4`). [`DefeaterConditions`] pins each
//! poisoned support monomial to its own defeater; the atom-global
//! [`Ann::defeaters`] is kept as the OR of them all for blame and citation. A
//! head is certain iff **some support monomial is certain and that monomial's own
//! defeater is not** — so an independent clean support is never demoted by an
//! unrelated poison. The map is empty for every positive-only atom, so positive
//! datalog is byte-identical.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    /// Some monomial is `Θ`-satisfiable but none is resolved-and-undefeated: the
    /// shadow layer.
    Possible,
    /// Some monomial holds on resolved events alone, with that monomial's own
    /// defeater not fully ruled: assertable.
    Certain,
}

/// Per-support-monomial defeater conditions — the poison, conditioned per
/// derivation path.
///
/// A negated body literal poisons the *particular* support monomial its
/// derivation rested on (`docs/spec/02-semantics.md §4`), so the defeater must be
/// keyed by that monomial, never pooled atom-globally: pooling wrongly demotes an
/// atom that also has an independent, clean, certain support. Each entry pins one
/// support monomial to the defeater `P⁻` that rode in on its path; a monomial
/// absent from the map carries no defeater (`FALSE`).
///
/// **Empty for every positive-only atom.** No negated literal ever fires, so the
/// map stays empty and positive datalog behaves and serialises exactly as before
/// (the field is omitted from the wire form when empty).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefeaterConditions {
    by_monomial: BTreeMap<Monomial, PosBool>,
}

impl DefeaterConditions {
    /// No conditioned defeaters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether no support monomial carries a defeater — true for every
    /// positive-only atom.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_monomial.is_empty()
    }

    /// The defeater polynomial pinned to a support monomial, or `FALSE` when the
    /// monomial carries none (a clean derivation path).
    #[must_use]
    pub fn defeater_of(&self, monomial: &Monomial) -> &PosBool {
        static FALSE: PosBool = PosBool::FALSE;
        self.by_monomial.get(monomial).unwrap_or(&FALSE)
    }

    /// Record the defeater for a support monomial reached on one derivation path.
    /// A `FALSE` (clean) defeater records nothing.
    pub fn set(&mut self, monomial: Monomial, defeater: PosBool) {
        if !defeater.is_false() {
            self.by_monomial.insert(monomial, defeater);
        }
    }

    /// Drop the entry for a monomial (used when absorption removes it from the
    /// support).
    pub fn remove(&mut self, monomial: &Monomial) {
        self.by_monomial.remove(monomial);
    }

    /// The conditioned entries, in canonical monomial order.
    pub fn iter(&self) -> impl Iterator<Item = (&Monomial, &PosBool)> + '_ {
        self.by_monomial.iter()
    }

    /// The atom-global defeater polynomial: the OR of every path's defeater. This
    /// is the citation/blame view (`Ann::defeaters`) — it says *which* rulings
    /// could defeat the atom, discarding which path each poisons.
    #[must_use]
    pub fn atom_global(&self) -> PosBool {
        let mut acc = PosBool::FALSE;
        for defeater in self.by_monomial.values() {
            acc = acc.or_(defeater);
        }
        acc
    }
}

/// The wire form of one conditioned defeater: the poisoned support monomial (as a
/// one-monomial support polynomial) and the defeater pinned to it. Reusing
/// [`PosBool`] for `path` keeps the serialisation on the existing literal wire
/// grammar with no new tokens.
#[derive(Serialize, Deserialize, JsonSchema)]
struct CondPairWire {
    /// The poisoned support monomial, carried as a single-monomial polynomial.
    path: PosBool,
    /// The defeater polynomial `P⁻` pinned to that path.
    poison: PosBool,
}

impl Serialize for DefeaterConditions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let pairs: Vec<CondPairWire> = self
            .by_monomial
            .iter()
            .map(|(monomial, defeater)| CondPairWire {
                path: PosBool(std::iter::once(monomial.clone()).collect()),
                poison: defeater.clone(),
            })
            .collect();
        pairs.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DefeaterConditions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let pairs = Vec::<CondPairWire>::deserialize(deserializer)?;
        let mut by_monomial = BTreeMap::new();
        for pair in pairs {
            if let Some(monomial) = pair.path.monomials().next()
                && !pair.poison.is_false()
            {
                by_monomial.insert(monomial.clone(), pair.poison);
            }
        }
        Ok(DefeaterConditions { by_monomial })
    }
}

impl JsonSchema for DefeaterConditions {
    fn schema_name() -> Cow<'static, str> {
        "DefeaterConditions".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "array",
            "items": <CondPairWire as JsonSchema>::json_schema(generator),
        })
    }
}

/// The annotation of an atom or rule: its support and defeater polynomials.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ann {
    /// `P⁺` — the support: the events under which the claim holds.
    pub support: PosBool,
    /// `P⁻` — the atom-global defeater polynomial: the OR of every path's
    /// defeater, kept for blame and citation. Which support monomial each
    /// defeater poisons lives in [`Ann::conditions`].
    pub defeaters: PosBool,
    /// The defeater, conditioned per support monomial — the sound view that
    /// `status` reads. Empty (and omitted from the wire form) for every
    /// positive-only atom.
    #[serde(default, skip_serializing_if = "DefeaterConditions::is_empty")]
    pub conditions: DefeaterConditions,
}

impl Ann {
    /// An annotation with the given support and no defeaters.
    #[must_use]
    pub fn supported(support: PosBool) -> Self {
        Self {
            support,
            defeaters: PosBool::FALSE,
            conditions: DefeaterConditions::new(),
        }
    }

    /// An unconditional annotation: support is identically true, no defeaters.
    #[must_use]
    pub fn unconditional() -> Self {
        Self::supported(PosBool::truth())
    }

    /// An annotation from a support and an **atom-global** defeater polynomial,
    /// attaching that defeater uniformly to every support monomial.
    ///
    /// This is the flat, path-agnostic constructor. It is exact whenever the
    /// support is a single monomial (the common poison case); for a multi-monomial
    /// support with genuinely path-specific poison, build the annotation from the
    /// conditioned form ([`Ann::conditioned`]) so an independent clean monomial is
    /// not spuriously poisoned.
    #[must_use]
    pub fn new(support: PosBool, defeaters: PosBool) -> Self {
        let mut conditions = DefeaterConditions::new();
        if !defeaters.is_false() {
            for monomial in support.monomials() {
                conditions.set(monomial.clone(), defeaters.clone());
            }
        }
        Self {
            support,
            defeaters,
            conditions,
        }
    }

    /// An annotation from a support and its per-monomial defeater conditions. The
    /// atom-global [`Ann::defeaters`] is derived as the OR of the conditions.
    #[must_use]
    pub fn conditioned(support: PosBool, conditions: DefeaterConditions) -> Self {
        Self {
            defeaters: conditions.atom_global(),
            support,
            conditions,
        }
    }

    /// The claim's standing under a valuation and constraint theory.
    ///
    /// Certainty needs a support monomial that holds on resolved events alone
    /// (open variables never carry it) **and whose own defeater is not fully
    /// ruled**. Because the defeater is conditioned per monomial, an atom with an
    /// independent clean certain support stays certain even when an unrelated
    /// path's defeater is ruled true (`docs/spec/02-semantics.md §4`). Possibility
    /// needs a `Θ`-satisfiable monomial; otherwise the claim is dead.
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
        let certain = self.support.monomials().any(|m| {
            monomial_certain(m, valuation, theory)
                && !poly_certain(self.conditions.defeater_of(m), valuation, theory)
        });
        if certain {
            Status::Certain
        } else {
            Status::Possible
        }
    }

    /// Whether the claim is certain yet every certain support monomial carries a
    /// still-live defeater — *"assertable, but a pending ruling could defeat it."*
    ///
    /// An independent clean certain monomial (one whose defeater can never become
    /// certain) makes the claim robustly certain, so it is **not** at risk: the
    /// clean path protects it no matter how the poison is ruled. False unless the
    /// claim is [`Status::Certain`].
    #[must_use]
    pub fn at_risk(&self, valuation: &Valuation, theory: &Theory) -> bool {
        if self.status(valuation, theory) != Status::Certain {
            return false;
        }
        // At risk unless some certain monomial's defeater can never fire — a
        // robustly-certain path.
        !self.support.monomials().any(|m| {
            monomial_certain(m, valuation, theory)
                && !poly_possible(self.conditions.defeater_of(m), valuation, theory)
        })
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

/// Whether a polynomial has a monomial that holds on resolved events alone — the
/// defeater's "has fired" test.
fn poly_certain(poly: &PosBool, valuation: &Valuation, theory: &Theory) -> bool {
    poly.monomials()
        .any(|m| monomial_certain(m, valuation, theory))
}

/// Whether a polynomial has a monomial that can still become certain in some
/// admissible world — the defeater's "can still fire" test. A `FALSE` defeater
/// can never fire.
fn poly_possible(poly: &PosBool, valuation: &Valuation, theory: &Theory) -> bool {
    poly.monomials()
        .any(|m| monomial_possible(m, valuation, theory))
}
