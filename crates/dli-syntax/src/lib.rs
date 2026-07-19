//! Deloreis Lore Interchange concrete syntax.
//!
//! The parser skeleton is CORE-1; the lexer, full grammar, and formatter are SURF-1 and are `todo!()`
//! today. What is real now is [`probe`]: a cheap, grammar-independent gate that
//! accepts the gentle subset the examples and docs are written in, so the
//! docs-oracle can hold the corpus honest before a real parser exists.

pub mod diag;
pub mod format;
pub mod lexer;
pub mod parser;
pub mod probe;

pub use probe::{KEYWORDS, ProbeError, probe_line, probe_source};
