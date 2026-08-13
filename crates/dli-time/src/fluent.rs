//! Fluent timelines: facts that acquire validity spans over chronology.
//!
//! A fluent is `atom × span` (`docs/spec/03-time.md` §2–§3): a value in force
//! over a stretch of story-time, `since` an initiating event and `until` a
//! terminating one. Because the event order is *partial*, projecting a fluent
//! onto a query point is **cautious and three-valued** ([`Projection`]).
//!
//! Time goes in the atom; provenance stays the annotation, and the two do not
//! entangle. So each span carries the [`Ann`] of the ruling that anchored it,
//! and [`FluentTimeline::state_at`] returns the **factorial verdict**: a
//! temporal cover *and* a support standing, kept separate because they call for
//! different repairs by different owners — a `before`-fact for the temporal side,
//! a ledger ruling for the support side.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use dli_core::ann::{Ann, Status, Theory};
use dli_core::valuation::Valuation;

use crate::chronology::{Chronology, TemporalVerdict, TimePoint};

/// The cautious three-valued temporal projection of a fluent onto a query point
/// (`docs/spec/03-time.md` §2). This is the *collapsed* reading of the factorial
/// verdict; the authoritative form is the `(cover × support)` pair on a
/// [`Holding`], which [`Holding::projection`] folds into one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Projection {
    /// An initiator certainly precedes the query point, no terminator possibly
    /// intervenes, and the anchoring ruling is certain.
    HoldsCertainly,
    /// It may hold — the anchoring ruling is merely possible (an unratified
    /// order or fact), though the temporal cover itself is settled.
    HoldsPossibly,
    /// The answer turns on an order no one has declared: the query point is
    /// unordered against one of the span's anchors. Repair by declaring a
    /// `before`-fact.
    OrderIndeterminate,
}

/// The pure-order half of the factorial verdict: does the span's interval cover
/// the query point, reckoned by precedence alone? A span that certainly does not
/// cover the point yields no cover at all (it is dropped from
/// [`FluentState::holdings`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TemporalCover {
    /// The initiator certainly precedes the point and no terminator does: the
    /// interval certainly contains it.
    Covers,
    /// The query point is unordered against an anchor, so whether the interval
    /// contains it depends on an undeclared order.
    OrderIndeterminate,
}

/// One value's validity span: the value is in force from `since` (inclusive) to
/// `until` (exclusive), anchored to named events, and backed by a ruling.
///
/// `None` for `since` means the value was in force from before all recorded time
/// (the migration default: an unanchored fact is `since` story-present, open);
/// `None` for `until` means it never ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Span<V> {
    /// The value the fluent takes over this span.
    pub value: V,
    /// The initiating event; `None` for an open (beginningless) span.
    pub since: Option<TimePoint>,
    /// The terminating event; `None` for an open (endless) span.
    pub until: Option<TimePoint>,
    /// The ruling that anchored this span — declaring a fluent's validity is an
    /// act of authorship, and its standing rides here, not in the interval.
    pub ann: Ann,
}

impl<V> Span<V> {
    /// A span with explicit bounds.
    pub fn new(value: V, since: Option<TimePoint>, until: Option<TimePoint>, ann: Ann) -> Self {
        Self {
            value,
            since,
            until,
            ann,
        }
    }

    /// A value in force from an initiating event onward, never ending.
    pub fn since(value: V, event: TimePoint, ann: Ann) -> Self {
        Self::new(value, Some(event), None, ann)
    }

    /// A value in force from before all recorded time up to a terminating event.
    pub fn until(value: V, event: TimePoint, ann: Ann) -> Self {
        Self::new(value, None, Some(event), ann)
    }

    /// A value in force over the closed-open interval `[start, end)`.
    pub fn during(value: V, start: TimePoint, end: TimePoint, ann: Ann) -> Self {
        Self::new(value, Some(start), Some(end), ann)
    }

    /// The span's pure-order cover of a query point, or `None` when the interval
    /// certainly excludes it.
    #[must_use]
    pub fn cover(&self, chronology: &Chronology, at: TimePoint) -> Option<TemporalCover> {
        let started = self.started(chronology, at);
        let running = self.running(chronology, at);
        match (started, running) {
            (Tri::No, _) | (_, Tri::No) => None,
            (Tri::Yes, Tri::Yes) => Some(TemporalCover::Covers),
            _ => Some(TemporalCover::OrderIndeterminate),
        }
    }

