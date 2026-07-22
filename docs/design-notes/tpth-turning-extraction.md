---
status: live
title: "Phase 4: extracting the Turning to crates/tpth"
verdict: "Move the whole Turning (types + instance + study + invariants + games export) into a thin world crate. The engine keeps SIM-1, the export core, and the generic wire unchanged — no contract change, no author ruling. Authority: DL-0017, DL-0018."
---

# Phase 4 — extracting the Turning to `crates/tpth`

**Authority: [DL-0017](../decisions.md#dl-0017--the-generaltpth-separation-engine-craft-doctrine-config-world-content)
and [DL-0018](../decisions.md#dl-0018--separation-layout-a-shipped-craft-library-a-thin-tpth-crate-a-generic-contract).**
This note elaborates the Phase-4 row of the
[separation plan](tpth-separation-plan.md) into a concrete extraction design.
The ledger wins if they ever disagree.

The Turning (TPtH's oracular card game) is world content baked into two **engine**
crates: `dli-sim/src/turning.rs` (1469 lines — the type vocabulary, the lowlands
instance, the honest-vs-rigged study, the structural invariants, D-0147/D-0148
provenance) and `dli-export/src/games.rs` (the card + Ludii export backend, built on
`dli_sim::turning::*`). DL-0018 fork 2 rules that this moves to a **thin
`crates/tpth/` world crate now**, with simulator genericization deferred as a
follow-on. This is the design that does it gate-green.

---

## The load-bearing discovery

**The Turning card/Ludii export never reads the compiled world.**
`dli-export/src/profile.rs::export_with` computes `certain_atoms_under` +
`apply_keep_stamps`, then for the `Cards`/`LudiiGdl` arms **discards them**: it calls
`games::ruleset_for(opts)` — pure TPtH Rust (`Ruleset::lowlands_honest()` /
`keep_stamped()` / `salt_fringe()`) — and compiles *that*. The `CompiledWorld` is
touched only for the manifest's layer/valuation. So the "generic export of a world"
framing is a facade over the Cards/LudiiGdl targets: they render a hardcoded ruleset,
not the input world.

Two consequences:

1. Moving the Turning to a world crate **severs a fake coupling**, not a real one.
   `tpth`'s export path needs no `dli-world` dependency and no `CompiledWorld`.
2. **At extraction time, the only live consumers of the Turning export were the
   tests we are moving.** The CLI `simulate`/`export` (`deloreis/src/cmd/drafts.rs`)
   were unwired "not yet" stubs at the time of this note, and the serve
   `Op::ExportRun` was capability-**gated**; the golden
   `contracts/dli-worker/fixtures/golden/export-ludii-v1.json` then asserted a
   `not-wired` answer that never dispatched to a renderer. Both have since been
   wired to the generic `dli-game` engine (DL-0021) — the golden now asserts a real
   `ok` manifest over the `french-deck` ruleset — but the card/Ludii render they
   reach is a world crate's own, never a hardcoded ruleset inside the engine, so the
   extraction argument stands.

This is why Phase 4 changes zero wire surface (§3).

---

## Target crate layout

```
crates/tpth/                      # NEW — world crate (package `tpth`, no dli- prefix)
  Cargo.toml                      # deps: dli-sim, dli-export, dli-core, serde, serde_json, schemars
  src/lib.rs                      # pub mod turning; pub mod export;
  src/turning.rs                  # moved whole from dli-sim/src/turning.rs
  src/export.rs                   # moved from dli-export/src/games.rs + the world-side dispatch
  tests/turning.rs                # moved from dli-sim/tests/turning.rs
  tests/export.rs                 # the Turning-specific tests carved out of dli-export/tests/smoke.rs

crates/dli-sim/                   # ENGINE — SIM-1 only after the move
  src/lib.rs                      # drop `pub mod turning;`
  src/{battery,checkpoint,manifest,rng}.rs   # unchanged (the generic simulator)

crates/dli-export/               # ENGINE — the generic export core after the move
  src/lib.rs                      # keep wire types; drop `pub mod games;`
  src/profile.rs                  # keep the mechanism; drop the Cards/LudiiGdl dispatch
  src/{context_pack,articy}.rs    # unchanged (verified turning-free)
  Cargo.toml                      # DROP the dli-sim dependency (unused once games leaves)
```

### What moves to `crates/tpth`

| Artifact | From | Rewire |
|---|---|---|
| The Turning: types, lowlands instance, study, invariants, D-0147/0148 | `dli-sim/src/turning.rs` → `tpth/src/turning.rs` | `crate::manifest::{…}` → `dli_sim::manifest::{…}`; `crate::rng::{…}` → `dli_sim::rng::{…}` (both are `pub` re-exports). Doctests `dli_sim::turning::` → `tpth::turning::`. |
| The card + Ludii export backend (`CardDeck`, `Leaf`, `LudiiExport`, `DifferentialStatus`, `CompiledFrom`, `Meta`, `compile_cards`, `compile_ludii`, `diff_decks`, `ruleset_for`, `follow_edge_count`, `hand_roles`, `layer_name`) | `dli-export/src/games.rs` → `tpth/src/export.rs` | `dli_sim::turning::{…}` → `crate::turning::{…}`; `crate::profile::{…}` → `dli_export::profile::{…}`. |
| The world-side export dispatch (lifted out of `export_with`'s Cards/LudiiGdl arms) | `dli-export/src/profile.rs` → `tpth/src/export.rs` | New `tpth::export::run(&Registry, ExportTarget, &ExportOpts) -> Result<TurningExport, ExportError>`: resolve the `ProfileSpec`, build the ruleset via `ruleset_for(opts)`, compile, pin an `ExportManifest`. **No `CompiledWorld`.** |
| The Turning tests | `dli-sim/tests/turning.rs`; the `worked_card_export_*` / `ludii_export_*` / `compile_*_directly` tests in `dli-export/tests/smoke.rs` | Rewrite the export tests to call `tpth::export::run`, not `export_with`. Fixture/registry paths (`../../worlds/tpth/turning/the-turning-rules.json`, `../../worlds/tpth/export/profiles.json`) keep the same `../../` depth — unchanged. |

### What stays generic in the engine

- **`dli-sim`** keeps SIM-1 (`battery`/`checkpoint`/`manifest`/`rng`). `turning` imported
  *from* `manifest`/`rng`, never the reverse, so removing it is a clean cut.
- **`dli-export`** keeps the export core: `profile.rs` (the mechanism —
  `Registry`/`ProfileSpec`/`export`/`export_with`, the keep-stamp filter, the
  manifest), `context_pack.rs`, `articy.rs` (both verified turning-free), and the
  frozen wire types in `lib.rs`.

### Dependency direction (acyclic, boundary-honest)

`tpth → { dli-sim, dli-export, dli-core }`. **No engine crate depends on `tpth`.** The
`dli-` prefix is dropped deliberately to mark `tpth` a world crate, not an engine one.

---

## The export-dispatch resolution (the crux)

Once `games.rs` lives in `crates/tpth`, how do `Cards`/`LudiiGdl` exports work
**without `dli-export` depending on `crates/tpth`**?

### Option A — move the dispatch to `tpth`; keep the generic taxonomy + wire in the engine. ✅ RECOMMENDED

`dli-export` keeps `ExportTarget::{ContextPack, Cards, LudiiGdl, Articy}` as generic
registry keys and keeps `ExportProfile` / `ExportTarget::from_wire` intact, but
`export_with` implements only the **world-atom** targets (`ContextPack`, `Articy`) and
returns a new fail-closed `ExportError::NoEngineRendererForTarget(target)` for the
ruleset targets. `tpth::export::run` owns the ruleset dispatch. `ExportArtifact` loses
its `Cards(CardDeck)` / `LudiiGdl(LudiiExport)` variants (and `as_cards`) — **that type
is not on the wire** (the serve `ExportRunResult` carries only `manifest`), so it is a
safe internal change; `tpth` returns `CardDeck` / `LudiiExport` directly.

Why this honors "thin crate now": `"cards"` and `"ludii-gdl"` are generic export
**format** names, not TPtH proper nouns — a standalone dli (the planned tarot exemplar)
uses the same card/Ludii machinery. So the generic *target taxonomy* belongs in the
engine; only the TPtH-hardwired renderer + ruleset leave. No trait machinery, no
plugin registry.

### Option B — a generic `GameExport` trait in `dli-export` that `tpth` implements

Rejected for now. The return types (`CardDeck`/`Leaf` carry ground/weather/reckoner/
figure/ring_next vocabulary) would keep Turning-shaped types in the engine, and it
needs a dynamic renderer-registration seam. This is exactly the "full genericization"
DL-0018 fork 2 **defers** — it is the follow-on's shape, not now.

### Option C — keep the generic `Ruleset` TYPE in the engine, move only the instance

Rejected. It contradicts DL-0018 fork 2 ("the Turning's **type vocabulary** … move into
a thin `crates/tpth`") and leaves the leak the plan's inventory row A names (`HandRole`
Taker/Keeper/Giver/Mender, `Weather`/`Ground`/`Figure`/`Reckoner`, the `"reckoner"`
hub, the `"weather-"` prefix) sitting in engine `src/`. It only makes sense *after* a
parameterized-`Ruleset` genericization — i.e. the deferred follow-on. In the interim it
is strictly worse than moving.

**Recommendation: A.** It moves everything DL-0018 names, changes zero wire surface,
and severs the fake world-coupling instead of preserving it behind a trait.

---

## The wire / contract question — no change, no ruling

Because the ruleset targets are generic formats (not TPtH proper nouns) and the only
live path is the moving tests, Phase 4 touches no contract surface:

| Surface | State |
|---|---|
| `ExportProfile::{ContextPack, Cards, LudiiGdl}`, `ExportTarget`, `from_wire` (`dli-export/src/lib.rs`, `profile.rs`) | **Unchanged** — generic export-format selectors. |
| `contracts/dli-worker/schemas/dli.export.run.{request,response}.json` | **Unchanged.** `xtask schema --check` stays green. |
| `worlds/tpth/export/profiles.json` | **Unchanged.** Its `"target":"cards"/"ludii-gdl"/"articy"` still deserialize into the retained `ExportTarget` variants; `tpth::export` loads it via `Registry::from_path`. |
| Golden `export-ludii-v1.json`, negative `export-run-schema-violation.json` | **Unchanged** — gated op, never dispatches; schema unchanged. |
| `ExportArtifact::{Cards, LudiiGdl}` (`dli-export/src/profile.rs`) | Variants removed — **internal type, not on the wire** (serve returns only `manifest`). |

No `dli-worker` contract change; **no author ruling required for the wire.** The single
judgment call is whether the engine may advertise target variants it has no built-in
renderer for (§ Hardest calls, item 1).

---

## The ordered implementation DAG

Each step lands gate-green (`ruff`-equivalent: `cargo fmt --check`, `cargo clippy`,
`cargo nextest`, `cargo xtask gate`).

**Step 1 — create the empty `crates/tpth` skeleton (mechanical).**
`Cargo.toml` + `src/lib.rs` (crate doc only). Workspace `members = ["crates/*"]`
auto-includes it; no `workspace.dependencies` entry yet (no engine crate depends on
it). Green: the empty crate builds.

**Step 2 — the extraction (one green commit; the single real refactor).**
`git mv` `turning.rs` (+ its test) and `games.rs` into `crates/tpth`; rewire the four
import lines; drop `pub mod turning;` (dli-sim) and `pub mod games;` + games
re-exports (dli-export); in `profile.rs` drop `use crate::games`, remove
`ExportArtifact::{Cards,LudiiGdl}` + `as_cards`, replace the two `export_with` arms with
the fail-closed error; add `tpth::export::run`; move the Turning-specific smoke tests to
`tpth/tests/export.rs` (rewritten onto `tpth::export::run`); drop `dli-sim` from
`dli-export/Cargo.toml`. **This must be atomic** — `profile.rs → games → turning` is a
hard dependency chain spanning both engine crates, so an intermediate step would need a
transient illegal `dli-export → tpth` edge. Verify: full gate green; the Turning tests +
the ludii/keep-stamped/salt-fringe conformance pass in `tpth`; the generic engine export
tests pass; `profile_serializes_kebab_case` (wire enum unchanged) and
`xtask schema --check` stay green.

- Two test-authoring carve-outs stay engine-side, reworked onto a neutral inline
  `ProfileSpec` pair (context-pack + a neutral articy) since they test *generic*
  mechanism, not the Turning: `a_profile_hash_is_stable_and_profile_specific` and
  `articy_scaffold_maps_knowledge_scopes_and_quests` (both currently lean on
  `tpth_registry()` / `ExportTarget::Cards`).

**Step 3 — boundary-guard cleanup (mechanical; depends on the Phase-1 guard).**
Delete the guard's exception rows for `dli-sim/src/turning.rs` and
`dli-export/src/games.rs`; confirm the guard's engine-`src/` scope excludes
`crates/tpth`. If Phase 1 has not landed, defer and note the dependency. Green.

**Step 4 — docs (docs-as-you-go).**
Mark Phase 4 done and flip inventory rows A (the deck, the type vocabulary, the study,
the invariants, the games export) to ✅ in `tpth-separation-plan.md`; fix module-doc
crate references. Docs-only, green.

---

## The hardest calls

1. **Wire variants — resolved as no-change; confirm the judgment.** Keep
   `ExportProfile` / `ExportTarget` `Cards`/`LudiiGdl` in the engine as generic format
   targets that fail closed until a world crate supplies the renderer. This mirrors how
   `Registry::builtin()` ships only the neutral `context-pack/default` while worlds
   supply catalogs as data. A ruling is needed *only* if the author disagrees and wants
   the engine to advertise strictly what it renders — which would force removing the
   variants → a wire contract change (schema regen + golden fixture + ruling). Not
   recommended.

2. **How far to genericize (DL-0018 fork 2) — move whole, don't parameterize.** Move
   types + instance + study + invariants + export; no `Ruleset` genericization now. Side
   effect to note: the Turning card export's manifest `valuation` basis changes from
   "digest of the world's valuation" to "digest of the ruleset" — unobserved by any
   test, and more honest (the ruleset is what actually produced the artifact).

3. **Crate naming.** Directory `crates/tpth`, package `tpth`, modules `tpth::turning` /
   `tpth::export`. Confirm the dropped `dli-` prefix as the marker of a world (not
   engine) crate.

4. **The single big extraction commit.** Step 2 is the one non-mechanical step; the
   `profile.rs → games → turning` chain forces it to be atomic. It is still
   individually gate-verifiable — the Turning tests and the ludii/keep-stamped
   conformance must stay green through it.
