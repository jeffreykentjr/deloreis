//! Queries and their answers.
//!
//! A [`Query`] is a goal atom; [`World::query`](crate::world::World::query)
//! answers it under the world's ambient valuation. [`World::query_with`] is the
//! richer entry point the serving surface calls: it layers a standing floor, an
//! as-of time filter, a fork valuation overlay, and optional `--why`
//! explanations onto the same evaluation, and `query` is exactly its
//! default-options case.

use std::collections::{BTreeMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_ast::term::AtomExpr;
use dli_core::ann::Status;
use dli_core::valuation::Valuation;
use dli_core::verdict::Verdict;
use dli_time::chronology::TimePoint;

use crate::error::EvalError;
use crate::why::Why;
use crate::world::{GroundAtom, World};

/// A query: a goal atom, possibly with variables to bind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Query {
    /// The goal to solve.
    pub goal: AtomExpr,
}

/// One answer row: the matched ground atom, its variable bindings, the verdict
/// (support and grade) for that binding, and the atom's status under the
/// valuation the query was evaluated in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryRow {
    /// The matched ground atom, rendered `pred(a, b)`.
    pub atom: String,
    /// Variable name to bound-term rendering.
    pub bindings: BTreeMap<String, String>,
    /// The support and grade for this binding.
    pub verdict: Verdict,
    /// The atom's standing under the ambient valuation.
    pub status: Status,
    /// The support antichain rendered for a reader, present only when the query
    /// asked for it ([`QueryOpts::why`]). Absent otherwise, so a plain query's
    /// wire form is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<Why>,
}

/// A full query answer.
///
/// # Exit status (the CLI contract)
///
/// The batch surface returns an [`Answer`]; a CLI (`CLI-1`, next wave) maps it
/// to a process exit status. The semantics this crate fixes, leaving the
/// concrete numeric codes to the CLI:
///
/// - **found** — at least one row satisfies the goal in the requested layer.
///   The success exit.
/// - **empty** — the query is well-formed and evaluated, but no row satisfies
///   the goal in the requested layer ([`Answer::is_empty`]). A *distinct*
///   non-success status: "asked and answered, nothing matched" is not the same
///   as "the call failed", and a script must be able to tell them apart.
/// - **error** — evaluation could not run: an [`EvalError`] (unknown predicate,
///   unstratifiable program). A third, separate status.
///
/// The three are disjoint. An empty answer is never an error — a diagnosed
/// absence is a result the consumer is meant to act on (`10-serving.md` §2.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Answer {
    /// The answer rows.
    pub rows: Vec<QueryRow>,
}

impl Answer {
    /// Whether the answer carries no rows — the *empty* exit status above.
    ///
    /// ```
    /// use dli_eval::query::Answer;
    /// assert!(Answer::default().is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// The standing floor a query answer is filtered to.
///
/// A `Layer` is a **minimum-standing floor** on the answer rows: a row is kept
/// when its [`Status`] is at least the floor. Because `Status` orders
/// `Dead < Possible < Certain`, the layers nest — `Certain ⊆ Possible ⊆ All`:
///
/// - [`Layer::Certain`] keeps only assertable rows (`Status::Certain`).
/// - [`Layer::Possible`] keeps the shadow **and** the certain layer
///   (`Status::Possible` and `Status::Certain`). Possibility does not exclude
///   certainty: a certain claim is a fortiori possible.
/// - [`Layer::All`] applies no floor — the default.
///
/// **Dead atoms never appear in any layer.** A claim whose every monomial is
/// `Θ`-unsatisfiable is fail-closed-excluded from the evaluated surface
/// (`02-semantics.md` §3: "excluded from every layer except the error report").
/// So `Possible` and `All` return the same rows on today's query surface and
/// differ only in declared intent; surfacing dead atoms is the error report's
/// job, not a query layer's.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// Only assertable rows.
    Certain,
    /// The shadow layer and the certain layer together.
    Possible,
    /// No standing floor.
    #[default]
    All,
}

impl Layer {
    /// The minimum standing this layer admits.
    #[must_use]
    fn floor(self) -> Status {
        match self {
            Layer::Certain => Status::Certain,
            Layer::Possible => Status::Possible,
            Layer::All => Status::Dead,
        }
    }

    /// Whether a row of the given standing survives this layer's floor.
    #[must_use]
    fn keeps(self, status: Status) -> bool {
        status >= self.floor()
    }
}

/// The options that shape a query: which standing layer to return, an optional
/// as-of time filter, an optional fork valuation overlay, and whether to attach
/// `--why` explanations.
///
/// [`QueryOpts::default`] — the `All` layer, no time filter, no fork, no
/// explanations — reproduces [`World::query`] exactly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct QueryOpts {
    /// The standing floor for the returned rows.
    pub layer: Layer,
    /// An era or event name: rows anchored to a time point the chronology
    /// orders strictly *after* it are dropped. `None` applies no time filter.
    ///
    /// This is the minimal seam — an unknown name filters nothing, and ordering
    /// is computed locally over the precedence edges. Deeper time integration
    /// (`03-time.md`, TIME-1's transitive `precedes`) lands next wave.
    pub as_of: Option<String>,
    /// A fork's valuation overlay, applied over the world's ambient valuation
    /// (the overlay wins) before evaluation. `None` evaluates under the ambient
    /// ledger. The fork lane owns producing the overlay — including ruling an
    /// option's exclusivity siblings false — because the overlay is applied
    /// verbatim, with no re-derivation (`02-semantics.md` §5, forks are
    /// valuations).
    pub fork: Option<Valuation>,
    /// Attach the support antichain ([`Why`]) to every row.
    pub why: bool,
}

