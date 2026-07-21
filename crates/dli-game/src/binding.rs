//! The `Registered` seam: named predicates a world supplies to the engine.
//!
//! Almost the whole ruleset is data. Two things genuinely cannot be — a world's
//! verdict predicate and its bespoke structural invariant — so they enter as named
//! Rust functions a world registers here, looked up by the `name` strings on
//! `crate::model::VerdictRule::Registered` and
//! `crate::model::StructuralInvariant::Registered`. A game with only generic rules
//! (a `completed` verdict and built-in invariants) passes `GameBindings::new()` —
//! the empty registry — and never touches this seam.

use std::collections::BTreeMap;

use crate::model::{Reading, Ruleset, Verdict};

/// A world verdict predicate: judges a reading under its ruleset.
pub type VerdictFn = fn(&Ruleset, &Reading) -> Verdict;

/// A world structural-invariant predicate: returns a breach reason, or `None` when
/// the invariant holds.
pub type InvariantFn = fn(&Ruleset) -> Option<String>;

/// The registry a world supplies: named verdict and structural-invariant
/// predicates, looked up by the `Registered { name }` strings on a ruleset.
///
/// ```
/// use dli_game::binding::GameBindings;
/// use dli_game::model::{Reading, Ruleset, Verdict};
///
/// fn always_open(_ruleset: &Ruleset, _reading: &Reading) -> Verdict {
///     Verdict::new("open")
/// }
/// let bindings = GameBindings::new().with_verdict("counsel", always_open);
/// assert!(bindings.verdict_fn("counsel").is_some());
/// assert!(bindings.verdict_fn("absent").is_none());
/// ```
#[derive(Debug, Clone, Default)]
pub struct GameBindings {
    verdicts: BTreeMap<String, VerdictFn>,
    invariants: BTreeMap<String, InvariantFn>,
}

impl GameBindings {
    /// An empty registry — the right one for a game with only generic rules.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a verdict predicate under a name, returning the registry (builder
    /// style).
    #[must_use]
    pub fn with_verdict(mut self, name: &str, predicate: VerdictFn) -> Self {
        self.register_verdict(name, predicate);
        self
    }

    /// Register a structural-invariant predicate under a name, returning the
    /// registry (builder style).
    #[must_use]
    pub fn with_invariant(mut self, name: &str, predicate: InvariantFn) -> Self {
        self.register_invariant(name, predicate);
        self
    }

    /// Register a verdict predicate under a name.
    pub fn register_verdict(&mut self, name: &str, predicate: VerdictFn) {
        self.verdicts.insert(name.to_owned(), predicate);
    }

    /// Register a structural-invariant predicate under a name.
    pub fn register_invariant(&mut self, name: &str, predicate: InvariantFn) {
        self.invariants.insert(name.to_owned(), predicate);
    }

    /// The verdict predicate registered under a name, if any.
    #[must_use]
    pub fn verdict_fn(&self, name: &str) -> Option<VerdictFn> {
        self.verdicts.get(name).copied()
    }

    /// The structural-invariant predicate registered under a name, if any.
    #[must_use]
    pub fn invariant_fn(&self, name: &str) -> Option<InvariantFn> {
        self.invariants.get(name).copied()
    }
}
