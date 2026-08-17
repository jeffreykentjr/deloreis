//! Predicate-identity state for the world/check seam.
//!
//! The declaration-carriage half of predicate identity belongs to the AST and
//! evaluator lanes. This module is the world-side typed seam for that same
//! channel: it owns closure, effective policy projections, and the incremental
//! observed-arity index without introducing a second textual or wire
//! representation.

use std::collections::{BTreeMap, BTreeSet};

use dli_core::alias::canon;
use dli_core::ann::Ann;
use dli_core::valuation::Valuation;
use dli_core::{Status, Theory};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One caller-declared predicate identity claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PredicateIdentityLink {
    /// One predicate spelling.
    pub left: String,
    /// The other predicate spelling.
    pub right: String,
    /// The defeasible support for the declaration.
    pub ann: Ann,
    /// Human-readable decision ids used by continuity findings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<String>,
}

impl PredicateIdentityLink {
    /// Construct a link, storing its endpoints in deterministic order.
    #[must_use]
    pub fn new(left: impl Into<String>, right: impl Into<String>, ann: Ann) -> Self {
        let (left, right) = (left.into(), right.into());
        if left <= right {
            Self {
                left,
                right,
                ann,
                citations: Vec::new(),
            }
        } else {
            Self {
                left: right,
                right: left,
                ann,
                citations: Vec::new(),
            }
        }
    }

    /// Add the decision ids that support this declaration.
    #[must_use]
    pub fn with_citations(mut self, citations: Vec<String>) -> Self {
        self.citations = citations;
        self
    }

    /// Whether the claim is a semantic self-merge.
    #[must_use]
    pub fn is_self_merge(&self) -> bool {
        self.left == self.right
    }
}

/// A structured arity mismatch found while validating one touched identity
/// component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PredicateArityMismatch {
    /// The predicate mutation that caused this component to be checked.
    pub touched_predicate: String,
    /// The predicates in the affected identity component.
    pub predicates: Vec<String>,
    /// The incompatible observed arities, sorted and deduplicated.
    pub arities: Vec<usize>,
}

/// World-side predicate identity state.
///
/// `observed_arities` is a derived incremental cache.  It is intentionally
/// omitted from serialization: the durable declaration and fact objects are
/// the source of truth, and a read can rebuild this cache from touched facts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PredicateIdentityState {
    /// Caller-declared identity links, in carriage order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<PredicateIdentityLink>,
    /// Observed predicate arities, retained only for incremental validation.
    #[serde(skip)]
    pub observed_arities: BTreeMap<String, BTreeSet<usize>>,
}

impl PredicateIdentityState {
    /// Whether this state contributes no serialized or semantic information.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Add a declaration and return the affected component's current arity
    /// finding, if any.  A self-merge is retained for provenance but has no
    /// effect on closure.
    pub fn add_link(&mut self, link: PredicateIdentityLink) -> Option<PredicateArityMismatch> {
        let touched = link.left.clone();
        self.links.push(link);
        self.validate_touched(&touched)
    }

    /// Record one observed arity and validate only the component containing
    /// `predicate`.  Unrelated identity classes are never scanned.
    pub fn observe_arity(
        &mut self,
        predicate: impl Into<String>,
        arity: usize,
    ) -> Option<PredicateArityMismatch> {
        let predicate = predicate.into();
        self.observed_arities
            .entry(predicate.clone())
            .or_default()
            .insert(arity);
        self.validate_touched(&predicate)
    }

