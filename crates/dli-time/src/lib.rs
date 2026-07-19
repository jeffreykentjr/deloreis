//! Deloreis Lore Interchange time domain.
//!
//! Time is not a number line here. A [`chronology::Chronology`] is a partial
//! precedence DAG over events; a [`wheel::Wheel`] is a cyclic calendar with
//! modular navigation. Nothing in this crate is `Ord`: linear comparison of
//! points would be a category error, so the type system refuses it.

pub mod chronology;
pub mod fluent;
pub mod wheel;

pub use chronology::{Chronology, TimePoint};
pub use fluent::FluentTimeline;
pub use wheel::{Wheel, WheelPos};
