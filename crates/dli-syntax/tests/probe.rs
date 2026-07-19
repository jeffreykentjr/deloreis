#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Smoke coverage for the gentle-subset probe.

use dli_syntax::probe::{probe_line, probe_source};

#[test]
fn probe_accepts_keyword_lines_and_rejects_gibberish() {
    assert!(probe_line("pack saltmarket").is_ok());
    assert!(probe_line("adjacent(the-shallows, saltmarket)").is_ok());
    assert!(probe_line("  a continuation line").is_ok());
    assert!(probe_line("# a comment").is_ok());
    assert!(probe_line("").is_ok());
    assert!(probe_line("zorble frobnicate").is_err());

    let errors = probe_source("pack saltmarket\nzorble frobnicate\n");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, 2);
    assert_eq!(errors[0].token, "zorble");
}

#[test]
#[ignore = "enabled at SURF-1"]
fn probe_diagnostic_snapshot() {
    // Once SURF-1 gives probe errors real spans, snapshot the rendered
    // diagnostic report rather than the debug form.
    let errors = probe_source("zorble frobnicate\n");
    insta::assert_debug_snapshot!(errors);
}