    /// Whether the initiator is at or before the query point.
    fn started(&self, chronology: &Chronology, at: TimePoint) -> Tri {
        match self.since {
            None => Tri::Yes,
            Some(since) if since == at => Tri::Yes,
            Some(since) => Tri::of_before(chronology.precedes(since, at)),
        }
    }

    /// Whether the query point is strictly before the terminator (still running).
    fn running(&self, chronology: &Chronology, at: TimePoint) -> Tri {
        match self.until {
            None => Tri::Yes,
            Some(until) if until == at => Tri::No,
            Some(until) => Tri::of_before(chronology.precedes(at, until)),
        }
    }
}

/// A single value's standing at a query point: the factorial verdict of the
/// span, keeping temporal cover and support standing separate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Holding<V> {
    /// The value in force.
    pub value: V,
    /// The temporal half: whether the span's interval covers the query point.
    pub temporal: TemporalCover,
    /// The support half: the anchoring ruling's standing (never `Dead` here —
    /// dead-support spans are dropped from [`FluentState::holdings`]).
    pub support: Status,
}

impl<V> Holding<V> {
    /// Collapse the factorial `(cover × support)` pair into the single
    /// three-valued [`Projection`]. This is lossy — the pair distinguishes a
    /// temporal gap (declare a `before`-fact) from an authorial one (make a
    /// ruling), which the collapse discards — so keep the pair when the
    /// difference matters and use this only for a headline reading.
    #[must_use]
    pub fn projection(&self) -> Projection {
        match self.temporal {
            TemporalCover::OrderIndeterminate => Projection::OrderIndeterminate,
            TemporalCover::Covers => match self.support {
                Status::Certain => Projection::HoldsCertainly,
                Status::Possible | Status::Dead => Projection::HoldsPossibly,
            },
        }
    }
}

/// The fluent's projected state at a query point: every value whose span could
/// be in force, each with its factorial verdict. Values whose spans certainly
/// exclude the point, or whose anchoring ruling is dead, are absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FluentState<V> {
    /// The values in force (certainly or possibly), with their verdicts.
    pub holdings: Vec<Holding<V>>,
}

impl<V> FluentState<V> {
    /// The values that hold certainly — temporal cover settled and support
    /// certain. In a well-formed fluent this is at most one value.
    pub fn certain_values(&self) -> Vec<&V> {
        self.holdings
            .iter()
            .filter(|holding| holding.projection() == Projection::HoldsCertainly)
            .map(|holding| &holding.value)
            .collect()
    }

    /// Whether two or more *distinct* values hold certainly at once — a genuine
    /// contradiction, as opposed to the same value backed twice. Disjoint spans
    /// (`until e` / `since e`) never trip this, which is the whole point of the
    /// water-status case.
    pub fn has_contradiction(&self) -> bool
    where
        V: PartialEq,
    {
        let certain = self.certain_values();
        certain
            .iter()
            .enumerate()
            .any(|(i, value)| certain.iter().skip(i + 1).any(|other| other != value))
    }
}

/// A fluent's history: the disjoint (or overlapping) spans of its value.
///
/// The migration default lives in the caller: an unanchored fact lowers to a
/// single `since`-story-present span, which leaves the whole existing corpus
/// valid unchanged (`docs/spec/03-time.md` §2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FluentTimeline<V> {
    /// The value spans, in declaration order.
    pub spans: Vec<Span<V>>,
}

impl<V> FluentTimeline<V> {
    /// An empty timeline.
    #[must_use]
    pub fn new() -> Self
    where
        V: Default,
    {
        Self { spans: Vec::new() }
    }

    /// Append a span.
    pub fn push(&mut self, span: Span<V>) {
        self.spans.push(span);
    }

