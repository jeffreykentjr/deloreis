---
status: live
title: "Syntax reference — the complete .dli rendering grammar"
verdict: "Every keyword and line form the .dli rendering uses, in one sheet: what the renderer emits and what the optional parser accepts. It is a reading surface, not an authoring one — lore is authored through the author.* tools."
---

# Syntax reference — the complete `.dli` rendering grammar

This sheet documents **the rendering**. `.dli` is one way of printing a world
for human eyes; it is not how lore is authored and not what the system stores.
Authoring happens through the `author.*` tool surface (`authoring-loop.md`), the
canonical form is the stored fact objects and their JCS byte serialization
(DL-0041), and a `.dli` rendering is an explicit opt-in on the way out
(`rendered: true`, DL-0046). Reach for this sheet when you are *reading* a
rendered diff or a bundled example and want to know exactly what a line means.

Hand-compiled from `crates/dli-syntax/src/parser.rs`, which is the round trip's
other half — the reader for the same grammar the renderer writes. That crate is
**feature-gated out of the default build** (roadmap LS-7): a default `deloreis`
binary links no parser and no formatter, serves fact objects, and returns an
empty `rendered` array. Everything on this sheet describes what a
`syntax`-enabled build prints and reads.

The grammar is **v0 and will not be frozen** — the freeze milestone (SURF-4) is
retired, not deferred (DL-0041), because a rendering has no surface contract to
freeze. What replaced the freeze is the killed-form discipline: a retired
keyword or line form is registered with its successor and refused by name, never
silently dropped (DL-0029 as relocated by DL-0041).

## 1. The frame

| Construct | Form | Notes |
|---|---|---|
| Scope header | `scope <name>` | First line of a file; names the scope. |
| Comment | `# …` | Whole-line only. |
| Record | one declaration per line | Flat; order-insensitive between records. |
| Continuation | leading indent under a header line | Groups child lines (entity slots, question options, rule bodies) into one record. **A dedent ends the group — the parser is indent-tolerant and the formatter owns layout** (autoformat guarantee, DL-0016; spec/01 §2). Tab counts as one column. |
| Expert block | `keyword … { … }` … `}` | Brace-delimited interiors for `practice`, `proof`, `dialect`, `ratify`. Braces inside string labels are plain text. |

## 2. Provenance (trails every claim-bearing line)

```text
<claim>   proposed [origin-token]
<claim>   decided d-NNNN [d-NNNN…]
<claim>   ratified d-NNNN
<claim>   provisional d-NNNN
<claim>   … assumes <hyp-or-option>
```

- A claim with **no marker** parses but warns (`origin-missing`) and is
  **fail-closed at load** (support = FALSE — it can never be Certain).
- `proposed` needs no citation; `decided/ratified/provisional` without a
  `d-ref` is a diagnostic and fails closed.
- One marker per line; extras diagnose (`conflicting-markers`).
- `assumes` conjoins a hypothesis/option condition onto the claim's support.

## 3. Keyword inventory by family

### World structure

| Keyword | Line form(s) |
|---|---|
| `era` | `era <id> ["Label"] [marker]` — or the chain: `era a < b < c [marker]` (spine order; provenance applies to the chain) |
| `place` / `person` / `thing` | `<kw> <id> ["Label"] [marker]` — slots as indented children: `  <id>.<key> = <value> [marker]` |
| `event` | `event <id> ["Label"] (after\|during\|since\|until) <anchor> [marker]` |
| `fact` | `fact <subj> <pred> <obj> [marker]` (triple) — also the slot form `<entity>.<key> = <value>` and the relation form `<pred>(<a>, <b>)`, both bare (no `fact` keyword needed) |
| `before` | `before <event-a> -> <event-b> [marker]` (declaring an order is a ruling) |
| `use` | `use <scope>` |
| `kind` | `kind <name> [= v1 \| v2 \| …]` |
| `functional` | `functional <pred> [marker]` — declares `pred` single-valued (a functional dependency): one certain value per subject |
| `narration` | `narration vantage <era>` (inert vantage; one per corpus) |

`functional` states a constraint, not a fact — it lowers to no atom, and there is
nothing to query. What it changes is the check: two distinct certain values for
one subject of a declared predicate raise `continuity.fd-violation`. Undeclared,
single-valuedness is inferred from canon's own precedent, which cannot judge a
first-seen predicate — the declaration is how an author settles that case.

```dli
functional ruler-of              ratified d-0146

ruler-of(camelot, arthur)        ratified d-0146
```

### Questions, forks, hypotheses

| Keyword | Line form(s) |
|---|---|
| `question` | `question <id> ["prompt"]` with children `  option <id> ["label"] [implies <atom>]` |
| `option` | child of `question`/`fork` (column-0 also folds into the preceding question) |
| `fork` | `fork <id>` with `  option …` children |
| `hypothesis` | `hypothesis <id>` (mints the fork-scoped supposition variable) |

Question bodies contain options only. Fork bodies may also retain fork commands
such as `suppose` and `add fact`; a malformed line beginning with `option` is
still an error. `option ... assumes` is a killed form: hypothesis gating belongs
on the provenance of the claim being gated, for example
`<claim> proposed assumes <hypothesis>`.

### Rules and doctrine

