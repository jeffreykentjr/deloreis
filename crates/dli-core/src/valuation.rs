//! Valuations: truth assignments over atoms.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::atom::Atom;

/// A (possibly partial) assignment of truth values to atoms.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Valuation(pub BTreeMap<Atom, bool>);

impl Valuation {
    /// An empty valuation.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The value assigned to an atom, if any.
    #[must_use]
    pub fn get(&self, atom: Atom) -> Option<bool> {
        self.0.get(&atom).copied()
    }

    /// Assign a value to an atom.
    pub fn set(&mut self, atom: Atom, value: bool) {
        self.0.insert(atom, value);
    }
}
