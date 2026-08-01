---
status: live
title: "Syntax"
verdict: "AST-first; one canonical rendering; one interface since DL-0043 (this spec's two-door section is retained as history); grammar versioned, formatter is the migrator."
---

# Syntax

This document specifies the surface of De Lore Is: what a `.dli` file is, what
in it is normative, the shape of a record, the two supported ways in, and the
discipline that keeps the written form from drifting away from the language it
renders. It closes by naming, plainly, which claims about the surface are
settled and which remain hypotheses awaiting measurement.

The decision ledger (`../decisions.md`) outranks this document; where they
disagree the ledger wins and this file is stale until fixed.

---

## 1. Normativity: the AST is the language

The language is defined **AST-first** (DL-0003). The normative definition is the
serializable abstract syntax tree's **JSON Schema**. The two-page EBNF is a
*rendering* of that schema, published in the same versioned document; when the
grammar and the schema disagree, the schema wins and the grammar is corrected.
The artifact the tool layer enforces *is* the language definition — there is no
second, prose definition that the schema could fall out of step with.

The schema is **generated from the implementation's own types** (schemars,
DL-0008): the Rust AST types are the single source, and the published JSON
Schema is derived from them. A definition and an enforcer that are two hand-kept
copies drift; a definition that is *computed from* the enforcer cannot. This is
the same anti-drift move the whole system makes about lore, applied to the
language's own grammar.

There is **exactly one canonical surface rendering**, defined by the formatter
and enforced by the gate. Non-canonical text — a sloppily spaced record, a
field in a different order, an idiosyncratic line break — is **always legal
input and never legal repo state**. A writer types approximately; saving
produces the one canonical form; the committed tree contains only canonical
text. This is koan 7 (*one way to write it*) made mechanical: the convenience of
loose input is paid once, at save, rather than by admitting a second surface
regime into the repository.

The grammar is **versioned**. A version bump ships a **canonical-to-canonical
migration**, and **the formatter is the migrator**: upgrading a corpus across a
grammar version is running the new formatter over the old canonical text.
Deriving the schema from the types, the surface from the schema, and the
migration from the formatter removes every seam at which the definition, the
parser, and the tool surface could come apart.

Consequences that fall out of AST-first normativity, each load-bearing
elsewhere in this suite:

- **There is no status slot in the AST.** A claim's standing is derived from its
  provenance, never stored as a field (see `02-semantics.md`). A bare `ratified`
  with no origin therefore has nowhere to go and *cannot parse* — koan 1
  (*claims come from somewhere*) enforced at the grammar rather than by a lint.
- **Agent tools render through the same formatter.** Whatever a tool constructs
  internally becomes canonical surface before it is diffed, reviewed, or
  committed (§6). The author reads one form of the language, always.
- **The reference card and the schema are the same object at two altitudes.**
  What a card shows is exactly what parses (§4); there is no lenient dialect
  hiding behind the taught one.

---

## 2. The shape of a record

A `.dli` file is a sequence of **flat, order-insensitive, line-oriented
records**. The commitments, stated once:

- **One declaration per line, or one shallow keyword-fronted block.** The unit
  of the language is a single claim on a single line. Where grouping is
  genuinely useful (an entity with its slots, a question with its options) the
  block is shallow and keyword-fronted — never deep nesting, never a tree the
  eye has to unwind.
- **Order-insensitive within a scope.** Declarations do not depend on their
  position relative to one another; a fact may precede the entity it is about.
  This is what makes nearly every edit an append or a one-line replacement, and
  what makes independent additions merge trivially in version control.
- **A closed vocabulary of roughly thirty domain keywords.** No macros, no
  synonyms, no positional shorthands, no user-defined syntactic forms. Each
  keyword is a worldbuilder's own artifact name — `era`, `place`, `person`,
  `thing`, `event`, `question`, `fact`, `rule`, `fork`, `cycle`, `ratify`, and
  their siblings. An unknown leading word is a parse error, not a silently
  tolerated extension.
