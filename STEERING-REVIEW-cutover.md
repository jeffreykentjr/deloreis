---
status: live
verdict: "Claude-authored (Anthropic Claude Code review session, 2026-08-10) steering review of the fantasy↔deloreis cutover. Supersedes RESUMPTION-GUIDE-cutover.md for state and plan. Non-canonical: the two decision ledgers outrank this document."
---

# STEERING REVIEW — the fantasy↔deloreis cutover, re-planned

**Authorship: Claude (Anthropic), Claude Code review session, 2026-08-10.** This is a
Claude-authored review document — distinguish it from Poolside agent docs; like them it
is non-canonical guidance, and the two decision ledgers outrank it.

**Written:** 2026-08-10, by a read-only review session (no repo state was changed by the
review itself).
**Supersedes:** `RESUMPTION-GUIDE-cutover.md` (2026-08-06) for *state* and *plan*. That
guide remains useful ONLY for its operational-traps checklist (§2 there) and the stage
vocabulary; every concrete sha, ledger tail, and stage status in it is expired. The
five narrative surfaces (RESUMPTION-GUIDE, `docs/integration/STATUS.md`,
`cutover-progress.md`, `POST-SECTION5-FRONTIER.md`, the plans-dir milestone log)
contradict each other and themselves; do not plan from any of them. Plan from THIS file,
verified against the live trees.

**Authority:** the two ledgers still outrank everything, including this file — deloreis
`docs/decisions.md` (DL-nnnn), fantasy `canon/decisions.md` (D-nnnn — NOT
`docs/decisions/`, which does not exist, and NOT `docs/decisions.md`, which is an empty
shell). User rulings recorded in §5 are binding.

---

## §0 Verdict

**Real progress, but not on track to finish under the current design.** The engine is
genuinely built and the Elixir factory has served lore from the committed content-addressed
lorestore since 2026-08-07 (`d3757474`). But:

1. The **Python half of fantasy — the domain modeling actually being retired — never cut
   over.** Nine Python consumers still read `worlds/tpth/generated/parity.json`, which
   `scripts/compile_lore_dli.py:520` still generates directly from six YAML files. DP-013's
   FW-6 requirement (parity.json's producer becomes the engine's export op) was silently
   dropped from the flip.
2. The in-flight re-point pattern **structurally prevents authority transfer**: every
   domain keeps YAML as a mandatory byte-exact fallback, required CI never sets
   `DELOREIS_BIN`, and the only store-integrity CI job is advisory and self-skips. The
   engine can mirror forever; it can never own.
3. **Both mainlines have been frozen since 2026-08-07**; the factory's merge throughput
   collapsed ~90% under the campaign's process weight; 71 commits of real TPtH product
   work are rotting on an unmerged branch that also forked the fantasy ledger id space.
4. An **urgent ledger hazard is one command from landing** (§2.1), and the cross-repo
   ordering that would let the open PRs land is documented nowhere (§2.2).
5. The "humans sign off on canon" legacy is scrubbed at the deloreis governance layer but
   alive in the spec/cards layer, essentially untouched on the fantasy side, and has one
   real modeling error underneath (provisional ≡ ratified in valuation). §4 prescribes
   the fixes; the user has ruled the target model (§5).

**Definition of done** (what "over the line" means): fantasy's Python AND Elixir halves
serve canon from deloreis as the single authority; agents author lore through `author.*`
MCP end-to-end; the byte-exact YAML fallback pattern is retired and the four flipped
domains' YAML deleted; both mains unfrozen; the product branch landed. Explicitly NOT
required for done: contract 2.0.0 / 0.x retirement (DM-6), FW-9's validator flip, the
FW-7a–d data expansions (§6).

---

## §1 Live-state map (verified 2026-08-10 ~12:00 BST; RE-VERIFY before acting — the
active agent is moving)

### Processes — do not collide
| PID | State | What |
|---|---|---|
| 714413 | running (`Sl+`, pts/10) | **The active cutover agent.** Owns deloreis PR #43 and fantasy PR #21. It has already reset fantasy `fw6-flip` at least once today (the 11:40 merge commit `0f06f829` was dropped; PR #21's head is back to `c37b6490`, now DIRTY vs main). Coordinate through the PRs / its lane, do not edit its worktrees. |
| 704532 | **suspended** (`Tl`, same tty) | Earlier Claude session; authored the deloreis merge `07f8d8f` and PR #43, then was suspended. **Do not foreground it** — it would resume as a second writer on `integration-status` and the single-writer ledger. Confirm its worktree is porcelain-clean, then kill it. |
| 1428862/1431660/1432856 | idle since Aug 04 | Leaked Python capability-broker loops from an old session. Harmless; kill at leisure. |

### Open PRs
| PR | Head | State | Meaning |
|---|---|---|---|
| deloreis **#43** (`fw6-flip-deliverable` → main) | `07f8d8f` | OPEN, all 5 checks green, MERGEABLE/CLEAN | **DO NOT MERGE AS-IS** — deletes released DL-0062 (§2.1). Head branch is byte-identical to local `integration-status`; `origin/integration-status` is 7 commits stale — judge campaign state from the PR head, not that ref. |
| fantasy **#21** (`fw6-flip` → main) | `c37b6490` | OPEN, DIRTY, 4/5 checks red | **Structurally un-greenable until #43 lands + a new tag exists** (§2.2). The active agent may be iterating on it right now without knowing that. |
| fantasy **#6** (`audio-cutover-20260804`) | `03a2aa1a` | OPEN, dormant since 08-04 | Unrelated TTS/SFX cutover. Merge or close so #21's state is unambiguous; not on the critical path. |

