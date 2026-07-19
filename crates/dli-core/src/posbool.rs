//! Positive-boolean support formulas in disjunctive normal form.
//!
//! A [`PosBool`] is a set of [`Monomial`] conjunctions; it is true when any one
//! monomial is satisfied. The empty set is [`PosBool::FALSE`]; the set holding
//! one empty monomial is [`PosBool::truth`].
//!
//! Wire form is `{"monomials":[["+h.x","-h.y"]]}`: each literal renders as a
//! sign followed by its atom id. The [`Serialize`]/[`Deserialize`] impls below
//! are real and round-trip today; the lattice operations are the CORE-2 work.

use std::borrow::Cow;
use std::collections::BTreeSet;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::ser::SerializeStruct as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::atom::{Atom, Lit, Rail};
use crate::valuation::Valuation;

/// A conjunction of literals.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Monomial(pub BTreeSet<Lit>);

impl Monomial {
    /// The empty conjunction — vacuously true.
    #[must_use]
    pub fn top() -> Self {
        Self(BTreeSet::new())
    }

    /// A monomial built from a run of literals.
    pub fn from_lits(lits: impl IntoIterator<Item = Lit>) -> Self {
        Self(lits.into_iter().collect())
    }

    /// The literals of this conjunction.
    pub fn lits(&self) -> impl Iterator<Item = Lit> + '_ {
        self.0.iter().copied()
    }

    /// Whether the conjunction is internally contradictory: some atom appears on
    /// both rails, so no assignment can satisfy it.
    #[must_use]
    pub fn is_contradictory(&self) -> bool {
        self.0
            .iter()
            .any(|lit| self.0.contains(&lit.flip()) && lit.rail == Rail::Pos)
    }

    /// Whether this conjunction subsumes `other`: every literal of `self` is a
    /// literal of `other`. When it does, `self` is the weaker (more general)
    /// conjunction and absorbs `other` in a disjunction.
    #[must_use]
    pub fn subsumes(&self, other: &Monomial) -> bool {
        self.0.is_subset(&other.0)
    }

    /// Evaluate this conjunction under the open-goes-false completion: every
    /// literal must agree with the valuation, with unassigned atoms read as
    /// false. The empty conjunction is true.
    #[must_use]
    pub fn eval_under(&self, valuation: &Valuation) -> bool {
        self.0
            .iter()
            .all(|lit| lit.satisfied_by(valuation.get(lit.atom).unwrap_or(false)))
    }

    /// Whether every literal is satisfied by a *resolved* value — the atom is
    /// assigned and agrees. An unassigned (open) atom fails: certainty rests on
    /// resolved events alone. The empty conjunction is certain (unconditional).
    #[must_use]
    pub fn is_certain_under(&self, valuation: &Valuation) -> bool {
        self.0
            .iter()
            .all(|lit| valuation.get(lit.atom).is_some_and(|v| lit.satisfied_by(v)))
    }

    /// Whether no literal is resolved *against* — no positive literal on a
    /// false atom, no negative literal on a true atom. Open atoms are allowed;
    /// this is the per-monomial half of the shadow (possible) test.
    #[must_use]
    pub fn is_live_under(&self, valuation: &Valuation) -> bool {
        self.0.iter().all(|lit| match valuation.get(lit.atom) {
            Some(v) => lit.satisfied_by(v),
            None => true,
        })
    }
}

/// A positive-boolean formula in disjunctive normal form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PosBool(pub BTreeSet<Monomial>);

impl PosBool {
    /// The identically-false formula: no satisfying conjunction.
    ///
    /// Const because an empty set is const-constructible.
    pub const FALSE: PosBool = PosBool(BTreeSet::new());

    /// The identically-true formula: a single empty conjunction.
    ///
    /// A function rather than a const because inserting into a `BTreeSet` is
    /// not a stable const operation.
    #[must_use]
    pub fn truth() -> Self {
        let mut monomials = BTreeSet::new();
        monomials.insert(Monomial::top());
        PosBool(monomials)
    }

