//! Predicate-identity closure for the evaluator's separate declaration channel.
//!
//! Predicate identity has the same equivalence semantics as entity identity,
//! but it must not share the entity declaration/fact channel. These helpers
//! provide the deterministic, lexicographically rooted view used by predicate
//! consumers while leaving provenance propagation to the evaluator's support
//! rules.

use std::collections::{BTreeMap, BTreeSet};

/// Build the deterministic canonical representative map for predicate pairs.
///
/// The relation is undirected and transitively closed. The lexicographically
/// smallest member of each component is its representative, independent of
/// declaration order. Self-pairs are semantic no-ops.
#[must_use]
pub fn predicate_canon_map<I: IntoIterator<Item = (String, String)>>(
    pairs: I,
) -> BTreeMap<String, String> {
    crate::alias::canon_map(pairs)
}

/// Return every distinct ordered pair in each predicate identity component.
///
/// The result excludes self-pairs and is sorted by `BTreeSet`, making it safe
/// to use as a deterministic rule-generation plan. This is a closure view;
/// each generated consequence still needs the declaration supports that
/// justified its path, which the evaluator carries separately.
#[must_use]
pub fn predicate_closure_pairs<I: IntoIterator<Item = (String, String)>>(
    pairs: I,
) -> Vec<(String, String)> {
    let pairs: Vec<(String, String)> = pairs.into_iter().collect();
    let map = predicate_canon_map(pairs.clone());
    let mut members = BTreeSet::new();
    for (left, right) in pairs {
        members.insert(left);
        members.insert(right);
    }

    let mut closure = BTreeSet::new();
    let members: Vec<String> = members.into_iter().collect();
    for left in &members {
        for right in &members {
            if left != right
                && crate::alias::canon(&map, left) == crate::alias::canon(&map, right)
            {
                closure.insert((left.clone(), right.clone()));
            }
        }
    }
    closure.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(left: &str, right: &str) -> (String, String) {
        (left.to_owned(), right.to_owned())
    }

    #[test]
    fn closure_is_symmetric_transitive_and_order_stable() {
        let forward = predicate_closure_pairs([
            pair("current-work", "current-job"),
            pair("current-job", "occupation"),
        ]);
        let reverse = predicate_closure_pairs([
            pair("occupation", "current-job"),
            pair("current-job", "current-work"),
        ]);
        assert_eq!(forward, reverse);
        assert_eq!(
            forward,
            vec![
                pair("current-job", "current-work"),
                pair("current-job", "occupation"),
                pair("current-work", "current-job"),
                pair("current-work", "occupation"),
                pair("occupation", "current-job"),
                pair("occupation", "current-work"),
            ]
        );
    }

    #[test]
    fn self_pair_is_a_no_op() {
        assert!(predicate_closure_pairs([pair("rank", "rank")]).is_empty());
        assert!(predicate_canon_map([pair("rank", "rank")]).is_empty());
    }
}
