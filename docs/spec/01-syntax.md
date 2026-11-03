---
status: live
title: "Syntax"
verdict: "Stored fact objects are schema-defined; the default build also ships the .dli text toolchain and MCP authoring tools."
---

# Syntax

This document specifies the `.dli` text surface and its relationship to stored
fact objects. The default build includes checking, formatting, querying, and
importing `.dli` documents. The MCP authoring tools offer a structured route for
agent sessions.

The decision ledger (`../decisions.md`) outranks this document; where they
disagree the ledger wins and this file is stale until fixed.

---

## 1. Normativity: fact objects are the language

The language is defined **schema-first** (DL-0003 as amended by DL-0041). The
normative definition is the schemars-derived JSON Schema of stored fact objects,
with RFC 8785 JCS bytes as canonical form. The AST and `.dli` grammar are
compatibility/rendering mechanisms; when either disagrees with the stored-object
schema, the schema wins and the renderer or parser is corrected.

The schema is **generated from the implementation's own types** (schemars,
DL-0008). A definition and an enforcer that are two hand-kept copies drift; a
definition computed from the types does not. This is the same anti-drift move
the whole system makes about lore, applied to its own data model.

The formatter provides a stable `.dli` rendering. Stored knowledge is kept as
canonical fact objects; text documents are supported input and an optional
rendering, not the bytes used as the store's canonical form.

The grammar remains versioned for compatibility. Retired text forms are refused
through the killed-form registry with a named successor; stored fact-object
schema evolution remains the canonical migration path.

Consequences that fall out of AST-first normativity, each load-bearing
elsewhere in this suite:

- **There is no status slot in the AST.** A claim's standing is derived from its
  provenance, never stored as a field (see `02-semantics.md`). A bare `ratified`
  with no origin therefore has nowhere to go and *cannot parse* — koan 1
  (*claims come from somewhere*) enforced at the grammar rather than by a lint.
  This also applies to provenance nested in `name` and `dialect` bodies;
  missing citations there produce the same error on the enclosing source line.
- **Proposal bundles replay by identity.** `proposed-bundle "scope#number"`
  names an existing anonymous proposal hypothesis. The renderer uses this form
  when reconstructing positional proposals; it cannot be combined with
  provenance tags or `assumes` (DL-0113).
- **Origins remain origins.** When an origin token resembles a positional
  bundle or cannot be recovered from a support expression, its declaration is
  stored alongside the fact so replay retains the caller's origin (DL-0115).
- **Agent tools lower to fact objects.** A client may request rendering, but
  structured diff rows and fact objects are the review and storage surface.
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

A **`functional`** declaration states a constraint on a predicate rather than a
fact about the world: `functional <pred>` says that predicate is a *functional
dependency* — each subject holds at most one certain value for it.

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

functional ruler-of                ratified d-0146

ruler-of(camelot, arthur)          ratified d-0146
```

A second, different certain `ruler-of` for `camelot` is then a
`continuity.fd-violation` — raised against canon's own certain layer, not only
against an incoming claim (`02-semantics.md §4`). Undeclared, the checker infers
single-valuedness from precedent: a predicate that carries at most one value for
every subject it appears on is treated as functional. That inference is blind
exactly where it is most wanted — the *first* use of a predicate, where two values
may equally be a slip or a genuinely multi-valued relation. Deciding between them
is a ruling about the world, and the engine does not make rulings (koan 3); the
declaration is how an author makes it. It lowers to no atom — there is no fact
"`ruler-of` is functional" to query — and rides, with its provenance, on the
stored declaration. Agents declare it with `author.declare_functional`
(`11-agent-authoring.md §3.11`; contract `1.6.0`, DL-0074).

A **`same-as`** declaration states a claim about the world rather than a
constraint on a predicate: `same-as <left> = <right>` says two names denote
one bearer.

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

same-as avalon = the-isle-of-glass    ratified d-0146
```

Unlike `before`, the relation is undirected — `left` and `right` name the two
sides of one claim, not an earlier/later pair — so there is no canonical
bearer and no normative weight on argument order (a deliberate rejection of a
"canonical name" design: it would put a trap for an LLM author in the argument
order it wrote). The engine closes it symmetric-transitively over synthesized
rules, never by rewriting anything stored; `02-semantics.md §9` is the full
identity semantics — closure, the claim-vs-bearer distinction, defeasibility,
and the v1 temporal boundary (a `same-as` naming a declared era or event is
refused, loud, rather than half-merging the chronology). Like every other
world claim it needs provenance — a bare `same-as` with none is fail-closed,
exactly as an unprovenanced `before` is. The kernel semantics land first
(DL-0078); the typed wire op that lets an agent assert it through the taught
`author.*` surface (DL-0014, DL-0043) is a following, separately-tracked
addition.

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

## 6. Text and structured authoring

A writer can check, format, and import `.dli` documents with the default CLI.
The same parser and deterministic checks apply regardless of who produced the
text. The [gentle card](../cards/gentle-card.md) introduces the format.

For agent sessions, the supported authoring workflow uses the `author.*` MCP
tools. They return structured findings and workspace state; `author.materialize`
commits accepted fact objects, with rendered text available on request. See
[agent authoring](11-agent-authoring.md) and `SKILL.md` for the current workflow.

Neither path makes a model call in the kernel. A well-formed claim can still be
possible, contradictory, or unsupported; the semantic checks report that state.

---

## 7. Drift discipline

The surface has three standing defenses against the definition, the examples,
and the corpus drifting apart.

**CI-compiled examples.** Every `.dli` example in every document in this suite,
and in the skill, is wired into a compile harness: an example either parses
cleanly against the current grammar or the gate is red (the docs-oracle runs the
real parser over every block). An example that produces an error-class
diagnostic fails the gate.

**The killed-form registry.** A surface form that is retired — a spelling the
language once accepted and no longer does — is *registered*, not forgotten. The
parser rejects a killed form with a pointer to its replacement rather than a
bare error, and never silently re-accepts it. This is the same discipline the
lore layer applies to killed names and banned vocabulary: history is the
guardrail corpus, and the machine refuses a quiet return.

**Deterministic offline gate.** Example parsing, formatter checks, and the
killed-form checks run without a network or model call in the gate.

---

## 8. What is unvalidated

The syntax has executable examples and parser tests, but those do not establish
that every writer finds it easy to read or use. The human-writer probe in
[`probe-protocol.md`](../cards/probe-protocol.md) describes a way to collect
that feedback. No claim of a completed study follows from the protocol.
