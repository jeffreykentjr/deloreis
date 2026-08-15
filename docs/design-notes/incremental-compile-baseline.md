---
status: reference
title: "Compile-cost baseline: where the time goes once compiles are cached"
verdict: "The measurement DL-0088 §3 asked for, taken. It does NOT confirm the prediction: check::run is 0–2% of a cold compile and its share FALLS as the world grows, so slicing the check battery would buy ~1%. The symbolic fixpoint is the residual O(world) term — 69% at 8 personas rising to 99% at 128, growing ≈O(n^2.9). The session cache (PR 3) delivers 3.2×–4.0× on the author.merge scan; compile_incremental (PR 2) measures 0.94–0.96× — a small net loss — on the identity workload, exactly as DL-0088 §1 predicted for same-as. Data only; no ruling. Reproduced by tests/compile_cost_baseline.rs."
---

# Compile-cost baseline

DL-0088 §3 refused to slice the whole-world check battery on a guess and
required instead that **"PR 3 instruments the check-battery cost so its slicing
trigger fires on a measurement rather than a guess."** This note is that
measurement, committed so the SubtleMemory instrumentation has a reproducible
baseline to be read against.

It is `status: reference` — **data, not a ruling.** It records what a run
measured. Any change of course it suggests belongs in `decisions.md`, not here.

## The headline

**`check::run` is not the next bottleneck.** DL-0088 §3 predicted it would
become the residual `O(world)` term once compiles were cached. On this workload
it is **0–2% of a cold compile at every size measured, and its share falls as the
world grows.** The symbolic fixpoint is the residual term instead, at 69% of a
small world's compile rising to **99%** of the largest measured — and it grows
faster than the world does.

That does not reverse DL-0088; it answers the question DL-0088 asked. The
instrument was built precisely so the slicing trigger would fire on evidence, and
the evidence says the trigger should not fire.

## How to reproduce

```text
cargo test -p dli-lorestore --all-features --release \
    --test compile_cost_baseline -- --ignored --nocapture --test-threads=1
```

The harness is `crates/dli-lorestore/tests/compile_cost_baseline.rs`. Every test
in it is `#[ignore]`d, so it never runs in a gating lane — which is what licenses
it to read a clock at all (DL-0013 binds the lanes that block a merge). Nothing
in it asserts on a duration.

`--release` is not optional: a debug-profile timing of a symbolic fixpoint is
noise with units, and a debug run is not comparable to the tables below.
`--test-threads=1` keeps the three tables from interleaving.

### Run parameters

| | |
|---|---|
| Commit | `7102bf5aea2df091f017994e28c1f086cf2cc796` (+ the PR 3 working tree) |
| Toolchain | `rustc 1.96.0 (ac68faa20 2026-05-25)`, `--release` |
| Machine | AMD Ryzen AI MAX+ 395, 16 cores / 32 threads, 125 GiB RAM, Linux 7.1.5 |
| Statistic | median of 5 repetitions per cell |
| Workload | synthetic identity world; one commit per persona; 5 declarations per persona (4 facts + 1 `same-as`), on a seed commit carrying `ratify D-1` and two rules |

The rule set is:

```text
appears($p, $s)      :- mentions($p, $s)
co-mentioned($p, $q) :- appears($p, $s), appears($q, $s)
```

**Read the fixpoint numbers against that second rule.** `co-mentioned` is a
deliberate self-join, so derived rows grow quadratically in shared scenes and the
fixpoint's dominance is partly a property of *this* rule set, not of the engine
alone. A corpus with no self-join would shift the split — how far is unmeasured.
The `same-as` per persona is likewise deliberate: DL-0088 §1 names identity
assertions as the *worst* case for fixpoint reuse, and the SubtleMemory pilot is
an identity workload, so this is the shape the campaign is actually aimed at.

## 1 — Phase split per cold `compile_at`

| personas | commits | decls | facts | derived | reconstruct | loader | fixpoint+seal | check::run | cold total | warm `compile_at` |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 8 | 9 | 43 | 48 | 160 | 0.7 ms (26%) | 0.1 ms (3%) | 1.9 ms (69%) | **0.1 ms (2%)** | 2.8 ms | 0.06 ms |
| 16 | 17 | 83 | 96 | 416 | 1.4 ms (14%) | 0.1 ms (1%) | 8.7 ms (84%) | **0.1 ms (1%)** | 10.3 ms | 0.16 ms |
| 32 | 33 | 163 | 192 | 1120 | 2.7 ms (6%) | 0.2 ms (0%) | 43.1 ms (93%) | **0.4 ms (1%)** | 46.4 ms | 0.37 ms |
| 64 | 65 | 323 | 384 | 3296 | 5.5 ms (2%) | 0.4 ms (0%) | 274.2 ms (98%) | **1.1 ms (0%)** | 281.2 ms | 0.99 ms |
| 128 | 129 | 643 | 768 | 10720 | 10.8 ms (0%) | 0.9 ms (0%) | 2443.3 ms (99%) | **3.5 ms (0%)** | 2458.6 ms | 9.46 ms |

`fixpoint+seal` is the ledger valuation, stratification, and saturation. It is
not separately callable from outside `dli-world`, so it is measured by
difference — which is why the loader is *timed against the identical documents*
rather than estimated.

What the columns say:

- **`check::run` never exceeds 2%, and trends to 0%.** Slicing it (the DL-0087
  deferral DL-0088 §3 reconsidered) would buy about one percent of a compile at
  the sizes that hurt. It is the cheapest phase measured at every size but the
  smallest.
