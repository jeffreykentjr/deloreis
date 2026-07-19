//! Claim grades: the monotone strength-of-knowledge order.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How strongly a claim is known, weakest to strongest.
///
/// The derived `Ord` is the semantics: `Asserted < Attested < Tested <
/// Checked`. A claim's grade can only rise as evidence accumulates.
///
/// The grade axis measures the *recheckability of the establishing procedure*,
/// not the authority of the speaker (`docs/spec/07-verdicts-and-llm-boundary.md`
/// §1). `Asserted` is the strongest word and the weakest evidence: authorial
/// fiat grades below a model's opinion because there is nothing to re-run — not
/// because the author is outranked. Authority lives in the valuation layer; the
/// lattice ranks evidence, the ledger ranks authority (charter koan 6).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    /// Grounded in will; there is nothing to re-run — author's word.
    Asserted,
    /// A stored model judgment: re-posable and contestable, not re-derivable.
    Attested,
    /// Re-derives statistically — a battery over a committed spec and seed.
    Tested,
    /// Re-derives exactly — anyone re-runs it and gets the same result.
    Checked,
}

impl Grade {
    /// Every grade, weakest to strongest.
    pub const ALL: [Grade; 4] = [
        Grade::Asserted,
        Grade::Attested,
        Grade::Tested,
        Grade::Checked,
    ];

    /// The canonical lowercase name — the exact token serde reads and writes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Grade::Asserted => "asserted",
            Grade::Attested => "attested",
            Grade::Tested => "tested",
            Grade::Checked => "checked",
        }
    }

    /// Weakest-link rollup over a procedure chain: the minimum grade.
    ///
    /// This is the meet along a *required* chain — every step must hold, so the
    /// chain is only as strong as its weakest link
    /// (`07-verdicts-and-llm-boundary.md` §1). Rollup is taken over the
    /// procedure chain, **never over the citations**: a checked procedure over
    /// fiat-ratified premises stays `checked`, because the fiat rides in the
    /// support channel, not here.
    ///
    /// The empty chain returns [`Grade::Checked`] — the lattice top and the
    /// identity of `min`, so an empty chain applies no weakening. This is the
    /// composable primitive; the *semantic* decision that a procedure with **no
    /// steps at all** is [`Grade::Asserted`] (nothing to re-run) lives in
    /// [`crate::verdict::verdict_of_procedure`], not here.
    ///
    /// # Examples
    /// ```
    /// use dli_core::Grade;
    /// assert_eq!(Grade::weakest([Grade::Checked, Grade::Checked]), Grade::Checked);
    /// assert_eq!(Grade::weakest([Grade::Checked, Grade::Tested]), Grade::Tested);
    /// assert_eq!(Grade::weakest(std::iter::empty()), Grade::Checked);
    /// ```
    #[must_use]
    pub fn weakest(grades: impl IntoIterator<Item = Grade>) -> Grade {
        grades.into_iter().fold(Grade::Checked, Grade::min)
    }

    /// Best-of rollup over *alternative* procedures: the maximum grade.
    ///
    /// Where [`weakest`](Grade::weakest) is the meet along one required
    /// chain, `join_alt` is the join across independent alternatives — a claim
    /// with two derivations is graded by its *strongest* available one. This is
    /// the "join" half of the join-of-meets in `02-semantics.md` §2, applied to
    /// the verification-grade lattice: meet along a conjunction/chain, join
    /// across a disjunction/alternatives. The two docs reconcile as one pair of
    /// combinators over two different lattices (support-kind vs. grade), not a
    /// contradiction.
    ///
    /// The empty set of alternatives returns [`Grade::Asserted`] — the lattice
    /// bottom and the identity of `max`. No amount of consensus lifts a grade:
    /// folding any number of `attested` alternatives yields `attested`, never
    /// `tested` (`07-verdicts-and-llm-boundary.md` §3). `Asserted` dominates
    /// nothing here — it never wins a join.
    #[must_use]
    pub fn join_alt(grades: impl IntoIterator<Item = Grade>) -> Grade {
        grades.into_iter().fold(Grade::Asserted, Grade::max)
    }
}

impl fmt::Display for Grade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error from parsing a [`Grade`] out of an unknown token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseGradeError(String);

impl fmt::Display for ParseGradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown grade {:?}; expected one of asserted, attested, tested, checked",
            self.0
        )
    }
}

impl std::error::Error for ParseGradeError {}

impl FromStr for Grade {
    type Err = ParseGradeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "asserted" => Ok(Grade::Asserted),
            "attested" => Ok(Grade::Attested),
            "tested" => Ok(Grade::Tested),
            "checked" => Ok(Grade::Checked),
            other => Err(ParseGradeError(other.to_owned())),
        }
    }
}
