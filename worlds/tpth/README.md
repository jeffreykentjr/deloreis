# To Pleach the Hedgeway — a world authored in De Lore Is

This directory is the modular home for **all** content belonging to *To Pleach
the Hedgeway* (TPtH), one fictional world authored in dli. dli itself is a
general-purpose lore tool (see [DL-0017](../../docs/decisions.md) and the
[separation plan](../../docs/design-notes/tpth-separation-plan.md)); nothing
here is engine code, and no engine crate depends on this tree. A different world
supplies its own directory like this one.

## What lives here

| Subdir | Holds | Status |
|---|---|---|
| `packs/` | The world as `.dli` source — `saltmarket.dli`, `cymric-fringe.dli` (the dialect + names), `true-naming.dli` + `second-practice.dli` (the Concordance / spoken-proof + the Turning as a practice). | landed |
| `practices/` | Practice declarations (spoken-proof). | landed |
| `turning/` | The Turning card game as **data** — `the-turning-rules.json` + `keep-stamped` / `salt-fringe` variant deltas, loaded by the generic simulator. | landed |
| `episodes/`, `season/` | Episode and season payloads over the world. | landed |
| `time/` | Time/cycle declarations (`cycles.dli`). | landed |
| `craft/` | TPtH's **craft configuration** — which craft-doctrine packs are enabled, mission-filters, the D-0128 pillars, thresholds, the no-battle-ten lexicon, material categories. **Produced by the factory** (the consuming repo maps `D-nnnn` rulings → records); dli only loads it. | landed |
| `export/` | Export profiles (`profiles.json`). | landed |
| `vendor/` | The pinned fantasy-repo snapshot, digest-isolated — provenance and regression corpus. | landed |
| `mutation/` | The detection corpus (seeded-violation fixtures). | landed |

## What does NOT live here

- **Craft doctrine** (Sanderson's laws, kishōtenketsu, MICE, …). Craft is
  *project config*, not world content — a togglable module library any world's
  project may enable. It has its own home; only TPtH's *selection and
  configuration* of it lives in `craft/`.
- **Engine mechanism.** The practice workbench, the naming oracle, the
  season-rule engine, the simulator, the export core — all generic, all in
  `crates/`. This tree is the data they consume.

The layout is gate-green; see the
[separation plan](../../docs/design-notes/tpth-separation-plan.md) for the DAG
and design context.