| Keyword | Line form(s) |
|---|---|
| `rule` | `rule <id> [over order\|season] [severity <sev>] [marker]` + indented `: <clause>` / `requires <clause>` body (body clauses are captured text; atoms extracted via the one clause→atom bridge). Bare = world rule; `over order` constrains a timeline; `over season` is a season-ledger rule |
| `rule` (world, datalog) | a bare `rule` derives its head from a `<head> :- <lit>, <lit>, …` body; a body literal `not <atom>` is **negated** (tests the certain layer, poisons the conclusion's defeaters — spec/02 §4). Range-restricted (every head/`not` variable also in a positive atom) and stratified (no negative-dependency cycle). A bare rule that is not valid Datalog is a load error, never an empty lowering — spec/08 §1 |
| `principle` | `principle <id> …` (craft KB; interior captured) |

### Time

| Keyword | Line form(s) |
|---|---|
| `state` | `state <entity>.<key> = <value> (since\|until\|during) <anchor> [marker]` |
| `cycle` | `cycle <name>(<spokes>) [= s1 \| s2 \| …] [marker]` (cyclic kind: succ/pred/across/distance; no epoch, no `<`) |

### Practice (magic-system workbench, expert tier)

`practice <id> [marker] { … }` — interior keywords, one per line:
`use`, `kind`, `role` (`role <id> : <kind> requires <conds>`), `resource`
(`resource <id> : linear conserved | affine bounded <n> | graded monotone
<dir>`), `rule` (`… :- <clauses>`), `condition`, `error` (`error <id> refines
L1–L7 <discriminant> [where …]`), `effect`, `cost`.

`proof <id> speaker <who> in <practice> { … }` — interior:
`premise <id> <atom> cited d-NNNN [cue "…"]`, `step <id> instantiate <rule>
with <bindings>`, `spend <premise>`, `close <atom> expect
holds|fails-at <error>`.

### Language layer

| Keyword | Line form(s) |
|---|---|
| `dialect` | `dialect <id> [marker] { … }` (name-parts/weights interior) |
| `name-part` | `name-part <id> …` (cited fact of the dialect) |
| `word` | `word <id> …` |
| `name` | `name <id> …` + composition children (typed derivation citing dialect + slots) |
| `term` | `term <id> …` (coined term) |
| `banned` | `banned <token> …` (lexicon ban) |

### Records of process

| Keyword | Line form(s) |
|---|---|
| `draft` | `draft <id> …` (payload anchor; fat payloads ride as data) |
| `attest` | `attest <id> …` (attestation reference) |
| `export` | `export <target> …` (profile reference) |
| `ratify` | `ratify <id> { … }` (ruling record block) |
| `variant` | `variant <id> …` (ruleset delta) |

## 4. Everything here is typed

Top-level line shapes, entity slots, facts (all three forms), provenance, time
clauses, cycles, states, question/option/implies, before, and era chains have
always been fully typed. Since LS-9 (DL-0047, DL-0049) **the interiors are too**:
every string-bodied declaration type — `rule`, `practice`, `proof`, `dialect`,
`name-part`, `name`, `ratify`, and the near-empty `fork` / `draft` / `export` /
`variant` / `principle` bodies — carries typed structure into the stored fact
object's `expert_body`, with its normalized clause text kept beside it so the
rendering round-trips byte-identically.

The practical consequence: no subsystem re-parses a stored declaration body to
check it. A proof is validated structurally, not by formatting and re-reading.
One surface-parse coupling remains at the load/check boundary
(`parse_clause_atom` over payload-claim and runtime-fact atoms in `dli-world`),
and retiring it is LS-7 work.

## 5. Not rendered yet

**Episodes** (production order, scenes, knowledge deltas) ride as data payloads
(a world's episodes YAML) and have no line form on this sheet until their
rendering lands with the `rule … over order` work (TIME-3).

## 6. Where to look deeper

- `docs/cards/authoring-loop.md` — how lore is actually added: the tool calls, the fact objects, the commit.
- `docs/cards/gentle-card.md` — the teaching subset (what a writer meets first).
- `docs/cards/expert-card.md` — the expert tier, its tool calls and its stored form.
- `docs/spec/01-syntax.md` — normativity, the record shape, the interface, drift discipline.
- `schemas/lorestore/fact-object-v1.json` — the canonical stored form this grammar renders.
- `schemas/ast/document.schema.json` — the AST the parser produces from a rendering.
- `examples/*.dli` — the living corpus (all gate-parsed).

## 7. Surface-review items — resolved by the surface-v1 pass (DL-0016)

Filed 2026-07-19 from the author's first review pass; **all three were resolved
by the does-what-it-says rename (DL-0016, 2026-07-20)**:

1. **`pack`** — **resolved: renamed to `scope`.** The header keyword and `use`
   target both read `scope` now.
2. **`fluent`** — **resolved: renamed to `state`.** The whole rename set landed
   with it (`wheel`→`cycle`, `ledger`→`proof`, `morpheme`→`name-part`,
   `lexeme`→`word`, `register`→`dialect`, `sort`→`kind`), and the rule family
   unified under one `rule … over <domain>` keyword.
3. **Indentation as load-bearing** — **resolved: the autoformat guarantee.**
   The parser stays indent-tolerant; the formatter owns all layout, and no
   diagnostic ever asks a writer to fix indentation (spec/01 §2).