### Branches and stores
- deloreis: `origin/main` = `3318022` (ledger tail **DL-0062** "epistemic infrastructure",
  PR #42). Local `integration-status` = `07f8d8f`, 12 ahead / 0 behind, carries canon
  projections + six ledger entries it numbered DL-0062..0067 (collision — §2.1).
- fantasy: `origin/main` = `d3757474` (the FW-6 flip). The real cutover lane is branch
  `fw6-flip` in the nested worktree
  `/home/jeff/Documents/scratch/fantasy-pool/.claude/worktrees/fw6-flip` (~18-19 commits
  ahead; pushed today as `origin/fw6-flip`). The primary checkout
  `/home/jeff/Documents/scratch/fantasy` is on `slice/milestone-a-reproject-handoff` —
  **71 commits of TPtH audio/publication product work, zero cutover content**, 27 behind
  main, 3 commits unpushed, no PR, diverging since 08-02, and carrying a **colliding
  D-0247/0248/0249** (voice rulings minted under ids main later used for governance).
- Store roots: fantasy main serves `db80a5a3` (556 objects — all published dual-serve
  evidence pins THIS root). `fw6-flip` ships `6a165d81` (578 objects, killed-token
  registry re-seed) — **no dual-serve run recorded against it** (§3 step 3).
- Binaries/targets: `/home/jeff/tg-dm4fix` and `/home/jeff/tg-findingsanitize` are live
  tooling named by STATUS.md — do not delete. The other eleven `/home/jeff/tg-*` are dead
  caches. `/tmp/s5` (the §5 scratch store STATUS defers work onto) **no longer exists**.
- Worktrees: ~26 deloreis worktrees are stale-but-clean; most hold UNMERGED commits, so a
  blanket prune discards work — leave them until a deliberate hygiene pass. The only hot
  fantasy worktree is `fw6-flip` (lorestore objects written today).

### Do-not-touch list
`fw6-flip` worktree (hot, active agent), `integration-status`/`fw6-flip-deliverable`
(active agent owns via PR #43), the suspended PID's terminal, `tg-dm4fix`/
`tg-findingsanitize`, and both `docs/decisions.md`/`canon/decisions.md` outside the
repair steps below (single-writer discipline: one lane at a time).

---

## §2 DROP EVERYTHING (hours, strict order)

### §2.1 PR #43 erases a released ledger ruling — repair before any merge

This morning's merge `07f8d8f` resolved the `docs/decisions.md` conflict by taking the
branch side wholesale. Consequence: **origin/main's released `DL-0062 — "De Lore Is is
epistemic infrastructure; the archive destination is peer plus service"` (PR #42) is
deleted**, and the number is re-used for the branch's killed-registry exemption. Nine doc
citations (PLAN.md front-matter + DIST-1..4 rows + four design notes) and two code
comments would dangle or point at the wrong ruling. Every check is green because
`validate_append_only` (`xtask/src/authority.rs:237`) compares only the FIRST parent, and
CI's gate job checks out at depth 1 so the baseline is unreadable and the check passes
vacuously. Nothing automated stands between `gh pr merge 43` and a corrupted ledger.

**Repair (a branch REBUILD, not a follow-up commit — authority compares first-parent, so
in-place correction of committed entries fails the checker; consistent with the
`ledger-edit-needs-amend` memory):**

```sh
git checkout -b fw6-flip-deliverable-v2 origin/main        # main tip 3318022
git diff origin/main...origin/fw6-flip-deliverable -- . ':(exclude)docs/decisions.md' \
  | git apply --index
```

Then hand-append the branch's six entries AFTER main's DL-0062, renumbered **+1**:

| was (branch) | becomes | entry |
|---|---|---|
| DL-0062 | **DL-0063** | `killed` registry predicate exempt from factcheck.contradicted scan |
| DL-0063 | **DL-0064** | exempt at all three scan sites (its "supersedes" now cites DL-0063) |
| DL-0064 | **DL-0065** | killed-tokens canon domain store-projected |
| DL-0065 | **DL-0066** | facts sub-maps (relations, constraints) |
| DL-0066 | **DL-0067** | chronology before-edges |
| DL-0067 | **DL-0068** | LS-7 by_pred / YAML cutover |

Fix every shifted reference: `crates/dli-world/src/check/continuity.rs:217` and
`crates/dli-world/src/check/factcheck.rs:123` ("DL-0062, refined by DL-0063" →
"DL-0063, refined by DL-0064"), all intra-ledger cross-refs (including the retrospective-
mint attribution paragraphs), and every `DL-006[2-7]` mention in the STATUS/progress docs
on the branch. Main's nine DL-0062 citations become correct automatically once main's
entry is restored. Force-push over `fw6-flip-deliverable` so PR #43 keeps its number.

**Verify:** `cargo xtask authority` green; `git diff origin/main...HEAD --
docs/decisions.md | grep '^-' | grep -v '^---'` empty (pure append); exactly one
`## DL-0062` heading and it is the epistemic-infrastructure entry; `cargo xtask gate` green.

**Then harden the guard so this class can't recur** (same PR or immediate follow-up):
baseline = merge-base with the PR base ref (not `HEAD^`); an unreadable baseline is a HARD
FAILURE, not a pass; `fetch-depth: 0` on the gate job (`.github/workflows/ci.yml:22`; the
`no-ai-authorship` job at :83-84 already does this).

### §2.2 Contract 1.4.0 — the canon surface must not ship as a same-version wire change

The branch adds `CorpusExportFormat::Canon` + a `canon` response field + two regenerated
schemas while `CONTRACT_VERSION` stays `"1.3.0"` — so "1.3.0" would mean two different
wire surfaces depending on the commit. In the rebuilt PR #43:

- `crates/dli-serve/src/lib.rs:71` → `"1.4.0"`; append to `ACCEPTED_CONTRACT_VERSIONS`
  (lib.rs:99). Gate the canon format ≥1.4.0 (the `added_in_1_3_0` pattern at
  `crates/dli-serve/src/ops.rs:270` is the template); a ≤1.3.0 frame requesting
  `format:"canon"` must refuse **byte-identically to the `contract/1.3.0` tag build's
  refusal today** — replay the same frame against both binaries and diff.
- `docs/integration/fantasy-contract.md` to 1.4.0 truth (it currently documents `format`
  as only `objects`/`rendered`); version-echo + canon-refusal fixtures; mint **DL-0069**
  (the bump ruling: canon export rides 1.4.0; the same-version wire change is repaired).
- One clean-context red-team sub-agent on the committed diff (§5 ruling), squash-merge,
  **annotated tag `contract/1.4.0` on the merge commit**, pushed. Deloreis main unfrozen.

**Keep 1.4.0 minimal.** The marker-vocabulary unification and the standing tools (§4
Fixes 1–2) ride **contract 1.5.0**, a separate later PR. Do not let 1.4.0 grow — it
exists to unblock fantasy PR #21 fast.

### §2.3 Fantasy PR #21 — stop iterating until the tag exists

PR #21 is structurally un-greenable from inside fantasy: its `worlds/tpth/DELOREIS_TAG`
pins `contract/1.3.0`, whose tree contains zero `canon_*` surfaces; the `lore-dual-run`
job's failures ("dli.corpus.export: worker exited 2", "the committed lorestore does not
rebuild to the same root hash") are correct behavior, not bugs. **The active agent may be
debugging this right now — the first message (§3 Part F) tells it to stop.** After
`contract/1.4.0` is tagged, PR #21 needs, in one pass: the re-pin + version constants,
the +1 DL-citation sweep, the hardcoded-absolute-path fix, two ruff E501s, the minimal V4
amendment, two D-mints, and the fresh dual-serve log (§3 step 4).

### §2.4 The suspended session

Do not resume PID 704532. Verify its checkout is porcelain-clean; kill it. Until PR #21
merges, `fw6-flip` has exactly one writer.

---

## §3 Recovery sequence (dependency-ordered; owner / deps / size / verification)

> Process baseline for every step, per user ruling (§5): local gate green → commit →
> ONE clean-context red-team sub-agent over the COMMITTED diff (non-trivial PRs only) →
> push → PR → auto-merge → verify green. No 20-cycle accruals, no byte-exact-parity
> ceremonies, no multi-round loop-until-dry.

### Step 0 — Lockdown and safety pushes. Owner: operator. Deps: none. Size: S.
- Deliver the Part F message (below) to the active agent before it pushes anything more.
- Kill (don't resume) PID 704532 after confirming its tree is clean.
- Push the product branch's 3 unpushed commits (`slice/milestone-a-reproject-handoff`) —
  pure backup, no review implied. Push mechanics per memory: `ssh -F /dev/null` +
  non-sandboxed git.
- **Verify:** `git status --porcelain` clean everywhere;
  `origin/slice/milestone-a-reproject-handoff` at the local tip.

### Step 1 — PR #43 ledger repair. Owner: deloreis. Deps: 0. Size: M.
Exactly §2.1. **Verify:** as listed there.

### Step 2 — Contract 1.4.0 in the same PR; merge; tag. Owner: deloreis. Deps: 1. Size: M.
Exactly §2.2. **Verify:** gate green with `ls8_zero_x_parity` untouched; version echo
`1.4.0`; the ≤1.3.0 canon-refusal diff empty; tag on the remote; main CI green post-merge.

### Step 3 — ONE dual-serve run at store head `6a165d81`. Owner: fantasy (fw6-flip
worktree). Deps: 2. Size: S.
All published evidence (217 predicates identical, 5-mutation battery) was recorded at
parent `db80a5a3`; the shipped head has none. Build both binaries at `contract/1.4.0`
(short `CARGO_TARGET_DIR`, e.g. `/home/jeff/tg-<lane>`), run `scripts/lore_dual_run.sh` +
`scripts/lore_pivot_dual_serve.py` covering all four canon domains (killed-tokens
especially — the re-seed touched it), plus ONE seeded divergence shown red (a harness
that cannot fail is not evidence). **Verify:** a committed log naming store head
`6a165d81` and tag `contract/1.4.0`, clean, with the seeded-red transcript beside it.
This commit rides PR #21.

### Step 4 — PR #21 to green and merged. Owner: fantasy. Deps: 1–3. Size: M.
In one pass:
1. `worlds/tpth/DELOREIS_TAG` → `contract/1.4.0`; flip both version definition sites
   (`src/fantasy/lore/contract.py` `DLI_CONTRACT_VERSION` / `DLI_CORPUS_CONTRACT_VERSION`
   as applicable, and `Factory.Lore.DliBridge.contract_version/0`) to `1.4.0`.
2. DL-citation sweep **+1** per the §2.1 table (`grep -rn 'DL-006[2-7]'` — e.g. the
   `compile_characters` commit's DL-0067 is now DL-0068).
3. `scripts/compile_lore_dli.py:39`: `CANON_PROJECT` default must derive from the repo
   root (`Path(__file__).resolve().parents[1] / "worlds/tpth/lorestore-project.toml"`),
   keeping the `DLI_CANON_PROJECT` override. The absolute worktree path must not merge.
4. Fix the two ruff E501s (`uv run ruff check .`).
5. **Minimal V4 amendment** (`src/fantasy/canon/validator.py:53,236-246`): machine-
   attributed entries carry `rationale:` + `evidence:` in lieu of `quote:`; `quote:`
   stays required for human entries. Required IN THIS PR — its own D-mints cannot pass
   V4 otherwise. (The full governance pass is §4 Fix 4, a separate PR.)
6. Mint **D-0248** (retroactive FW-6 exit entry — the flip landed 08-07 without its
   required D-entry; honest about the retro-mint) and **D-0249** (the B–E canon
   re-points, citing the step-3 log and the renumbered DL-0065..0068). D-id rule: **mint
   against origin/main's tail at mint time; fetch first** — the numbers here are
   illustrative of order, not fixed.
7. The `factory` job's Hex `cache.ets: :badfile` failure is a corrupted runner cache —
   rerun, don't debug Elixir.
8. Red team; merge. **Fantasy main unfrozen.**
**Verify:** `checks` aggregate green; `uv run fantasy validate` green including machine
entries; no `contract/1.3.0` or stale `DL-006x` reference; `git grep fantasy-pool` empty.

### Step 5 — Land the product branch. Owner: fantasy. Deps: 4. Size: M.
Immediately after the mains unfreeze, BEFORE authority work — it is the actual product,
rotting since 08-02, and nothing later depends on it.
- Merge `origin/main` into `slice/milestone-a-reproject-handoff` (rebasing 71 commits is
  churn; the PR squash-merges anyway).
- Resolve the forked ledger: **main's ids win** (published + load-bearing). The branch's
  three voice rulings currently numbered D-0247/0248/0249 renumber to the next free ids
  after step 4 (illustratively D-0250/0251/0252); sweep every citation and `affects` in
  `canon/narrative/tone-style.md` and `canon/characters/{protagonist,apprentice,anchor}.md`;
  each renumbered entry gains a `note:` recording the renumber.
- `scripts/greengate.sh` (private venv per memory), red team, PR, squash-merge.
**Verify:** exactly one `## D-0247` heading in `canon/decisions.md` (the governance one);
`fantasy validate` + `checks` green; no factory episode-gate regression.

### Step 6 — Authority transfer: the producer flip. Owner: fantasy (deloreis read-only).
Deps: 4. Size: M.
**The design decision:** committed canon snapshot, diffed offline (REQUIRED); real-binary
lane stays advisory. A required real-binary lane needs a deloreis read credential fantasy
CI lacks (that's why `lore-dual-run` self-skips) and puts a network fetch in a gating
lane, which the determinism law forbids. Instead:
- `worlds/tpth/generated/parity.json` keeps its name and shape but its **producer flips**
  to the engine: a new `scripts/refresh_canon_projection.sh` (or `--from-store` mode of
  `compile_lore_dli.py`) runs the pinned-tag binary, exports
  `dli.corpus.export{format:"canon"}` over `worlds/tpth/lorestore`, writes parity.json
  with a provenance header `{tag, store_head, digest}`, committed.
- The **required** offline CI check (in the `python` job + a re-aimed clause in
  `scripts/check-lore-cutover-boundaries.sh`) enforces the chain: header `store_head` ==
  the commit in `worlds/tpth/lorestore/refs/main`, header `tag` == `DELOREIS_TAG`, digest
  matches the file. A store advance without regeneration goes red.
- The nine Python call sites (`world/compile.py:461`, `coverage.py:101`, `build.py:287`,
  `canon/validator.py:425`, `naming/cast.py:63`, `propagate/check.py:294`, `report.py:73`,
  `shadow_lore.py:24`) keep reading parity.json unchanged — producer changes, shape
  doesn't. **Delete the byte-exact-fallback branches** in the four re-pointed lanes
  (deleted, not merely unexercised).
- `lore-dual-run` flips advisory → required as a strict upgrade if/when
  `DELOREIS_DEPLOY_KEY` is provisioned; the cutover does not wait for it.
- Mint the D-entry (illustratively D-0253).
**Verify:** seeded canary both ways — mutate the snapshot only: every consumer sees it;
mutate the store ref without regenerating: CI goes red.

### Step 7 — Per-domain YAML deletion (one batched PR). Owner: fantasy. Deps: 6. Size: M.
**Exit condition per domain licensing deletion:** (i) consumers read only the snapshot,
fallback code gone (step 6); (ii) a dual-run log at the CURRENT store head committed (the
step-3 log serves unless the store advanced — then one fresh run); (iii) the boundaries
tripwire re-aimed to "domain YAML absent + snapshot head == store head" (today it guards
only the old `canon/data/` paths while identical bytes live in `tests/fixtures/` — the
D-0245 "flip" was a byte-identical file move, sha256-verified); (iv) a machine D-mint per
deletion. Delete: `src/fantasy/propagate/data/killed_tokens.yaml`,
`tests/fixtures/{facts,timeline,characters}.yaml`, and the corresponding emitter sections
of `compile_lore_dli.py`. YAML of un-migrated domains (`naming/data/*.yaml`,
`canon/data/{lexicon,banned-vocabulary}.yaml`, `episodes/*.yaml`) **stays** — FW-7 scope,
deferred (§6). Red team priority lens: deletion-with-live-caller grep.
**Verify:** boundaries script green in re-aimed form; full pytest + ExUnit green with the
YAML gone; `git grep` of each deleted filename returns only docs/history.

### Step 8 — FW-8 authoring + closeout. Owner: fantasy (deloreis serves). Deps: 6, and
§4 Fixes 1–2 at contract 1.5.0 (without Fix 1, authored facts carry `Decided` and canon
fails closed on them). Size: M.
- Register `deloreis mcp --project worlds/tpth/lorestore-project.toml` at the pinned tag;
  adapt the engine's `SKILL.md` into the factory agent instructions (attribution +
  decision state supplied as data on every mutating call, DL-0058). **The authoring path
  is a stateful session** — hold `author.open`'s workspace across all frames; the one-shot
  `DliBridge` pattern fails with `unknown-doc` (learned the hard way; FW-8 needs a
  session-shaped transport beside the bridge).
- Run ONE real end-to-end session: `author.add_*` → graded findings → structured diff →
  `author.materialize` → store head advances → `refresh_canon_projection.sh` → commit →
  CI green → machine D-mint. Draft-canon promotion/defeat stays a data lifecycle (§5).
- Closeout: advance deloreis `STATUS.md` rows with 1.4.0/1.5.0 evidence; update the
  fantasy mirror (`docs/reference/deloreis-integration-status.md` — last written at FW-5)
  in the same cycle; delete `RESUMPTION-GUIDE-cutover.md` (this file supersedes it).
**Verify (the done-check):** a canon change authored only through MCP reaches a shipped
factory read with no YAML anywhere in the path; both mains green; product branch merged;
the STATUS pair agrees.

### Part F — the first message to the active agent (send verbatim, step 0)

> **Stop before merging either PR — both have a defect that must not reach main, and
> there is a cross-repo ordering you can't see from inside either PR.**
> 1. **PR #43 must not merge as-is.** Its `docs/decisions.md` deletes main's released
>    `DL-0062` ("epistemic infrastructure", cited by PLAN.md and four design notes) — the
>    morning merge resolved the ledger `--ours`, and the branch re-used DL-0062–DL-0067
>    for its own rulings. CI is green only because the append-only check is
>    one-step-first-parent. Rebuild the branch from `origin/main` with the six entries
>    renumbered DL-0063–DL-0068 (fix `continuity.rs:217`, `factcheck.rs:123`, and all
>    cross-refs), and add the **contract 1.4.0 bump** in the same PR — the canon export
>    surface is currently a same-version wire change at 1.3.0. Force-push over
>    `fw6-flip-deliverable` so the PR number survives. Tag `contract/1.4.0` on the merge
>    commit after it lands.
> 2. **Stop iterating PR #21 until that merges and the tag exists.** It is structurally
>    un-greenable: its `DELOREIS_TAG` pins `contract/1.3.0`, which lacks the `canon_*`
>    surfaces the branch consumes — the `lore-dual-run` reds are correct behavior. Once
>    the tag is up: re-pin + bump both version constants to 1.4.0, shift your DL
>    citations +1 (your DL-0067 is now DL-0068), fix the absolute `CANON_PROJECT` default
>    in `scripts/compile_lore_dli.py:39` to a repo-relative path, fix the two E501s,
>    amend `validator.py` so machine-attributed entries carry `rationale`+`evidence` in
>    lieu of `quote` (per D-0247), mint D-0248 (retro FW-6 exit) and D-0249 (the B–E
>    flips), and commit one fresh dual-serve log at store head `6a165d81` against the
>    tagged binary — the published evidence stops at its parent `db80a5a3`. The
>    `factory` job's Hex `cache.ets` failure is a runner cache fault — rerun it.
> 3. Do not resume or coordinate with the suspended session (PID 704532); you are the
>    only writer on this lane. One clean-context red-team sub-agent per PR before merge;
>    the 20-cycle accrual and byte-exact-parity ceremonies are cut by user ruling.
>    Priority order: PR #43 → tag → PR #21 → the product branch. Full plan:
>    `deloreis/STEERING-REVIEW-cutover.md`.

---

## §4 Design prescriptions (the flaws causing the friction, and their fixes)

Sequencing: Fix 1 first among these (it un-walls FW-8 and carries contract 1.5.0); Fix 2
rides the same 1.5.0 PR; Fix 3 depends on Fix 1; Fix 4 is fantasy-side and independent
(its minimal V4 slice already rode PR #21); Fix 5 is a docs-only batch, land any time.

### Fix 1 — One marker vocabulary: retire `Decided` (deloreis, contract 1.5.0)

**The fault:** the store has two provenance vocabularies by entry door — `.dli` lowering
emits `Ratified`/`Provisional`/`Proposed`; the `author.*` tools hardcode `Decided`
(`crates/dli-serve/src/author/tools.rs:1973`). The canon projections fail closed on
`Decided` (`handlers.rs:2852-2863`; canon_timeline at :2773-2783 also wrongly refuses
`Proposed`). Net: **facts authored through the one taught interface can never reach
canon.** That is the mechanical form of "the bot hangs on proposed-vs-ratified".

**Fix:** the single vocabulary is `proposed | provisional | ratified` (absent =
asserted; defeated is a valuation outcome, Fix 2). Retire `Decided` via the killed-form
registry. At 1.5.0 the wire `Provenance::Cited` gains a required
`state: "ratified"|"provisional"`; `annotation()` lowers to that marker + citations.
Keep the `Decided` variant in `dli-ast` one cycle for deserialization safety; forbid it
at every entry point; delete at 2.0.0. **Migration:** SCHEMA_VERSION 3→4 upcast-on-read
mapping `Decided`→`Provisional` (fail-safe: still certain in valuation, spend-gated until
promoted). Lineage-safe because `FactProvenance` is OUTSIDE the identity hash
(`object.rs:161-186` "recorded, never hashed") — no content-hash churn. Byte-compat
ruling goes in the DL: the multi-version law freezes wire shapes and 0.x rendered lines,
not store metadata a schema upcast rewrites (precedent: v2→v3). Fallback if pinned
fixtures assert `Decided` bytes on ≤1.3.0 frames: per-version rendering alias. **Canon
filters by status VALUE, never by door:** one shared `status_of` for all canon
projections; accepted set `Ratified|Provisional|Proposed|None→asserted`; anything else
fails closed as real corruption (retargets DL-0067's red-team property instead of
deleting it).
Rejected alternative: keep `Decided` and teach canon a fourth value — makes the split
permanent and erases the ratified/provisional distinction at the one boundary (spend
gating) that needs it.
**Ledger:** one DL (machine): unification + 1.5.0 wire + schema-v4 upcast + byte-compat
ruling; amends DL-0068(-née-0067)'s fail-closed detail and DL-0058's cited→Decided
lowering. Plus the killed-form registry entry for `decided` naming successors.
**Verify:** gate green; multi-version author tests green on every accepted version; the
path-E red-team replayed with a garbage marker → ok=false; NEW e2e test: author via
1.5.0 `cited{state:"provisional"}` → canon export ok=true, row status `provisional` —
**that test IS the FW-8 un-wall.**

### Fix 2 — Standing: engine models support; promote/defeat are agent data operations
(deloreis, same 1.5.0 PR)

**The fault:** valuation collapses `Decided`/`Ratified`/`Provisional` into one outcome
(`crates/dli-world/src/lib.rs:255-280`, `crates/dli-eval/src/world.rs:565-600`,
`lower.rs:812-830` — which also re-emits everything as `Ratified` on the way out), while
fantasy's D-0230 makes ratified-vs-provisional load-bearing (expensive work must not
proceed on provisional material). Today `worlds/tpth/generated/timeline.dli` carries 26
rows marked `provisional D-0072` that all read `certain`, because
`compile_lore_dli.py:85-89` emits `ratify D-nnnn` for EVERY cited decision. Standing also
rides the `ratify __decl` objects while the export serves `Commit.decisions` — two
sources, one recorded silent mutation-test no-op.

**Fix (matches the user ruling — draft canon promoted or defeated by agents, nothing
hangs):** keep the three-valued derived `Status` (`Dead|Possible|Certain`,
`crates/dli-core/src/ann.rs:77-93`) as the whole truth algebra. Ratified-vs-provisional
is standing METADATA (the stored marker) — both are Certain, which is exactly what D-0230
needs (provisional material is assertable, just spend-gated). Two new 1.5.0 tools:
- **`author.set_standing`** (promote): rewrites the marker provisional→ratified citing
  the ratifying decision (existing `citation.unknown-decision` integrity path). Marker is
  outside the identity hash → new commit, same claim identity, no re-evaluation, never
  blocks.
- **`author.defeat`**: records the defeating/superseding decision and rules the defeated
  decision's variable FALSE via the existing Ref valuation overlay
  (`store.rs:161-180`; `world.rs:547-551` documents "revocable later by revaluing the
  variable false"). Claims resting on it drop to `Dead`; the objects stay stored —
  promoted-or-defeated draft canon STAYS in the store. Never blocks.
Consequences: single-source the decisions array (derive `Commit.decisions` from the decl
objects at fold time, or minimally canon_export asserts set-equality fail-closed);
`derive_annotation` uses `FactProvenance::marker` when present (v3 added it for exactly
this, `lib.rs:63-74`) with the Ratified fallback only for marker-less v2-lineage;
`ratify` keyword → `decision` via killed-form registry (SEVERABLE — may defer to 2.0.0;
the single-sourcing and round-trip fixes are NOT severable); D-0230's spend gate becomes
a consumer-side preflight where large-spend jobs launch ("all transitive provisional
inputs ratified", failing loudly by name). The 26 provisional-reads-certain rows are
CORRECT under this model — the defect was the distinction's invisibility, and no data
migration is needed.
Rejected alternative: a real third/fourth valuation state — `Status` is derived, never
stored; provisional-ness is caller-stated and underivable, so it would reintroduce the
stored-status slot the schema-first law forbids, fork every Status consumer, and make
provisional material non-assertable (violating both D-0230 and the no-hangs ruling).
Defeated needs no new state: `Dead` already expresses it.
**Ledger:** one DL (machine): standing model + both tools + single-sourcing; confirms
DL-0054/DL-0058. **Verify:** marker round-trip unit test (provisional survives
lower→store→derive_annotation); the once-silent decisions-array mutation now reds
(honest miss = xfail-strict per DL-0006); defeat e2e (defeat → query reads dead → canon
reflects); gate green.

### Fix 3 — Display names into the store; supersede the "human-ratified" criterion
(deloreis + fantasy, after Fix 1)

**The fault:** DL-0068(-née-0067) keeps display-name surfaces YAML-sourced because
"human-ratified" — but its own text shows the real criterion is by_pred-insufficiency
(the store carries only slug atoms), and the killed-token registry is equally
human-ratified yet store-projected (DL-0065-née-0064). "Human-ratified" as a storage
criterion is the old sign-off concept re-imported as an engineering category; left
standing, it teaches future agents that ratification status determines storage location.

**Fix:** supersede the criterion: **a surface lives in canon iff the store carries it
(by_pred-sufficiency).** Fire the entry's own revisit trigger: author the 6 core-character
display-name facts (from `characters.yaml`, cited to the D-0132/D-0134 lineage) THROUGH
THE TOOL SURFACE after Fix 1, using the existing predicate pattern
(`fact the-old-balance display-name "..."`, timeline.dli:14). `canon_characters` projects
`name` from `by_pred["display-name"]`; fantasy `compile_characters` drops the YAML
re-join; `characters.yaml` remains the offline fallback and must stay byte-equal until
step 7 deletes it. Scope guard: the Path-D rejection (factcheck-alias re-point,
byte-neutral) STANDS.
**Verify:** output bytes IDENTICAL before/after (same names, different source);
`compile_lore_dli.py --check` green in both lanes; `git diff worlds/tpth/generated/`
empty; gate green. **Ledger:** one DL (machine) superseding the criterion language, in
the DL-0058-amends-DL-0054 pattern.

### Fix 4 — Fantasy governance: finish the scrub D-0247 started (fantasy, own PR)

Ground truth: main's D-0247 already rules machine minting with attribution AND explicitly
licenses the validator change as "a permitted follow-up". Yet **zero machine-attributed
D-entries have ever been minted** (every remaining lane requires one; FW-6 landed with
none), the workaround marker string was never used, and the only programmatic ledger
writer can't emit `attribution:`. The minimal V4 slice rode PR #21 (step 4.5). The full
pass:

1. **Loader + V4** (`src/fantasy/canon/loader.py`, `validator.py:53,220-246`): parse
   `attribution`/`rationale`/`evidence`/`ratified-by`. Required: `status`,`date` always;
   `quote` iff attribution absent-or-human; `rationale`+`evidence` iff machine; unknown
   attribution value = violation; entries after D-0247 must carry explicit attribution
   (absent-reads-human becomes a bounded historical rule). Retire the unused mechanical
   quote-marker string from the preamble.
2. **New V13 — no self-ratification:** `attribution: machine` + `status: ratified` is a
   violation unless `ratified-by:` points at the author's ruling record (workboard
   submission digest/date). Machine entries are born provisional — matching what
   AGENTS.md:257-263 already claims as policy but nothing enforces.
3. **V11 tightening — no self-authorization:** a ratified section must be covered (via
   `affects`) by ≥1 HUMAN-attributed cited decision (pre-D-0247 entries count as human);
   machine-attributed decisions authorize up to provisional only. This breaks the
   forged-promotion loop (agent writes entry + affects + flips the section) at the only
   level that matters, while the provisional flow stays fully machine-performable —
   nothing hangs.
4. **`author_rulings.ex:386-395`:** `ledger_entry` emits `- attribution: human` (correct
   — it records the author's own workboard submissions); add
   `machine_ledger_entry(id, standing, rationale, evidence, source)` with
   `status: provisional` HARDCODED so V13 is structural, not reviewed-for.
5. **Doc scrub:** `canon/SCHEMA.md:18-24` statuses → D-0247's async form; `:85` quote key
   → conditional-on-attribution + document the new keys; `:106-113` add the V11
   human-covering rule; `:168` "Promotion … is Jeff's act" → "Ratification is the
   author's act, recorded and applied asynchronously (D-0247); provisional entry is the
   machine path (D-0230)". `AGENTS.md:243-244` "only the author can mark anything
   ratified" → "`status: ratified` is written only when recording the author's own
   ruling — never for a machine's own decision (D-0247)".
6. **Collision defense** (the D-0247/48/49 fork resolved in step 5): mint the discipline
   as a D-entry — D-ids mint against origin/main's tail, fetch before mint; blind branch
   mints renumber-at-merge with an audit note; NEW CI check in the validator lane: any
   branch D-id also present on origin/main must match parsed title+quote or go red
   (`git show origin/main:canon/decisions.md` — local git objects, no network; skipped
   when the ref is absent so offline lanes stay deterministic). Rejected alternative:
   per-entry files (structurally correct — git would surface collisions as filename
   conflicts — but the churn is too large now; named escalation path if collisions recur).
7. The **first machine-minted D-entry** should be done deliberately as part of this PR's
   verification — it exercises V4-conditional, V13, and the machine writer path in one
   motion.
**Verify:** `uv run fantasy validate` green on the merged ledger; unit tests: machine
entry missing rationale reds; machine+ratified sans ratified-by reds; ratified section
covered only by a machine decision reds; the collision check reds on an injected
mismatched D-0247; grep for the old branch ids returns only renumber notes.

### Fix 5 — Context-poison scrub: one batch deloreis docs PR + a retired-vocabulary lint

The deloreis governance layer is scrubbed (DL-0052/0054/0058, SKILL §0, AGENTS §2); the
spec/cards layer still teaches the retired law. One docs-only PR + one DL (machine,
cites DL-0054/DL-0058 as the authority the docs must match):

| # | file:line | stale claim → replacement |
|---|---|---|
| 1 | `docs/spec/11-agent-authoring.md:209-214` | "cited … is human-only (B1) … refused" (contradicted by its own §5 at :548-565) → versioned truth: ≥1.3.0 records caller-supplied decision state; <1.3.0 keeps the refusal permanently. Delete "The agent proposes; the human confirms the citation in review." |
| 2 | `docs/cards/gentle-card.md:104-107` | "decided is a human's act … refused" → "a citation must resolve to a declared decision; unknown raises `citation.unknown-decision` and bars materialize." |
| 3 | `docs/cards/expert-card.md:257-258` | "nothing is true until the author rules it so" → "nothing is true until a declared decision is cited; rulings live in the consuming ledger (koan 1, koan 3)." |
| 4 | `docs/spec/07-verdicts-and-llm-boundary.md:95` | "always — sign-off *is* ratification" → "async, lagging (DL-0052); ratification is a consumer-ledger act recorded by citation (DL-0054)." |
| 5 | `docs/spec/04-practices.md:195-196` | "only when the author rules it" → "when a ruling lands in the consuming ledger." |
| 6 | `docs/spec/05-language-layer.md:195` | "awaiting the author's ruling" → "awaiting a ruling in the consuming ledger." |
| 7 | `SKILL.md:219` | "only a human ruling makes it certain" → "only a resolvable decision citation makes it certain." |
| 8 | `crates/dli-serve/src/author/render.rs:1-30` | "Door H author", "the human's only supervision surface" → "hand-written `.dli` author", "the supervision surface" (comment-only; the forgery-proofing itself is load-bearing and stays). |
| 9 | `docs/integration/POST-SECTION5-FRONTIER.md:94` (+ the resumption guide, which this file retires) | drop red-team dimension "propose-only-enforced" — it tests a property DL-0054 deliberately removed (guaranteed false SEV-B) → "citation-integrity-enforced" (unknown decision blocks materialize; a forged id never reads certain). |
| 10 | two finding codes stay DISTINCT | `provenance.cited-refused` (<1.3.0, permanently served byte-exact; permission-era refusal, error-shaped) vs `citation.unknown-decision` (≥1.3.0; resolvability failure, finding-shaped). Add a two-row decision table to spec/11 §5. Fantasy hygiene: `dli_bridge_test.exs:157-163` asserts a top-level error while `episode_script_gate_bridge_test.exs:94` asserts a findings entry — both real, but each test must pin WHICH surface and negotiated version. |

**Retired-vocabulary lint** (confirmed feasible): sibling module to
`xtask/src/docs_tokens.rs` (the existing gate-wired forbidden-token walk), scanning
`docs/**`, `docs/cards/**`, `SKILL.md` for phrase-exact, case-insensitive:
`cited-refused`, `human-only`, `sign-off is ratification`, `only a human ruling`,
`Door H`. Allowlist (append-only history legitimately describes retired behavior):
`docs/decisions.md`, `docs/design-notes/`, `docs/integration/migration.md`, and the
spec/11 versioned-behavior table. Wire into `gate.rs` beside docs_tokens; prove live with
red-then-green in the lint's own unit test.
**Verify:** docs-oracle + authority + new lint + full gate green; a fresh-context agent
given only spec/11, the cards, and SKILL.md can no longer learn the retired model.

---

## §5 Process diet (user rulings, 2026-08-10 — binding)

**Ruled by the user, verbatim intent:**
1. *Rigor:* "it needs to do *some* checking of its work. presuming non trivial pr sizes,
   i'd want a clean context sub agent to do a red team, but the accrual shit and byte
   exact parity proof are absurd. so, trim to a reasonably safe procedure."
2. *Proposed vs ratified:* "we do want to have the ability to have draft canon that gets
   promoted or defeated. we just cant have the bot hang on that shit. agents will be
   doing the promotion and such."

**Encoded:**
- KEEP: commit-before-red-team (a reviewer's `git restore` once destroyed uncommitted
  work unrecoverably); ONE clean-context red-team sub-agent over the committed diff for
  non-trivial PRs; seeded-divergence non-vacuity for corruption-detecting harnesses
  (these caught three SEV-A silent corruptions, a null-oracle battery, and five false
  universal claims in the ledger — they earn their cost).
- CUT: the 20-genuinely-elapsed-cycle §5 accruals (structurally unfinishable anyway —
  every store change re-roots the hash and voids the pinned proof set, and FW-8 exists
  to change the store; the record already claims it "done" in three documents and
  "pending" in a fourth); byte-exact parity as an acceptance criterion (it is the
  mechanism that blocked authority transfer); multi-round loop-until-dry red-teams
  (take the written descope instead); the DL-mint serialization ritual beyond "one
  DL-minting PR at a time, fetch-first" (it already deadlocked against a dead lane and
  forced retrospective mints).
- CONSOLIDATE the narrative surfaces: the two ledgers + ONE board per side
  (deloreis `docs/integration/STATUS.md`, fantasy mirror) are the record;
  `cutover-progress.md` and `POST-SECTION5-FRONTIER.md` get a final "superseded by
  STEERING-REVIEW-cutover.md" note; `RESUMPTION-GUIDE-cutover.md` is deleted at step 8.
- FIX THE ENVIRONMENT STRUCTURALLY (each of these burned at least one full debug cycle
  and is currently managed by agent memory instead of code): commit a `.cargo/config.toml`
  or gate preflight enforcing a short target path (the 107-char socket limit); make
  `docgen` HARD-FAIL when the syntax feature is absent (today it silently guts cli.md and
  exits 0); per-invocation greengate venvs (the shared one was re-pointed mid-run twice);
  per-lane `CARGO_TARGET_DIR` (a shared rebuild clobbered a pinned proof binary); the
  `authority` merge-base + fetch-depth fix (§2.1); decide once whether the fantasy
  `:database` lane gets a CI Postgres service.

---

## §6 Descoped / deferred (do NOT resume these without new evidence)

- **DM-6 / contract 2.0.0 / 0.x retirement + killed-version ceremony — deferred
  indefinitely.** Retiring accepted versions no caller sends buys nothing toward the
  cutover. Neither `ls8_one_x_parity` nor `KILLED_VERSIONS` exists anywhere in the tree —
  Stage 7 is 0% started; leave it that way. Revisit trigger: a real cost of keeping
  dual-mode.
- **FW-9 (shadow validator flip) — deferred.** `fantasy validate` gates today; its
  store-facing input re-points via the step-6 snapshot anyway; its accrual apparatus is
  cut by ruling. Revisit after step 7 is stable.
- **FW-7a–d data expansion** (lexicon/banned, episode continuity, canon-md facts,
  Q-ledger) — out of the "done" definition; additive migrations that resume on the
  settled base afterward.
- **EXT-F1/F2/F5 vendored snapshot re-pins** — one batch re-vendor after step 5 (they
  are already flagged void; incremental re-pins against a moving tree just re-void them).
- **Worktree/branch hygiene** — a deliberate pass AFTER the mains unfreeze; most stale
  worktrees hold unmerged commits (LS-7/LS-8/DM-4/native-validator lanes), so pruning
  discards work if done blindly.
- **fantasy PR #6** — merge or close, either is fine; just don't leave it ambiguous.

---

## §7 Why this happened (so the next design doesn't repeat it) — condensed

- The engine surfaces the migration needed most (corpus import/export, standing/interner)
  were the newest and least-exercised; the consumer found the bugs (import lowering walk,
  verbatim-accepted claim_ids, unreachable `init`, one-scope-per-store) by building
  against them. A mandatory engine-constraint read before approving a consumer-side model
  would have saved at least one full lane cycle (the store-scope model was decided three
  times in one day; the deciding constraint was two lines of handlers.rs).
- Proof-by-pinned-hash creates a re-verification treadmill: every proof pinned to a root
  hash dies on every store change. Pin proofs to CONTRACTS and PROJECTIONS (shape,
  integrity chain), not to content hashes, wherever the thing proven is a mechanism
  rather than a snapshot.
- Single-file append-only ledgers fork their id space on every long-lived branch (two
  independent D-0247 collisions and one DL-0062 collision in one week). Fetch-first
  minting + the cross-ref CI check (Fix 4.6) is the cheap defense; per-entry files the
  escalation.
- Five single-writer narrative surfaces drifted apart and then agreed with each other's
  staleness ("both halves of a handshake designed to catch drift drifted together").
  One board per side, updated in the same PR as the work it records.
- Heavy process was applied uniformly instead of by blast radius — the user has now ruled
  the calibration (§5). Note the asymmetry the record shows: the riskiest work of the
  campaign (store re-seeds, ledger mints) happened OUTSIDE the ritual (on unmerged
  branches, unreviewed), while docs PRs got full ceremony. Blast radius, not habit,
  chooses the check.

---

## §8 Verification checklist for the finish line

- [ ] deloreis main: exactly one `## DL-0062` (epistemic infrastructure); `authority`
      hardened (merge-base baseline, hard-fail on unreadable, fetch-depth 0) with a
      red-then-green test.
- [ ] `contract/1.4.0` tagged; fantasy pins it; PR #21 merged; fantasy main unfrozen.
- [ ] Dual-serve log at the CURRENT store head committed (one run + one seeded red).
- [ ] Product branch merged; exactly one `## D-0247` in `canon/decisions.md`.
- [ ] parity.json carries a `{tag, store_head, digest}` provenance header; required CI
      check enforces the chain; seeded canary red/green both directions.
- [ ] Four flipped domains' YAML deleted; boundaries tripwire re-aimed; full suites green.
- [ ] `contract/1.5.0`: `Decided` retired via killed-form registry; e2e
      author-cited-provisional → canon-export-ok test green (the FW-8 un-wall);
      `author.set_standing` + `author.defeat` shipped; decisions array single-sourced.
- [ ] Display names store-projected; DL-0067-successor criterion is by_pred-sufficiency.
- [ ] Fantasy V4/V11/V13 + machine_ledger_entry landed; ≥1 machine D-entry exists and
      validates; collision CI check red-tested.
- [ ] Poison scrub PR merged; retired-vocabulary lint in the gate, red-then-green proven.
- [ ] One canon change authored ONLY through `author.*` MCP reaches a shipped factory
      read with no YAML in the path.
- [ ] `RESUMPTION-GUIDE-cutover.md` deleted; both STATUS boards agree and match reality.
