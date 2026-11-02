---
status: live
verdict: "The release north star (DL-0101): De Lore Is is a continuity engine for fictional worlds that agents and authors write lore into. Every cleanup, doc, example, and the launch blog post is judged against this page."
---

# North star: what De Lore Is is, for release

This page is the reference for the fiction-release reset. Use it to decide what
stays, what goes, and what the docs and examples say. The decision ledger
(`../decisions.md`, DL-0101) outranks it. The execution plan is
[`HANDOVER.md`](HANDOVER.md), and the verified state of the code is in
[`assessment-2026-11.md`](assessment-2026-11.md).

## One paragraph

De Lore Is (DLI) is a continuity engine for fictional worlds. Authors, and the
AI agents that write with them, record lore as structured claims about
characters, places, events, eras and rules. Each claim keeps where it came from
and how settled it is: **canon** (certain), **unconfirmed or contested**
(possible), or **retconned** (dead). DLI refuses to commit lore that
contradicts ruled canon, names the rulings involved, keeps every retcon
inspectable, and gives the same answer every time from the same inputs. It
calls no model.

**Tagline:** *A series bible that remembers every ruling, catches every
contradiction, and never forgets a retcon.*

## Who it is for

- Writers' rooms on serialized fiction: audio drama, web serials, long series,
  shared worlds.
- Game masters and designers running long campaigns or game narratives with
  branching states.
- Worldbuilders with a large bible that has started to contradict itself.
- People building AI storytelling tools who want lore checked against canon,
  not pasted into a prompt. DLI is agent-ready, with an MCP tool surface and
  typed, repairable findings. That ergonomics is part of the pitch, but the
  domain is fiction.

## Who it is not for (do not pitch these)

- **General agent memory or open-domain knowledge bases.** We tried this
  direction (2026-07 to 2026-08, the dli-benchmarks program) and abandoned it.
  The docs must not imply it.
- **Prose plot-hole detection.** DLI does not read a manuscript and find holes.
  It checks structured claims against canon you have already ruled on. An
  experiment that had a model extract claims from prose failed its efficacy
  gate.
- **A wiki or a prompt injector.** DLI is the checker behind those tools.

## What is good, and true today

Every item below was exercised live over MCP on the default build at
`c1ef2a9`. Evidence is in the assessment.

1. **Contradictions are caught before they become canon.** Declare a slot
   single-valued, for example `home`. When two ruled claims give Wren two
   homes, `author.check` raises `continuity.fd-violation`, naming both values
   and both rulings, and `author.materialize` refuses to commit
   (`materialize-blocked`).
2. **A retcon never erases anything.** `author.defeat` records a new ruling
   that overrides the old one. The old claim stays inspectable, and everything
   derived from it updates. Defeating the retcon itself restores the original,
   and a retcon cycle stays honestly undecided.
3. **Rival in-world accounts stay contested.** An open question with
   mutually exclusive options keeps each account `possible`, each with its own
   chain of support, until someone rules. A fact implied by every option is
   still not called canon.
4. **"Why is this canon?" has a mechanical answer.** `author.why` names the
   supporting rulings and the defeats that block a claim. It also flags claims
   that are safe now but depend on an open question.
5. **The bible is a folder you can put in git.** The store is
   content-addressed, and every verdict can be replayed from committed inputs.
   No clock or model is involved.
6. **Agents can't silently invent things.** Each mutating call returns graded
   findings with typed repairs, for example "unknown entity, did you mean…".
   An unknown citation can never make a claim read as certain.
7. **Story time is a partial order.** "Unordered" is a real answer, and state
   that changes over time (fluents) projects with three values.
8. **Readable lore text.** The `.dli` language is the human face of a world,
   and DLI ships tools for it: `check`, `fmt`, `query --why`, and `check-proof`
   for magic-system rules. DL-0101 revives it as a first-class, hardened
   surface. The store remains the canonical form.

## Honest limits to state in launch copy

- "Who knew what when" works in production order, not story order. Don't
  promise modelling of unreliable narrators.
- Conflicting proposed values on a declared-functional slot raise a warning;
  the author still decides which account, if any, becomes canon.
- The contradiction check fires only on slots you declared single-valued.

## The example world (to build; replaces every former private consumer example)

The world is original, small, and public-domain-safe. Its working name is
**The Hollow**: a courier named Wren, the villages of Thornfield and Ashby,
and a disputed death. It must contain, in this order:

1. Entities, eras, and a first canon ruling (Wren's home is Thornfield).
2. A contradiction. A later "episode" agent says Wren lives in Ashby. The
   contradiction is caught, the commit is refused, and the error names both
   rulings.
3. A contested account: two witnesses disagree about how the miller died. Both
   accounts stay `possible`, each with its own `why`.
4. A retcon: one account is ruled canon and the other is defeated. Then
   restore it by defeating the retcon.
5. A what-if branch: "what if Wren never left Thornfield?", with the
   consequence diff.
6. Replay: restart the server and get the same answers.

The world ships as `.dli` text plus a store-backed project. A scripted MCP
driver reproduces the transcript, and CI runs the driver, so the blog's
transcript cannot drift from the engine. Camelot and the Norse worlds stay as
secondary examples.

## Launch blog outline (for the post on mesite; not written yet)

- **Hook.** "Episode 3 said she grew up in Thornfield." Continuity drifts in
  every long story. AI co-writers make it worse, because today's lore tools put
  lore into a prompt and never check what comes back.
- **What DLI is.** One picture: an agent makes `author.*` calls, each claim
  carries provenance and standing, and canon comes out. Show one fact as
  `.dli` next to the same fact as a tool call.
- **Beats 1–6** from the example world above. Each beat uses real transcript
  output produced by the CI driver.
- **What it isn't.** Use the honest limits and the not-for list above.
- **Try it.** Quickstart, the MCP config snippet, the example world, and the
  glossary.
- **What's next.** Multi-authority canon, such as books versus show or canon
  versus fanon (GH #85), and character knowledge in story time.

## Target docs set for release

| Doc | Purpose |
|---|---|
| `README.md` | The front door: the paragraph above, a 30-second contradiction-catch transcript, and links to the quickstart, skill, and glossary. Governance moves to a short "Contributing" footer. |
| `docs/quickstart.md` | Build or install, `deloreis mcp` registration (`claude mcp add`), a blank world, and the first eight calls. |
| `docs/concepts.md` | A glossary that maps engine words to fiction words: materialize = commit to canon; record_decision = a canon ruling; defeat = retcon; certain/possible/dead = canon/unconfirmed/retconned; functional = single-valued slot; fluent = state over time; scope = world id. |
| `docs/cards/authoring-loop.md` | The worked tutorial on The Hollow, re-captured from the CI driver. |
| `SKILL.md` | The agent manual for the current contract only. Core tools come first, every example is fiction, and there are no ledger IDs. |
| `CHANGELOG.md` | Version history, moved out of AGENTS.md and SKILL.md. |
| mdBook | Ordered for users: getting started, concepts, reference, then governance as an appendix. |
