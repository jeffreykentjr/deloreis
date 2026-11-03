---
name: deloreis
description: Author structured fiction lore with the De Lore Is MCP tools, inspect findings, and commit checked changes.
status: live
verdict: "Author and inspect a fictional world through the current 2.0.0 tool surface."
---

# Authoring with De Lore Is

Use the `author.*` tools to add lore. Read every returned finding and repair
error-severity findings before materializing. Do not edit `.dli` as an agent
authoring a world; it is a readable import and export format. The store is the
source of truth. The [quickstart](docs/quickstart.md) gives a complete first
session.

Start `deloreis mcp --project <directory>` and use MCP `tools/call`. The server
also offers newline-delimited JSON through `deloreis serve --project
<directory>`. Contract 2.0.0 is the served contract. `--tools all` advertises
specialist tools beyond the default profile.

## Authoring loop

1. `author.open` a scope and retain its returned workspace ID and digest.
2. Use typed operations such as `author.add_entity`, `author.add_fact`,
   `author.add_relation`, `author.add_event`, and `author.add_before`.
   Declare single-valued predicates with `author.declare_functional` and
   changing predicates with `author.declare_inertial` where appropriate.
3. For a ruling, use `author.record_decision` and cite that decision with
   `{"kind":"cited","decisions":["d-0001"],"state":"ratified"}`. An
   unruled proposal can use `{"kind":"proposed","origin":"session-name"}`.
   Unknown citations are errors and cannot make a claim certain.
4. Inspect `author.check`, `author.findings`, `author.query`, and `author.why`.
   `author.diff` shows staged consequences. Check the returned findings even
   if the call itself succeeded.
5. `author.materialize` checks and commits staged facts. It refuses errors.
   `author.discard` rejects staged work. Keep returned root hashes and claim
   IDs when they are useful for review or later retcons.

`author.record_decision`, `author.defeat`, and standing operations can update
the store directly; do not assume that every mutation is only staged. The
caller supplies attribution, decision state, and review policy. The server
checks references and continuity, not who is authorized to rule.

## Core tools

The default MCP profile advertises `dli.check`, `dli.name.check`, and:

- Workspace and inspection: `author.open`, `author.status`, `author.check`,
  `author.findings`, `author.predicates`, `author.query`, `author.why`,
  `author.as_of`, `author.diff`, and `author.entity_candidates`.
- Lore: `author.add_entity`, `author.add_fact`, `author.add_relation`,
  `author.add_event`, `author.add_before`, `author.add_state`,
  `author.declare_functional`, and `author.declare_inertial`.
- Rulings and exploration: `author.record_decision`, `author.open_question`,
  `author.add_option`, `author.defeat`, `author.set_standing`,
  `author.add_hypothesis`, and `author.suppose`.
- Acceptance: `author.materialize` and `author.discard`.

`author.add_option` accepts an optional typed ground `implies` atom. Give
options distinct names and add them before materializing their question.

Use `tools/list` for the exact schemas. `author.query` returns claim IDs when
available; `author.why` explains supporting and blocked paths. For a retcon,
`author.defeat` takes a claim ID, a new decision, and the decision it defeats.
Defeating the retcon can restore the original support. Use `--tools all` for
named branches, specialist authoring, corpus import/export, and other tools.

The [Hollow transcript](examples/hollow/transcript.md) is produced by a test
that exercises the server. The [concept glossary](docs/concepts.md) explains
the terms. DLI checks structured claims, not prose, and a functional conflict
is detected only for a predicate declared single-valued.

## Reading rendered lore

These snippets show the text rendering of tool-authored lore, not calls to
make by editing a file:

```dli
person wren "Wren" proposed session
```

```dli
functional home ratified d-0001
```

```dli
home(wren, thornfield) ratified d-0001
```
