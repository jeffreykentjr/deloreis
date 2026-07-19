---
name: deloreis
description: Author De Lore Is (.dli) lore through Door T — the deloreis server's author.* semantic tools — reading the graded findings each call returns; raw .dli editing is a measured fallback only.
status: live
verdict: "The v0 authoring skill: Door T is how agents author; the compile is the oracle; raw editing is discouraged; the gentle-subset card; provenance discipline."
---

# Skill: authoring De Lore Is

## 0. Scope

This skill is how an agent adds lore to a De Lore Is world. It covers the
supported path (Door T), the fallback (raw `.dli` editing) and why to avoid it,
and the gentle subset for the rare case you must read or write surface by hand.
It does not cover ratification — the machine never writes canon; you produce
proposals and evidence, and the author rules.

## 1. Door T is how you author

Author by talking to the `deloreis` server's `author.*` tools — never by typing
`.dli` text. The shape of a session:

```
author.open  →  author.add_* (entity / fact / relation / event / before /
                 question / option / hypothesis / suppose)  →
author.diff  →  author.materialize
```

**Every mutating call returns three things, immediately:** graded **findings**
(with repair data), the **rendered** canonical lines, and the **workspace
digest**. Read the findings — they tell you exactly what is wrong and how to
repair it (which check to run, which screen to pass). Do not guess and re-emit;
the tool already told you.

`author.materialize` returns `.dli` file contents **as data**. That output is
what you commit — write it to your side's staging path and commit it through your
own review. The kernel does not write your repo.

Use `author.status` to see what is staged and `author.diff` to see the full
rendered block plus every finding (including the merely-possible status of
proposed atoms) before you materialize. Use `author.discard` to drop a bad
staged declaration.

## 2. The compile is the oracle

`deloreis check` — and the findings the `author.*` tools return — are the
authority on whether your lore is well-formed and consistent. **The compile
outranks this file.** If a tool's finding disagrees with anything written here,
the finding is right and this file is stale.

## 3. Raw `.dli` editing is discouraged

Editing `.dli` files by hand is a **measured fallback**, not a supported door. It
meets the same gate and the same graded errors — nothing is lost — but no tooling
is built for it and **no reliability claim is made for it**. Prefer Door T. If
you must edit surface directly, keep to the gentle subset below and let the
formatter canonicalize on save.

## 4. The gentle-subset card

Most of worldbuilding is declarations: entities, facts, relations, timeline,
citations, `proposed`, and `question`. A first world in the gentle subset — the
Saltmarket skeleton:

```dli
pack saltmarket

era founding   "The Landing"      decided d-0001
era now        "The Ninth Tide"   after founding

place saltmarket   "Saltmarket"     decided d-0001
place the-shallows "The Shallows"   decided d-0003
place breakwater   "The Breakwater" proposed

adjacent(saltmarket, the-shallows)  decided d-0003
```

```dli
person mara "Mara of the Weirs"     decided d-0002
  mara.trade = eel-fisher           decided d-0002
  mara.home  = the-shallows         proposed

thing tide-bell "The Tide-Bell"     decided d-0004
  tide-bell.kept-at = saltmarket    decided d-0004
```

```dli
question q-0001 "Who raised the Tide-Bell?"
  option guild      implies raised-by(tide-bell, guild)
  option marsh-folk implies raised-by(tide-bell, marsh-folk)
```

Read it as notes, not as English: what parses is exactly what this card shows.
Every claim ends in its origin — `decided d-nnnn` (a recorded ruling) or
`proposed` (a claim awaiting one). A claim with no origin does not parse:
*claims come from somewhere.* `breakwater` and `mara.home` are `proposed` — fine
as notes, out of canon until decided.

## 5. The expert tier

Rules, forks, wheels, practices (magic systems), ledgers, registers, and
attestations are the expert tier — a second reference card, not this one. No
expert-tier word appears on the gentle card. When you reach for them, read the
lane's spec under `docs/spec/` and the charter's koans on the shape they take.

## 6. Killed forms

Retired keywords, operations, and names are registered and refused — the tool or
compiler names the successor. If a finding says a form is killed, use the named
replacement; do not reintroduce the old one. (For example, `dli.suppose` is
killed in favor of `dli.fork.suppose`.)

## 7. Provenance discipline

- **Nothing you write mints authority.** Everything you add enters `proposed`,
  carrying your agent provenance. Only the author's ruling makes a thing true.
- **A rename is a supersession, never an in-place edit.** The new name is ruled
  in; the old name dies into lineage and the killed-form registry, with every
  claim's history intact. There is no edit that changes the past quietly.
- **An open question is a debt.** File it (`author.open_question`), surface it,
  and never carry it in your head — a waiting decision lives in the world, not in
  memory.
