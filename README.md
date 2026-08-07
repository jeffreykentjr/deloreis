# De Lore Is

De Lore Is (DLI) is deterministic epistemic infrastructure for agent systems.
It stores structured claims with provenance and explicit defeaters, then derives
what holds, what remains possible, what is defeated, and why. It is an engine,
not an agent: applications own orchestration, policy, and decisions; DLI makes
their recorded knowledge replayable and checkable.

Fictional worlds and the TPtH integration are first-class reference domains.
The same core supports any declared corpus where agents need durable state,
structured disagreement, and inspectable derivation.

## Start here

1. [Charter](docs/charter.md) — the governing principles.
2. [Specification overview](docs/spec/00-overview.md) — the engine and its
   domain modules.
3. [Scale and distribution](docs/spec/12-scale-and-distribution.md) — the
   peer/service destination.
4. [Decision ledger](docs/decisions.md) — the authority of record.

## Development

```bash
cargo xtask gate
```
