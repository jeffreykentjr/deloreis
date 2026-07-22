# Agent & contributor guide

Status: live — the working reference for De Lore Is. `CLAUDE.md` is a symlink
to this file so Claude- and Codex-style agents read the same instructions. If
you are starting cold: read this file, then `docs/charter.md`, then
`docs/spec/00-overview.md`.

## 1. Cold-start read order

1. This file — how the repo works.
2. `docs/charter.md` — the twelve koans; the constitution.
3. `docs/spec/00-overview.md` — what the language is; the seven programs.
4. `docs/PLAN.md` — the dependency DAG; the frontier tells you what is runnable.
5. `docs/decisions.md` — the ledger. It **outranks every other doc**, this one
   included.

## 2. Doc map and docs discipline

| Path | What |
|---|---|
| `docs/decisions.md` | The append-only decision ledger. Outranks all docs. |
| `docs/charter.md` | The twelve koans, tiebreak, budget, tensions, trials. |
| `docs/PLAN.md` | The build as a dependency graph (no durations). |
| `docs/spec/` | The specification: `00-overview.md` plus per-lane specs. |
| `docs/integration/` | The fantasy contract, migration, and the STATUS board. |
| `docs/design-notes/` | Reasoned notes behind decisions; evidence, not authority. |

Discipline (`decisions.md` DL-0011):

- **Frontmatter is mandatory.** Every doc carries `status` and `verdict`. A doc
  without status frontmatter is suspect — treat it as unverified.
- **The ledger outranks.** When any doc disagrees with `decisions.md`, the
  ledger wins and the doc is stale until fixed.
- **DL-IDs are minted single-writer**, in the ledger only, mint-and-use in one
  commit. No other file invents a `DL-nnnn`.
- **The tree is flat.** No `INDEX` files, no index generators. The ledger and
  frontmatter carry authority; a generator to keep honest is a liability.
- **Design notes are not re-litigated.** A `docs/design-notes/` note is settled
  reasoning; it is reopened only by new ledger evidence — a `DL` entry — never by
  a fresh argument in the note itself.

## 3. Engineering laws

- **AST-first** (DL-0003). The AST's JSON Schema is the language definition; the
  EBNF renders it; the formatter defines the one canonical surface. The schema is
  derived from the Rust types (schemars) — it cannot drift from them.
- **Replayable judgments.** Every verdict re-derives from committed inputs. A
  judgment you cannot replay is not a verdict.
- **The gate is offline and deterministic** — no network, ever, in a gating
  lane. The kernel calls no model and reads no clock.
- **Doc examples are gate-checked.** Every fenced `.dli` example in the docs
  and the reference cards runs through the docs-oracle in CI; the oracle runs
  `dli_syntax::parse_document` — the real front end — over every fenced `.dli`
  block and committed example. A block producing any error-class diagnostic
  fails the gate.
- **Killed forms are registered, never deleted.** A retired keyword, operation,
  or name lands in the killed-form registry and the server refuses it, naming its
  successor.
- **Grades are honest** (DL-0006). `checked > tested > attested > asserted`;
  rollup is weakest-grade over the procedure chain. An honest miss in the
  mutation harness is `xfail-strict`, never hidden.
- **Salsa-readiness without salsa** (DL-0009). Pure phases, per-invocation
  environments, no stateful `Compiler` struct. Incrementality is a later drop-in,
  not a rewrite.

## 4. CI discipline

- The language gate is **deterministic offline lanes only** (DL-0013). These are
  what block a merge.
- Models, the Ludii baseline, and benchmarks run in **non-gating** lanes. Nothing
  flaky or live is ever in the verdict.

## 5. Infra and the commit ritual

- **The gate is one command:** `cargo xtask gate` (DL-0015) — CI parity as a
  compiled Rust test.
- **All commits go through `scripts/safe-commit.sh "<msg>" <file…>`** — stage
  only your named files, gate the new HEAD, push only on green, wait on CI.
- **Never `git add -A`, `git add .`, or `git commit -a`.** Another agent's staged
  work rides along into your commit. Stage by explicit filename only.
- **`scripts/greengate.sh` is the only pre-push verdict that counts** — it runs
  the full gate against the committed tree in a throwaway worktree. A local run in
  the live tree is polluted by other agents' WIP and does not count.
- **CI verification is always by SHA:** `scripts/ci-wait.sh <sha>`. Never read
  `gh run list` after a push — it races the webhook.
- **A red main is a drop-everything event.** Whoever pushed it (or first sees it)
  fixes forward or reverts within one cycle. No new work lands on red main.
- **Leave the staging area empty** when you finish or report (`git restore
  --staged .` if a `git mv` auto-staged).
- **Plain commit messages. No AI `Co-Authored-By` trailers** — hook-enforced via
  `.githooks/`.

## 6. Integration

- **Two ledgers, two prefixes.** `DL-nnnn` is this repo's decision ledger
  (`docs/decisions.md`); `D-nnnn` is the consuming fantasy repo's ledger — the
  authority lore citations point at. TPtH = *To Pleach the Hedgeway*, the
  consuming audio-drama project.
- `docs/integration/STATUS.md` is **single-writer, deloreis side**. Write only
  your own file; the fantasy mirror is theirs. The pair is the handshake.
- **Never commit cross-repo.** deloreis does not write the fantasy tree, and
  fantasy does not write this one.
- **Three artifacts cross the boundary**, each with one owner: the contract
  (deloreis owns; fantasy vendors, digest-checked — DL-0010), the vendored corpus
  snapshots (fantasy owns the source; deloreis owns the pinned snapshot), and the
  worked-scenario fixture (deloreis owns; fantasy owns the matching transcript).

## 7. The agent-interface law

**Agents author lore through Door T — the `author.*` MCP tools against the
`deloreis` server — never by editing `.dli` text** (DL-0014). Every mutating tool
call returns graded findings with repair data, the rendered canonical lines, and
the workspace digest; read them, do not guess. `author.materialize` returns file
contents as data — you commit them through review. `SKILL.md` governs the
authoring session; supervision reviews the **rendered diffs**, always in surface
form. Raw `.dli` editing is a measured fallback with no reliability claim, not the
taught path.
