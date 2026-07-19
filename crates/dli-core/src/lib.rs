//! Deloreis Lore Interchange core: the support lattice and grade order.
//!
//! Claims carry a positive-boolean support formula (a DNF over hypothesis
//! literals) and a monotone [`grade::Grade`]. Support tracks *under which
//! hypotheses* a claim holds; grade tracks *how strongly* it is known. Both are
//! the currency the evaluator and the wire contract move around.

pub mod atom;
pub mod derivation;
pub mod grade;
pub mod posbool;
pub mod valuation;
pub mod verdict;

pub use atom::{Atom, Interner, Lit, Rail};
pub use derivation::{ClaimId, Derivation, RuleId};
pub use grade::Grade;
pub use posbool::{Monomial, PosBool};
pub use valuation::Valuation;
pub use verdict::Verdict;
