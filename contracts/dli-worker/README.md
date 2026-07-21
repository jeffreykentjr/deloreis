# dli-worker contract

This directory is the **authority** for the generic wire contract between the
`deloreis` engine and any worker consumer — the fantasy production system is the
first such consumer. It is owned here and vendored by consumers, pinned and
digest-checked.

## Layout

- `registry.yaml` — the closed fixture registry: every fixture, digest-pinned
  over its exact bytes. Authority: `closed_fixture_registry`.
- `schemas/` — per-op request/response JSON schemas, emitted from `dli-serve` by
  `cargo run -p xtask -- schema`. Never edit by hand; the gate's
  `schema --check` step fails on drift.
- `fixtures/golden/` — valid calls and their expected responses.
- `fixtures/negative/` — malformed or refused calls and their expected error
  codes (plus the golden-shaped framing fixture, whose verdict is unchanged by
  the adversarial framing).
- `fixtures/raw/` — transport-level hazards (stdout noise, truncated frames,
  duplicate keys) that the frame reader must survive.
- `fixtures/compat/` — v(N-1) goldens once a v2 exists; empty at v0.

## Sync rule

fantasy vendors these schemas and fixtures pinned and digest-checked. The update
protocol — how a contract change lands here and re-vendors into fantasy — is
`docs/integration/fantasy-contract.md` §5. Run
`cargo run -p xtask -- fixtures --update-digests` after adding or editing a
fixture, and `cargo run -p xtask -- fixtures` to verify.
