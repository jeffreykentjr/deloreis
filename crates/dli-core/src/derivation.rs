//! Derivations: proof trees over claims and rules, and their two-channel rollup.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::grade::Grade;
use crate::posbool::PosBool;

/// A claim identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct ClaimId(pub u32);

/// A rule identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct RuleId(pub u32);

/// A derivation tree witnessing why a claim holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Derivation {
    /// A leaf: the claim is an axiom (asserted directly).
    Axiom {
        /// The asserted claim.
        claim: ClaimId,
    },
    /// An internal node: a rule applied to sub-derivations.
    Step {
        /// The rule applied.
        rule: RuleId,
        /// The premises this step consumed.
        from: Vec<Derivation>,
    },
}

/// The base grades of the leaves a derivation stands on: how each axiom claim
/// and each rule was *established*.
///
/// This is the ground the procedure rollup ([`Derivation::grade_of`]) takes its
/// minimum over. A leaf absent from the basis defaults to [`Grade::Asserted`] —
/// the fail-closed reading of `02-semantics.md` §3: with no recorded procedure
/// there is nothing to re-run, so the weakest grade stands.
///
/// The maps are `BTree`-backed so serialization is key-sorted and byte-stable,
/// which the [`crate::judgment`] digest depends on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GradeBasis {
    /// The grade at which each axiom claim was established.
    pub claims: BTreeMap<ClaimId, Grade>,
    /// The grade at which each rule was established.
    pub rules: BTreeMap<RuleId, Grade>,
}

impl GradeBasis {
    /// The grade of an axiom claim, or [`Grade::Asserted`] if unrecorded.
    #[must_use]
    pub fn claim_grade(&self, claim: ClaimId) -> Grade {
        self.claims.get(&claim).copied().unwrap_or(Grade::Asserted)
    }

    /// The grade of a rule, or [`Grade::Asserted`] if unrecorded.
    #[must_use]
    pub fn rule_grade(&self, rule: RuleId) -> Grade {
        self.rules.get(&rule).copied().unwrap_or(Grade::Asserted)
    }
}

/// The base support formulas of the leaves a derivation stands on: what each
/// axiom claim and each rule rests on in channel one.
///
/// This is the ground the support rollup ([`Derivation::support_of`]) multiplies
/// through. A leaf absent from the basis defaults to [`PosBool::FALSE`] — the
/// fail-closed theorem of `02-semantics.md` §3: an annotation that fails to
/// attach yields `P⁺ = 0`, excluded from every layer but the error report. A
/// structural, always-available rule must therefore be given [`PosBool::truth`]
/// explicitly; absence is never silently read as "always on".
///
/// The maps are `BTree`-backed so serialization is key-sorted and byte-stable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SupportBasis {
    /// What each axiom claim rests on.
    pub claims: BTreeMap<ClaimId, PosBool>,
    /// What each rule rests on (its own authorial variable).
    pub rules: BTreeMap<RuleId, PosBool>,
}

impl SupportBasis {
    /// What an axiom claim rests on, or [`PosBool::FALSE`] if unrecorded.
    #[must_use]
    pub fn claim_support(&self, claim: ClaimId) -> PosBool {
        self.claims.get(&claim).cloned().unwrap_or(PosBool::FALSE)
    }

    /// What a rule rests on, or [`PosBool::FALSE`] if unrecorded.
    #[must_use]
    pub fn rule_support(&self, rule: RuleId) -> PosBool {
        self.rules.get(&rule).cloned().unwrap_or(PosBool::FALSE)
    }
}

impl Derivation {
    /// The grade of this derivation: the weakest-link minimum over its entire
    /// **procedure chain** — every rule applied and every axiom leaf, combined
    /// with `min` (`07-verdicts-and-llm-boundary.md` §1; charter koan 6).
    ///
    /// The rollup walks the procedure and never descends into the *citations*: a
    /// proof over fiat-ratified premises stays `checked` when its every step
    /// re-derives exactly, because the fiat rides in the support channel
    /// ([`Derivation::support_of`]), not here. This is the answer to the collapse
    /// worry — grade and support are two evaluations of one tree.
    ///
    /// A premise-less step is graded by its rule alone: the empty premise chain
    /// contributes the `min` identity ([`Grade::weakest`]).
    #[must_use]
    pub fn grade_of(&self, basis: &GradeBasis) -> Grade {
        match self {
            Derivation::Axiom { claim } => basis.claim_grade(*claim),
            Derivation::Step { rule, from } => Grade::min(
                basis.rule_grade(*rule),
                Grade::weakest(from.iter().map(|premise| premise.grade_of(basis))),
            ),
        }
    }

    /// The support of this derivation: the channel-one formula rebuilt from the
    /// forest.
    ///
    /// An axiom rests on its recorded base support. A step multiplies its rule's
    /// own support into the conjunction of its premises' supports — a derived
    /// relation is no more certain than the rule and the premises it stands on
    /// together (`02-semantics.md` §2; V9 recovered as a theorem). The result is
    /// absorbed to stay minimal.
    ///
    /// This consumes [`PosBool::and_`] and [`PosBool::absorb`]; a step over an
    /// unsupported leaf (base [`PosBool::FALSE`]) yields `FALSE`, the fail-closed
    /// reading.
    #[must_use]
    pub fn support_of(&self, basis: &SupportBasis) -> PosBool {
        match self {
            Derivation::Axiom { claim } => basis.claim_support(*claim),
            Derivation::Step { rule, from } => {
                let mut acc = basis.rule_support(*rule);
                for premise in from {
                    acc = acc.and_(&premise.support_of(basis));
                }
                acc.absorb()
            }
        }
    }
}
