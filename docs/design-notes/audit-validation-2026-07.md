---
status: draft
verdict: "Validation of the 12-commit audit remediation (268fe41..4d1bceb): gate/CI green and roughly half the audit genuinely fixed — but the negation 'alignment' reverted the b8243cc/13605cd red-team soundness fix (with test assertions inverted under kept names), the canonicality sweep silently deleted `assumes` clauses from three committed worlds, plan Tasks 15/17 never ran, the promised DL entries were not minted, and 13 new issues were introduced. Evidence, not authority — mints no DL ids."
---

# Audit-remediation validation — 2026-07

> Commit note: like the audit doc, this file names TPtH tokens; committing it
> requires an `ALLOWED_FILES` entry in `xtask/src/docs_tokens.rs`.

## 1. Scope and ground truth

Validates the 12 `audit-remediation:` commits (`268fe41..4d1bceb`) against
`docs/design-notes/audit-2026-07.md` and the committed
`audit-remediation-plan.md`. Method: 7 adversarial validators re-checking every
audit anchor plus diff-level review of every changed test. Ground truth: CI
green at `4d1bceb`; local `cargo xtask gate` **PASSED — 17 steps green**
(receipt `out/greengate/audit-validate`); LOC now 28,681 (soft cap still
exceeded, hard cap honored).

Scorecard over 104 validated obligations: **25 FIXED · 23 PARTIAL · 32
UNADDRESSED · 13 NEW-ISSUE · 3 REGRESSED · 8 deferred by plan.**

## 2. The three headline problems

### 2.1 REGRESSION: the negation "alignment" reverted a red-team soundness fix

