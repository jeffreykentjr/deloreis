---
status: live
verdict: "The spec entry point: what De Lore Is is, the knowledge-family position, the seven programs and their ops, where world specifics reside, the generality and its single-authority limit, and the reading order per role."
---

# Spec 00 — overview

The entry point to the specification. It states what the language is, locates it
among existing narrative languages, censuses the programs built on it, and points
each reader at the file they need next. The per-lane specs elaborate; the
decision ledger (`../decisions.md`) outranks all of them.

## What De Lore Is is

De Lore Is is a **knowledge language**: a typed record of what a fictional world
holds — geography, characters, magic, story-time, games — and, for every claim,
where it came from and on whose say-so. Source files carry the `.dli` extension;
the compiler-and-server binary is `deloreis`.

It is not a runtime. It does not simulate a world at play time, and it does not
deliver a narrative at play time. It answers a different question, the one no
play-time language answers: *is this true, since when, on whose ruling, and what
breaks if it changes?* Every line is a declarative claim about what the world
holds — which is the name, read one way.

## The third family

Two families of narrative language already ship. De Lore Is is a third.

| Family | Models | Runs at | Exemplars |
|---|---|---|---|
| Simulation | world state | play | Inform, TADS, Dialog |
| Flow | discourse and choice | play | Ink, Twine, ChoiceScript |
| **Knowledge** | **what is true, on whose say-so** | **authoring** | articy (commercial, no provenance semantics); **De Lore Is** |

Positioning against both play-time lineages is the load-bearing claim of the
export layer: a knowledge layer that is *not* a runtime is exactly what can feed
many runtimes. The kernel is the authoritative layer those tools lack — it does
not compete with them; it feeds them.

## The seven programs

Seven ordinary programs share one library over versioned data files in a git
repo:

1. **A magic-system workbench** — the headline. An author (or an agent under
   author review) writes a magic system as *data*: its sorts, its laws as
   annotated rules, its costs as declared resources, its failures as declared
   error classes. The runtime derives a checker and evaluator for that system.
   Nothing about any particular magic is hardcoded in the kernel — a magic
   system is a practice with laws.
2. **A compiler/checker** — `deloreis check`: assertability, continuity, the
   pack, practice well-formedness.
3. **A query engine** — `deloreis query` with `--layer/--as-of/--fork/--why`.
4. **A fork manager** — `fork new/suppose/add/diff/promote`, `preflight`:
   what-if worlds as cheap, reviewable, diffable values.
5. **A ledger checker for any declared practice** — `deloreis check-proof in
   <practice>`: any practice's proof ledger is one instance of a generic
   declarative-ledger checker.
6. **A practice simulator** — `deloreis simulate`: batteries over any declared
   practice with declared metrics and invariants; the Turning is its first
   instance.
7. **A language workbench** — registers, names as typed derivations, the
   diegetic lexicon.

**The ops, in one line each.** `check` (compile + check); `query` (read the
compiled world); `fork` (create/suppose/add/diff/promote/preflight); `check-proof`
(a proof ledger in a practice); `check-draft` (the battery over a draft payload);
`simulate` (batteries with metrics + invariants); `timeline`
(show/order/knows/episodes/phases); `export` (one-way compiles to targets);
`name` (the kernel-half vet); and `serve` (the server hosting the batch `dli.*`
operations and **Door T** — the `author.*` semantic tools agents author
through). The full op inventory and its return shapes live in
`../integration/fantasy-contract.md`.

## Where the world's specifics live

A consuming repo holds four residencies, and the distinction is load-bearing:

- **Source** (author/agents, reviewed) — `decisions.md` (the author's verbatim
  rulings), the prose canon, and the `.dli` packs (`world`, `cast`, `chronology`,
  `turning`, the ledger mirror of `decisions.md`).
- **Experiment-state** (agents) — forks: small committed files holding an
  assumption list plus local declarations, cheap to create, deleted or archived
  on promotion or abandonment.
- **Evidence** (machine-written, agent-committed) — simulation run manifests and
  the attestation store: replayable records, not truth.
- **Derived** (machine) — the `.dli-cache/` compiled snapshots: content-hash
  keyed, gitignored, never a source of anything.

**The machine's write surface is exactly** `.dli-cache/`, run manifests, and
ruling-request *drafts*. It never writes `canon/` or ledger content that does not
cite an existing ruling. The machine never writes canon (charter, koan 3).

## Generality, and the one limit

Nothing in the kernel is world-specific; the domain keywords are a small
standard prelude. The language generalizes:

- **Another fiction world** — direct; only the Turning is bespoke, and it is a
  module.
- **An in-world magic system** — a different `practice` block; the core is
  untouched.
- **A card/board game plus variants** — decks, variants-as-deltas, house rules
  as forks, invariants checked by enumeration or Monte Carlo (graded honestly).
- **A curated-provenance knowledge base** — event variables map onto
  sources-of-evidence; "if we trust this source" is a fork.
- **A TTRPG campaign** — possibly the best fit outside fiction: the GM is the
  valuation authority, sessions are episode acceptances, retcons are
  supersession-with-lineage.

**What does not generalize: the single-authority valuation model.** Everything
above assumes one sovereign whose valuations are the world. Collaborative
worldbuilding — writers' rooms, shared universes, wiki canon — needs
multi-authority valuation (per-author valuations, a merge policy, consensus
rules). The annotation layer is indifferent, but the valuation regime and the
promotion workflow are built for one author. Multi-authority is explicitly
outside v1 (`PLAN.md` DEF-4). Likewise house-specific and *not* claimed as
language: the ratification SOP, the propagate machinery, the naming oracle's
external screens, the banned lexicon — pipeline, not grammar.

## Reading order, per role

- **A new contributor:** the charter (`../charter.md`) → this file → your lane's
  spec → `../PLAN.md` → `../decisions.md`, which outranks everything, including
  this file.
- **A human worldbuilder (Door H):** the gentle-subset reference card and
  `SKILL.md`; the language reads like structured notes, and what parses is
  exactly what the card shows.
- **An agent (Door T):** `SKILL.md`, then the `author.*` tools — talk to the
  server, read the graded findings, commit what `materialize` returns.
- **An integrator (fantasy side):** `../integration/fantasy-contract.md` →
  `../integration/migration.md` → `../integration/STATUS.md`.