- **References by name, never by position.** A relation names its participants;
  a fact names its subject; a citation names its decision. Nothing is addressed
  by ordinal position in a list, so nothing breaks when a list is reordered or
  appended to.
- **Indentation groups; it never nests deeply.** In the landed v0 grammar
  (unfrozen; the AST is normative) a leading-indented line is a *continuation or
  child* of the record above it — an entity's slots, a question's options, a
  rule's body — so a dedent ends the group and does change which record a line
  belongs to. Expert blocks are delimited by braces `{ … }` instead. The
  formatter owns the canonical amount of indentation (leading whitespace is
  otherwise insignificant, and a tab counts as one column); a writer types
  loosely and saving normalizes it. This is the one shallow, one-level grouping
  the language admits — never deep nesting, never a tree the eye must unwind.

The grammar is kept **GBNF-expressible** — line-oriented, no significant
whitespace, no context-sensitive layout — as a *cheap standing property*, not as
a commitment to ship a second grammar artifact. The reason to preserve the
property, and the reason not to pay for a maintained grammar-constrained
emission channel, both live in §6.

There is **one composition rule**: the flat record. A writer's second hour with
the language is predictable from the first, because there is no second syntax
regime waiting in an expert tier — only more keywords in the same shape. The
expert tier (§3) adds vocabulary, never grammar.

---

## 3. The gentle subset and the expert tier

The surface is split into two reference cards, and the split is a design
commitment, not a documentation convenience.

**The gentle subset is most of worldbuilding.** Declarations only: entities
(`place`, `person`, `thing`), facts and relations, timeline points, citations,
`proposed`, and `question`/`option`. This covers the actual daily work of a
fiction writer or a tabletop designer. A complete first world fits in twenty
lines of it:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

era uthers-time   "Uther's Time"    decided d-0001
era arthurs-reign "Arthur's Reign"  after uthers-time

place camelot  "Camelot"      decided d-0001
place the-lake "The Lake"     decided d-0003
place avalon   "Avalon"       proposed

adjacent(camelot, the-lake)   decided d-0003

person merlin "Merlin the Counsellor"  decided d-0002
  merlin.trade = counsellor            decided d-0002
  merlin.home  = the-lake              proposed

thing excalibur "Excalibur"       decided d-0004
  excalibur.kept-at = camelot     decided d-0004

question q-0001 "Who drew Excalibur from the stone?"
  option arthur  implies drawn-by(excalibur, arthur)
  option mordred implies drawn-by(excalibur, mordred)
```

Question bodies accept only
`option <name> ["<label>"] [implies <atom>]`; every other child
is an error rather than discarded text. Fork bodies retain their command clauses
(`suppose`, `add fact`, and the other fork operations), while malformed children
that begin with `option` are errors. The former `option ... assumes` clause is a
killed form. A hypothesis condition gates the claim itself through provenance
(`<claim> proposed assumes <hypothesis>`), not the option declaration.

`deloreis check` answers in the same register the file is written in:

> *camelot: 10 claims certain, 3 proposed, 1 open question (q-0001).
> `avalon` has no decision behind it — fine as notes; it stays proposed and
> out of canon until you decide.*

The writer's ledger is just their own decision list — `d-0001` is a line in a
file recording what they chose and when. The full ratification procedure is
house process; the *concept* the language enforces — a claim, and where it came
from — costs a solo writer one word per line.

**The expert tier lives on a second card.** Rules, forks, cycles, practices,
ledgers, and the reasoning surface are all here. The commitment is strict:
**no word from the expert tier appears on the first card.** A writer can work
for a long time entirely in the gentle subset and be using the full
provenance semantics — the dual-rail annotation algebra of `02-semantics.md` —
without ever learning that a polynomial exists. The power is present from the
first line; the vocabulary for it is introduced only when a writer reaches for
it.

A **world rule** is a bare `rule` whose body derives its head. The body is a
comma-separated conjunction of atoms, and a body literal written **`not`** is a
**negated** literal:

```dli
# grammar: v0 (unfrozen; AST normative)
scope wards