    /// The single-literal formula `{{lit}}`.
    #[must_use]
    pub fn literal(lit: Lit) -> Self {
        let mut monomial = BTreeSet::new();
        monomial.insert(lit);
        PosBool(std::iter::once(Monomial(monomial)).collect())
    }

    /// The positive-literal formula `{{+atom}}` — an atom's bare support.
    #[must_use]
    pub fn pos(atom: Atom) -> Self {
        Self::literal(Lit::pos(atom))
    }

    /// The negative-literal formula `{{-atom}}`.
    #[must_use]
    pub fn neg(atom: Atom) -> Self {
        Self::literal(Lit::neg(atom))
    }

    /// A single-conjunction formula from a run of literals.
    pub fn conjunction(lits: impl IntoIterator<Item = Lit>) -> Self {
        PosBool(std::iter::once(Monomial::from_lits(lits)).collect())
    }

    /// The formula's monomials in canonical order.
    pub fn monomials(&self) -> impl Iterator<Item = &Monomial> + '_ {
        self.0.iter()
    }

    /// Whether the formula is identically false (no satisfying conjunction).
    #[must_use]
    pub fn is_false(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether the formula is identically true (holds the empty conjunction).
    #[must_use]
    pub fn is_true(&self) -> bool {
        self.0.contains(&Monomial::top())
    }

    /// Disjunction (lattice join): the union of both formulas' monomials,
    /// reduced to its absorptive antichain.
    ///
    /// ```
    /// use dli_core::atom::{Atom, Lit};
    /// use dli_core::posbool::PosBool;
    /// let general = PosBool::pos(Atom(0));
    /// let specific = PosBool::conjunction([Lit::pos(Atom(0)), Lit::pos(Atom(1))]);
    /// // {+0} ∨ {+0,+1} absorbs the more specific monomial: the result is {+0}.
    /// assert_eq!(general.or_(&specific), general);
    /// ```
    #[must_use]
    pub fn or_(&self, other: &PosBool) -> PosBool {
        let mut monomials = self.0.clone();
        monomials.extend(other.0.iter().cloned());
        PosBool(monomials).absorb()
    }

    /// Conjunction (lattice meet): the cross product of monomials — each pair
    /// unioned into one conjunction — reduced to its absorptive antichain.
    /// Contradictory products drop out during absorption.
    ///
    /// ```
    /// use dli_core::atom::Atom;
    /// use dli_core::posbool::PosBool;
    /// let a = PosBool::pos(Atom(0));
    /// let b = PosBool::pos(Atom(1));
    /// // {+0} ∧ {+1} = {+0,+1}: one monomial, two literals.
    /// let meet = a.and_(&b);
    /// assert_eq!(meet.monomials().count(), 1);
    /// // Meeting with a contradicting literal is identically false.
    /// assert!(a.and_(&PosBool::neg(Atom(0))).is_false());
    /// ```
    #[must_use]
    pub fn and_(&self, other: &PosBool) -> PosBool {
        let mut monomials = BTreeSet::new();
        for left in &self.0 {
            for right in &other.0 {
                let mut lits = left.0.clone();
                lits.extend(right.0.iter().copied());
                monomials.insert(Monomial(lits));
            }
        }
        PosBool(monomials).absorb()
    }

    /// Reduce to the minimal equivalent DNF: drop contradictory conjunctions
    /// (identically false) and any conjunction that is subsumed by another. The
    /// result is the antichain of minimal monomials, and the operation is
    /// idempotent — the linchpin that keeps supports narrow.
    #[must_use]
    pub fn absorb(&self) -> PosBool {
        let live: Vec<Monomial> = self
            .0
            .iter()
            .filter(|m| !m.is_contradictory())
            .cloned()
            .collect();
        let mut kept = BTreeSet::new();
        for (i, monomial) in live.iter().enumerate() {
            let dominated = live
                .iter()
                .enumerate()
                .any(|(j, other)| j != i && other.subsumes(monomial));
            if !dominated {
                kept.insert(monomial.clone());
            }
        }
        PosBool(kept)
    }

    /// Evaluate the formula under the open-goes-false completion: true when some
    /// monomial holds with every unassigned atom read as false. This is a total
    /// boolean evaluation, so it is a lattice homomorphism onto `{false, true}`.
    ///
    /// ```
    /// use dli_core::atom::Atom;
    /// use dli_core::posbool::PosBool;
    /// use dli_core::valuation::Valuation;
    /// let mut nu = Valuation::new();
    /// nu.set(Atom(0), true);
    /// assert!(PosBool::pos(Atom(0)).eval_under(&nu));
    /// assert!(!PosBool::pos(Atom(1)).eval_under(&nu)); // open atom reads false
    /// ```
    #[must_use]
    pub fn eval_under(&self, valuation: &Valuation) -> bool {
        self.0.iter().any(|m| m.eval_under(valuation))
    }

    /// Whether some monomial is satisfied by resolved events alone — the
    /// certainty (assertability) test of the support channel.
    #[must_use]
    pub fn is_certain_under(&self, valuation: &Valuation) -> bool {
        self.0.iter().any(|m| m.is_certain_under(valuation))
    }

    /// The De Morgan complement, returned in absorptive DNF. Negating the
    /// support of a merely-possible premise is how a defeater polynomial is
    /// formed (see the negation regime in the semantics spec).
    ///
    /// **Not a structural involution.** `x.negate().negate()` is *logically*
    /// equivalent to `x` — equal under [`eval_under`](PosBool::eval_under) for
    /// every valuation — but need not be structurally equal, because absorption
    /// is not a full canonicalization: it drops subsumed and contradictory
    /// monomials but performs no resolution, so it does not recognize tautologies
    /// such as `x ∨ ¬x`. Compare double negations by evaluation, never by `==`.
    #[must_use]
    pub fn negate(&self) -> PosBool {
        // ¬(⋁ mᵢ) = ⋀ ¬mᵢ, and ¬mᵢ = ⋁ over its literals flipped.
        let mut acc = PosBool::truth();
        for monomial in &self.0 {
            let mut clause = PosBool::FALSE;
            for lit in &monomial.0 {
                clause = clause.or_(&PosBool::literal(lit.flip()));
            }
            acc = acc.and_(&clause);
        }
        acc
    }
}

