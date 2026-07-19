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
