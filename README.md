---
status: live
verdict: "De Lore Is checks and explains continuity in fictional worlds."
---

# De Lore Is

De Lore Is (DLI) is a continuity engine for fictional worlds. Authors and
agents record structured claims about characters, places, events, and rules;
DLI checks them against declared canon and explains the rulings behind its
answers. Retcons preserve earlier claims, and the same committed inputs
produce the same verdict without a model call.

DLI checks structured lore: it does not read a manuscript and discover every
plot hole. Callers decide who may make rulings and how those rulings are
reviewed.

The fiction-focused release is in progress. Today the text tools require the
`syntax` feature:

```bash
cargo run -p deloreis --features syntax -- check examples/camelot.dli
```

See the [overview](docs/spec/00-overview.md), [serving specification](docs/spec/10-serving.md),
and [authoring guide](SKILL.md). The [release plan](docs/release/HANDOVER.md)
tracks the remaining work.

Contributions are checked with `cargo xtask gate`; see [AGENTS.md](AGENTS.md)
and the [charter](docs/charter.md) for project conventions.
