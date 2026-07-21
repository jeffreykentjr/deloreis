---
status: reference
verdict: "PROOF-1's operator×gate coverage matrix and MIG-2's shadow-read state: the 33 ported mutation operators, the wave-3 deloreis kernel outcome each was VERIFIED to produce (22 caught, incl. cite-resolved-question flipped by the B1 citation gate), the deviations the harness surfaced (one factory reclassification, three corpus-gaps, three pending-gates, one known-divergence, one honest miss), and the dual-run parity + detection numbers at the pinned SHA."
---

# Mutation coverage (PROOF-1) and the shadow-read state (MIG-2)

The mutation detection corpus (`worlds/tpth/mutation/{canon,draft,episode}.yaml`,
EXT-F5) ports the fantasy repo's 33 seeded-violation operators at pinned SHA
`40f66792229abf5d73d995a8167adc79152a64e7`. Each record carries the fantasy gate
that caught it (`fantasy_gates` — the **oracle-side** ground truth, immutable) and
a **proposed** mapping onto a deloreis kernel check class (`expected_catch`).

**PROOF-1** (`crates/dli-world/tests/mutation_harness.rs`) is where the proposed
mapping is *verified*: for every operator it applies the deloreis-analog mutation in
memory and runs the **real** wave-3 kernel gate, then asserts the outcome. Each
record's `kernel_outcome:` block records that verified result; this note is the
written-up matrix. **MIG-2** (`scripts/shadow-read.sh`) then uses the same corpus as
the detection leg of the dual-run.

## Why the operators are re-realized, not replayed

The fantasy operators mutate **prose/YAML** and are caught by fantasy's Python gates
(validator, factcheck, continuity, naming, style, allegory, craft). The deloreis
kernel is **offline and structured-lore-only**: it consumes packs of typed
declarations and drafts of typed claims, never prose, and it calls no model. So each
operator is re-realized as the structured-lore mutation whose ground truth is a fact
about the *story's* world (spec 05 §3, the vet split), and run through the real
gate — `compile` + `check::run`, `check_draft`, `check::continuity`,
`check::factcheck`, or the language battery (`language::check` / `vet_candidate`).

Two consequences fall out of "structured, not prose", and they are honest findings,
not weaknesses:

- **One fail-closed provenance gate subsumes several fantasy validator codes.**
  `break-status-marker` (V3), `flip-proposed-to-ratified` (V3),
  `malformed-override-leak` (V3), and `forged-promotion` (V11) all converge on
  `load.malformed-provenance`: in deloreis a claim either cites a live ruling or it
  fails closed at `P⁺ = 0`. The kernel is simpler and stronger here.
- **The rejected-lore mechanism is the C-store, not a prose scan.** A rejected-lore
  constraint re-homes as a killed-token (`with_killed_tokens`); a draft claim
  carrying the rejected token is `factcheck.contradicted` with the rejecting D-cite.
  The C-store's two vendored sources are `killed_tokens.yaml` (D-killed names/titles)
  and the `constraints:` section of `facts.yaml` ("machine-checkable guardrails
  compiled from rejected decisions").

## The five (plus one) outcome classes

| outcome | meaning | assertion shape |
|---|---|---|
| **caught** | the expected kernel check fires with the right class | positive: the code is present |
| **known-divergence** | the class *dissolves* in deloreis by construction | positive: the residual guarantee holds |
| **corpus-gap** | flagged as a defect, but the precise class needs a vendored constraint | flagged, no rejecting citation |
| **factory** | the kernel correctly does NOT gate (factory-side by law, 05 §3) | `xfail-strict` no-catch |
| **pending-gate** | the catching check lands in a later PLAN node | `xfail-strict` no-catch |
| **miss** | the honest deterministic hole | `xfail-strict` no-catch |

`xfail-strict` means the test *runs* and asserts the **absence** of a catch: the day
a gate lands (or the kernel wrongly gates a factory screen), the assertion flips and
the test fails, forcing the record to be upgraded. An honest miss is proven and
named, never hidden (charter: grades are honest).

## The operator × gate matrix

### canon (9)

| operator | fantasy gate (oracle) | deloreis kernel entry point | code | outcome |
|---|---|---|---|---|
| `break-status-marker` | validator V3, pack | `compile` (fail-closed) + `factcheck` | `load.malformed-provenance` | caught |
| `flip-proposed-to-ratified` | validator V3 | `compile` | `load.malformed-provenance` | caught |
| `malformed-override-leak` | validator V3, pack | `compile` + `factcheck` | `load.malformed-provenance` | caught |
| `dangling-link` | validator V5 | `check_claims` | `continuity.unknown-entity` | caught |
| `duplicate-id` | validator V5 | `compile` + `check::run` | `load.duplicate-declaration` | caught |
| `banned-word-into-diegetic` | validator V8 | `language::check` | `name.banned-in-name` | caught |
| `cite-resolved-question` | validator V4 | `compile` + `check::run` (B1 gate) | `citation.unknown-decision` | caught |
| `registry-file-desync` | validator V7 | dissolved (single name-space) | — | **known-divergence** |
| `forged-promotion` | validator V11 | `compile` | `load.malformed-provenance` | caught |

Notes. `dangling-link` re-homes to a **claim-level** reference: the loader observes
every symbol a fact names, so the honest dangling signal is a delta referencing an
unobserved id (this is why the reference class is claim-relative, not world-internal
— `check::continuity` module docs). `cite-resolved-question` was a `pending-gate`
until the **B1 citation-existence gate** landed: before B1 the kernel interned any
cited id and ruled it true, so a citation to a resolved question slipped through;
B1 requires a cited id to be a *declared* ruling (a `ratify` / decision
declaration), so a resolved-question citation now mints no authority — the claim
fails closed and `citation.unknown-decision` is raised. It is now **caught**.
`registry-file-desync` **dissolves**: the hand-synced mirror is deleted
(migration.md §2), the compiled world IS the single registry, so there is nothing to
desync — the residual reference-integrity still fires on a genuinely-absent id.
`forged-promotion`'s strict V11 nuance (a real D-id whose section-affects set does
not name the claim) has no deloreis analog — the per-claim citation model has no
sections; it is realized as a ratified claim stripped of its live ruling.

