---
status: reference
verdict: "The fiction-reset mutation suite checks eight explicit core cases, including one strict expected miss for unstructured paraphrase. The earlier consumer-specific operator census is retired under DL-0101 and remains available in git history."
---

# Mutation coverage

The deterministic suite in `crates/dli-world/tests/mutation_harness.rs` checks
that the retained engine detects a malformed ruling, a duplicate entity, a
conflicting ruled value, an unknown entity, a proposed claim presented as fact,
an unknown decision, a prior-episode contradiction, and a retired form. It
also records an explicit strict expected miss: a
paraphrase expressed with unrelated predicates is not recognized as a
contradiction. The assertion fails if that behavior changes, so the result must
be reclassified rather than silently hidden.

This suite is narrower than the former consumer-specific operator census. It
does not claim prose, style, craft, or factory coverage. DL-0101 retired the
private corpus and its integration checks; their historical record is available
at `pre-fiction-reset`.