- **The fixpoint is the term.** 4× the personas (32 → 128) costs **57×** the
  fixpoint time — roughly `O(n^2.9)` — while derived rows grow 9.6×. Time grows
  faster than the relation does.
- **Reconstruction is cheap and its share collapses**, 26% → 0%. The store's read
  path — two DAG walks and a read of every fact object — is not the problem it
  might have looked like.
- **The warm read is 45×–260× the cold one** (0.06 ms vs 2.8 ms; 9.5 ms vs
  2459 ms). That is the session cache doing its whole job: a repeat read of an
  already-compiled commit stops being a compile.

## 2 — Delta curve: `compile_incremental` over a 64-persona base

Base: 323 declarations, full compile 280.2 ms, `check::run` 1.0 ms.

| delta personas | delta decls | full compile of union | `compile_incremental` | speedup | loader reuse live | fixpoint reuse live | strata skipped | check::run on union |
|---:|---:|---:|---:|---:|:--:|:--:|---:|---:|
| 1 | 5 | 295.8 ms | 310.0 ms | 0.95× | yes | no | 0 | 1.1 ms |
| 2 | 10 | 316.2 ms | 335.1 ms | 0.94× | yes | no | 0 | 1.1 ms |
| 4 | 20 | 344.7 ms | 359.9 ms | 0.96× | yes | no | 0 | 1.2 ms |
| 8 | 40 | 400.2 ms | 420.9 ms | 0.95× | yes | no | 0 | 1.3 ms |
| 16 | 80 | 565.3 ms | 607.6 ms | 0.93× | yes | no | 0 | 1.6 ms |
| 32 | 160 | 989.6 ms | 1055.1 ms | 0.94× | yes | no | 0 | 2.1 ms |

**`compile_incremental` is a small net loss on this workload** — 0.93×–0.96×,
i.e. 4–7% *slower* than a full compile. This is not a defect; it is DL-0088 §1's
prediction, sharpened by measurement:

- **Loader reuse is live at every delta size** and does exactly what it claims —
  but the loader is 0.4 ms of a 280 ms compile, so reusing all of it saves less
  than a quarter of one percent.
- **Fixpoint reuse is refused at every delta size** (`strata skipped: 0`). This is
  the structural `same-as` case DL-0088 §1 documents: synthesized alias rules are
  appended at the end of the program, so once the *base* declares any identity
  the prior's facts and rules stop being a prefix of the union's,
  `saturate_incremental` correctly declines, and the full fixpoint runs.
- The residual 4–7% is the incremental path's bookkeeping — cloning the prior
  loader state and pack — paid against a fixpoint that was never reused.

So on the identity workload PR 2 buys nothing, honestly measured. Its win needs a
delta high in the dependency order in a world that does *not* declare identity;
that regime is not represented here and is not the SubtleMemory shape.

## 3 — What the session cache removes from an `author.merge` scan

DL-0087 §1 names the concrete problem: `author.merge`'s scan recompiles the whole
world 3–8× per call. The scan compiles target, source, base, and the merge
commit; with the session cache a rescan of the same heads compiles nothing.

| personas | scan (cold cache) | scan (warm cache) | speedup | worlds compiled cold | warm |
|---:|---:|---:|---:|---:|---:|
| 8 | 19.7 ms | 6.1 ms | 3.21× | 3 | 0 |
| 16 | 53.2 ms | 15.5 ms | 3.42× | 3 | 0 |
| 32 | 216.3 ms | 62.1 ms | 3.48× | 3 | 0 |
| 64 | 1198.8 ms | 326.1 ms | 3.68× | 3 | 0 |
| 128 | 10315.9 ms | 2564.6 ms | 4.02× | 3 | 0 |

**3.2×–4.0×, rising with world size, and three pre-merge recompiles become
zero.** The residual warm cost is the merge commit itself — a fresh two-parent
commit whose hash nobody has seen before, so it is a genuine compile every time,
and it is the *whole* remaining cost of a warm scan.

Note what this does **not** depend on: unlike DL-0086's `recompiles_saved`
memoization, whose useful life DL-0087 §1 bounded by the pilot's own progress
("it helps only while `base == target_head`"), the session cache spans scans and
is keyed on content. A branch merged twice, a head already opened for authoring,
a dry run followed by the real merge — all hit.

## 4 — What this baseline says about the next lever

Stated as measurement, not as a ruling:

1. **The check-battery slicing trigger did not fire.** At 0–2% and falling,
   slicing three whole-world check classes with silent-miss failure modes would
   trade a real correctness surface for about one percent. DL-0088 §3's refusal
   to slice on a guess now has a measurement behind it, pointing the same way.
2. **The fixpoint is the residual term, at `O(n^2.9)` on this rule set.** The
   levers that touch it are the ones DL-0088 §1 already priced: per-derivation
   provenance (needed to beat ~1.0× for `same-as`, and heavier than the DRed
   machinery DL-0087 rules out), or a rule-level change to the self-join.
3. **Caching redundant recompiles was the right first move and is now spent.**
   It removed 3 of 4 compiles from a merge scan and made repeat reads free. What
   remains is the *first* compile of each new commit, which no cache can remove.
4. **This is one workload on one machine.** The self-join in the rule set is
   doing real work in the fixpoint column; a corpus without one is unmeasured.
   Before any of the above becomes a decision, it wants a second shape.
