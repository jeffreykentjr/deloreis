//! Valuations: truth assignments over atoms.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::atom::{Atom, Interner};

/// A (possibly partial) assignment of truth values to atoms.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

/// A valuation whose keys are stable event-variable names rather than
/// interner-local numeric atoms.
///
/// Persisted valuations remain compact [`Valuation`]s. This runtime form is the
/// mandatory bridge whenever an assignment crosses between independently
/// produced interners: resolve against the source table, then resolve again in
/// the receiving table. Numeric atoms are never portable by themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamedValuation(pub BTreeMap<String, bool>);

impl NamedValuation {
    /// Resolve an indexed valuation through the exact table that produced it.
    ///
    /// Duplicate source names are rejected even when only one duplicate is
    /// assigned: such a table cannot provide an unambiguous inverse mapping.
    pub fn from_indexed(
        valuation: &Valuation,
        source_names: &[String],
    ) -> Result<Self, ValuationRemapError> {
        reject_duplicate_names(source_names, InternerSide::Source)?;
        let mut named = BTreeMap::new();
        for (&atom, &value) in &valuation.0 {
            let name = source_names.get(atom.0 as usize).ok_or(
                ValuationRemapError::SourceAtomOutOfRange {
                    atom,
                    source_len: source_names.len(),
                },
            )?;
            named.insert(name.clone(), value);
        }
        Ok(Self(named))
    }

    /// Resolve this named assignment into a receiving interner.
    pub fn to_indexed(&self, target: &Interner) -> Result<Valuation, ValuationRemapError> {
        reject_duplicate_names(target.names(), InternerSide::Target)?;
        let by_name: BTreeMap<&str, Atom> = target
            .names()
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name.as_str(),
                    Atom(u32::try_from(index).expect("atom count fits in u32")),
                )
            })
            .collect();
        let mut indexed = Valuation::new();
        for (name, &value) in &self.0 {
            let atom = by_name
                .get(name.as_str())
                .copied()
                .ok_or_else(|| ValuationRemapError::TargetNameMissing { name: name.clone() })?;
            indexed.set(atom, value);
        }
        Ok(indexed)
    }

    /// Apply this overlay to a receiving valuation by exact name.
    pub fn apply_to(
        &self,
        target: &Interner,
        valuation: &mut Valuation,
    ) -> Result<(), ValuationRemapError> {
        for (atom, value) in self.to_indexed(target)?.0 {
            valuation.set(atom, value);
        }
        Ok(())
    }
}

/// A set of interner-local atoms carried across compilation boundaries by
/// their stable exact names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamedAtomSet(pub BTreeSet<String>);

impl NamedAtomSet {
    /// Resolve indexed atoms through the exact table that produced them.
    pub fn from_indexed(
        atoms: &BTreeSet<Atom>,
        source_names: &[String],
    ) -> Result<Self, ValuationRemapError> {
        reject_duplicate_names(source_names, InternerSide::Source)?;
        let mut named = BTreeSet::new();
        for &atom in atoms {
            let name = source_names.get(atom.0 as usize).ok_or(
                ValuationRemapError::SourceAtomOutOfRange {
                    atom,
                    source_len: source_names.len(),
                },
            )?;
            named.insert(name.clone());
        }
        Ok(Self(named))
    }