    /// The fluent's state at a query point: the cautious, three-valued
    /// projection of every span, composed factorially with each span's support
    /// standing under the valuation and constraint theory.
    ///
    /// A span that certainly excludes the point, or whose anchoring ruling is
    /// dead, contributes nothing. The result is safe to read from two eras that
    /// hold different values over disjoint spans without raising a false
    /// contradiction.
    ///
    /// ```
    /// use dli_core::ann::{Ann, Theory};
    /// use dli_core::valuation::Valuation;
    /// use dli_time::chronology::{Chronology, TimePoint};
    /// use dli_time::fluent::{FluentTimeline, Projection, Span};
    ///
    /// // flashback(0) -> enclosure(1) -> present(2)
    /// let chrono = Chronology {
    ///     edges: vec![(TimePoint(0), TimePoint(1)), (TimePoint(1), TimePoint(2))],
    /// };
    /// let mut water: FluentTimeline<String> = FluentTimeline::new();
    /// water.push(Span::until("commons".to_owned(), TimePoint(1), Ann::unconditional()));
    /// water.push(Span::since("enclosed".to_owned(), TimePoint(1), Ann::unconditional()));
    ///
    /// let nu = Valuation::new();
    /// let theory = Theory::new();
    /// // In the flashback, the water is commons — cleanly, with no rival value.
    /// let past = water.state_at(&chrono, &nu, &theory, TimePoint(0));
    /// assert_eq!(past.certain_values(), vec![&"commons".to_owned()]);
    /// assert!(!past.has_contradiction());
    /// ```
    pub fn state_at(
        &self,
        chronology: &Chronology,
        valuation: &Valuation,
        theory: &Theory,
        at: TimePoint,
    ) -> FluentState<V>
    where
        V: Clone,
    {
        let mut holdings = Vec::new();
        for span in &self.spans {
            let Some(temporal) = span.cover(chronology, at) else {
                continue;
            };
            let support = span.ann.status(valuation, theory);
            if support == Status::Dead {
                continue;
            }
            holdings.push(Holding {
                value: span.value.clone(),
                temporal,
                support,
            });
        }
        FluentState { holdings }
    }

    /// The fluent's state at a query point on a predicate the author declared
    /// **inertial** (`docs/design-notes/inertia-fluent-persistence.md`,
    /// `docs/decisions.md` DL-0081): same return shape as [`Self::state_at`], but the
    /// effective upper boundary of an open, forward-persisting value is
    /// *derived* here — at projection time — from which competing assertions are
    /// live under `valuation`, never baked into the stored span.
    ///
    /// This is the whole architecture of the feature (plan B.0): a defeated
    /// defeater must let the earlier value re-extend, which a compile-time
    /// `until` — expressible only in positive support — could never retract. So
    /// the boundary is recomputed every projection, and every defeat case falls
    /// out of the ordinary [`Ann::status`] read with no recompile.
    ///
    /// **Capping is active only while the inertia declaration itself is certain.**
    /// If `decl_ann` is not [`Status::Certain`] under `(valuation, theory)` —
    /// proposed, or its citing decision ruled false (defeated) — this behaves
    /// **byte-identically to [`Self::state_at`]**: an inertial predicate whose
    /// declaration does not hold reverts to plain open-interval projection, so no
    /// value ever silently vanishes.
    ///
    /// With capping active, for each span `vᵢ` with `since = Some(tᵢ)` and **no
    /// authored `until`**, a live, distinct, **certain** competitor `vⱼ since tⱼ`
    /// (with `tⱼ` at-or-before the query point) shapes `vᵢ`'s cover:
    /// - **no cover** (defeated) when `tᵢ` certainly precedes `tⱼ`: the successor
    ///   has taken over by the query point;
    /// - **[`TemporalCover::OrderIndeterminate`]** when `tⱼ ≠ tᵢ` and their order
    ///   is undeclared (settleable by a `before`-fact either way);
    /// - otherwise `vᵢ` keeps its own [`Span::cover`] — **including the
    ///   same-point `tⱼ == tᵢ` case**, where both cover certainly and the pair is
    ///   a genuine simultaneity clash, never an order-repair candidate.
    ///
    /// A span **with** an authored `until` overrides derivation (its own
    /// [`Span::cover`] wins) but still counts as a competitor of the others. A
    /// non-certain competitor never caps: a merely-proposed transition does not
    /// retract a certain prior value.
    pub fn state_at_inertial(
        &self,
        chronology: &Chronology,
        valuation: &Valuation,
        theory: &Theory,
        at: TimePoint,
        decl_ann: &Ann,
    ) -> FluentState<V>
    where
        V: Clone + PartialEq,
    {
        // The declaration is a defeasible claim like any other: capping only
        // applies while it is certain. Anything weaker reverts to the plain
        // projection, byte-for-byte.
        if decl_ann.status(valuation, theory) != Status::Certain {
            return self.state_at(chronology, valuation, theory, at);
        }
        let mut holdings = Vec::new();
        for (index, span) in self.spans.iter().enumerate() {
            let support = span.ann.status(valuation, theory);
            if support == Status::Dead {
                continue;
            }
            // Only an open (no authored `until`), anchored (`since`) span has a
            // derived boundary; everything else keeps its plain cover.
            let temporal = match (span.since, span.until) {
                (Some(_), None) => {
                    self.inertial_cover(chronology, valuation, theory, index, span, at)
                }
                _ => span.cover(chronology, at),
            };
            let Some(temporal) = temporal else {
                continue;
            };
            holdings.push(Holding {
                value: span.value.clone(),
                temporal,
                support,
            });
        }
        FluentState { holdings }
    }

