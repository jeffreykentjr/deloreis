# Syntax reference — the complete `.dli` rendering grammar

This sheet documents the `.dli` text surface for writing and reading lore.
The default build includes its parser and formatter, so people can check,
format, import, and query `.dli` documents. Agents use the `author.*` tools
(`authoring-loop.md`). Stored fact objects and their JCS serialization remain
the canonical form; `author.materialize` returns committed objects by default
and can also return `.dli` rendering on request.

The syntax implementation lives in `crates/dli-syntax`. Builds made with
`--no-default-features` omit the text parser and formatter.

Retired keywords and line forms are refused with a message naming their
successor.

## 1. The frame

| Construct | Form | Notes |
|---|---|---|
| Scope header | `scope <name>` | First line of a file; names the scope. |
| Comment | `# …` | Whole-line only. |
| Record | one declaration per line | Flat; order-insensitive between records. |
| Continuation | leading indent under a header line | Groups child lines (entity slots, question options, rule bodies) into one record. **A dedent ends the group — the parser is indent-tolerant and the formatter owns layout**. Tab counts as one column. |
| Expert block | `keyword … { … }` … `}` | Brace-delimited interiors for `practice`, `proof`, `dialect`, `ratify`. Braces inside string labels are plain text. |

## 2. Provenance (trails every claim-bearing line)

```text
<claim>   proposed [origin-token]
<claim>   proposed-bundle "scope#number"
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
- `proposed-bundle "scope#number"` preserves an anonymous proposal's exact
  identity on text replay; the scope may be empty for scope-less source. It cannot
  be combined with origin tokens,
  or `assumes`.

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

### Rules and principles

| Keyword | Line form(s) |
|---|---|
| `rule` | `rule <id> [over order\|season] [severity <sev>] [marker]` + indented `: <clause>` / `requires <clause>` body (body clauses are captured text; atoms extracted via the one clause→atom bridge). Bare = world rule; `over order` constrains a timeline; `over season` is a season-ledger rule |
| `rule` (world, datalog) | a bare `rule` derives its head from a `<head> :- <lit>, <lit>, …` body; a body literal `not <atom>` is **negated** (tests the certain layer, poisons the conclusion's defeaters — spec/02 §4). Range-restricted (every head/`not` variable also in a positive atom) and stratified (no negative-dependency cycle). A bare rule that is not valid Datalog is a load error, never an empty lowering — spec/08 §1 |
| `principle` | `principle <id> …` (declaration body captured; no craft checker is provided) |

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
| `draft` | `draft <id> …` (draft declaration) |
| `attest` | `attest <id> …` (attestation reference) |
| `export` | `export <target> …` (profile reference) |
| `ratify` | `ratify <id> { … }` (ruling record block) |
| `variant` | `variant <id> …` (ruleset delta) |

## 4. Stored form

Declarations carry structured data into the lorestore. The parser reads the
text into that structure; tools can then inspect rules, practices, proofs, and
other declarations without treating their bodies as arbitrary text.

## 5. Not rendered yet

**Episodes** (production order, scenes, and knowledge deltas) have no `.dli`
line form on this sheet.

## 6. Where to look deeper

- `docs/cards/authoring-loop.md` — how lore is actually added: the tool calls, the fact objects, the commit.
- `docs/cards/gentle-card.md` — the teaching subset (what a writer meets first).
- `docs/cards/expert-card.md` — the expert tier, its tool calls and its stored form.
- `docs/spec/01-syntax.md` — normativity, the record shape, the interface, drift discipline.
- `schemas/lorestore/fact-object-v1.json` — the canonical stored form this grammar renders.
- `schemas/ast/document.schema.json` — the AST the parser produces from a rendering.
- `examples/*.dli` — the living corpus (all gate-parsed).
