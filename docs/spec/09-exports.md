---
status: live
title: "Exports"
verdict: "One-way, profile-driven exports of evaluated lore to context packs, Articy-shaped JSON, CSV, and static Markdown wikis."
---

# Exports

DLI exports evaluated lore to a context pack, Articy-shaped JSON, CSV, or a
static Markdown wiki. Exports are one-way projections: changes to an exported
artifact do not write back into the lorestore.

By default an export reads the certain layer. A profile selects the sorts and
relations to include and can specify rendering and provenance options. Rust
library callers may explicitly supply a fork valuation. Every artifact carries
a manifest with the source anchor, selected layer, valuation fingerprint, and
profile hash.
These fields make an export traceable and reproducible from its inputs.

The CLI exposes `deloreis export` on a syntax-enabled build. The JSON/MCP
operation is `dli.export.run`, with request and response schemas in
`contracts/dli-worker/schemas/`. A project can select a profile catalog;
`profile_name` selects a named profile. An invalid target, profile, selector,
or unsupported option is refused rather than silently ignored.
