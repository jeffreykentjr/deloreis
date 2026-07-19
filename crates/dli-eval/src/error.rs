//! Evaluation errors.

use miette::Diagnostic;
use thiserror::Error;

/// A failure encountered while lowering or evaluating a world.
#[derive(Debug, Clone, PartialEq, Eq, Error, Diagnostic)]
pub enum EvalError {
    /// The rule set has no valid stratification (negation/aggregation cycle).
    #[error("program is not stratifiable: {reason}")]
    #[diagnostic(code(dli::eval::unstratifiable))]
    Unstratifiable {
        /// What made stratification impossible.
        reason: String,
    },
    /// A goal or rule referenced a predicate that is not defined.
    #[error("unknown predicate '{name}'")]
    #[diagnostic(code(dli::eval::unknown_predicate))]
    UnknownPredicate {
        /// The undefined predicate name.
        name: String,
    },
    /// The world derived contradictory ground facts.
    #[error("contradiction: {detail}")]
    #[diagnostic(code(dli::eval::contradiction))]
    Contradiction {
        /// A description of the conflicting derivation.
        detail: String,
    },
}
