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
        monomials.insert(Monomial(BTreeSet::new()));
        PosBool(monomials)
    }

    /// Disjunction (lattice join).
    #[must_use]
    pub fn or_(&self, _other: &PosBool) -> PosBool {
        todo!("CORE-2: DNF disjunction followed by absorption")
    }

    /// Conjunction (lattice meet).
    #[must_use]
    pub fn and_(&self, _other: &PosBool) -> PosBool {
        todo!("CORE-2: DNF conjunction (cross product) followed by absorption")
    }

    /// Remove subsumed monomials, yielding the minimal equivalent DNF.
    #[must_use]
    pub fn absorb(&self) -> PosBool {
        todo!("CORE-2: drop any monomial that is a superset of another")
    }

    /// Evaluate the formula under a total or partial valuation.
    #[must_use]
    pub fn eval_under(&self, _valuation: &Valuation) -> bool {
        todo!("CORE-2: a monomial holds when every literal agrees with the valuation")
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
        let monomials: Vec<Vec<String>> = self
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
        Ok(PosBool(formula))
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