    /// Resolve the names into a receiving interner.
    pub fn to_indexed(&self, target: &Interner) -> Result<BTreeSet<Atom>, ValuationRemapError> {
        reject_duplicate_names(target.names(), InternerSide::Target)?;
        let by_name: BTreeMap<&str, Atom> = target
            .names()
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name.as_str(),
                    Atom(u32::try_from(index).expect("atom count fits in u32")),
                )
            })
            .collect();
        self.0
            .iter()
            .map(|name| {
                by_name
                    .get(name.as_str())
                    .copied()
                    .ok_or_else(|| ValuationRemapError::TargetNameMissing { name: name.clone() })
            })
            .collect()
    }

    /// Remove these explicitly open atoms from a receiving valuation.
    pub fn remove_from(
        &self,
        target: &Interner,
        valuation: &mut Valuation,
    ) -> Result<(), ValuationRemapError> {
        for atom in self.to_indexed(target)? {
            valuation.0.remove(&atom);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum InternerSide {
    Source,
    Target,
}

fn reject_duplicate_names(names: &[String], side: InternerSide) -> Result<(), ValuationRemapError> {
    let mut seen = std::collections::BTreeSet::new();
    for name in names {
        if !seen.insert(name) {
            return Err(match side {
                InternerSide::Source => {
                    ValuationRemapError::DuplicateSourceName { name: name.clone() }
                }
                InternerSide::Target => {
                    ValuationRemapError::DuplicateTargetName { name: name.clone() }
                }
            });
        }
    }
    Ok(())
}

/// A persisted valuation cannot be mapped into a receiving compiled world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValuationRemapError {
    /// The persisted valuation names an atom outside its paired source table.
    SourceAtomOutOfRange {
        /// The invalid persisted atom.
        atom: Atom,
        /// Number of entries in the paired source interner.
        source_len: usize,
    },
    /// The source table contains the same exact name more than once.
    DuplicateSourceName {
        /// The duplicated exact name.
        name: String,
    },
    /// The receiving table contains the same exact name more than once.
    DuplicateTargetName {
        /// The duplicated exact name.
        name: String,
    },
    /// A persisted exact name is absent from the receiving compiled world.
    TargetNameMissing {
        /// The exact persisted name absent from the receiver.
        name: String,
    },
}

impl std::fmt::Display for ValuationRemapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceAtomOutOfRange { atom, source_len } => write!(
                f,
                "persisted valuation atom {} is outside its source interner of length {source_len}",
                atom.0
            ),
            Self::DuplicateSourceName { name } => {
                write!(f, "source interner contains duplicate name `{name}`")
            }
            Self::DuplicateTargetName { name } => {
                write!(f, "receiving interner contains duplicate name `{name}`")
            }
            Self::TargetNameMissing { name } => {
                write!(
                    f,
                    "receiving interner does not contain persisted name `{name}`"
                )
            }
        }
    }
}

impl std::error::Error for ValuationRemapError {}

#[cfg(test)]
mod tests {
    use super::{NamedAtomSet, NamedValuation, Valuation, ValuationRemapError};
    use crate::atom::{Atom, Interner};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn remaps_by_name_when_positions_are_skewed() {
        let mut persisted = Valuation::new();
        persisted.set(Atom(1), false);
        let named =
            NamedValuation::from_indexed(&persisted, &["other".to_owned(), "decision".to_owned()])
                .expect("valid source");
        let target = Interner::from_names(vec!["decision".to_owned(), "other".to_owned()]);
        let remapped = named.to_indexed(&target).expect("exact target name");
        assert_eq!(remapped.get(Atom(0)), Some(false));
        assert_eq!(remapped.get(Atom(1)), None);
    }

    #[test]
    fn refuses_out_of_range_and_ambiguous_or_missing_names() {
        let mut persisted = Valuation::new();
        persisted.set(Atom(2), true);
        assert!(matches!(
            NamedValuation::from_indexed(&persisted, &["only".to_owned()]),
            Err(ValuationRemapError::SourceAtomOutOfRange { .. })
        ));

        assert!(matches!(
            NamedValuation::from_indexed(
                &Valuation::new(),
                &["same".to_owned(), "same".to_owned()]
            ),
            Err(ValuationRemapError::DuplicateSourceName { .. })
        ));

        let named = NamedValuation::from_indexed(&Valuation::new(), &["source".to_owned()])
            .expect("empty assignment is valid");
        assert!(matches!(
            named.to_indexed(&Interner::from_names(vec![
                "same".to_owned(),
                "same".to_owned()
            ])),
            Err(ValuationRemapError::DuplicateTargetName { .. })
        ));

        let named = NamedValuation([("missing".to_owned(), false)].into_iter().collect());
        assert!(matches!(
            named.to_indexed(&Interner::from_names(vec!["other".to_owned()])),
            Err(ValuationRemapError::TargetNameMissing { .. })
        ));
    }

    #[test]
    fn remaps_explicitly_open_atoms_by_name() {
        let source = BTreeSet::from([Atom(1)]);
        let named =
            NamedAtomSet::from_indexed(&source, &["other".to_owned(), "decision".to_owned()])
                .expect("valid source");
        let target = Interner::from_names(vec!["decision".to_owned(), "other".to_owned()]);
        assert_eq!(named.to_indexed(&target), Ok(BTreeSet::from([Atom(0)])));

        let mut valuation = Valuation(BTreeMap::from([(Atom(0), true), (Atom(1), true)]));
        named
            .remove_from(&target, &mut valuation)
            .expect("exact target name");
        assert_eq!(valuation.0, BTreeMap::from([(Atom(1), true)]));
    }
}
