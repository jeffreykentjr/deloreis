---
status: reference
title: "Compile-cost baseline: where the time goes once compiles are cached"
verdict: "The measurement DL-0088 §3 asked for, taken. It REFUTES the prediction: check::run is 0–1% of the residual per-author.merge cost and its share falls as the world grows, so slicing the check battery would buy about one percent. The symbolic fixpoint is the residual O(world) term — 49% of a small merge's residual rising to ~100% at 128 personas, growing ≈O(n^2.9). The PR 3 session cache takes a warm author.merge scan from 10.7× to 124.6× faster (4 whole-world compiles to 0); compile_incremental (PR 2) measures 0.97–0.98× — a small net loss — on the identity workload, exactly as DL-0088 §1 predicted for same-as. Data only; no ruling. The retired delta measurement is reproducible from the pre-fiction-reset tag; the remaining measurements use tests/compile_cost_baseline.rs."
---

# Compile-cost baseline

DL-0088 §3 refused to slice the whole-world check battery on a guess and
required instead that **"PR 3 instruments the check-battery cost so its slicing
trigger fires on a measurement rather than a guess."** This note is that
measurement, committed as a reproducible historical baseline.

It is `status: reference` — **data, not a ruling.** It records what a run
measured. Any change of course it suggests belongs in `decisions.md`, not here.

## The headline

**`check::run` is not the next bottleneck.** DL-0088 §3 predicted it would
become the residual `O(world)` term once compiles were cached. Measured directly
against that question — a merge whose three pre-merge worlds are already cached,
so it pays only for what is genuinely new — `check::run` is **0–1% of the
residual at every size, and its share falls as the world grows.** The symbolic
fixpoint is the residual term instead, at 49% of a small merge's residual rising
to essentially all of a large one.

That does not reverse DL-0088; it answers the question DL-0088 asked. The
instrument was built precisely so the slicing trigger would fire on evidence, and
the evidence says the trigger should not fire.

## How to reproduce

The delta-curve measurement was retired by DL-0102 along with the unused
compiler. To reproduce all four historical tables, check out `pre-fiction-reset`
first. On the current tree, the command below reproduces the remaining tables.

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
`--test-threads=1` keeps the four tables from interleaving. **All four tables
below come from one invocation**, so they can be read against each other; a
repeat run moves individual cells by a few percent without moving any conclusion.

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
assertions as the *worst* case for fixpoint reuse, so this measurement
uses an identity-heavy workload.

## 1 — The residual per-`author.merge` cost split

**This is the table that answers DL-0088 §3.** With the session cache in effect
the redundant recompiles collapse and each merge pays only for the merge commit
itself, which no cache can have seen before. The three pre-merge worlds are
warmed before the clock starts — that is the post-PR-3 steady state, not a trick.

| personas | derived rows | residual scan | reconstruct | loader | fixpoint+seal | check::run | scan detection |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 8 | 224 | 6.7 ms | 1.5 ms (22%) | 0.1 ms (1%) | 3.3 ms (49%) | **0.1 ms (1%)** | +1.8 ms (+26%) |
| 16 | 496 | 18.7 ms | 2.4 ms (13%) | 0.2 ms (1%) | 11.7 ms (62%) | **0.2 ms (1%)** | +4.3 ms (+23%) |
| 32 | 1232 | 65.1 ms | 4.9 ms (8%) | 0.2 ms (0%) | 53.3 ms (82%) | **0.4 ms (1%)** | +6.2 ms (+10%) |
| 64 | 3472 | 335.3 ms | 8.7 ms (3%) | 0.5 ms (0%) | 307.2 ms (92%) | **1.2 ms (0%)** | +17.7 ms (+5%) |
| 128 | 11024 | 2530.8 ms | 17.5 ms (1%) | 0.9 ms (0%) | 2594.2 ms (103%) | **9.8 ms (0%)** | -91.6 ms (-4%) |

