//! Atoms, literals, and the string interner.

use std::borrow::Cow;
use std::collections::HashMap;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

impl Lit {
    /// A positive occurrence of `atom` (the `+atom` rail).
    #[must_use]
    pub fn pos(atom: Atom) -> Self {
        Self {
            atom,
            rail: Rail::Pos,
        }
    }

    /// A negative occurrence of `atom` (the `-atom` rail).
    #[must_use]
    pub fn neg(atom: Atom) -> Self {
        Self {
            atom,
            rail: Rail::Neg,
        }
    }

    /// The same atom on the opposite rail.
    #[must_use]
    pub fn flip(self) -> Self {
        Self {
            atom: self.atom,
            rail: match self.rail {
                Rail::Pos => Rail::Neg,
                Rail::Neg => Rail::Pos,
            },
        }
    }

    /// Whether this literal is satisfied by `value` under the given assignment
    /// of its atom: a positive literal wants `true`, a negative literal `false`.
    #[must_use]
    pub fn satisfied_by(self, value: bool) -> bool {
        match self.rail {
            Rail::Pos => value,
            Rail::Neg => !value,
        }
    }
}

/// A bidirectional string interner: name to [`Atom`] and back. Real and usable
/// now; the evaluator interns predicate and hypothesis names through it.
///
/// It serializes as the flat name list alone — the reverse index is derived, so
/// a round-tripped interner assigns the same atoms and the serialized form is
/// byte-stable (deterministic order, no map).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    /// Rebuild an interner from its ordered name list, restoring the atom
    /// assignment names had when they were first interned.
    #[must_use]
    pub fn from_names(names: Vec<String>) -> Self {
        let mut index = HashMap::with_capacity(names.len());
        for (i, name) in names.iter().enumerate() {
            index.insert(
                name.clone(),
                Atom(u32::try_from(i).expect("atom count fits in u32")),
            );
        }
        Self { names, index }
    }

    /// The interned names in atom order.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
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

impl Serialize for Interner {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.names.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Interner {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let names = Vec::<String>::deserialize(deserializer)?;
        Ok(Self::from_names(names))
    }
}

impl JsonSchema for Interner {
    fn schema_name() -> Cow<'static, str> {
        "Interner".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "array",
            "items": { "type": "string" }
        })
    }
}
