<!-- status: draft; verdict: §5 closed (scratch author.* ruling + accrual re-verified green at 1.2.0). Next gate: contract-pin lane re-pin @1.3.0 (LS-7 cutover). GitHub Actions repo-wide outage blocks deloreis PR #39 *display* only; local gate is authoritative (DL-0015). -->

# Post-§5 frontier plan

Handoff to the next orchestrating sub-agent. Facts here are **verified** (see
§0); forward steps are **hypotheses** gated on the accrual battery (marked HYP).
Red-team notes live in §4. This doc is pure markdown (no `.dli`/`json` fences)
so `cargo xtask docs-oracle` is trivially green.

## 0. State as of this handoff (verified)

- **§5 CLOSED.** `author.*` MCP on a scratch store (`/tmp/s5`, throwaway copy of
  `worlds/tpth`, `store="lorestore"`): real ruling
  `author.open → add_entity{…s5witness…} → materialize`. Store root
  `db80a5a3668d3f00adbeb421519555b358b6ef1190122bb704c233e1433272e1`
  → `ba63c029eb6e0b86899c0fa0e86933cce9090df1ada517166fd4b400a06233fa` (shift);
  `dli.check` stays `grade=checked, findings=[]` (parity); `dli.query` AGREE ×20
  (12 rows, 1 frame). Committed canon (`worlds/tpth`) left byte-identical.
- **§5 accrual re-verified @ dli 1.2.0 (GREEN).** `lore_dual_run.sh` on the dm4fix
  binary (`contract/1.3.0` engine): pack `69dbc1ea034580b922fff70dccf886d171fb716daaac8a6c7a26c9dd6668c3cd`
  **reproduces** from the `.dli` corpus; store root `db80a5a3…` **reproduces**;
  dual-serve `217 predicate(s) answered identically, 6 refused, 0 divergence(s)`;
  **all 5 seeded divergences RED for their own reason** (drop-fact, invent-fact,
  drop-family, drop-decisions, drop-dialect); rollback `byte-identical,
  grade=checked`; verdict `lore-dual-run: PASSED`. `ACCRUAL_VERIFY_EXIT=0`.
  Evidence: fantasy `fw6-flip` @ `56c5c574` (§5-log `d069cde0` + ruff-E501
  `9764f343` + format `bec1c3fb` + frontmatter/INDEX `56c5c574`); deloreis
  `e137bb4` (STATUS §5-closure) + `eafab31` (CI re-trigger).
- **deloreis `origin/main` = `6a626e1`** (DL-0060+DL-0061 merged). Local
  `CI=1 cargo xtask gate --ci` = `21/21 green` (CI parity, DL-0015).
  **PR #39** (docs-only: §5 STATUS note `e137bb4` + this frontier plan) OPEN. **GitHub Actions is
  repo-wide OUTAGED** (pull_request events dropped/cancelled 18:56 onward; 3 polls
  0 runs on `eafab31`; prior 18:56 rerun cancelled by queue churn). `main`
  branch-protection requires `gate`/`docs-oracle`/`deny`/`no-ai-authorship`
  (all satisfied locally; display blocked by the outage). Merge is infra-gated,
  **not** correctness-gated.
- **fw6-flip lane** committed `56c5c574`. Shadow-gate green at 1.2.0 (§5 accrual +
  `shadow_lore.py --repeat 20` AGREE). Full greengate RED **only** on the
  PRE-EXISTING `test_contract_pin`:
  `DLI_CORPUS_CONTRACT_VERSION = "1.2.0"` vs `DELOREIS_TAG = contract/1.3.0`.
  Bumping the constant to 1.3.0 **BREAKS the accrual** (217→216 identical,
  `lore-dual-run: FAIL` — the `.dli` is 1.2.0-calibrated; the 1.3.0-negotiated
  engine serving diverges at 1 predicate). So this is **not** a constant flip;
  it is a lane re-pin (see §1).
- **Phase-1 residual red-team (done this handoff).** `shadow_factcheck.py`
  exit-code parity = **HONEST** (`subprocess.run(check=False)` + `raise` on
  non-zero + `return 1` on findings; no `|| true`, no `2>/dev/null`, stderr
  surfaced in `RuntimeError`). `PROMPT_DIGEST = sha256:ccb05b65…` = **UNVERIFIABLE**
  (sha256 mismatches every candidate: `factcheck/__init__.py`=7fa4b317…,
  `production_gates.py`=d989dd8c…, `shadow_factcheck.py`=c6525c9d…,
  `_factcheck_gate` src=196e5244…; referenced nowhere as a derivation; the
  named extractor `_factcheck_gate` is deterministic Python with **no prompt
  string**). RESOLVED: frozen verbatim with an honest-provenance comment;
  `shadow_factcheck.py` committed at `fw6-flip` `d870fb22`; derive-vs-freeze is a lane call.

## 1. Frontier (gate/ranked)