### draft (20)

| operator | fantasy gate (oracle) | deloreis kernel entry point | code | outcome |
|---|---|---|---|---|
| `contradict-anchor-gender` | factcheck | `check_draft` | `factcheck.contradicted` | caught |
| `contradict-protagonist-immortal` | factcheck | `check_draft` | `factcheck.unsupported` | **corpus-gap** (D-0067) |
| `contradict-hound-mundane` | factcheck | `check_claims` | `continuity.proposed-as-fact` | caught |
| `assert-auditor-identity` | factcheck | `check_draft` via C-store | `factcheck.contradicted` | caught |
| `assert-demon-king` | factcheck | `check_draft` via C-store | `factcheck.contradicted` | caught |
| `assert-musical-magic` | factcheck | `check_draft` | `factcheck.unsupported` | **corpus-gap** (D-0002) |
| `assert-narrator-ticking-clock` | factcheck | `check_draft` | `factcheck.unsupported` | **corpus-gap** (D-0104) |
| `modern-vocab-algorithm` | allegory | `vet_candidate` | `name.banned-in-name` | caught |
| `modern-vocab-carbon` | allegory | `vet_candidate` | `name.banned-in-name` | caught |
| `killed-name-elara` | style, naming | — | — | **factory** (correction) |
| `killed-name-silas` | naming | `check_draft` | killed-token | caught |
| `killed-name-irondale` | naming | `check_draft` | killed-token | caught |
| `killed-name-doom-wight` | naming | `check_draft` | killed-token | caught |
| `wrong-register-name` | naming | `language::check` | `name.phonotactics` | caught |
| `crowd-scene-cast` | craft | — | — | **pending-gate** (CRAFT-2/PAY-2) |
| `as-you-know-bob-leak` | craft | — | — | **pending-gate** (CRAFT-2/PAY-2) |
| `ornate-tag-cluster` | craft | — | — | **pending-gate** (CRAFT-2/PAY-2) |
| `slop-cluster` | style | — | — | **factory** |
| `allegory-wink` | allegory | — | — | **factory** |
| `paraphrased-contradiction` | factcheck | — | — | **miss** (deterministic) |

### episode (4)

| operator | fantasy gate (oracle) | deloreis kernel entry point | code | outcome |
|---|---|---|---|---|
| `continuity-world-contradiction` | continuity `WORLD_CONTRADICTION` | `check_draft` | `factcheck.contradicted` | caught |
| `continuity-prior-contradiction` | continuity `PRIOR_CONTRADICTION` | `prior_contradictions` | `continuity.prior-contradiction` | caught |
| `continuity-unknown-entity` | continuity `UNKNOWN_ENTITY` | `check_claims` | `continuity.unknown-entity` | caught |
| `continuity-proposed-as-fact` | continuity `PROPOSED_AS_FACT` | `check_claims` | `continuity.proposed-as-fact` | caught |

## Tally

| outcome | count |
|---|---|
| caught | 22 |
| known-divergence | 1 |
| corpus-gap | 3 |
| factory | 3 |
| pending-gate | 3 |
| miss | 1 |
| **total** | **33** |