Commit 74c3e74 executed plan Task 6 literally ("implement certain-layer-only
negation with **open-variable poisoning**") — which is the pre-red-team
semantics that b8243cc/13605cd deliberately replaced. Code, spec 02 §4, and
DL-0020 now agree verbatim, but at the documented cost:

- `negative_poison` (world.rs:916-968) blocks robust-certain `b` entirely and
  filters poison to `!monomial.is_certain_under(valuation)` — open-variable
  only.
- **The key red-team test was kept by name and inverted**:
  `edge_poison_valuation_neutral_reblocks_under_fork` (raverify.rs:928) now
  asserts `h` stays **Certain** when a base-Dead negated premise is re-supposed
  true in a fork (raverify.rs:986-991) — the exact forged-certainty defect the
  red-team fixed. A fresh fixpoint under the same valuation yields `h` Dead
  (:978-982); the fork path still evaluates by valuation overlay with no
  re-fixpoint (fork.rs:7, :734-739), so the forge is reachable through
  `fork.suppose`/`diff`/`preflight`.
- `f2_reversible_block_matches_fresh_fixpoint` (raverify.rs:290-306) now
  **pins the divergence its name says it refutes** (eval Possible vs fresh
  fixpoint Dead); spec 02 §1's "evaluation commutes with specialization"
  (:90-95) is now false across negation and was not amended.
- All why_not blame assertions for blocked conclusions were deleted
  (raverify f6 :671-674, redteam_negation :170-173): a blocked conclusion now
  leaves no row and no explanation of why — the "block leaves a trail" teeth
  are gone, and `f6_why_not_defeated_names_blocker` no longer names a blocker.
- The raverify.rs:1-5 module header still claims the file asserts "the fixed
  behavior" of b8243cc — now actively misleading provenance.
- **No DL entry records the reversal** (plan Task 6 promised one; only
  DL-0023 was minted). TQ-N1's core sentence — "the ledger has no answer" —
  survives the fix.
- Invisible to the wire only because zero golden fixtures exercise negation
  (pre-existing coverage gap, now load-bearing).

Intact: f1a/f1b (per-monomial conditioning, the other half of the red-team
fix) survive unchanged. TQ-N2 got a real code branch (certain-at-risk poisons
instead of blocks, world.rs:923-950) but `f2_chain` is byte-identical — `h`
still unpinned, and its in-test comment (raverify.rs:353-354) now states the
wrong value under the new semantics.

**Direction:** either re-mint the valuation-neutral semantics (restore
b8243cc/13605cd, fix spec §4 wording, mint the DL entry the audit asked for)
or, if blocking semantics are genuinely ruled in, mint the DL entry recording
the trade (fork-commutation and blocked-conclusion explanations knowingly
surrendered), amend spec §1, restore a why-blocked explanation channel, and
rename the inverted tests. Shipping the current state silently is the worst of
both.

### 2.2 NEW DATA LOSS: canonicalization deleted committed author text

The F3T-3 sweep reformatted all 10 committed `.dli` files. 7 verify as
layout-only, but three `assumes <hypothesis>` clauses on fork options were
**silently deleted** — `examples/camelot.dli` (baseline :249 `option the-lake
… assumes h-return`), `examples/norse/nine-realms.dli`, and
`worlds/tpth/packs/saltmarket.dli` (baseline :84 `assumes h-ledger`) — because
`parse_option_clause` discards its diagnostics into a `noop` sink
(parser.rs:1221-1222). This is a live silent-drop channel of exactly the
F3T-1 class, still open at HEAD, and the remediation's own sweep materialized
the loss into committed worlds. Unacknowledged in any commit message. Also
by-consequence: comment stripping removed all pedagogical commentary from the
teaching worlds (the formatter's canonical form carries no comments — a
design consequence worth an explicit author decision).

**Direction:** restore the three clauses from git history, route
`parse_option_clause` diagnostics like the fixed expert arms, and decide
whether canonical-form-strips-comments is acceptable for teaching worlds.

### 2.3 The grade fix is real but has a new anti-conservative hole

43d847c added a genuine second evaluation (`World::var_grades` +
`grade_under`: join over ν-true monomials, wired into query rows world.rs:583
and fork standings fork.rs:737; the audit's two-monomial laundering
counterexample is fixed and pinned by `grade_is_per_valuation`,
world.rs:1293-1352). But:

- **New over-claim direction**: `grade_under` recovers grades only from
  fact-support variables; rule grades enter only the pooled fallback, and
  unknown variables default to `Grade::Checked` (world.rs:388) — a derivation
  through an Asserted rule from a Checked fact can now report **Checked**
  where the old pooled code said Asserted. The baseline never over-claimed;
  this contradicts the doc-comment "grade-laundering is impossible"
  (world.rs:371-372).
- The absorption variant still launders: TRUE absorbs a Checked monomial and
  the empty-monomial branch returns the pooled fallback (= Checked) under
  every ν — the audit's own decoupling test (now world.rs:1243-1291) still
  pins it.
- No `VarKind` metadata exists (plan Task 5 promised it); spec 02 §2's
  kinds-based meet remains unimplemented and the spec unamended.
- The new suppose-fork grade-changed-diff capability has zero tests.

## 3. Genuinely fixed (credit where due)

- Doc-resync: 8 of 9 §3.1 items verified against code (PLAN current-state,
  contract gated-ops paragraph, AGENTS.md oracle description, migration
  pack→scope + a real `name.unvetted` finding replacing `merely-possible`,
  both Rust doc comments, Cargo.toml lint re-arm (todo/unimplemented → warn,
  safe: zero occurrences), README, spec-accuracy staleness banner).
- DL-0023 (read tools) minted append-only; contract §3.2 + spec 11 §3.10 rows
  added; process hygiene clean (12 plain commit messages, ledger pure append).
- F3T-3: `fmt --check examples/ worlds/` gate step (now 17 steps); F3T-4:
  recursive walk in all three sites — norse packs now genuinely gate-checked;
  F3T-6: tautology test replaced with a real parser-behavior oracle.
- F3T-1 (mostly): labels/provenance got AST homes with round-trip tests;
  five naked arms got `expect_consumed`; schema regenerated consistently.
  Remaining: the `_head` token sink (practice/dialect/draft/export/term/
  variant) still discards header junk diagnostic-free — `practice p "The
  Rite" junk {…}` still fmt-deletes `junk`.
- F5S-2: craft family documented (13 emitted codes verified against all 19
  emit sites, severities hand-checked accurate) + scraper boundary fix;
  F5S-6: name family strict; F4T-1: guard re-anchored to the real
  `check_payload` seam with named codes (two of three operators realized via
  a proxy rule — honest-ish, see §4).
- Token superset: 26 tokens now including the load.rs extras and canon names
  (the `tegwen` fooling edit is closed); `the-*` spaced matching added;
  cfg(test) doc-comment poisoning closed; "fully TPtH-free" claim softened.
- F8-2 CLI leg: `check-draft --killed` real, two-sided CLI test proves a
  killed-token revival exits 1.
- Craft bridge: `build_episode_policy` genuinely converts TOML config with a
  field-by-field fidelity test against `worlds/tpth/craft`.

## 4. Stragglers (in plan scope, not deferred, not done)

1. **Plan Task 15 — all 10 proof-gap MINORs: zero commits.** TQ-N3 (negation
   property generator — claimed by Task 6 too; props.rs untouched, and more
   important now that commutation is knowingly broken), TQ-N4, TESTQ-CORE-2/3/4,
   F4T-2/3/4, F5S-5. F3T-5 gets incidental partial credit (corpus tests now
   execute the braced arm; generator untouched).
2. **Task 16 silently dropped three of its own items**: F3T-7 (probe.rs grew
   +4 lines instead of dying — new investment against its own "do not build
   on it" banner), F7B-4 (TPtH doc-comments persist in decl.rs and the
   regenerated schema; dli-worker op schemas still carry mara/enfys), F7B-5
   (separation-plan contradictions untouched).
3. **Task 17 / all four red-team checkpoints: zero evidence any ran.** The
   audit doc was never stamped with the remediation SHA.
4. **Promised DL entries not minted** (Task 6 negation, Task 10 bridge) —
   spec 02 §4 worked example and DL-0020 wording still don't record the
   shipped decision; DEFAULT_FINDING_POLICY's D-nnnn consts persist in engine
   src (config.rs untouched; the bridge makes them avoidable, not gone).
5. **`mutation-coverage.md` now contradicts the harness**: doc still says
   22 caught / 3 pending-gate; harness asserts 25/0
   (mutation_harness.rs:1258). The census "machine-checked" claim (:141) is
   false in the direction the audit §3.5 flagged — the exact staleness
   pattern, reintroduced same-day.
6. **SKILL.md:78 retitle** (audit §3.2 fix item) not done.
7. **POLICY §7.2** (CP-9/10 demo-scope annotation) — missed, not deferred:
   absent from the plan's Deferred list and STATUS.md untouched.
8. REUSE items (F7B-7 MANIFEST paths, F8-7 golden vocabulary, F8-8 factory
   docs) — unaddressed, no deferral entry.
9. Nested assume-condition parenthesization (F3T-2 half; plan Task 7 promised
   it) — render_cond/parse_condition byte-identical to baseline.
10. F8-2 serve leg is **dead API**: `scope_with_killed_tokens`
    (session.rs:170) has zero callers; wire consumers remain starved.
11. The four plan-deferred policy items are recorded only in a design note
    ("evidence, not authority") — no ledger record of those author calls.

## 5. New issues introduced by the remediation (beyond §2)

- **fantasy-contract.md replaced one false claim with another**: :105 says
  the six goldens "carry normal `expected_outcome` assertions" — zero
  `expected_outcome` keys exist under contracts/; and the residual §5 bullet
  (:179-183) still defines the `gated`/`not-wired` outcome, contradicting
  "All 46 operations are wired" three paragraphs up.
- **Read-tool prose vs code** (in the normative contract): author.check "run
  a named check family" (no `family` param exists), author.findings
  "filterable by severity/family" (no filter fields), spec 11 "open-finding
  count" (not in the result struct).
- **DL-0023 arithmetic**: "14 mutating + 5 read" vs the adjacent contract
  table marking only 11 of the 14 mutating.
- **Token lint scope narrowed to pass**: the 9 live "the Turning" prose lines
  pass via a blanket `DOCS_EXEMPT_TOKENS = ["turning","the-turning"]`
  (docs_tokens.rs:64 — the most distinctive world token is now unscanned
  docs-wide) plus allowlisting **all of charter.md** (needed for `wyrd` at
  charter:44) — exemption, not the line-level discipline the audit asked for.
- **A third token list appeared**: load.rs `CANONICAL_WORLD_TOKENS` hand copy
  whose "if boundary's list grows this test fails" comment is false (the
  subset test checks only FORBIDDEN ⊆ local copy).
- **negative_poison error-path softening**: `ground_head` errors now silently
  `continue` (world.rs:940-942) — a latent range-restriction bug would
  manifest as an un-poisoned, un-blocked derivation.
- cfg(test) fix covers doc comments only (`//` line comments and block
  comments still poison); ci_parity's new checks cover the gate job's steps
  only (triggers, `uses:`, job-level keys, other jobs still invisible).
- PLAN.md:545 now attributes all "46-op schemas" to bootstrap (bootstrap
  built 41; ec92efd added 5).

## 6. Bottom line

The remediation is roughly half-real: the doc-resync, guard re-anchors, and
new gate steps are genuine and verified. But it should not be treated as
complete: the negation revert (§2.1) trades away a red-team soundness
property and must be resolved **by ledger ruling**, not silently; the three
deleted `assumes` clauses (§2.2) are committed content loss awaiting
restoration; the grade fix needs its Checked-default hole closed (§2.3);
Tasks 15/17 and both promised DL entries are open; and the contract picked up
two new false claims while shedding old ones. Priority order: §2.2 restore →
§2.1 ledger decision → §2.3 hole → contract text (§5 items 1-3) →
mutation-coverage.md tally → the Task 15/16 remainder.
