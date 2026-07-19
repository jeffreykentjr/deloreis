---
status: resolved
title: "Surface v1 naming — the does-what-it-says pass"
verdict: "RULED 2026-07-19: the full rename set is approved; pack becomes scope; the rule family unifies as one rule keyword with over <domain>. The rename wave executes against this document."
---

> **RULING (the author, 2026-07-19).** The recommended rename set is approved
> in full: `fluent`→`state`, `wheel`→`cycle`, proof-block `ledger`→`proof`,
> `morpheme`→`name-part`, `lexeme`→`word`, `register`→`dialect`,
> `sort`→`kind`. `pack` is **renamed to `scope`** (the header stays; option
> (a)). The rule family **unifies**: one `rule` keyword with `over <domain>`
> (`over order`, `over season`; bare = world rule), retiring `order-rule` and
> `craft` as keywords. Indentation stays with the autoformat guarantee. Old
> words become killed forms with renamed-to diagnostics. Ledger entry lands
> with the rename commit.

# Surface v1 naming — the does-what-it-says pass

**The author's review verdict (2026-07-19, verbatim in substance):** the
keyword terminology lacks coherence — `fluent`, `wheel`, `craft` are
non-obvious; compound words are rare yet `order-rule` is one; `morpheme` and
`lexeme` are questionable for non-experts; `register` is borderline; `pack`
is unjustified. Indents are acceptable **if autoformatted**. Overall: too
cute, too many nods to the fantasy world; the whole syntax should read
"does what it says." First-drafty; needs tweaking.

**The naming principle this proposal applies everywhere:**

> A reserved word says what the line does, in the plainest word a working
> fiction writer already owns. No linguistics or CS terms of art on the
> writer path. No fantasy flavor in the machinery — the flavor belongs to
> the world being described, never to the language describing it. Compound
> words only where a single word genuinely does not exist.

## The census — every reserved word, judged

### Keep (already does what it says)

`era place person thing event fact before question option hypothesis use
draft attest export ratify variant banned principle role resource condition
error effect cost premise step spend close proposed decided ratified
provisional assumes severity requires implies since until during after
narration vantage speaker in with where refines expect`

These stay. They are the reason the gentle card works.

### Rename — recommended set

| Current | Proposed | Why |
|---|---|---|
| `fluent` | **`state`** | "A state of the world that holds for a span." `state gwern.water-status = commons until enclosure-of-the-parish-water` reads as English. `fluent` is AI-planning jargon. |
| `wheel` | **`cycle`** | It IS a cycle. `cycle tide(4) = ebb \| slack \| flood \| high`. "Wheel" was a fantasy nod. |
| `ledger` (in-practice proof block) | **`proof`** | The block is a diegetic proof being checked: `proof e01-s04 speaker protagonist in spoken-proof { … }`. Also ends the collision with "the decision ledger." |
| `morpheme` | **`name-part`** | What it is to a writer: a part names are built from. Linguistics jargon dropped. (One compound, earned: no single plain word exists.) |
| `lexeme` | **`word`** | A word of the dialect. If review finds `word` too generic in context, fold it into `term` entirely — the two overlap today. |
| `register` | **`dialect`** | Borderline term of art → the plain word for "how this region/culture speaks." (`voice` is the alternate if dialect reads too regional.) |
| `pack` | **`scope`** — or drop the header | The honest options: (a) `scope saltmarket` — names what the file's claims are about; (b) NO header keyword at all — the filename is the namespace, cross-file accrual by matching name. (b) is the most does-what-it-says (one less word to learn); (a) keeps multi-scope files possible. Recommendation: **(b)**, with `scope` as the fallback if multi-scope files matter. |
| `sort` | **`kind`** | `sort` is logic jargon; `kind <name> = a \| b` says it plainly. |

### Unify — the rule family (removes `order-rule` and `craft`)

Three keywords today express "a rule the checker enforces": `rule`,
`order-rule`, `craft`. Proposal: **one keyword, `rule`, with its domain
stated in the form** —

```text
rule accomplishment-earned  over season   severity error
  : beat $b kind accomplishment magnitude >= major
    requires beat $f  thread($f) = thread($b)

rule full-team-floor  over order   ratified D-0162
  : listener-has(configuration full-team) requires slot >= 10

rule ward-form(ground, fibre)  ratified D-0146 :- ...        # world rule, unchanged
```

`over order` / `over season` route to the order-rules and season-ledger
engines respectively; bare `rule` stays a world rule. This kills the lone
compound word AND the non-obvious `craft`, and the checker's dispatch is
explicit instead of keyword-encoded. (Fallback if `over` reads wrong:
`rule … for order`.)

### Indentation — kept, with the autoformat guarantee made explicit

The parser stays indent-tolerant (write sloppy); the formatter owns ALL
layout (save canonical) — tabs normalized, alignment produced, never
hand-maintained. Commitment: no diagnostic ever asks a writer to fix
indentation; the formatter does it. spec/01 §2 gets this as a stated
guarantee; the gate already enforces canonical form only on committed files.

## What a ruling triggers

One rename wave, while the grammar is unfrozen and cheap: parser keyword
table, formatter, the corpus (`examples/`), the cards, spec dli-blocks,
SKILL.md, the AST serde names where surface-visible, and the killed-form
registry gets the OLD words (so `fluent`, `wheel`, `pack`… are refused with
a "renamed to…" diagnostic, not silently accepted). The formatter-as-migrator
does the mechanical work; the docs-oracle proves every example still parses.

## Open for the author

1. The recommended set above — approve / amend per row.
2. `pack`: drop-the-header (b) or `scope` (a)?
3. The rule-family unification: one `rule` with `over <domain>`, or keep
   three keywords with better names?