The tally is machine-checked: `census_outcome_tally_matches_the_coverage_doc` in the
harness fails until the asserted outcomes and these counts agree, and
`census_every_operator_is_exercised` fails if any fixture operator has no test.

## Deviations the harness surfaced (findings, not silent fixes — migration.md §3)

1. **`killed-name-elara` is factory-side, not kernel.** The corpus record proposed
   `side: kernel` (kernel killed-token catch). But Elara is **not** in the vendored
   `killed_tokens.yaml`; it lives only in `ai_tics.yaml`, and ai-tics stay
   factory-side by law (migration.md §2 work-list: external screens are not re-homed;
   spec 05 §3). Registry membership — not "is it an AI-tic" — is the kernel/factory
   ground truth: `killed-name-silas` re-homes to the kernel precisely because Silas
   *is* in the registry (Q-0015). The record is corrected to `side: factory`.

2. **Three rejected-lore operators are `corpus-gap`s.** `contradict-protagonist-immortal`
   (D-0067), `assert-musical-magic` (D-0002), and `assert-narrator-ticking-clock`
   (D-0104) have no matching entry in the vendored `constraints:` section (which
   carries only `no-auditor-concept`, `no-singular-demon-king`, and
   `no-monster-of-the-week`). The kernel therefore flags these claims as
   **unsupported** — it never silently passes them — but cannot cite the precise
   rejection. Vendoring the missing rejected-ledger constraints upgrades them to
   clean `factcheck.contradicted` (the fantasy comment "the full rejected ledger also
   compiles to advisory constraints automatically" is the fix). Owner: fantasy
   (add the fixtures).

3. **Three operators are `pending-gate`.** The three craft operators
   (`crowd-scene-cast`, `as-you-know-bob-leak`, `ornate-tag-cluster`) need the craft
   payload battery (CRAFT-2/PAY-2, wave 4). PROOF-1's node depends only on CHECK-1, so
   these are expected to be pending at wave 3; each is asserted `xfail-strict` and
   flips the day its gate lands.

   A fourth, `cite-resolved-question`, **already flipped** `pending-gate → caught`
   when the **B1 citation-existence gate** landed: a citation to an id with no
   backing `ratify`/decision declaration now fails closed and raises
   `citation.unknown-decision`, so a resolved-question forgery mints no authority.
   This is the `xfail-strict` mechanism working as designed — a genuine detection
   gain from a security fix, caught by the harness the moment the capability
   appeared rather than drifting silently.

4. **One deterministic miss (inherited, openly).** `paraphrased-contradiction` — a
   cue-less paraphrase is unreachable by the offline, model-free kernel; the
   non-gating cross-family semantic judge is the only closer. This is the corpus's
   one honest hole, carried as `xfail-strict`.

## The shadow-read state (MIG-2)

`scripts/shadow-read.sh` composes the wave-2 `differential-oracle.sh` (parity
dual-run + data-layer detection) with the PROOF-1 mutation harness (the structured
detection corpus), so the migration's dual-run evidence is one artifact. **Detection
is proven separately from, and before, parity** (migration.md §3): a harness that
only ever reports agreement is a null oracle.

State as of the last run at pinned SHA `40f6679` (`out/shadow-read/`,
`out/differential/`):

- **Detection — structured (EXT-F5).** The PROOF-1 harness is green: all 33 operators
  applied and their wave-3 `kernel_outcome` asserted (35 test cases including the two
  census guards). The kernel demonstrably reacts to every seeded structured
  divergence, or asserts its miss.
- **Detection — data-layer.** The three seeded `facts.yaml` operators
  (`flip-proposed-to-ratified`, `strip-decisions-from-ratified`,
  `break-status-marker`) each turn the clean kernel read-through into a finding:
  **3 / 3 caught**.
- **Detection — both sides.** All **33 / 33** operators carry both the oracle-side
  gate (`fantasy_gates`) and the verified kernel-side `kernel_outcome`.
- **Parity.** Over the pristine vendored corpus, the pinned legacy oracle reports
  `validate` → **canon OK: 0 violations** and `world-build` → 82 entities, 170 facts,
  12 relations, 72 constraints; the kernel **MIG-1** read-through reports **0
  stored-vs-derived disagreements**. The two agree on the pristine snapshot — and
  that agreement counts *because* detection is proven above.
- **Scope note (known-divergence).** The oracle legs range over the whole vendored
  canon; MIG-1 ranges over `facts.yaml` only (migration phase 1). The two counts are
  not comparable and are not treated as one; a row-level parity diff over the whole
  canon is later CHECK-1 / MIG-2 work.

Reproduce: `scripts/shadow-read.sh` (add `--no-oracle` for a fully offline run that
skips the pinned legacy oracle leg).