rule ward-holds  ratified d-0146
  ward-holds($g) :- seated($g), not breached($g)
```

`not breached($g)` reads *"and `breached($g)` is not settled."* Its meaning is
the certain-layer test and the defeater-poisoning of `02-semantics.md §4`: a
merely-proposed `breached` never blocks the ward — it records the risk. Two
load-time disciplines guard the form (`08-reasoning.md §1`): **range
restriction** — every variable in the head or in a negated literal must also
appear in a positive body atom, because a negated literal binds nothing — and
**stratification**, which refuses a negative-dependency cycle (`p :- not q`,
`q :- not p`) rather than evaluate it. `rule … over order` and `rule … over
season` are checker rules for other engines, not world derivation rules. A bare
top-level rule that cannot be read as Datalog emits an error-class load finding;
it never silently lowers no rule. Practice-internal `rule ... requires` remains
a practice law, not a top-level world rule.

---

## 4. Three stances at the surface

Three deliberate stances make the surface humane rather than merely small.

**Errors speak the domain (koan 9, *errors are lore*).** A diagnostic is a
world-record artifact, authored with the same care as any other surface, and it
never speaks in compiler idiom. Not *"unbound identifier"* but:

> *`avalon` needs a ruling before this can be asserted — record a decision,
> mark it proposed, or move the claim into a fork.*

Not *"no Ord instance"* but:

> *`sort` would put the leaves in a line, but the year is a cycle — there is no
> first season.*

Diagnostics carry a stable machine-readable class *and* a domain-spoken message
(the two-channel error model of the charter's tensions); the message is designed
and snapshot-tested (insta, DL-0008), because the errors are an API.

**Anti-Inform-7: what parses is exactly what the card shows.** A natural-language
surface is easy to read and hard to write — its English affordance lures authors
into arbitrary phrasings the parser cannot accept. De Lore Is refuses the lure:
it **reads like notes, not English.** The reference card is the whole grammar a
writer meets; there is no gap between the phrasing a card shows and the phrasing
the parser takes. Nobody has ever been lured into writing an unparseable
infobox, and the surface stays that close to notes on purpose.

**Proposed-first authoring.** Incompleteness is a first-class state, not an
error. A claim with no decision behind it is `proposed` — legal, retained,
visibly out of canon; an undecided fork is a `question` with `option`s, a
tracked debt rather than a blank (koan 5). A writer sketching incomplete notes
is never told they are wrong and never silently loses a line (koan 4). Choosing
truth is a separate, later, deliberate act.

---

## 5. The cognitive-dimensions ledger

The surface is accountable in the vocabulary of Green & Petre's cognitive
dimensions — used here as a tradeoff ledger, not a scorecard to pass. A review
that names no costs has not used the framework; the two paid dimensions below
are the bill.

Six dimensions the design **wins**, and how:

- **Viscosity — low by construction.** Order-insensitivity makes nearly every
  edit an append or a one-line replacement; no edit ripples through layout, and
  independent additions merge trivially.
- **Hidden dependencies — killed as a class.** Hidden dependency is this
  repository's recurring pathology (the entire propagation apparatus exists
  because prose canon hides its dependencies). Every claim carries its
  citations, and `--why`/blame render the dependency graph on demand. A
  dependency you can always ask for is not hidden.
- **Premature commitment — removed at every axis.** Proposed-first authoring
  means a writer never chooses truth before writing; `question`/`option` defer
  world commitments into tracked debts; order-insensitivity removes commitment
  to layout.
- **Closeness of mapping — one line.** The keywords are the worldbuilder's own
  artifact names; the record mirrors the infobox, the timeline bullet, the
  decision log. The distance between the writer's mental object and its notation
  is a single line.
- **Consistency — one shape.** One composition rule means the second hour of use
  is predictable from the first; there is no expert syntax regime, only more
  keywords in the same shape.
- **Error-proneness — designed out, not linted out.** The closed vocabulary
  makes unknown words parse errors; no synonyms, no positional arguments, no
  significant whitespace, no optional shorthands. The constructs the framework
  flags as error-generating are excluded structurally.

Two dimensions where the design **deliberately pays**:

- **Diffuseness.** One claim per line is verbose — ten facts are ten lines —
  and this is bought on purpose, in exchange for per-line blame and clean
  semantic diffs. A change to one claim touches one line, and its provenance is
  attached to that line.
- **Abstraction gradient — deliberately flat.** There are no user-defined
  abstractions at all. Power users will feel the absence of macros; the budget's
  answer is that the domain licenses *records, not macros*. Lore is a finite set
  of claims about a world, and a claim is not a function.

---

## 6. The two doors (retired: one interface)

*Retired semantic (DL-0041, DL-0043).* This section defined the v1 two-door
model; it is retained because its drift-discipline and demoted-condition rules
survive. The standing law is koan 11, *one way in*: a single declarative
interface for every author, with authority — not channel — distinguishing
them. The text surface described below is an opt-in rendering since DL-0041.

The count of supported paths — not any single path's quality —
predicts maintenance load and failure surface; every additional path is a
support surface, a drift surface, and a bust-rate multiplier. That argument
survives the doors it once counted: it is why the interface count is one.

**The text surface (historical Door H) — humans write surface text.** The formatter (write
loosely, save canonical), the graded error ladder speaking the domain (§4), the
gentle subset, and the reference cards. Everything, whatever path produced it, is
**reviewed as surface** — the author reads one form of the language, always. A
nicer keyboard is not a new door: a click-to-edit form over lore entities in a
factory UI serves the human population and is Door H with a different keyboard,
its output surface text reviewed as surface.

**the tool surface — agents author through semantic tools (DL-0014).** Agents do not write
`.dli` text as their supported path; they call a curated set of semantic tools
registered over MCP against the `deloreis` server — `author.open`,
`author.add_fact`, `author.add_entity`, `author.add_relation`, `author.add_event`,
`author.add_before`, `author.open_question`, `author.suppose`, `author.diff`,
`author.materialize`, and their siblings. Every mutating call returns, at once:
**graded findings** (with repair data), the **rendered canonical lines**, and the
**workspace digest**. `author.materialize` returns `.dli` contents *as data*; the
calling agent commits them through its own review. The full tool surface is
specified in `11-agent-authoring.md`.

**Raw AST-document emission is a mechanism, not a door.** A frontier model can be
constrained to emit a schema-valid AST at its decode layer, and that capability
is used *inside* the tool surface's tools and as a benchmark arm — but raw AST emission is
not itself a supported authoring channel. It solves syntactic validity only:
wrong-but-well-formed content (invented names, bad citations, contradicted
claims) passes the decoder and dies in the semantic gates, exactly as it should.

**The demoted conditions, named plainly.**

- **Textual agent editing of existing `.dli` files** is a **measured fallback**,
  not a channel. It happens incidentally — an agent with a text editor will
  sometimes edit text — and the gate treats it identically to any other input:
  same pipeline, same graded errors. The skill file does not teach it, no tooling
  is built for it, and no reliability claim is made for it. Its standing role is
  the benchmark's control condition and the standing measurement of what happens
  off-interface.
- **Grammar-constrained local emission** is a **benchmark condition**, not a
  shipped channel. No maintained grammar artifact is kept under an equivalence
  tax for an unwalked path. What is kept costs nothing: the grammar stays
  line-oriented and GBNF-expressible (§2), so the option remains cheap to reach
  if evidence ever calls for it.

**The promotion trigger is explicit and double-sided.** A demoted condition
becomes a supported path only on evidence that the tool surface *fails* for named local
models the owner actually uses **and** that constrained surface *passes* on the
same tasks — **plus** a written argument against the interface budget. Paths and
reasoning seats are the same species of scope creep and follow the same protocol.

**Off-interface input meets the full pipeline.** The bounded interface limits what is
*supported* — taught, tooled, measured — and never authorizes a silent drop. An
agent's textual edit arriving off-interface is neither refused nor ignored; it meets
the same graded errors as everything else (koan 4, *nothing is silently
dropped*). The gate stays total; an unsupported path is a support decision, never a
data-loss decision.

**The benchmark existed to close routes.** Its output was a verdict about which
route to shut or keep shut — never a menu of options to offer. It is retired
with the semantic it measured (DL-0041, DL-0043).

---

## 7. Drift discipline

The surface has three standing defenses against the definition, the examples,
and the corpus drifting apart.

**CI-compiled examples.** Every `.dli` example in every document in this suite,
and in the skill, is wired into a compile harness: an example either parses
cleanly against the current grammar or the gate is red (the docs-oracle runs the
real parser over every block). Documentation cannot rot into showing forms the
language no longer accepts, because a rotted example fails the build. (The
grammar is at v0 — unfrozen, with the AST normative; the marker on the first
line of every `.dli` block records exactly that.)

**The killed-form registry.** A surface form that is retired — a spelling the
language once accepted and no longer does — is *registered*, not forgotten. The
parser rejects a killed form with a pointer to its replacement rather than a
bare error, and never silently re-accepts it. This is the same discipline the
lore layer applies to killed names and banned vocabulary: history is the
guardrail corpus, and the machine refuses a quiet return.

**Versioned grammar, formatter as migrator.** A grammar version bump ships a
canonical-to-canonical migration; the formatter *is* that migration (§1;
DL-0003). Before a version ships, its migration is rehearsed on a synthetic
version bump — the AST schema diff across the bump must be empty where no change
was intended — so the migrator is exercised before any real corpus meets it.

**The gate is deterministic and offline (DL-0013).** The compile-examples
harness, the formatter idempotence check, and the killed-form checks all run
with no network and no model in the loop. The authoring benchmark and any
model-in-the-loop lane are **non-gating**: they informed the now-retired route verdict, they do
not block a merge.

---

## 8. What is unvalidated

The surface stands on a published basis, and it is honest about the edge of that
basis.

**The basis is prior art, accounted once elsewhere.** Word keywords over
punctuation, the cognitive-dimensions method (§5), the language's named position
in the DSL pattern space, the natural-programming footing under "structured
notes," and the fiction-language lineage that places this language as a third
family — *knowledge, not simulation or flow* — are the ground under the surface.
Each cited line of published work licenses specific named decisions and nothing
more; the accounting lives in `../design-notes/prior-art-licenses.md` and is not
restated here.

**No study of *this* grammar exists** — for humans or for agents. The basis sets
the *direction* of the surface (evidence-oriented word-based design); it does not
*validate* this particular surface. The regrounding narrows the bet; it does not
discharge it.

**Two instruments, planned and not presumed:**

1. The **P0 authoring benchmark** — the agent-side instrument. Its pass is
   defined by the *real gate's verdict*, not by a proxy score. Its matrix is the
   two routes against a set of models, plus the two demoted conditions as
   experimental arms. Its output was the route verdict of §6 (retired).
2. A **small human-writer probe** taken at the G1 milestone, before the surface
   freezes — a handful of non-programmer writers, the gentle subset, comprehension
   and transcription tasks, with a cognitive-dimensions walkthrough as the
   interview scaffold. G1 is the last point at which a surface change is cheap;
   after it, a change is a versioned migration.

Until both instruments report, **every approachability claim in this document is
a hypothesis with good priors**, and is written as one. The surface freeze
(recorded as a decision when it lands) is gated on the human-writer probe having
run first.