impl World {
    /// Answer a query under a set of [`QueryOpts`] — the layered, forkable,
    /// explainable entry point behind `dli.query` (`10-serving.md`).
    ///
    /// The pipeline is: evaluate under the ambient valuation (or the fork
    /// overlay laid over it); drop rows below the [`Layer`] floor; drop rows
    /// anchored strictly after the `as_of` point; then, if asked, attach each
    /// row's [`Why`]. The symbolic saturation is never touched — a fork or a
    /// filter is a read over the precomputed supports, so this costs one
    /// evaluation pass and no re-lowering (`08-reasoning.md` §2).
    ///
    /// [`World::query`] is the default-options case:
    /// `query_with(q, &QueryOpts::default())` returns the same rows.
    ///
    /// ```
    /// use dli_ast::ident::Ident;
    /// use dli_ast::term::{AtomExpr, Term};
    /// use dli_core::ann::{Ann, Status};
    /// use dli_core::atom::Atom;
    /// use dli_core::posbool::PosBool;
    /// use dli_eval::query::{Layer, Query, QueryOpts};
    /// use dli_eval::world::{GroundAtom, World};
    ///
    /// let mut world = World::new();
    /// // One unconditional (certain) fact and one open-variable (possible) fact.
    /// let keeps = |who: &str| GroundAtom { pred: "keeps".to_owned(), args: vec![who.to_owned()] };
    /// world.assert_fact(keeps("fen"), Ann::unconditional());
    /// world.assert_fact(keeps("mara"), Ann::supported(PosBool::pos(Atom(0))));
    /// let world = world.fixpoint().expect("stratifiable");
    ///
    /// let goal = Query {
    ///     goal: AtomExpr {
    ///         pred: Ident::synthetic("keeps"),
    ///         args: vec![Term::Var(Ident::synthetic("who"))],
    ///     },
    /// };
    /// // The certain layer keeps only the unconditional fact.
    /// let certain = world
    ///     .query_with(&goal, &QueryOpts { layer: Layer::Certain, ..Default::default() })
    ///     .expect("known predicate");
    /// assert_eq!(certain.rows.len(), 1);
    /// assert_eq!(certain.rows[0].status, Status::Certain);
    /// // The possible layer keeps the shadow row too.
    /// let possible = world
    ///     .query_with(&goal, &QueryOpts { layer: Layer::Possible, ..Default::default() })
    ///     .expect("known predicate");
    /// assert_eq!(possible.rows.len(), 2);
    /// ```
    pub fn query_with(&self, query: &Query, opts: &QueryOpts) -> Result<Answer, EvalError> {
        let mut answer = match &opts.fork {
            Some(overlay) => {
                let mut valuation = self.valuation.clone();
                for (atom, value) in &overlay.0 {
                    valuation.set(*atom, *value);
                }
                self.eval_under(&valuation, query)?
            }
            None => self.query(query)?,
        };

        answer.rows.retain(|row| opts.layer.keeps(row.status));

        if let Some(as_of) = &opts.as_of {
            let after = self.strictly_after(as_of);
            if !after.is_empty() {
                let blocked = self.time_blocked_atoms(&after);
                answer.rows.retain(|row| !blocked.contains(&row.atom));
            }
        }

        if opts.why {
            for row in &mut answer.rows {
                row.why = Some(Why::of_support(&row.verdict.support, &self.interner));
            }
        }

        Ok(answer)
    }

    /// The time points the chronology orders strictly after `name`: a forward
    /// reachability sweep over the precedence edges. An unknown name yields the
    /// empty set (nothing is known to follow it), so an as-of filter over an
    /// unrecognized point drops nothing.
    ///
    /// Computed locally rather than through `Chronology::precedes` (TIME-1,
    /// still unlanded); the seam moves there next wave.
    fn strictly_after(&self, name: &str) -> HashSet<TimePoint> {
        let mut after = HashSet::new();
        let Some(start) = self.time_point_index(name) else {
            return after;
        };
        let mut frontier = vec![start];
        while let Some(point) = frontier.pop() {
            for (from, to) in &self.chronology.edges {
                if *from == point && *to != start && after.insert(*to) {
                    frontier.push(*to);
                }
            }
        }
        after
    }

    /// The interned time point of a name, if the timeline knows it.
    fn time_point_index(&self, name: &str) -> Option<TimePoint> {
        self.time_points
            .names()
            .iter()
            .position(|candidate| candidate == name)
            .and_then(|index| u32::try_from(index).ok())
            .map(TimePoint)
    }

    /// The rendered atoms anchored to a time point in `after` — the rows an
    /// as-of filter drops.
    fn time_blocked_atoms(&self, after: &HashSet<TimePoint>) -> HashSet<String> {
        self.derived
            .iter()
            .filter(|derived| self.ground_after(&derived.atom, after))
            .map(|derived| derived.atom.render())
            .collect()
    }

    /// Whether a ground atom is anchored to a time point the sweep marks as
    /// strictly-after. An argument anchors when it renders `era:<name>` or
    /// `event:<name>` and that name is a known, after-set time point.
    fn ground_after(&self, atom: &GroundAtom, after: &HashSet<TimePoint>) -> bool {
        atom.args.iter().any(|arg| {
            time_anchor_name(arg)
                .and_then(|name| self.time_point_index(name))
                .is_some_and(|point| after.contains(&point))
        })
    }
}

/// The era/event name a ground argument anchors to, stripping the render
/// prefix, or `None` when the argument is not a time anchor.
fn time_anchor_name(arg: &str) -> Option<&str> {
    arg.strip_prefix("era:")
        .or_else(|| arg.strip_prefix("event:"))
}
