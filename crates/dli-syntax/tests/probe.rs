#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the gentle-subset probe.

use dli_syntax::probe::{probe_line, probe_source};

#[test]
fn probe_accepts_keyword_lines_and_rejects_gibberish() {
    assert!(probe_line("scope saltmarket").is_ok());
    assert!(probe_line("adjacent(the-shallows, saltmarket)").is_ok());
    assert!(probe_line("  a continuation line").is_ok());
    assert!(probe_line("# a comment").is_ok());
    assert!(probe_line("").is_ok());
    assert!(probe_line("zorble frobnicate").is_err());

    let errors = probe_source("scope saltmarket\nzorble frobnicate\n");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, 2);
    assert_eq!(errors[0].token, "zorble");
}

#[test]
fn probe_diagnostic_snapshot() {
    // The probe is the demoted pre-parser relic; snapshot its rejection so its
    // shape stays pinned for the one sibling gate that still calls it.
    let errors = probe_source("zorble frobnicate\n");
    insta::assert_debug_snapshot!("probe_rejection", errors);
}