    /// The derived cover of one open inertial span `vᵢ` (`since = tᵢ`, no
    /// authored `until`) at a query point, consulting the live competitors that
    /// cap it. Returns `None` when a certain successor has defeated it by the
    /// query point (no cover at all), else the span's own cover degraded to
    /// [`TemporalCover::OrderIndeterminate`] when a competitor's order is
    /// undeclared.
    fn inertial_cover(
        &self,
        chronology: &Chronology,
        valuation: &Valuation,
        theory: &Theory,
        index: usize,
        span: &Span<V>,
        at: TimePoint,
    ) -> Option<TemporalCover>
    where
        V: PartialEq,
    {
        // The caller only reaches this arm for an anchored span; an unanchored
        // one has no `tᵢ` to cap against and keeps its plain cover.
        let Some(since) = span.since else {
            return span.cover(chronology, at);
        };
        // If the span certainly does not cover the point on its own terms, no
        // competitor can revive it.
        let own = span.cover(chronology, at)?;
        let mut indeterminate = false;
        for (other_index, other) in self.spans.iter().enumerate() {
            if other_index == index || other.value == span.value {
                continue;
            }
            // A non-certain competitor never caps a certain prior value.
            if other.ann.status(valuation, theory) != Status::Certain {
                continue;
            }
            // A competitor caps `vᵢ` only from its own anchor `tⱼ` onward, and
            // only once `tⱼ` is at or before the query point.
            let Some(other_since) = other.since else {
                continue;
            };
            if !at_or_before(chronology, other_since, at) {
                continue;
            }
            match chronology.precedes(since, other_since) {
                // The successor certainly follows and has taken over: `vᵢ` is
                // defeated at the query point. Certain, so it wins outright.
                TemporalVerdict::Before => return None,
                // Distinct anchors of undeclared order: neither is certainly
                // current here. The same-point case (`tⱼ == tᵢ`) is excluded —
                // that is a simultaneity clash, so `vᵢ` keeps its own cover.
                TemporalVerdict::Unordered if other_since != since => indeterminate = true,
                _ => {}
            }
        }
        if indeterminate {
            Some(TemporalCover::OrderIndeterminate)
        } else {
            Some(own)
        }
    }
}

/// Whether `earlier` is at or before `later` — the same interned point, or a
/// proven `Before`. The half-open reading is not this helper's concern; it is
/// the plain "has this competitor's anchor arrived by the query point" test.
fn at_or_before(chronology: &Chronology, earlier: TimePoint, later: TimePoint) -> bool {
    earlier == later || matches!(chronology.precedes(earlier, later), TemporalVerdict::Before)
}

/// A three-valued yes/no/maybe, the shape every anchor comparison collapses to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tri {
    Yes,
    No,
    Maybe,
}

