//! Verdicts: a support formula paired with a grade — two orthogonal channels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::grade::Grade;
use crate::posbool::PosBool;

/// The outcome of asking about a claim, carried over two orthogonal channels
/// that never entangle (`docs/spec/07-verdicts-and-llm-boundary.md` §1):
///
/// - **support** — the positive-boolean formula over citations and assumptions:
///   *what the conclusion rests on* (`02-semantics.md`).
/// - **grade** — *how the procedure stacked on that ground was verified.*
///
/// The channels are independent: a conclusion may rest on weak-authority
/// citations in the support channel and still be `checked` in the grade
/// channel, because grade rolls up over the procedure chain and never descends
/// into the citations. See [`Verdict::of_procedure`] for the worked answer to
/// the collapse worry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Verdict {
    /// Channel one: the hypotheses under which the claim holds.
    pub support: PosBool,
    /// Channel two: the strength of the establishing procedure.
    pub grade: Grade,
}

impl Verdict {
    /// Pair a support formula with a grade directly.
    #[must_use]
    pub fn new(support: PosBool, grade: Grade) -> Self {
        Self { support, grade }
    }

    /// Build a verdict from an independent support channel and a procedure
    /// chain, rolling the chain up with weakest-link semantics
    /// ([`verdict_of_procedure`]).
    ///
    /// This is the two-channel constructor: `support` is passed verbatim, and
    /// the grade is computed **only** from `steps` — the two never mix.
    ///
    /// # The collapse worry, answered
    ///
    /// A `checked` conclusion may cite evidence of any authority in its support
    /// channel; the citation lives in channel one and does not drag the grade
    /// down. Here the conclusion rests on a single cited premise, yet every step
    /// of the procedure re-derives exactly, so the verdict is `checked`:
    ///
    /// ```
    /// use std::collections::BTreeSet;
    ///
    /// use dli_core::atom::{Atom, Lit, Rail};
    /// use dli_core::posbool::{Monomial, PosBool};
    /// use dli_core::{Grade, Verdict};
    ///
    /// // Channel one: the conclusion rests on one cited premise. Its authority
    /// // — even a mere attestation — is metadata outside the algebra; the
    /// // support formula sees only a Boolean variable.
    /// let cited = PosBool(BTreeSet::from([Monomial(BTreeSet::from([Lit {
    ///     atom: Atom(7),
    ///     rail: Rail::Pos,
    /// }]))]));
    ///
    /// // Channel two: every step of the procedure re-derives exactly.
    /// let v = Verdict::of_procedure(cited.clone(), [Grade::Checked, Grade::Checked]);
    /// assert_eq!(v.grade, Grade::Checked); // stays checked — rollup ignores the citation
    /// assert_eq!(v.support, cited); //         the citation is untouched in channel one
    ///
    /// // Weaken the *procedure* and only the grade moves; support is unchanged.
    /// let w = Verdict::of_procedure(cited.clone(), [Grade::Checked, Grade::Attested]);
    /// assert_eq!(w.grade, Grade::Attested);
    /// assert_eq!(w.support, cited);
    /// ```
    #[must_use]
    pub fn of_procedure(support: PosBool, steps: impl IntoIterator<Item = Grade>) -> Self {
        Self::new(support, verdict_of_procedure(steps))
    }

    /// Whether this verdict meets a grade floor — the `--min-grade` filter.
    ///
    /// `deloreis report --min-grade tested` asks what still rides on say-so: a
    /// verdict passes iff its grade is at least `floor`.
    #[must_use]
    pub fn meets_min_grade(&self, floor: Grade) -> bool {
        self.grade >= floor
    }

    /// Chain this verdict onto the next step of a procedure, combining both
    /// channels at once.
    ///
    /// Support is `self ∧ next` — the conclusion now rests on *both* steps'
    /// citations. Grade is the weakest of the two — the minimum over the
    /// procedure chain, **never over the citations**
    /// (`07-verdicts-and-llm-boundary.md` §1; charter koan 6). Folding a whole
    /// procedure with `chain` rolls the grade channel up exactly as
    /// [`verdict_of_procedure`] does, while accumulating the support on the
    /// other channel.
    ///
    /// ```
    /// use dli_core::atom::Atom;
    /// use dli_core::posbool::PosBool;
    /// use dli_core::{Grade, Verdict};
    ///
    /// // Two steps, each resting on its own citation.
    /// let step1 = Verdict::new(PosBool::pos(Atom(0)), Grade::Checked);
    /// let step2 = Verdict::new(PosBool::pos(Atom(1)), Grade::Tested);
    /// let chained = step1.chain(&step2);
    ///
    /// // Channel two: the weakest link of the chain.
    /// assert_eq!(chained.grade, Grade::Tested);
    /// // Channel one: the conjunction of what each step rests on.
    /// assert_eq!(
    ///     chained.support,
    ///     PosBool::pos(Atom(0)).and_(&PosBool::pos(Atom(1)))
    /// );
    /// ```
    #[must_use]
    pub fn chain(&self, next: &Verdict) -> Verdict {
        Verdict::new(self.support.and_(&next.support), self.grade.min(next.grade))
    }
}

/// Weakest-link rollup of a procedure chain to a single grade.
///
/// The grade is the minimum over the chain — every step must hold, so the
/// verdict is only as strong as its weakest procedure
/// (`07-verdicts-and-llm-boundary.md` §1; charter koan 6). A procedure with
/// **no steps at all** rolls up to [`Grade::Asserted`]: there is no procedure,
/// so there is nothing to re-run. (This is the one place the empty case means
/// `asserted` rather than the `min` identity — contrast [`Grade::weakest`],
/// which combines the *steps within* a procedure and whose empty case is the
/// top.)
///
/// # Examples
/// ```
/// use dli_core::verdict::verdict_of_procedure;
/// use dli_core::Grade;
///
/// assert_eq!(
///     verdict_of_procedure([Grade::Checked, Grade::Tested, Grade::Checked]),
///     Grade::Tested,
/// );
/// assert_eq!(verdict_of_procedure(std::iter::empty()), Grade::Asserted);
/// ```
#[must_use]
pub fn verdict_of_procedure(steps: impl IntoIterator<Item = Grade>) -> Grade {
    let mut steps = steps.into_iter();
    match steps.next() {
        None => Grade::Asserted,
        Some(first) => steps.fold(first, Grade::min),
    }
}
