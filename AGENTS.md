# Agent & contributor guide

Status: live — the working reference for De Lore Is. `CLAUDE.md` is a symlink
to this file so Claude- and Codex-style agents read the same instructions. If
you are starting cold: read this file, then `docs/charter.md`, then
`docs/spec/00-overview.md`.

## 1. Cold-start read order

1. This file — how the repo works.
   **Current work:** the fiction-release reset (DL-0101). Read
   `docs/release/north-star.md` and `docs/release/HANDOVER.md` before anything
   else; where they conflict with older text in this file, DL-0101 wins.
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
| `docs/release/` | Fiction-release direction, assessment, and handover. |
| `docs/design-notes/` | Reasoned notes behind decisions; evidence, not authority. |

Discipline (`decisions.md` DL-0011):

- **Frontmatter is mandatory.** Every doc carries `status` and `verdict`. A doc
  without status frontmatter is suspect — treat it as unverified.
- **The ledger outranks.** When any doc disagrees with `decisions.md`, the
  ledger wins and the doc is stale until fixed.
- **DL-IDs are minted single-writer**, in the ledger only, mint-and-use in one
  commit. No other file invents a `DL-nnnn`.
- **Entries carry an `Attribution:` marker** — `machine` or `human` (DL-0052).
  Agents mint their own rulings rather than filing requests; human supervision
  is async and lagging, steering as repair, never a merge-blocking sign-off.
- **The tree is flat.** No `INDEX` files, no index generators. The ledger and
  frontmatter carry authority; a generator to keep honest is a liability.
- **Design notes are not re-litigated.** A `docs/design-notes/` note is settled
  reasoning; it is reopened only by new ledger evidence — a `DL` entry — never by
  a fresh argument in the note itself.

## 3. Engineering laws

- **Schema-first** (DL-0003 as amended by DL-0041). The schemars-derived JSON
  Schema is the language definition — it cannot drift from the Rust types. Under
  the lorestore pivot the schema describes the stored fact objects themselves;
  canonical form is their JCS byte serialization. The default build supports
  `.dli` text for human authoring and reading; agents use the semantic tools
  below. The earlier cutover is recorded in
  `docs/design-notes/lorestore-roadmap.md`.
- **Replayable judgments.** Every verdict re-derives from committed inputs. A
  judgment you cannot replay is not a verdict.
- **The gate is offline and deterministic** — no network, ever, in a gating
  lane. The kernel calls no model and reads no clock.
- **Doc examples are gate-checked.** Every fenced `.dli` example in the docs
  and the reference cards runs through the docs-oracle in CI; the oracle runs
  `dli_syntax::parse_document` — the real front end — over every fenced `.dli`
  block and committed example. A block producing any error-class diagnostic
  fails the gate. The law survives the lorestore pivot with its oracle
  re-aimed (DL-0041, roadmap LS-7): fenced `json` examples validate against
  the fact-object schemas as fenced `.dli` recedes; fenced examples are never
  unchecked.
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
- Live services and nondeterministic experiments never participate in the verdict.

## 5. Infra and committing

- **The gate is one command:** `cargo xtask gate` (DL-0015) — CI parity as a
  compiled Rust test.
- **Plain git is fine.** Commit with ordinary `git add <file…>` + `git commit`.
  Run the check that matches your blast radius before pushing — docs-only
  changes want `cargo xtask docs-oracle` and `cargo xtask authority`; code
  changes want the gate. `scripts/safe-commit.sh`, `scripts/greengate.sh`, and
  `scripts/ci-wait.sh` remain available as the belt-and-suspenders path for
  risky or multi-agent-contended moments, not as a mandatory ritual.
- **Never `git add -A`, `git add .`, or `git commit -a`.** Another agent's staged
  work rides along into your commit. Stage by explicit filename only. Leave the
  staging area empty when you finish.
- **A red main is a drop-everything event.** Whoever pushed it (or first sees it)
  fixes forward or reverts within one cycle. No new work lands on red main.
- **Plain commit messages. No AI `Co-Authored-By` trailers** — hook-enforced via
  `.githooks/`.

### Build artifacts and worktrees

- **Worktrees live at `.claude/worktrees/<name>`** inside the primary checkout
  (gitignored) — not as `~/dli-*` siblings.
- **Each worktree builds into its own `target/`.** Never point
  `CARGO_TARGET_DIR` at a shared or home-level dir (`~/tg-*`); no test needs a
  short target path. `scripts/greengate.sh` keeps its own cache dir.
- The workspace profiles keep dev/test debug info to line tables; set
  `CARGO_PROFILE_DEV_DEBUG=2` for a debugger session.
- **After merge or abandon:** `cargo clean` in the worktree, then
  `git worktree remove <path>` and `git worktree prune`.

## 6. Repositories

A commit touches exactly one repository. Changes to a consuming project belong
in a separate change set.

## 7. The agent-interface law

Agents author lore through `author.*` MCP tools, not by editing `.dli` text.
Read returned findings and repairs before committing. `author.materialize`
checks and commits the staged objects; `author.discard` rejects staged work.
Mutating tools may also write to the store directly. The server serves contract
2.0.0 only. `SKILL.md` describes the current session workflow. Caller-supplied
attribution and decision state are data; authorization and review policy belong
to the consuming application. Review structured diffs and their standing.
