//! Rule-body literals: a polarity-aware wrapper around an [`AtomExpr`].
//!
//! A derivation rule's body is a conjunction of **literals**, each an atom that
//! must hold (a positive literal) or must *not* hold in the certain layer (a
//! negated literal, written `not <atom>` at the surface). Negation is stratified
//! and range-restricted; the semantics live in `docs/spec/02-semantics.md §4`
//! (a negated literal tests against the certain layer only and poisons the
//! conclusion's defeater polynomial) and `08-reasoning.md §1` (load-time
//! stratification + range restriction).
//!
//! The type is shared: the parser ([`dli_syntax`](../../dli_syntax/index.html))
//! produces it, and the evaluator's typed rule intake consumes it, so a single
//! `BodyLiteral` crosses the whole lane.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::term::AtomExpr;

/// One literal of a rule body: an atom on a polarity rail.
///
/// A positive literal ([`negated`](BodyLiteral::negated) is `false`) joins over
/// the relation and binds its variables; a negated literal filters against the
/// certain layer and binds nothing (datalog safety: every variable in a negated
/// literal must also appear in a positive body atom).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BodyLiteral {
    /// The atom pattern.
    pub atom: AtomExpr,
    /// Whether the atom is negated (`not <atom>` at the surface).
    pub negated: bool,
}

impl BodyLiteral {
    /// A positive body literal.
    #[must_use]
    pub fn pos(atom: AtomExpr) -> Self {
        Self {
            atom,
            negated: false,
        }
    }

    /// A negated body literal (`not <atom>`).
    #[must_use]
    pub fn not(atom: AtomExpr) -> Self {
        Self {
            atom,
            negated: true,
        }
    }
}

/// A bare atom is a positive body literal — the zero-friction conversion that
/// lets a positive-only caller pass `Vec<AtomExpr>` where body literals are
/// expected. A negated literal is only ever constructed explicitly.
impl From<AtomExpr> for BodyLiteral {
    fn from(atom: AtomExpr) -> Self {
        Self::pos(atom)
    }
}
