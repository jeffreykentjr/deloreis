---
status: live
verdict: "The specification entry point: De Lore Is records and checks the continuity of fictional worlds, with replayable provenance and rulings."
---

# Spec 00 — overview

De Lore Is (DLI) is a continuity engine for fictional worlds. It records
structured claims about characters, places, events, rules, and changing
states, together with their provenance and the rulings that support or defeat
them. Authors and agents can check those claims, ask why they hold, and
explore alternatives without rewriting the established record.

DLI checks the structured lore it is given; it does not read a manuscript and
discover every plot hole. It does not call a model or decide who may make a
ruling. A consuming application supplies attribution, decisions, and its own
review policy. DLI checks the integrity of references and derives the result
from committed inputs, so the same inputs yield the same answer.

The lorestore holds immutable, content-addressed fact objects. Their
schemars-derived schemas and canonical JSON bytes define the stored form.
The `.dli` language provides a readable text surface for authoring and
inspection. The decision ledger (`../decisions.md`) outranks this overview
and the other specification files.

## Working with lore

- **Check continuity.** Declare the rules a world needs, such as a
  single-valued home for a character, and check claims against them. A
  contradiction between ruled claims prevents a commit and identifies its
  support.
- **Explain and revise.** Query whether a claim is certain, possible, or dead,
  and inspect its support. A retcon records a defeat rather than erasing the
  earlier claim; subsequent rulings can change the result again.
- **Explore alternatives.** Fork a world and compare the consequences of
  hypothetical assumptions without changing the committed record.
- **Model time and rules.** Eras and events form a partial chronology, while
  fluents describe states that change. Practices express declared rules and
  proof obligations; the language layer can check names and registers.
- **Integrate.** The command line, JSON server, and MCP authoring tools expose
  these operations to people and to applications working with agents. Exports
  provide one-way projections for other tools.

See [semantics](02-semantics.md) for claims and standing,
[time](03-time.md) for chronology and fluents,
[practices](04-practices.md) for rules and proofs,
[language](05-language-layer.md) for names and registers,
[reasoning](08-reasoning.md) for derivation and explanation,
[exports](09-exports.md) for projections, and
[serving](10-serving.md) and [agent authoring](11-agent-authoring.md) for the
interfaces. The charter (`../charter.md`) states the project's principles;
`../PLAN.md` records implementation status.