    /// Validate only the identity component touched by `predicate`.
    #[must_use]
    pub fn validate_touched(&self, predicate: &str) -> Option<PredicateArityMismatch> {
        let predicates = self.component(predicate);
        if !self.links.iter().any(|link| {
            !link.is_self_merge()
                && (predicates.contains(&link.left) || predicates.contains(&link.right))
        }) {
            return None;
        }
        let mut arities = BTreeSet::new();
        for member in &predicates {
            arities.extend(
                self.observed_arities
                    .get(member)
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        (arities.len() > 1).then(|| PredicateArityMismatch {
            touched_predicate: predicate.to_owned(),
            predicates,
            arities: arities.into_iter().collect(),
        })
    }

    /// Build the symmetric-transitive canonical map from certain links only.
    #[must_use]
    pub fn certain_canon(
        &self,
        valuation: &Valuation,
        theory: &Theory,
    ) -> BTreeMap<String, String> {
        dli_core::alias::canon_map(
            self.certain_pairs(valuation, theory)
                .map(|(left, right)| (left.to_owned(), right.to_owned())),
        )
    }

    /// The certain links, preserving declaration order for citation reporting.
    pub fn certain_links<'a>(
        &'a self,
        valuation: &'a Valuation,
        theory: &'a Theory,
    ) -> impl Iterator<Item = &'a PredicateIdentityLink> + 'a {
        self.links.iter().filter(move |link| {
            link.ann.status(valuation, theory) == Status::Certain
                && self.validate_touched(&link.left).is_none()
        })
    }

    /// The certain endpoint pairs used by [`certain_canon`](Self::certain_canon).
    pub fn certain_pairs<'a>(
        &'a self,
        valuation: &'a Valuation,
        theory: &'a Theory,
    ) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.certain_links(valuation, theory)
            .filter(|link| !link.is_self_merge())
            .map(|link| (link.left.as_str(), link.right.as_str()))
    }

    /// Return the members of the (possibly singleton) component containing a
    /// predicate.  This is the only graph walk used by arity validation.
    #[must_use]
    pub fn component(&self, predicate: &str) -> Vec<String> {
        let mut members = BTreeSet::from([predicate.to_owned()]);
        let mut changed = true;
        while changed {
            changed = false;
            for link in &self.links {
                if members.contains(&link.left) || members.contains(&link.right) {
                    changed |= members.insert(link.left.clone());
                    changed |= members.insert(link.right.clone());
                }
            }
        }
        members.into_iter().collect()
    }

    /// Canonicalize a predicate when the supplied map came from this state.
    #[must_use]
    pub fn canonical<'a>(
        &self,
        canon_map: &'a BTreeMap<String, String>,
        predicate: &'a str,
    ) -> &'a str {
        canon(canon_map, predicate)
    }

    /// Project union functionality onto canonical predicate representatives.
    #[must_use]
    pub fn effective_functional(
        &self,
        declared: &BTreeSet<String>,
        canon_map: &BTreeMap<String, String>,
    ) -> BTreeSet<String> {
        let mut effective = BTreeSet::new();
        for predicate in declared {
            effective.insert(canon(canon_map, predicate).to_owned());
        }
        effective
    }

    /// Project union inertia onto canonical predicate representatives.
    #[must_use]
    pub fn effective_inertial(
        &self,
        declared: &BTreeMap<String, Ann>,
        canon_map: &BTreeMap<String, String>,
    ) -> BTreeMap<String, Ann> {
        let mut effective = BTreeMap::new();
        for (predicate, ann) in declared {
            dli_eval::world::or_inertial(&mut effective, canon(canon_map, predicate), ann.clone());
        }
        effective
    }

    /// Citations of certain identity links that participate in a canonical
    /// predicate class.
    #[must_use]
    pub fn citations_for(
        &self,
        predicate: &str,
        canon_map: &BTreeMap<String, String>,
        valuation: &Valuation,
        theory: &Theory,
    ) -> Vec<String> {
        let representative = canon(canon_map, predicate);
        let mut citations = Vec::new();
        for link in self.certain_links(valuation, theory) {
            let left = canon(canon_map, &link.left);
            let right = canon(canon_map, &link.right);
            if left == representative || right == representative {
                for citation in &link.citations {
                    if !citations.contains(citation) {
                        citations.push(citation.clone());
                    }
                }
            }
        }
        citations
    }
}
