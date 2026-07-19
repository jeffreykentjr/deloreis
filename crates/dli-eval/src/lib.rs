//! Deloreis Lore Interchange evaluator.
//!
//! The pipeline is: [`lower`] an AST document into a [`world::World`],
//! [`stratify`] its rules, compute the [`fixpoint`], then [`query()`] it. Every
//! answer row carries a support formula and grade, so a query result records not
//! just *whether* a claim holds but under which hypotheses. [`fork_eval`]
//! re-runs a query under an assumed set of fork options.

pub mod error;
pub mod fork;
pub mod query;
pub mod strata;
pub mod support;
pub mod why;
pub mod world;

pub use error::EvalError;
pub use query::{Answer, Query, QueryRow};
pub use strata::Strata;
pub use support::SupportIndex;
pub use world::World;

use dli_ast::decl::Document;
use dli_ast::ident::Ident;

/// Lower an AST document into an evaluable world program.
pub fn lower(document: &Document) -> Result<World, EvalError> {
    World::lower(document)
}

/// Partition the world's rules into evaluation strata.
pub fn stratify(world: &World) -> Result<Strata, EvalError> {
    world.stratify()
}

/// Compute the least fixpoint of the stratified program.
pub fn fixpoint(world: &World, strata: &Strata) -> Result<World, EvalError> {
    world.saturate_with(strata)
}

/// Answer a query against a saturated world under its ambient valuation.
pub fn query(world: &World, query: &Query) -> Result<Answer, EvalError> {
    world.query(query)
}

/// Answer a query under an assumed set of fork options pinned true.
pub fn fork_eval(world: &World, query: &Query, assume: &[Ident]) -> Result<Answer, EvalError> {
    world.fork_eval(query, assume)
}