fn lit_to_wire(lit: Lit) -> String {
    let sign = match lit.rail {
        Rail::Pos => '+',
        Rail::Neg => '-',
    };
    format!("{sign}{}", lit.atom.0)
}

fn lit_from_wire(token: &str) -> Result<Lit, String> {
    let mut chars = token.chars();
    let rail = match chars.next() {
        Some('+') => Rail::Pos,
        Some('-') => Rail::Neg,
        _ => return Err(format!("literal '{token}' must start with '+' or '-'")),
    };
    let id: u32 = token[1..]
        .parse()
        .map_err(|_| format!("literal '{token}' has a non-numeric atom id"))?;
    Ok(Lit {
        atom: Atom(id),
        rail,
    })
}

impl Serialize for PosBool {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Emit the canonical antichain so the wire form is byte-stable and two
        // logically-equal supports serialize (and digest) identically.
        let canonical = self.absorb();
        let monomials: Vec<Vec<String>> = canonical
            .0
            .iter()
            .map(|monomial| monomial.0.iter().copied().map(lit_to_wire).collect())
            .collect();
        let mut state = serializer.serialize_struct("PosBool", 1)?;
        state.serialize_field("monomials", &monomials)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for PosBool {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            monomials: Vec<Vec<String>>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let mut formula = BTreeSet::new();
        for monomial in wire.monomials {
            let mut lits = BTreeSet::new();
            for token in monomial {
                lits.insert(lit_from_wire(&token).map_err(D::Error::custom)?);
            }
            formula.insert(Monomial(lits));
        }
        // Canonicalize on the way in: drop contradictory and subsumed monomials
        // so untrusted wire input can never poison `==`-based change detection
        // or judgment equality with a non-minimal support.
        Ok(PosBool(formula).absorb())
    }
}

impl JsonSchema for PosBool {
    fn schema_name() -> Cow<'static, str> {
        "PosBool".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "properties": {
                "monomials": {
                    "type": "array",
                    "items": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            },
            "required": ["monomials"]
        })
    }
}
