---
status: live
title: "Syntax reference — the complete v0 surface"
verdict: "Every keyword and line form the parser accepts today, in one sheet; compiled from the parser for the author's surface review; superseded by the generated grammar page when docgen grows it."
---

# Syntax reference — the complete v0 surface

Hand-compiled from `crates/dli-syntax/src/parser.rs` (the implementation) for
the author's surface review. The AST schema
(`schemas/ast/document.schema.json`) is normative; the grammar is **v0,
unfrozen** — every word on this sheet is changeable until the freeze (SURF-4),
and renames are cheap now, expensive after.

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
| `narration` | `narration vantage <era>` (inert vantage; one per corpus) |

### Questions, forks, hypotheses

| Keyword | Line form(s) |
|---|---|
| `question` | `question <id> ["prompt"]` with children `  option <id> [implies <atom>]` |
| `option` | child of `question`/`fork` (column-0 also folds into the preceding question) |
| `fork` | `fork <id>` with `  option …` children |
| `hypothesis` | `hypothesis <id>` (mints the fork-scoped supposition variable) |

### Rules and doctrine

| Keyword | Line form(s) |
|---|---|
| `rule` | `rule <id> [over order\|season] [severity <sev>] [marker]` + indented `: <clause>` / `requires <clause>` body (body clauses are captured text; atoms extracted via the one clause→atom bridge). Bare = world rule; `over order` constrains a timeline; `over season` is a season-ledger rule |
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

## 4. Typed vs captured

Top-level line shapes, entity slots, facts (all three forms), provenance,
time clauses, cycles, states, question/option/implies, before, and era
chains are **fully typed** in the AST. The **interiors** of `rule` bodies,
`practice`/`proof`/`dialect`/`ratify` blocks are **captured clause text**
(normalized, lossless, round-tripping); each owning subsystem structures its
own interior, and `dli_syntax::parse_clause_atom` is the single bridge from
clause text to typed atoms.

## 5. Not surface yet

**Episodes** (production order, scenes, knowledge deltas) ride as data
payloads (a world's episodes YAML, kept beside its `.dli`) until their surface
form lands with the `rule … over order` work (TIME-3).

## 6. Where to look deeper

- `docs/cards/gentle-card.md` — the teaching subset (what a writer meets first).
- `docs/cards/expert-card.md` — the expert tier with parsing examples.
- `docs/spec/01-syntax.md` — normativity, the record shape, the doors, drift discipline.
- `schemas/ast/document.schema.json` — the normative AST.
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
