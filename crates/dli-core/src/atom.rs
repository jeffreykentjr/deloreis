//! Atoms, literals, and the string interner.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An interned atom handle. The u32 indexes into an [`Interner`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct Atom(pub u32);

/// A polarity rail: a literal is a positive or negative occurrence of an atom.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum Rail {
    /// A positive occurrence.
    Pos,
    /// A negative occurrence.
    Neg,
}

/// A literal: an atom on a polarity rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lit {
    /// The atom.
    pub atom: Atom,
    /// The polarity rail.
    pub rail: Rail,
}

/// A bidirectional string interner: name to [`Atom`] and back. Real and usable
/// now; the evaluator interns predicate and hypothesis names through it.
#[derive(Debug, Clone, Default)]
pub struct Interner {
    names: Vec<String>,
    index: HashMap<String, Atom>,
}

impl Interner {
    /// Create an empty interner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a name, returning its stable [`Atom`]. Idempotent.
    pub fn intern(&mut self, name: &str) -> Atom {
        if let Some(&atom) = self.index.get(name) {
            return atom;
        }
        let atom = Atom(u32::try_from(self.names.len()).expect("atom count fits in u32"));
        self.names.push(name.to_owned());
        self.index.insert(name.to_owned(), atom);
        atom
    }

    /// Resolve an atom back to its interned name, if present.
    #[must_use]
    pub fn resolve(&self, atom: Atom) -> Option<&str> {
        self.names.get(atom.0 as usize).map(String::as_str)
    }

    /// Number of interned atoms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether no atoms are interned yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
