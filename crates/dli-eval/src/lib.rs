//! Deloreis Lore Interchange evaluator.
//!
//! The pipeline is: [`lower`] an AST document into a [`world::World`],
//! [`stratify`] its rules, compute the [`fixpoint`], then [`query`] it. Every
//! answer row carries a support formula and grade, so a query result records not
//! just *whether* a claim holds but under which hypotheses. [`fork_eval`]
//! re-runs a query under an assumed set of fork options.

pub mod error;
pub mod query;
pub mod strata;
pub mod world;

pub use error::EvalError;
pub use query::{Answer, Query, QueryRow};
pub use strata::Strata;
pub use world::World;

use dli_ast::decl::Document;
use dli_ast::ident::Ident;

/// Lower an AST document into an evaluable world program.
pub fn lower(_document: &Document) -> Result<World, EvalError> {
    todo!("CORE-2: lower declarations into ground facts, rules, and time structures")
}

/// Partition the world's rules into evaluation strata.
pub fn stratify(_world: &World) -> Result<Strata, EvalError> {
    todo!("CORE-2: stratify on negation/aggregation dependency; reject cycles")
}

/// Compute the least fixpoint of the stratified program.
pub fn fixpoint(_world: &World, _strata: &Strata) -> Result<World, EvalError> {
    todo!("CORE-2: semi-naive evaluation to a fixpoint, stratum by stratum")
}

/// Answer a query against a saturated world.
pub fn query(_world: &World, _query: &Query) -> Result<Answer, EvalError> {
    todo!("CORE-2: match the goal and collect support-carrying answer rows")
}

/// Answer a query under an assumed set of fork options.
pub fn fork_eval(_world: &World, _query: &Query, _assume: &[Ident]) -> Result<Answer, EvalError> {
    todo!("CORE-2: evaluate with the given hypotheses pinned true")
}