**P0 — Lane re-pin @ dli 1.3.0 (UNBLOCKS the flip PR).** This is the LS-7
lorestore serving-flip cutover (RESUMPTION-GUIDE §4 *§5 ACCRUAL / FLIP PR*).
The contract-pin fails *because* the engine advanced to 1.3.0 while the
corpus proof artifacts are 1.2.0-calibrated; re-pinning re-derives them at 1.3.0.

 1. `src/fantasy/lore/contract.py`: `DLI_CORPUS_CONTRACT_VERSION = "1.3.0"`
    (comment stays; matches `DELOREIS_TAG contract/1.3.0`).
 2. `compile_lore_pack.py` @ 1.3.0 (via `lore_dual_run.sh`): re-lower the `.dli`
    corpus with the 1.3.0 engine → NEW pack digest (≠ `69dbc1ea`).
 3. `lore_store_seed` @ 1.3.0: new store root (≠ `db80a5a3`).
 4. `lore_dual_run.sh` @ 1.3.0 **(HYP):** dual-serve `PASSED` — ≥217 identical,
    5 seeded divergences RED for their own reason (re-verify the ratify-decl
    no-op trap on `drop-decisions`), rollback `byte-identical, grade=checked`.
    Mitigation if the 1-predicate divergence persists/worsens: bisect the
    predicate, engage dm4fix engine. (Testable locally — `lore_dual_run.sh`
    uses the dm4fix binary directly, independent of the GH outage.)
 5. `shadow_lore.py --repeat 20` @ 1.3.0: AGREE at the new pack/store digests.
 6. `dli.check` @ 1.3.0: `grade=checked, findings=[]`.
 7. Commit the re-pin (new pack/store digests + constant 1.3.0) → fw6-flip
    full-greengate GREEN. RT the battery reds per RESUMPTION-GUIDE §3.

**P0 — FW-6 flip PR (after re-pin).** Flip serving to the lorestore manifest;
re-point `parity.json` producer to `dli.corpus.export`; **real** rollback drill
(not a rehearsal); re-aim the boundaries tripwire clause-by-clause. Cite the
stage-A machine `D-nnnn`. Note `before/2` is MOOT (engine-lowered store dissolves
the 4 legacy-only rows — verify in the flip, §3 KEY CATCH).

**P1 — §5 ACCRUAL (20 elapsed cycles; RESUMPTION-GUIDE §4).** 20 genuinely-elapsed
dual-run cycles, append-only log (UTC + pack-digest + binary versions + store-head
+ verdict), ≥1 ratified canon change ~run 10 (pack digest visibly shifts),
seeded-divergence battery red-on-demand at BOTH ends, counter→0 on any red.
Mint the stage-A `D-nnnn`. (Can run locally now; not blocked by the GH outage.)

**P2 — FW-8 MCP authoring integration (after flip PR).** MCP config + `SKILL.md`
adaptation + structured-diff review renderer + store-integrity CI. Authors into
the collapsed single-`tpth` store (scope-collapse approved). RT:
supervision-bypass, citation-integrity-enforced; authoring is a **stateful session** —
`author.open`'s workspace lives in the serving process; hold it open across all
authoring frames (one-shot bridge fails with `unknown-doc`). DL-0061 hardens
its review surface.

**P3 — Phases 3–4 (blocked on flip + EXT-F4).** DM-5 SP-4v → FW-9 (`shadow_validate`
lands + accrues) flip; Stage 6 V8/FW-8/FW-7a→d. Strictly **after** Stage 7/DM-6.

## 2. Gating reality (this handoff)

- **GitHub Actions outage** is the *only* deloreis blocker. The 4 required
  checks (`gate`/`docs-oracle`/`deny`/`no-ai-authorship`) are GREEN locally
  (local gate 21/21; commits carry no AI trailers → hook passes). Do **not**
  block deloreis on the GH display. When GH recovers: `gh pr merge 39 --auto
  --squash` (or owner `git push` if protection allows). Until then PR #39
  remains OPEN on `origin/integration-status` (evidence landed).
- **fw6-flip uses `scripts/greengate.sh` (Python, GH-Actions-independent)** — the
  re-pin + flip PR are NOT blocked by the outage.
- **Stage 7 (DM-6, contract 2.0.0) is strictly last** (HARD order): needs
  EXT-F4 verified + fantasy canary ZERO 0.x. Out of scope until P0–P2 clear.

## 3. Non-negotiables carried forward (AGENTS.md / DL)

- No `git add -A`/`.`/`-a`; stage explicit filenames; leave staging empty.
- `Jeff Smith <jeffreyksmithjr@gmail.com>`; no AI trailers (githook-enforced).
- Offline+deterministic gating only (kernel calls no model, reads no clock).
- Schema-first; schemars JSON Schema = the language definition; canonical form
  = JCS byte serialization of stored fact objects (.dli is opt-in rendering).
- Killed forms registered, never deleted.
- `author.*` MCP tools are the ONE interface for authoring lore (DL-0014/DL-0043);
  never hand-edit `.dli` canon. The §5 proof respected this (scratch store only).

## 4. Red-team of THIS plan

- **"1.3.0 re-lower dissolves the 1-predicate divergence"** — HYPOTHESIS,
  not yet executed. The divergence is `.dli(1.2.0)`-calibrated vs
  `store(1.3.0)`-negotiated; re-lowering the `.dli` at 1.3.0 is the canonical
  fix. Mitigation: if it persists, bisect the predicate + engage dm4fix.
  Independently testable (dm4fix binary path is GH-outage-independent).
- **"PROMPT_DIGEST unverifiable"** — VERIFIED (sha256 mismatches + no derivation
  site; extractor is prompt-less). Recommend: derive from the real artifact or
  freeze as an explicitly-unverified provenance constant.
- **"local gate authoritative"** — DL-0015 + AGENTS.md §5 (offline deterministic
  gating; CI parity as a compiled Rust test).
- **"no-ai-authorship hook"** — `.githooks/`; my commits carry no AI trailers.
