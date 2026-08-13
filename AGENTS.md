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
  canonical form is their JCS byte serialization. The formatter is one renderer
  among several, and `.dli` text is an opt-in rendering, not the canonical
  surface. The staged cutover is `docs/design-notes/lorestore-roadmap.md`.
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
- Models, the Ludii baseline, and benchmarks run in **non-gating** lanes. Nothing
  flaky or live is ever in the verdict.

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

## 6. Integration

- **Two ledgers, two prefixes.** `DL-nnnn` is this repo's decision ledger
  (`docs/decisions.md`); `D-nnnn` is the consuming fantasy repo's ledger — the
  authority lore citations point at. TPtH = *To Pleach the Hedgeway*, the
  consuming audio-drama project.
- `docs/integration/STATUS.md` is **single-writer, deloreis side**. Write only
  your own file; the fantasy mirror is theirs. The pair is the handshake.
- **No single commit or PR spans repos.** A commit touches exactly one repo's
  tree — a deloreis commit never writes the fantasy tree, and vice versa. The
  same agent may open *separate* PRs to deloreis and to a consuming repo
  (fantasy, dli-benchmarks) in one task; what is barred is one commit reaching
  across trees.
- **Three artifacts cross the boundary**, each with one owner: the contract
  (deloreis owns; fantasy vendors, digest-checked — DL-0010), the vendored corpus
  snapshots (fantasy owns the source; deloreis owns the pinned snapshot), and the
  worked-scenario fixture (deloreis owns; fantasy owns the matching transcript).

## 7. The agent-interface law

**Agents author lore through the one interface — the `author.*` MCP tools
against the `deloreis` server — never by editing `.dli` text** (DL-0014 as
restated by DL-0043: entry doors are retired and the tool surface is the single
way in. Attribution and decision state are caller-supplied data; policy about
who may authorize them belongs to the consuming application, DL-0054). Every
mutating tool call returns graded findings with repair data, the structured
result, and the workspace digest; read them, do not guess.
`author.materialize` is a mutation and the accept decision: it commits the
staged fact objects to the lorestore and returns them as JSON by default, with
`.dli` rendering as the `rendered: true` opt-in (DL-0046); `author.discard` is
the reject decision; tools write fact objects to the store per-add (DL-0045).
The contract is **1.9.0** (DL-0051, DL-0053, DL-0055, DL-0056, DL-0058,
DL-0059, DL-0069, DL-0070, DL-0074, DL-0077, DL-0078, DL-0080, DL-0081,
DL-0082), served multi-version dual-mode: `0.5.1`, `0.6.0`–`0.6.4`, and
`1.0.0`–`1.9.0` stay accepted — a `0.5.1` caller keeps the rendered-lines
return. EXT-F4 is complete, which licenses the `0.x` retirement but has not
performed it: that is DM-6's, at `2.0.0`, and it runs through the killed-form
registry naming each version's successor, never silently. `1.3.0` added seven
expert authoring tools; `1.5.0` adds `author.set_standing` and
`author.defeat`, taking the surface to 59 operations. `1.6.0` adds
`author.declare_functional`, taking the surface to 60 operations. `1.7.0`
adds `author.assert_same_as` and `author.add_rule` (the wire-surface-
completion campaign, DL-0077), taking the surface to 62 operations. `1.8.0`
adds `author.declare_inertial`, taking the surface to 63 operations —
declaring a predicate inertial reshapes its fluent projection, bounding an
open value's forward persistence at the next distinct certain value. `1.9.0`
adds `author.as_of`, taking the surface to 64 operations — the one
inertia-aware read, projecting `check::slot_state_at` at a query point
without ever routing through the eval query path. `SKILL.md` governs the
authoring session; supervision reviews
**structured diffs** — the LS-5 contract's per-fact rows with before/after
standing (deloreis serves the JSON; rendering is consumer-owned). Raw `.dli`
editing is a measured fallback with no reliability claim, not the taught
path.

DL-0054 amends the authority half of that law, and the scrub **landed at
`1.3.0`** (DL-0058). The engine no longer gates on *who may rule*: at `1.3.0` and
above a mutating call may carry citation-bearing provenance, and caller-supplied
attribution and decision state are recorded as data — a citation of a **declared**
decision is recorded as `decided` and the claim reads certain, which is the point
of the scrub, not a side effect. What it keeps is referential integrity: a cited
decision must resolve, and an unknown one raises the `citation.unknown-decision`
error-severity finding rather than a refused frame, with `check::run` barring
materialize and the decision's variable left open so the claim can never read
certain. Review and approval workflow belong to the consuming application.
**Below `1.3.0` nothing changed** — `provenance.cited-refused` is the served
behavior on every earlier accepted version, permanently, and every `0.x` wire
keeps it byte-exact.

At `1.5.0`, cited provenance requires caller-supplied
`state: provisional | ratified`. Stored standing uses
`asserted | proposed | provisional | ratified`; schema v4 upcasts legacy
`Decided` metadata to `Provisional`. Promotion records the ratifying decision;
defeat records a new decision and rules the old supporting decision false while
keeping the object reachable. Canon excludes dead claims and verifies the
commit decision list against stored `Ratify` carriage objects.
