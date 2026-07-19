//! Terms and atomic predicate expressions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ident::Ident;

/// A predicate applied to a list of argument terms, e.g. `adjacent(a, b)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AtomExpr {
    /// The predicate name.
    pub pred: Ident,
    /// The argument terms, in order.
    pub args: Vec<Term>,
}

/// A term: a variable, symbol, literal, or time reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Term {
    /// A logic variable (unifies during evaluation).
    Var(Ident),
    /// A ground symbol naming an entity.
    Sym(Ident),
    /// A string literal.
    Str(String),
    /// An integer literal.
    Int(i64),
    /// A reference into the time domain.
    Time(TimeRef),
}

/// A reference to a named era or event on the timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TimeRef {
    /// A named era (a span).
    Era(Ident),
    /// A named event (a point).
    Event(Ident),
}
