//! Deloreis Lore Interchange time domain.
//!
//! Time is not a number line here. A [`chronology::Chronology`] is a partial
//! precedence DAG over events; a [`wheel::Wheel`] is a cyclic calendar with
//! modular navigation. Nothing in this crate is `Ord`: linear comparison of
//! points would be a category error, so the type system refuses it. Two points
//! are related through [`Chronology::precedes`] — which may answer
//! [`TemporalVerdict::Unordered`] — and a contradiction surfaces as
//! [`blame::CycleBlame`], the exact `before`-facts that close a loop.
//!
//! [`fluent::FluentTimeline`] layers *validity spans* onto that order: a value
//! in force `since`/`until`/`during`, projected onto a query point with the
//! cautious three-valued [`fluent::Projection`], composed factorially with the
//! support standing of the ruling that anchored it.

pub mod blame;
pub mod chronology;
pub mod fluent;
pub mod wheel;

pub use blame::{BlamedEdge, CycleBlame};
pub use chronology::{Chronology, TemporalVerdict, TimePoint};
pub use fluent::{FluentState, FluentTimeline, Holding, Projection, Span, TemporalCover};
pub use wheel::{Wheel, WheelPos};
