//! The inverted support index: the blame and revaluation read surface.

use std::collections::{BTreeMap, BTreeSet};

use dli_ast::term::AtomExpr;
use dli_core::atom::Atom;
use dli_core::derivation::ClaimId;
use dli_core::posbool::PosBool;

use crate::world::{GroundAtom, World};

/// An inverted view of a saturated world's provenance: which atoms each event
/// variable touches, and — read the other way — the support and defeater of any
/// atom. This is the read surface that blame (`why`), defeat (`defeater`), and
/// revaluation (`dependents`) consume. It is built once from the fixpoint and
/// never mutates it.
///
/// Event variables map to [`ClaimId`]s by index: `ClaimId(i)` is the claim the
/// world's interner names at position `i` (`world.interner.names()[i]`).
#[derive(Debug, Clone, Default)]
pub struct SupportIndex {
    by_var: BTreeMap<Atom, BTreeSet<GroundAtom>>,
    rows: BTreeMap<GroundAtom, Provenance>,
}

/// One atom's stored support and defeater polynomials.
#[derive(Debug, Clone)]
struct Provenance {
    support: PosBool,
    defeaters: PosBool,
}

impl SupportIndex {
    /// Build the index from a saturated world's derived relation.
    ///
    /// ```
    /// use dli_core::ann::Ann;
    /// use dli_core::atom::Atom;
    /// use dli_core::posbool::PosBool;
    /// use dli_eval::SupportIndex;
    /// use dli_eval::world::{GroundAtom, World};
    /// let mut world = World::new();
    /// let fact = GroundAtom { pred: "keeps".to_owned(), args: vec!["fen".to_owned()] };
    /// world.assert_fact(fact, Ann::supported(PosBool::pos(Atom(0))));
    /// let index = SupportIndex::build(&world.fixpoint().expect("stratifiable"));
    /// // Event variable 0 supports the keeps atom.
    /// assert!(index.dependents(Atom(0)).is_some());
    /// ```
    #[must_use]
    pub fn build(world: &World) -> SupportIndex {
        let mut by_var: BTreeMap<Atom, BTreeSet<GroundAtom>> = BTreeMap::new();
        let mut rows = BTreeMap::new();
        for derived in &world.derived {
            for monomial in derived.support.monomials() {
                for lit in monomial.lits() {
                    by_var
                        .entry(lit.atom)
                        .or_default()
                        .insert(derived.atom.clone());
                }
            }
            rows.insert(
                derived.atom.clone(),
                Provenance {
                    support: derived.support.clone(),
                    defeaters: derived.defeaters.clone(),
                },
            );
        }
        SupportIndex { by_var, rows }
    }

    /// The support antichain of an atom, as claim ids: the outer vector is the
    /// disjunction of minimal witnesses, each inner vector the conjunction of
    /// claims that hold together in one witness. A single empty inner vector
    /// means unconditional support; an empty outer vector means the atom is
    /// unknown or the pattern is not ground.
    #[must_use]
    pub fn why(&self, atom: &AtomExpr) -> Vec<Vec<ClaimId>> {
        let Some(ground) = GroundAtom::from_pattern(atom) else {
            return Vec::new();
        };
        let Some(provenance) = self.rows.get(&ground) else {
            return Vec::new();
        };
        provenance
            .support
            .monomials()
            .map(|monomial| monomial.lits().map(|lit| ClaimId(lit.atom.0)).collect())
            .collect()
    }

    /// The first defeating claim of an atom, if its defeater polynomial is
    /// non-empty; `None` when the atom rests on no defeasible default.
    #[must_use]
    pub fn defeater(&self, atom: &AtomExpr) -> Option<ClaimId> {
        let ground = GroundAtom::from_pattern(atom)?;
        let provenance = self.rows.get(&ground)?;
        provenance
            .defeaters
            .monomials()
            .next()
            .and_then(|monomial| monomial.lits().next())
            .map(|lit| ClaimId(lit.atom.0))
    }

    /// The atoms whose support mentions an event variable — the read that lets a
    /// valuation change touch only the atoms it can affect.
    #[must_use]
    pub fn dependents(&self, var: Atom) -> Option<&BTreeSet<GroundAtom>> {
        self.by_var.get(&var)
    }
}
