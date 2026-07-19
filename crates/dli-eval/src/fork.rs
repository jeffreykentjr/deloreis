//! The fork manager (`ENG-2`): fork stores, suppositions, and
//! derived-consequence diffs.
//!
//! Forks are O(1) valuation overlays — exploration over an unchanged store
//! (charter koan 2). This lane defines its OWN delta types; it never imports
//! `dli-serve` wire shapes (dependencies flow the other way).
