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
The optional story-time cutoff (`--as-of` in the CLI, `as_of` on the wire)
projects state claims through the fluent timeline before selection. The manifest
records the cutoff and binds it in the profile hash. A timeline that cannot be
projected faithfully is refused; a timeless claim remains independent of an
identical temporal claim. Exports continue to contain source atoms, not derived
query rows.
Historical exports require a freshly compiled world with its temporal source
mapping. An older serialized compiled world without that mapping must be
recompiled from its source or lorestore before a historical export; it is
refused rather than assigning a claim's support to the wrong source.

The CLI exposes `deloreis export` on a syntax-enabled build. The JSON/MCP
operation is `dli.export.run`, with request and response schemas in
`contracts/dli-worker/schemas/`. A project can select a profile catalog;
`profile_name` selects a named profile. An invalid target, profile, selector,
or unsupported option is refused rather than silently ignored.