- **`check::run` is 0–1% of the residual at every size, falling to 0%.** Slicing
  three whole-world check classes with silent-miss failure modes would buy about
  one percent of the cost that actually remains.
- **The fixpoint is the residual term**, 49% → ~100%.
- **The scan's own detection work** (exclusivity, FD grouping, the
  standing-downgrade sweep over every derived row) is real at small sizes — 26%
  of a 8-persona merge — but its *share* falls as the fixpoint outgrows it.

**Two columns need a caveat, stated rather than hidden.** `fixpoint+seal` is
measured by difference (`compile` − `loader`, both timed directly), and `scan
detection` by a second difference (`residual` − `reconstruct` − `compile` −
`check`). At 128 personas the compile being subtracted out takes ~2.5 s, and its
run-to-run variance exceeds the quantity being measured — which is why that row
reads 103% and **−91.6 ms**. The negative is printed signed on purpose: it is the
honest report that the method has reached its noise floor there, not a
measurement of zero. Read the detection column only where it is large and
positive; the `check::run` column is timed directly and carries no such caveat.

## 2 — Phase split per cold `compile_at`

The same split for a single cold compile, across world sizes — the cost of the
*first* compile of any commit, which no cache removes.

| personas | commits | decls | facts | derived | reconstruct | loader | fixpoint+seal | check::run | cold total | warm `compile_at` |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 8 | 9 | 43 | 48 | 160 | 0.8 ms (26%) | 0.1 ms (3%) | 2.0 ms (69%) | **0.1 ms (2%)** | 2.9 ms | 0.07 ms |
| 16 | 17 | 83 | 96 | 416 | 1.4 ms (14%) | 0.1 ms (1%) | 8.7 ms (84%) | **0.1 ms (1%)** | 10.5 ms | 0.15 ms |
| 32 | 33 | 163 | 192 | 1120 | 2.8 ms (6%) | 0.2 ms (1%) | 43.0 ms (93%) | **0.4 ms (1%)** | 46.4 ms | 0.35 ms |
| 64 | 65 | 323 | 384 | 3296 | 5.3 ms (2%) | 0.5 ms (0%) | 272.7 ms (98%) | **1.0 ms (0%)** | 279.5 ms | 1.28 ms |
| 128 | 129 | 643 | 768 | 10720 | 11.2 ms (0%) | 0.9 ms (0%) | 2265.1 ms (99%) | **3.3 ms (0%)** | 2280.4 ms | 11.26 ms |

- **The fixpoint grows faster than the world does.** 4× the personas (32 → 128)
  costs **53×** the fixpoint time — roughly `O(n^2.9)` — while derived rows grow
  9.6×.
- **Reconstruction is cheap and its share collapses**, 26% → 0%. The store's read
  path — two DAG walks and a read of every fact object — is not the problem it
  might have looked like.
- **The warm read is 40×–200× the cold one.** That is the session cache doing its
  whole job: a repeat read of an already-compiled commit stops being a compile.
  (The warm figure is dominated by the clone `compile_at` performs to return by
  value; `compile_at_shared` avoids even that.)

## 3 — What the session cache removes from an `author.merge` scan

DL-0087 §1 names the concrete problem: `author.merge`'s scan recompiles the whole
world 3–8× per call. The scan needs target, source, base, and the merge commit;
with the session cache a rescan of the same heads compiles **nothing**. Every
scan here is a dry run, so the target ref never moves and the cold and warm scans
are genuinely over the same two heads.

| personas | scan (cold cache) | scan (warm cache) | speedup | worlds compiled cold | warm |
|---:|---:|---:|---:|---:|---:|
| 8 | 18.6 ms | 1.7 ms | 10.70× | 4 | 0 |
| 16 | 52.9 ms | 3.5 ms | 15.16× | 4 | 0 |
| 32 | 207.3 ms | 6.9 ms | 29.89× | 4 | 0 |
| 64 | 1186.5 ms | 19.3 ms | 61.35× | 4 | 0 |
| 128 | 9235.4 ms | 74.1 ms | 124.60× | 4 | 0 |

