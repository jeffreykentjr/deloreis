//! Valuations: truth assignments over atoms.

use std::collections::BTreeMap;

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
    use super::{NamedValuation, Valuation, ValuationRemapError};
    use crate::ann::{Ann, Status, Theory};
    use crate::atom::{Atom, Interner};
    use crate::posbool::PosBool;

    const INGESTION_ORDERS: [[&str; 3]; 6] = [
        ["control", "defeated", "open"],
        ["control", "open", "defeated"],
        ["defeated", "control", "open"],
        ["defeated", "open", "control"],
        ["open", "control", "defeated"],
        ["open", "defeated", "control"],
    ];

    fn atom_named(interner: &Interner, name: &str) -> Atom {
        interner
            .names()
            .iter()
            .position(|candidate| candidate == name)
            .and_then(|position| u32::try_from(position).ok())
            .map(Atom)
            .expect("fixture name is interned")
    }

    fn status(valuation: &Valuation, support: PosBool) -> Status {
        Ann::supported(support).status(valuation, &Theory::new())
    }

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
    fn every_legal_ingestion_order_has_the_same_exact_named_standings() {
        let target =
            Interner::from_names(["open", "control", "defeated"].map(str::to_owned).to_vec());
        let control = atom_named(&target, "control");
        let defeated = atom_named(&target, "defeated");
        let open = atom_named(&target, "open");
        let mut reference = Valuation::new();
        reference.set(control, true);
        reference.set(defeated, false);

        for order in INGESTION_ORDERS {
            let source_names = order.map(str::to_owned);
            let source = Interner::from_names(source_names.to_vec());
            let mut persisted = Valuation::new();
            persisted.set(atom_named(&source, "control"), true);
            persisted.set(atom_named(&source, "defeated"), false);
            let named = NamedValuation::from_indexed(&persisted, &source_names)
                .expect("legal ingestion table");
            let received = named.to_indexed(&target).expect("same exact names");

            assert_eq!(received, reference, "ingestion order {order:?}");
            assert_eq!(status(&received, PosBool::pos(control)), Status::Certain);
            assert_eq!(status(&received, PosBool::pos(defeated)), Status::Dead);
            assert_eq!(status(&received, PosBool::pos(open)), Status::Possible);
            let redundant = PosBool::pos(defeated).or_(&PosBool::pos(control));
            assert_eq!(status(&received, redundant), Status::Certain);
        }
    }

    #[test]
    fn post_compile_overlay_scales_by_name_and_preserves_redundant_support() {
        for depth in [1_usize, 2, 4, 8] {
            for shared_consequences in [1_usize, 2, 5] {
                let mut source_names: Vec<String> =
                    (0..depth).map(|index| format!("padding-{index}")).collect();
                source_names.extend(["defeated".to_owned(), "control".to_owned()]);
                let mut persisted = Valuation::new();
                persisted.set(Atom(u32::try_from(depth).expect("depth")), false);
                persisted.set(Atom(u32::try_from(depth + 1).expect("depth")), true);

                let target = Interner::from_names(vec![
                    "control".to_owned(),
                    "defeated".to_owned(),
                    "receiver-only".to_owned(),
                ]);
                let defeated = atom_named(&target, "defeated");
                let control = atom_named(&target, "control");
                let consequences =
                    vec![Ann::supported(PosBool::pos(defeated)); shared_consequences];
                let redundant = Ann::supported(PosBool::pos(defeated).or_(&PosBool::pos(control)));
                let mut post_compile = Valuation::new();
                post_compile.set(defeated, true);
                post_compile.set(control, true);

                let named = NamedValuation::from_indexed(&persisted, &source_names)
                    .expect("persisted overlay");
                named
                    .apply_to(&target, &mut post_compile)
                    .expect("post-compile exact-name application");
                assert!(
                    consequences
                        .iter()
                        .all(|ann| { ann.status(&post_compile, &Theory::new()) == Status::Dead }),
                    "depth={depth}, shared={shared_consequences}"
                );
                assert_eq!(
                    redundant.status(&post_compile, &Theory::new()),
                    Status::Certain,
                    "independent support survives: depth={depth}, shared={shared_consequences}"
                );
            }
        }
    }
}
