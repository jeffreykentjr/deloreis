---
name: deloreis
description: Keep fiction and worldbuilding consistent with De Lore Is. Use when maintaining lore for AI-assisted novels, series bibles, campaigns, competing story ideas, retcons, or checks on generated additions to a fictional world. Record and query persistent lore through local MCP tools. Also use for explicit De Lore Is, DLI, or deloreis project requests.
license: MIT
---

# Authoring with De Lore Is

Use De Lore Is when a writing task needs an established world to check against:
continuing a novel, adding lore, revising an account, or exploring another version
of a story. It records structured lore and checks the rules supplied for that
world. Keep the manuscript and prose references in their existing files.

For installation and a complete first check, read
[First session](references/first-session.md). The skill does not install the
runtime. Start with a small section of the user's notes, not a whole-book
migration. Ask which details are established and which are ideas. Propose rules
instead of assuming, for example, that every character can have only one home.

Use the `author.*` tools to add lore, and read every finding a call returns,
even when the call succeeds. Repair mechanical input errors yourself. Resolve
continuity findings with the user's rulings; don't change lore just to make a
check pass. Don't edit `.dli` files: the store is the source of truth for an
authoring session. People can write `.dli` directly and import or check it.

Register `deloreis mcp --project <directory>` with the client, then call its
tools. If they are not available, run the first-session setup; don't simulate a
stored world in the chat. The server also offers newline-delimited JSON through
`deloreis serve --project <directory>`. The served contract is 2.0.0.
`--tools all` advertises named branches, specialist authoring, corpus
import/export, and other tools beyond the default profile. Take exact arguments
from installed help and `tools/list`; don't guess tool schemas.

## Authoring loop

1. `author.open` a scope and retain its returned workspace ID and digest.
   When fencing writes, pass the current digest as `expected_digest`. Each
   change returns a new `workspace_digest` (also in `author.status`); use that
   one for the next write.
2. Use typed operations such as `author.add_entity`, `author.add_fact`,
   `author.add_relation`, `author.add_event`, and `author.add_before`.
   Declare single-valued predicates with `author.declare_functional` and
   predicates whose value changes over story time with
   `author.declare_inertial`.
3. Check the project's source/decision index before choosing a new decision ID;
   reuse an ID only for that same ruling. For a ruling, use `author.record_decision` and cite that decision with
   `{"kind":"cited","decisions":["d-0001"],"state":"ratified"}`. An
   unruled proposal can use `{"kind":"proposed","origin":"session-name"}`.
   Unknown citations are errors and cannot make a claim certain.
4. Inspect `author.check`, `author.findings`, `author.query`, and `author.why`.
   `author.diff` shows staged consequences.
5. `author.materialize` checks and commits staged changes, and refuses them if
   any finding is an error. `author.discard` rejects staged work. Keep the
   returned root hashes and claim IDs; a later retcon needs the claim ID.

`author.record_decision`, `author.defeat`, and standing operations can write to
the store directly, and `author.discard` won't undo those writes. Make these calls only for
rulings the user has agreed to, under the host's review policy, and never to
turn an idea into an accepted account on your own. The server checks references
and continuity. Attribution, decision state, and who may rule come from the
caller.

## Use it during a writing task

- **Continue a chapter:** query the people, places, and established facts relevant
  to the scene. After drafting, propose the new factual claims through the tools,
  check them, and show the user any conflicts. A clean check covers only the
  claims you extracted, so compare them with the prose.
- **Brainstorm:** keep alternatives proposed or in a hypothetical fork. Use
  `author.open_question` and `author.add_option` for a choice between accounts;
  `author.add_hypothesis` and `author.suppose` support hypothetical exploration.
  A note in the store is canon only once it evaluates certain; query
  established claims with `layer: "certain"`. Use `layer: "possible"` or `"all"`
  to inspect open accounts as well; those layers also include certain claims.
  A known claim can evaluate dead, but there is no `dead` query layer: inspect
  its blocked support with `author.why`.
- **Revise:** inspect `author.diff` and `author.why`, then record the user's chosen
  new ruling before calling `author.defeat` on the earlier support, which takes
  the old claim ID. Use a named branch (advertised with `--tools all`) for a
  separate durable version.
- **Track change within the story:** record event-anchored states and query
  `author.as_of`. A character who changes allegiance mid-story is a state
  change on the timeline; a retcon revises the account itself. Declare known event ordering; leave unknown ordering
  open.

New workspaces see current committed lore; an already-open workspace retains its
base. Open a fresh workspace when another session has changed the accepted
world. Concurrent-write policy, like approval and access control, belongs to the
host application. Don't present DLI as general-purpose agent memory, and don't
infer a character's knowledge from world facts.

For CLI reads and wiki export, see [Read and export](references/first-session.md#read-and-export).

## Core tools

The default MCP profile keeps 29 common tools. It advertises `dli.check`, `dli.name.check`, and:

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

`author.add_option` can name a fact that follows if that option is chosen,
using its optional `implies` field. Give options distinct names and add them
before materializing their question.

`author.query` returns claim IDs when available; `author.why` explains
supporting and blocked paths. For a retcon, `author.defeat` takes a claim ID, a
new decision, and the decision it defeats. Defeating the retcon can restore the
original support.

The [Holmes casebook](https://jeffreykentjr.github.io/deloreis/casebook-01.html) is produced from a
replay that exercises the server, and the [concept glossary](https://jeffreykentjr.github.io/deloreis/concepts.html)
defines the terms. A functional conflict is detected only for a predicate
declared single-valued.

## Reading rendered lore

Tools and exports render lore in readable `.dli` forms like these. Read them;
author through the tools:

```dli
person wren "Wren" proposed session
```

```dli
functional home proposed session
```

```dli
home(wren, thornfield) ratified d-0001
```
