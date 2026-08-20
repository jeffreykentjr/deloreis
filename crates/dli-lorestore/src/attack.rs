//! Durable conditional attacks between recorded decisions.
//!
//! An attack is append-only evidence that `attacker` defeats `target` while the
//! attacker itself remains in standing.  Standing is projected from the folded
//! attack graph at read time; the edge never mutates either decision and never
//! stores an interner-local numeric atom.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Authority, ClaimId, ContentHash, SCHEMA_VERSION};
use dli_core::atom::Atom;
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
    /// Storage hash of the exact folded fact object carried by `claim_id`.
    pub subject_fact_hash: ContentHash,
    /// The attacking decision, matched by exact name.
    pub attacker: String,
    /// The attacked decision, matched by exact name.
    pub target: String,
}

/// One vertex's grounded standing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundedLabel {
    /// The decision is undefeated.
    In,
    /// At least one undefeated decision attacks it.
    Out,
    /// No grounded assignment resolves the cyclic component.
    Undecided,
}

/// A standing overlay can explicitly remove a compiler-provided assignment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveValuation {
    /// Explicit true/false assignments, including legacy fixed seeds.
    pub valuation: dli_core::valuation::Valuation,
    /// Atoms whose compiler-provided defaults must be removed.
    pub unassigned: BTreeSet<Atom>,
}

/// Compute the unique grounded labelling, including explicit undecided nodes.
///
/// `fixed` is the legacy Ref overlay, already translated to exact decision
/// names. Those assignments are immutable seeds: v5 adds conditional attacks
/// without reinterpreting a pre-v5 one-way defeat.
#[must_use]
pub fn grounded_labels(
    decisions: &[String],
    attacks: &[DecisionAttack],
    fixed: &BTreeMap<String, bool>,
) -> BTreeMap<String, GroundedLabel> {
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
    let vertices: BTreeSet<String> = attacks
        .iter()
        .flat_map(|attack| [attack.attacker.clone(), attack.target.clone()])
        .filter(|name| known.contains(name.as_str()))
        .collect();
    vertices
        .into_iter()
        .map(|name| {
            let label = match labels.get(&name) {
                Some(true) => GroundedLabel::In,
                Some(false) => GroundedLabel::Out,
                None => GroundedLabel::Undecided,
            };
            (name, label)
        })
        .collect()
}

impl DecisionAttack {
    /// Construct a sovereign attack edge.
    #[must_use]
    pub fn new(
        claim_id: ClaimId,
        subject_fact_hash: ContentHash,
        attacker: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            authority: Authority::sovereign(),
            claim_id,
            subject_fact_hash,
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
            subject_fact_hash: &'a ContentHash,
            attacker: &'a str,
            target: &'a str,
        }
        crate::content_hash(&Projection {
            authority: &self.authority,
            claim_id: &self.claim_id,
            subject_fact_hash: &self.subject_fact_hash,
            attacker: &self.attacker,
            target: &self.target,
        })
    }
}

/// Whether the exact carrier establishes `decision` on every positive path.
pub(crate) fn carrier_supports_decision(
    object: &crate::FactObject,
    interner: &[String],
    decision: &str,
) -> bool {
    if let Some(body) = &object.expert_body {
        if body.kind == crate::ExpertDeclKind::Ratify && body.name == decision {
            return true;
        }
        let Some(text) = body.body.first() else {
            return false;
        };
        let Ok(decl) = serde_json::from_str::<dli_ast::decl::Decl>(text) else {
            return false;
        };
        let annotation = match &decl {
            dli_ast::decl::Decl::Entity(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Attr(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Fact(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Before(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Question(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Dialect(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::NamePart(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Name(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Term(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Banned(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::State(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Functional(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Inertial(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Attest(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::SameAs(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::PredicateSameAs(value) => value.provenance.as_ref(),
            dli_ast::decl::Decl::Rule(value) => value.provenance.as_ref(),
            _ => None,
        };
        return annotation.is_some_and(|annotation| {
            annotation
                .citations
                .iter()
                .any(|citation| citation.id.text == decision)
        });
    }
    let Some(index) = interner.iter().position(|name| name == decision) else {
        return false;
    };
    let Ok(index) = u32::try_from(index) else {
        return false;
    };
    let atom = Atom(index);
    let mut monomials = object.support.monomials().peekable();
    monomials.peek().is_some()
        && monomials.all(|monomial| {
            monomial
                .lits()
                .any(|literal| literal.atom == atom && literal.rail == dli_core::atom::Rail::Pos)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(attacker: &str, target: &str) -> DecisionAttack {
        DecisionAttack::new(
            ClaimId::from_label(target),
            ContentHash(format!("sha256:{:064x}", target.len())),
            attacker,
            target,
        )
    }

    #[test]
    fn grounded_depth_restores_every_second_decision() {
        let decisions = ["d1", "d2", "d3", "d4"].map(str::to_owned).to_vec();
        let labels = grounded_labels(
            &decisions,
            &[edge("d2", "d1"), edge("d3", "d2"), edge("d4", "d3")],
            &BTreeMap::new(),
        );
        assert_eq!(labels.get("d1"), Some(&GroundedLabel::Out));
        assert_eq!(labels.get("d2"), Some(&GroundedLabel::In));
        assert_eq!(labels.get("d3"), Some(&GroundedLabel::Out));
        assert_eq!(labels.get("d4"), Some(&GroundedLabel::In));
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
            ("d1".to_owned(), GroundedLabel::Out),
            ("d2".to_owned(), GroundedLabel::Out),
            ("d3".to_owned(), GroundedLabel::In),
            ("d4".to_owned(), GroundedLabel::In),
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
        assert_eq!(open.get("a"), Some(&GroundedLabel::Undecided));
        assert_eq!(open.get("b"), Some(&GroundedLabel::Undecided));
        assert_eq!(open.get("c"), Some(&GroundedLabel::Undecided));

        let fixed = BTreeMap::from([("a".to_owned(), false)]);
        let seeded = grounded_labels(&decisions, &attacks, &fixed);
        assert_eq!(seeded.get("a"), Some(&GroundedLabel::Out));
        assert_eq!(seeded.get("b"), Some(&GroundedLabel::In));
        assert_eq!(seeded.get("c"), Some(&GroundedLabel::Out));
    }
}
