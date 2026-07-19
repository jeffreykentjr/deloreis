# De Lore Is

De Lore Is is a **knowledge language** for fiction: a typed record of what a
world holds — geography, characters, magic, story-time, games — and, for every
claim, where it came from and on whose ruling. It is the third family beside the
simulation languages (Inform, Dialog) and the flow languages (Ink, Twine): not a
runtime, but the authoritative knowledge layer that *feeds* runtimes. Source
files are `.dli`; the compiler-and-server binary is `deloreis` — every line is a
declarative claim about what the world holds, which is the name, read one way.

## Status

Spike scaffold. The AST is normative and the grammar is **unfrozen** — the
surface freezes at the G1 grammar gate, not before. The build is a dependency
graph, not a schedule; what is runnable now is the graph's frontier. See
[`docs/PLAN.md`](docs/PLAN.md) for the frontier and the milestone node-sets.

## Where to start (in order)

1. [`docs/charter.md`](docs/charter.md) — the twelve koans; the constitution.
2. [`docs/spec/00-overview.md`](docs/spec/00-overview.md) — what the language is
   and the seven programs on it.
3. Your lane's spec under [`docs/spec/`](docs/spec/).
4. [`docs/PLAN.md`](docs/PLAN.md) — the dependency DAG and the frontier.
5. [`docs/decisions.md`](docs/decisions.md) — the decision ledger, which
   **outranks everything, including this file**.

## Build and gates

```bash
cargo xtask gate
```

One command runs the full CI-parity gate. Details, discipline, and the commit
ritual are in [`AGENTS.md`](AGENTS.md).

## Layout

| Path | What |
|---|---|
| `crates/` | The kernel crates (evaluator, checks, practices, time, language, serve). |
| `xtask/` | The compiled gate — `cargo xtask gate` runs CI as a Rust test. |
| `schemas/` | The normative AST JSON Schema, derived from the Rust types. |
| `contracts/fantasy-worker/` | The owned integration contract, vendored by fantasy. |
| `fixtures/` | Golden / negative / framing fixtures; `fixtures/mutation/` is the detection corpus. |
| `examples/` | CI-compiled `.dli` examples (the reference-card examples must compile). |
| `docs/` | The doc layer: charter, decision ledger, plan, spec, integration, design notes. |
| `scripts/` | The three git-orchestration scripts (`safe-commit`, `greengate`, `ci-wait`). |
| `.githooks/`, `.github/` | Commit hooks and the deterministic CI lanes. |

## Integration

De Lore Is owns the contract that the fantasy factory consumes. See
[`docs/integration/fantasy-contract.md`](docs/integration/fantasy-contract.md),
the [`migration`](docs/integration/migration.md) plan, and the
[`STATUS`](docs/integration/STATUS.md) board.
