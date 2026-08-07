# De Lore Is

**De Lore Is** (DLI) is deterministic epistemic infrastructure for agent
systems. It evaluates structured claims with provenance and defeaters, producing
replayable standing and explanations. Fiction and TPtH are first-class reference
domains; DLI is an engine for agents, not an agent or policy authority itself.

This book is a rendered view of the repository's canonical documents. Nothing
here is authored twice — every chapter includes the real file under `docs/`, so
the book and the repo cannot drift. See `book/README.md` for how the wiring
works.

Reading order, cold:

1. **[Charter](charter.md)** — the twelve koans; the constitution.
2. **[Specification · Overview](spec-00.md)** — the engine, domain modules, and
   reading paths.
3. **[Scale and distribution](spec-12.md)** — the deferred peer/service
   destination.
4. **[The plan](plan.md)** — the dependency graph; the runnable frontier.
5. **[Decision ledger](decisions.md)** — the authority of record. It outranks
   every other document, this book included.

The **Generated reference** chapters (the wire operation inventory and the error
codes) are emitted by `cargo xtask docgen` from the crate types, so they cannot
drift from the code.