impl Tri {
    /// Read a precedence verdict as a yes/no/maybe: a settled `Before` is yes, a
    /// settled `After` is no, and `Unordered` is the maybe that makes projection
    /// order-indeterminate.
    fn of_before(verdict: TemporalVerdict) -> Tri {
        match verdict {
            TemporalVerdict::Before => Tri::Yes,
            TemporalVerdict::After => Tri::No,
            TemporalVerdict::Unordered => Tri::Maybe,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod inertia_tests {
    //! Unit tests for the valuation-lazy inertia projection
    //! ([`FluentTimeline::state_at_inertial`], DL-0081). These pin the two
    //! properties the check-level tests cannot see directly: the **derived
    //! boundary** (a value capped at its successor, not open forever) and the
    //! **defeat-without-recompile** discipline (flip a valuation over the SAME
    //! compiled timeline and the boundary moves — no re-lowering).

    use dli_core::ann::{Ann, Theory};
    use dli_core::atom::Atom;
    use dli_core::posbool::PosBool;
    use dli_core::valuation::Valuation;

    use crate::chronology::{Chronology, TimePoint};
    use crate::fluent::{FluentTimeline, Span};

    /// An annotation certain exactly when event variable `k` is ruled true.
    fn on(k: u32) -> Ann {
        Ann::supported(PosBool::pos(Atom(k)))
    }

    /// dawn(0) -> noon(1) -> dusk(2), a total order.
    fn dawn_noon_dusk() -> Chronology {
        Chronology {
            edges: vec![(TimePoint(0), TimePoint(1)), (TimePoint(1), TimePoint(2))],
        }
    }

    fn ruled(vars: &[u32]) -> Valuation {
        let mut nu = Valuation::new();
        for &v in vars {
            nu.set(Atom(v), true);
        }
        nu
    }

    fn certain(state: &crate::fluent::FluentState<String>) -> Vec<String> {
        let mut values: Vec<String> = state.certain_values().into_iter().cloned().collect();
        values.sort();
        values
    }

    /// The returning-value chain (A.1 pin): `commons@dawn`, `enclosed@noon`,
    /// `commons@dusk`. Each point carries exactly one certain value, and the
    /// FIRST `commons` never re-extends past `noon` — at dusk the certain value
    /// is `enclosed` until the third span opens a fresh `commons`.
    #[test]
    fn returning_value_chain_caps_and_reopens() {
        let chrono = dawn_noon_dusk();
        let theory = Theory::new();
        let nu = ruled(&[0, 1, 2, 10]);
        let decl = on(10);

        // Without the third span, the first `commons` is capped at noon and
        // `enclosed` persists forward — at dusk it is `enclosed`, never a
        // re-extended `commons`.
        let mut two: FluentTimeline<String> = FluentTimeline::new();
        two.push(Span::since("commons".to_owned(), TimePoint(0), on(0)));
        two.push(Span::since("enclosed".to_owned(), TimePoint(1), on(1)));
        assert_eq!(
            certain(&two.state_at_inertial(&chrono, &nu, &theory, TimePoint(0), &decl)),
            vec!["commons".to_owned()]
        );
        assert_eq!(
            certain(&two.state_at_inertial(&chrono, &nu, &theory, TimePoint(1), &decl)),
            vec!["enclosed".to_owned()],
            "the first `commons` is capped at noon by `enclosed`"
        );
        assert_eq!(
            certain(&two.state_at_inertial(&chrono, &nu, &theory, TimePoint(2), &decl)),
            vec!["enclosed".to_owned()],
            "the first `commons` does NOT re-extend to dusk"
        );

        // The full chain: the THIRD span reopens `commons` at dusk cleanly.
        let mut three = two.clone();
        three.push(Span::since("commons".to_owned(), TimePoint(2), on(2)));
        for at in [TimePoint(0), TimePoint(1), TimePoint(2)] {
            let state = three.state_at_inertial(&chrono, &nu, &theory, at, &decl);
            assert!(!state.has_contradiction(), "chain is silent at {at:?}");
        }
        assert_eq!(
            certain(&three.state_at_inertial(&chrono, &nu, &theory, TimePoint(2), &decl)),
            vec!["commons".to_owned()],
            "the third span reopens `commons` at dusk"
        );
    }

    /// Two distinct values `since e` (the SAME point) certainly co-hold — a
    /// clash, not a settleable order gap (B.1 pin). `precedes(e, e)` is
    /// `Unordered`, but the `tⱼ != tᵢ` guard excludes it, so both keep their own
    /// certain cover and `has_contradiction` fires.
    #[test]
    fn same_point_distinct_values_clash() {
        let chrono = Chronology { edges: vec![] };
        let theory = Theory::new();
        let nu = ruled(&[0, 1, 10]);
        let decl = on(10);
        let mut tl: FluentTimeline<String> = FluentTimeline::new();
        tl.push(Span::since("commons".to_owned(), TimePoint(0), on(0)));
        tl.push(Span::since("enclosed".to_owned(), TimePoint(0), on(1)));
        assert!(
            tl.state_at_inertial(&chrono, &nu, &theory, TimePoint(0), &decl)
                .has_contradiction(),
            "same-point distinct values are a simultaneity clash"
        );
    }

    /// THE critical property: defeat is a valuation flip over an already-built
    /// timeline, never a recompile. The same `FluentTimeline` projects
    /// differently as the valuation moves — the defeater's death re-extends the
    /// earlier value; the declaration's death reverts to plain projection; the
    /// originating assertion's death drops only its own span.
    #[test]
    fn defeat_is_a_valuation_flip_without_recompile() {
        let chrono = dawn_noon_dusk();
        let theory = Theory::new();
        // commons since dawn (var 0), enclosed since noon (var 1); inertia decl
        // is var 10. A clean transition when all are ruled.
        let mut tl: FluentTimeline<String> = FluentTimeline::new();
        tl.push(Span::since("commons".to_owned(), TimePoint(0), on(0)));
        tl.push(Span::since("enclosed".to_owned(), TimePoint(1), on(1)));

        // (i) all live: at dusk the successor `enclosed` holds, `commons` capped.
        let all = ruled(&[0, 1, 10]);
        assert_eq!(
            certain(&tl.state_at_inertial(&chrono, &all, &theory, TimePoint(2), &on(10))),
            vec!["enclosed".to_owned()]
        );

        // (ii) defeat the DEFEATER (var 1 false): `enclosed` goes dead, so
        // `commons` re-extends and holds at dusk — no recompile, same timeline.
        let no_defeater = ruled(&[0, 10]); // var 1 left false-by-absence? set explicitly:
        let mut no_defeater = no_defeater;
        no_defeater.set(Atom(1), false);
        assert_eq!(
            certain(&tl.state_at_inertial(&chrono, &no_defeater, &theory, TimePoint(2), &on(10))),
            vec!["commons".to_owned()],
            "defeating the defeater re-extends the earlier value"
        );

        // (iii) defeat the DECLARATION (var 10 false): capping off, reverts to
        // plain `state_at` — the two open spans now both cover dusk and clash
        // (the value never vanished, its open reading returned).
        let mut no_decl = ruled(&[0, 1]);
        no_decl.set(Atom(10), false);
        let inertial = tl.state_at_inertial(&chrono, &no_decl, &theory, TimePoint(2), &on(10));
        let plain = tl.state_at(&chrono, &no_decl, &theory, TimePoint(2));
        assert_eq!(
            inertial, plain,
            "a defeated declaration projects byte-identically to plain state_at"
        );
        assert!(
            inertial.has_contradiction(),
            "the open spans clash exactly as they would with no inertia"
        );

        // (iv) defeat the ORIGINATING assertion (var 0 false): only `commons`
        // drops; `enclosed` still stands.
        let mut no_origin = ruled(&[1, 10]);
        no_origin.set(Atom(0), false);
        assert_eq!(
            certain(&tl.state_at_inertial(&chrono, &no_origin, &theory, TimePoint(2), &on(10))),
            vec!["enclosed".to_owned()],
            "defeating the originating assertion drops only its span"
        );
    }

    /// An authored `until` overrides derivation for its own span but the span
    /// still counts as a competitor: `commons until noon` / `enclosed since
    /// noon` is a clean disjoint transition, silent at every point.
    #[test]
    fn authored_until_overrides_but_still_competes() {
        let chrono = dawn_noon_dusk();
        let theory = Theory::new();
        let nu = ruled(&[0, 1, 10]);
        let decl = on(10);
        let mut tl: FluentTimeline<String> = FluentTimeline::new();
        tl.push(Span::until("commons".to_owned(), TimePoint(1), on(0)));
        tl.push(Span::since("enclosed".to_owned(), TimePoint(1), on(1)));
        for at in [TimePoint(0), TimePoint(1), TimePoint(2)] {
            assert!(
                !tl.state_at_inertial(&chrono, &nu, &theory, at, &decl)
                    .has_contradiction(),
                "authored-until transition is disjoint and silent at {at:?}"
            );
        }
    }
}
