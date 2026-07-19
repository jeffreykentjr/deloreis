# De Lore Is

**De Lore Is** (DLI) is a typed lore-interchange language and worker for
audio-drama-first fantasy fiction: a story is a program, and a contradiction is a
compile error you can replay.

This book is a rendered view of the repository's canonical documents. Nothing
here is authored twice — every chapter includes the real file under `docs/`, so
the book and the repo cannot drift. See `book/README.md` for how the wiring
works.

Reading order, cold:

1. **[Charter](charter.md)** — the twelve koans; the constitution.
2. **[Specification · Overview](spec-00.md)** — what the language is; the seven
   programs.
3. **[The plan](plan.md)** — the dependency graph; the runnable frontier.
4. **[Decision ledger](decisions.md)** — the authority of record. It outranks
   every other document, this book included.

The **Generated reference** chapters (the wire operation inventory and the error
codes) are emitted by `cargo xtask docgen` from the crate types, so they cannot
drift from the code.
