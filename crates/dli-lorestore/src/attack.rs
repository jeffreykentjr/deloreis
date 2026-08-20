//! Durable conditional attacks between recorded decisions.
//!
//! An attack is append-only evidence that `attacker` defeats `target` while the
//! attacker itself remains in standing.  Standing is projected from the folded
//! attack graph at read time; the edge never mutates either decision and never
//! stores an interner-local numeric atom.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Authority, ClaimId, ContentHash, SCHEMA_VERSION};
use std::collections::{BTreeMap, BTreeSet};

/// A content-addressed, exact-name attack between two recorded decisions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionAttack {
    /// Schema version; excluded from the semantic identity projection.
    pub schema_version: u32,
    /// The authority recording the attack (unset in sovereign deployments).
    pub authority: Authority,
    /// The stable claim whose support established `target` for this defeat.
    pub claim_id: ClaimId,
    /// The attacking decision, matched by exact name.
    pub attacker: String,
    /// The attacked decision, matched by exact name.
    pub target: String,
}

/// Compute the unique grounded labelling of a decision-attack graph.
///
/// `fixed` is the legacy Ref overlay, already translated to exact decision
/// names. Those assignments are immutable seeds: v5 adds conditional attacks
/// without reinterpreting a pre-v5 one-way defeat. Decisions left undecided by
/// a cycle are omitted from the result.
#[must_use]
pub fn grounded_labels(
    decisions: &[String],
    attacks: &[DecisionAttack],
    fixed: &BTreeMap<String, bool>,
) -> BTreeMap<String, bool> {
    let known: BTreeSet<&str> = decisions.iter().map(String::as_str).collect();
    let fixed_decisions: BTreeSet<&str> = fixed
        .keys()
        .map(String::as_str)
        .filter(|name| known.contains(name))
        .collect();
    let mut labels: BTreeMap<String, bool> = fixed
        .iter()
        .filter(|(name, _)| known.contains(name.as_str()))
        .map(|(name, value)| (name.clone(), *value))
        .collect();
    let mut attackers: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for attack in attacks {
        if known.contains(attack.attacker.as_str()) && known.contains(attack.target.as_str()) {
            attackers
                .entry(attack.target.as_str())
                .or_default()
                .insert(attack.attacker.as_str());
        }
    }

    loop {
        let mut changed = false;
        for decision in decisions {
            if labels.contains_key(decision) || fixed_decisions.contains(decision.as_str()) {
                continue;
            }
            let incoming = attackers.get(decision.as_str());
            let next = match incoming {
                None => Some(true),
                Some(incoming)
                    if incoming
                        .iter()
                        .any(|attacker| labels.get(*attacker) == Some(&true)) =>
                {
                    Some(false)
                }
                Some(incoming)
                    if incoming
                        .iter()
                        .all(|attacker| labels.get(*attacker) == Some(&false)) =>
                {
                    Some(true)
                }
                Some(_) => None,
            };
            if let Some(value) = next {
                labels.insert(decision.clone(), value);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    labels
}

impl DecisionAttack {
    /// Construct a sovereign attack edge.
    #[must_use]
    pub fn new(claim_id: ClaimId, attacker: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority: Authority::sovereign(),
            claim_id,
            attacker: attacker.into(),
            target: target.into(),
        }
    }

    /// Semantic identity of this attack, stable across schema-version bumps.
    pub fn content_hash(&self) -> Result<ContentHash, crate::CanonicalError> {
        #[derive(Serialize)]
        struct Projection<'a> {
            authority: &'a Authority,
            claim_id: &'a ClaimId,
            attacker: &'a str,
            target: &'a str,
        }
        crate::content_hash(&Projection {
            authority: &self.authority,
            claim_id: &self.claim_id,
            attacker: &self.attacker,
            target: &self.target,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(attacker: &str, target: &str) -> DecisionAttack {
        DecisionAttack::new(ClaimId::from_label(target), attacker, target)
    }

    #[test]
    fn grounded_depth_restores_every_second_decision() {
        let decisions = ["d1", "d2", "d3", "d4"].map(str::to_owned).to_vec();
        let labels = grounded_labels(
            &decisions,
            &[edge("d2", "d1"), edge("d3", "d2"), edge("d4", "d3")],
            &BTreeMap::new(),
        );
        assert_eq!(labels.get("d1"), Some(&false));
        assert_eq!(labels.get("d2"), Some(&true));
        assert_eq!(labels.get("d3"), Some(&false));
        assert_eq!(labels.get("d4"), Some(&true));
    }

    #[test]
    fn multiple_attackers_and_order_are_deterministic() {
        let a = vec![edge("d2", "d1"), edge("d3", "d1"), edge("d4", "d2")];
        let mut b = a.clone();
        b.reverse();
        let decisions_a = ["d1", "d2", "d3", "d4"].map(str::to_owned).to_vec();
        let mut decisions_b = decisions_a.clone();
        decisions_b.reverse();
        let expected = BTreeMap::from([
            ("d1".to_owned(), false),
            ("d2".to_owned(), false),
            ("d3".to_owned(), true),
            ("d4".to_owned(), true),
        ]);
        assert_eq!(
            grounded_labels(&decisions_a, &a, &BTreeMap::new()),
            expected
        );
        assert_eq!(
            grounded_labels(&decisions_b, &b, &BTreeMap::new()),
            expected
        );
    }

    #[test]
    fn cycles_stay_open_and_legacy_seeds_remain_fixed() {
        let decisions = ["a", "b", "c"].map(str::to_owned).to_vec();
        let attacks = [edge("a", "b"), edge("b", "a"), edge("b", "c")];
        let open = grounded_labels(&decisions, &attacks, &BTreeMap::new());
        assert!(!open.contains_key("a"));
        assert!(!open.contains_key("b"));
        assert!(!open.contains_key("c"));

        let fixed = BTreeMap::from([("a".to_owned(), false)]);
        let seeded = grounded_labels(&decisions, &attacks, &fixed);
        assert_eq!(seeded.get("a"), Some(&false));
        assert_eq!(seeded.get("b"), Some(&true));
        assert_eq!(seeded.get("c"), Some(&false));
    }
}
