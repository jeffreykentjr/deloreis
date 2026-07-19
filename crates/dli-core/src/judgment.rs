//! Replayable judgments: a derivation, its verdict, and a digest of the exact
//! inputs that produced it.
//!
//! A judgment is a committed, self-describing record. Serialize it, ship it,
//! deserialize it, and re-derive its verdict from the same bases — the bytes are
//! identical and the verdict reproduces. The digest fingerprints the full input
//! tuple (claim, derivation, and both bases), so a replay against a *different*
//! environment is a detectable mismatch, never a silent divergence (charter
//! koan 4: nothing is silently dropped).

use std::fmt::{self, Write as _};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::derivation::{ClaimId, Derivation, GradeBasis, SupportBasis};
use crate::verdict::Verdict;

/// A `sha256:<hex>` fingerprint of a judgment's canonical inputs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Digest(pub String);

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A replayable judgment: a claim, the derivation that settles it, the resulting
/// two-channel [`Verdict`], and a digest of the inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Judgment {
    /// The claim this judgment settles.
    pub claim: ClaimId,
    /// The derivation forest witnessing it.
    pub derivation: Derivation,
    /// The rolled-up verdict (support × grade).
    pub verdict: Verdict,
    /// A `sha256` digest over the canonical inputs (claim, derivation, bases).
    pub inputs_digest: Digest,
}

/// The exact inputs a judgment is derived from, in canonical field order.
/// Serialized via `serde_json` — which preserves struct field order — over the
/// `BTree`-backed bases, so the digest pre-image is byte-stable.
#[derive(Serialize)]
struct Inputs<'a> {
    claim: ClaimId,
    derivation: &'a Derivation,
    grades: &'a GradeBasis,
    support: &'a SupportBasis,
}

fn digest_of(
    claim: ClaimId,
    derivation: &Derivation,
    grades: &GradeBasis,
    support: &SupportBasis,
) -> Digest {
    let inputs = Inputs {
        claim,
        derivation,
        grades,
        support,
    };
    // serde_json over BTree-backed, order-stable types is canonical here: sorted
    // map keys, declaration-ordered struct fields, sorted PosBool monomials.
    let canonical = serde_json::to_vec(&inputs).expect("judgment inputs serialize to JSON");
    let hash = Sha256::digest(&canonical);
    let mut hex = String::with_capacity(64);
    for byte in hash {
        // Writing to a String is infallible.
        let _ = write!(hex, "{byte:02x}");
    }
    Digest(format!("sha256:{hex}"))
}

/// The outcome of replaying a judgment against a set of bases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    /// Whether the re-derived verdict equals the stored one.
    pub verdict_matches: bool,
    /// Whether the recomputed digest equals the stored one.
    pub digest_matches: bool,
    /// The verdict re-derived from the supplied bases.
    pub verdict: Verdict,
    /// The digest recomputed from the supplied bases.
    pub digest: Digest,
}

impl Replay {
    /// A replay is faithful iff both channels reproduce and the digest matches.
    #[must_use]
    pub fn is_faithful(&self) -> bool {
        self.verdict_matches && self.digest_matches
    }
}

impl Judgment {
    /// Derive a judgment for `claim` from its `derivation` and the two bases:
    /// roll the grade channel up over the procedure chain, rebuild the support
    /// channel from the forest, and fingerprint the exact inputs.
    #[must_use]
    pub fn derive(
        claim: ClaimId,
        derivation: Derivation,
        grades: &GradeBasis,
        support: &SupportBasis,
    ) -> Self {
        let verdict = Verdict::new(derivation.support_of(support), derivation.grade_of(grades));
        let inputs_digest = digest_of(claim, &derivation, grades, support);
        Self {
            claim,
            derivation,
            verdict,
            inputs_digest,
        }
    }

    /// Re-derive this judgment's verdict and digest from a set of bases and
    /// compare them to what was stored.
    ///
    /// A faithful replay reproduces both channels and matches the digest; any
    /// divergence is reported as a [`Replay`] whose flags are `false`, never a
    /// silent difference (charter koan 4). The digest pins the *whole* input
    /// environment, so replaying against different bases is caught even when the
    /// re-derived verdict happens to coincide.
    #[must_use]
    pub fn replay(&self, grades: &GradeBasis, support: &SupportBasis) -> Replay {
        let verdict = Verdict::new(
            self.derivation.support_of(support),
            self.derivation.grade_of(grades),
        );
        let digest = digest_of(self.claim, &self.derivation, grades, support);
        Replay {
            verdict_matches: verdict == self.verdict,
            digest_matches: digest == self.inputs_digest,
            verdict,
            digest,
        }
    }
}