**10.7×–124.6×, rising with world size, and four whole-world compiles become
zero.** All four — not three: the merged world is served from the cache too, so
the WS-3 sequence of a dry run followed by the real merge of the same two heads
pays for one compile rather than two.

Note what this does **not** depend on: unlike DL-0086's `recompiles_saved`
memoization, whose useful life DL-0087 §1 bounded by the pilot's own progress
("it helps only while `base == target_head`"), the session cache spans scans and
is keyed on content. A branch merged twice, a head already opened for authoring,
a dry run followed by the real merge — all hit.

## 4 — Delta curve: `compile_incremental` over a 64-persona base

Base: 323 declarations, full compile 290.5 ms, `check::run` 1.0 ms.

| delta personas | delta decls | full compile of union | `compile_incremental` | speedup | loader reuse live | fixpoint reuse live | strata skipped | check::run on union |
|---:|---:|---:|---:|---:|:--:|:--:|---:|---:|
| 1 | 5 | 293.8 ms | 303.4 ms | 0.97× | yes | no | 0 | 1.1 ms |
| 2 | 10 | 309.8 ms | 320.9 ms | 0.97× | yes | no | 0 | 1.1 ms |
| 4 | 20 | 341.8 ms | 347.6 ms | 0.98× | yes | no | 0 | 1.2 ms |
| 8 | 40 | 405.4 ms | 419.1 ms | 0.97× | yes | no | 0 | 1.3 ms |
| 16 | 80 | 574.8 ms | 586.0 ms | 0.98× | yes | no | 0 | 1.6 ms |
| 32 | 160 | 1006.7 ms | 1025.6 ms | 0.98× | yes | no | 0 | 2.3 ms |

**`compile_incremental` is a small net loss on this workload** — 0.97×–0.98×,
i.e. 2–3% *slower* than a full compile. This is not a defect; it is DL-0088 §1's
prediction, sharpened by measurement:

- **Loader reuse is live at every delta size** and does exactly what it claims —
  but the loader is 0.5 ms of a 290 ms compile, so reusing all of it saves less
  than a quarter of one percent.
- **Fixpoint reuse is refused at every delta size** (`strata skipped: 0`). This is
  the structural `same-as` case DL-0088 §1 documents: synthesized alias rules are
  appended at the end of the program, so once the *base* declares any identity
  the prior's facts and rules stop being a prefix of the union's,
  `saturate_incremental` correctly declines, and the full fixpoint runs.
- The residual 2–3% is the incremental path's bookkeeping — cloning the prior
  loader state and pack — paid against a fixpoint that was never reused.

So on the identity workload PR 2 buys nothing, honestly measured. Its win needs a
delta high in the dependency order in a world that does *not* declare identity;
that regime is not represented here.

## 5 — What this baseline says about the next lever

Stated as measurement, not as a ruling:

1. **The check-battery slicing trigger did not fire.** At 0–1% of the residual
   and falling, slicing three whole-world check classes with silent-miss failure
   modes would trade a real correctness surface for about one percent. DL-0088
   §3's refusal to slice on a guess now has a measurement behind it, pointing the
   same way.
2. **The fixpoint is the residual term, at `O(n^2.9)` on this rule set.** The
   levers that touch it are the ones DL-0088 §1 already priced: per-derivation
   provenance (needed to beat ~1.0× for `same-as`, and heavier than the DRed
   machinery DL-0087 rules out), or a rule-level change to the self-join.
3. **Caching redundant recompiles was the right first move and is now spent.**
   It removed all four compiles from a repeat merge scan and made repeat reads
   free. What remains is the *first* compile of each new commit, which no cache
   can remove — and that first compile is 99% fixpoint.
4. **This is one workload on one machine.** The self-join in the rule set is
   doing real work in the fixpoint column; a corpus without one is unmeasured.
   Before any of the above becomes a decision, it wants a second shape.
