---
status: reference
verdict: "The contract 2.0.0 schemas and replayable worker fixtures."
---

# dli-worker contract

This directory contains the schemas and fixtures for contract `2.0.0`, the
only contract served by this build. Earlier versions and retired operations
are refused as killed forms with their successor named.

- `registry.yaml` lists fixtures and pins their exact byte digests.
- `schemas/` contains generated request and response schemas. Regenerate with
  `cargo run -p xtask -- schema`.
- `fixtures/golden/` and `fixtures/negative/` record calls and their expected
  results or errors.
- `fixtures/framing/` holds paired control and attack cases. The replay harness
  checks safety projections, confines input to declared data paths, probes
  mutations, and reparses materialized output.
- `fixture-project/` supplies the digest-pinned project used for replay.
- `fixtures/raw/` exercises noise, truncated frames, and duplicate keys.

Run `cargo run -p xtask -- fixtures --update-digests` after editing fixtures,
then `cargo run -p xtask -- fixtures` to verify the registry and schemas.
