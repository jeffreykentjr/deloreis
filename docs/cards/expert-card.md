---
status: live
title: "Expert-tier reference card"
verdict: "The second card: the expert vocabulary — rules, forks, cycles, practices, proofs, dialects, states — reached through the same author.* tool surface, stored as fact objects with typed interiors, and rendered in the same flat shape as the first card."
---

# The expert tier — reference card

This is the second card. It adds **vocabulary, never grammar** (spec
`../spec/01-syntax.md` §2–§3): every form below is the same flat, order-
insensitive record you already know, and every claim still ends by saying where
it came from. The expert tier is not a second interface and not a second syntax
regime — it is more kinds of claim in the same store, and where a form has an
interior it stays shallow, never a deep tree.

Two things carry over from the gentle card and are worth restating, because the
expert forms are where people expect them to break:

- **The interiors are typed, not text.** Since LS-9 (DL-0047, DL-0049) an expert
  declaration's body rides in its stored fact object as a structured
  `expert_body`, not as captured source. That is why a `proof` is *checked as
  data* — structurally, over its typed premises and steps — instead of being
  formatted and re-read.
- **The tool surface reaches most of this card today, and that is worth being
  precise about.** Forks, hypotheses, and suppositions are first-class calls
  (`author.open {fork}`, `author.add_hypothesis`, `author.suppose` — worked
  below). `dialect`, `name-part`, `name`, `term`, `banned`, `state`, and
  `attest` each have a per-declaration `author.*` tool since contract 1.3.0
  (`author.add_dialect`, `author.add_name_part`, `author.add_name`,
  `author.add_term`, `author.add_banned`, `author.add_state`,
  `author.add_attest` — `SKILL.md` §2) — author them the ordinary way, not
  by loading a corpus. Only `rule`, `cycle`, `practice`, and `proof` have no
  per-declaration `author.*` tool yet; they remain **tier 3**, deliberately
  parked until a consumer needs them (DL-0058), and enter a world when its
  corpus is loaded. That is a missing tool, not a second interface: there is
  still exactly one way in, and hand-edited text carries no reliability claim
  (DL-0043).

Nothing on this card is needed to build a world; the gentle card
(`gentle-card.md`) is most of worldbuilding. Reach here when you want rules that
check, forks that explore, cyclic time, magic systems, or a constructed
language. Each lane has a full spec under `../spec/`; this card is the map.

---

## Rules — claims that check

A `rule` is a checkable constraint. A bare `rule` checks the world; `rule …
over order` constrains a timeline; `rule … over season` is a season-ledger
rule. All carry a severity and their origin — the domain is stated in the form,
not encoded in a separate keyword.

```dli
scope camelot

rule uthers-time-held-back  over order  severity error  decided d-0009
  : listener-learns(during uthers-time _) requires act > 1

rule show-the-price  over season  severity warn  proposed
  : listener-learns(cost magnitude >= major) requires season >= 2
```

## States — facts that hold over an interval

A `state` is a fact true across a stretch of time, until something ends it.

```dli
scope camelot

state excalibur.keeper = in-the-stone  until the-sword-drawn  decided d-0008
```

## Functional predicates — one value per subject

`functional <pred>` declares a predicate single-valued: each subject holds at
most one certain value for it. Two distinct certain values are then a
`continuity.fd-violation` against canon itself.

```dli
scope camelot

functional ruler-of                ratified d-0146
```

Undeclared, the checker infers single-valuedness from precedent — a predicate
that carries one value everywhere it appears is read as functional — which cannot
judge a predicate's *first* use. Whether a relation admits one value or many is a
ruling about the world (koan 3), so the author makes it. The declaration lowers
to no atom; it is carried, with its provenance, on the stored declaration.

## Cycles — cyclic time that refuses rank

A `cycle` is a cyclic kind: it has `succ`, `pred`, `across`, and `distance`, but
**no first, no largest, no `<`** (charter koan 8). Asking for the "greatest"
season is a type error, not a silent tie-break.

```dli
scope camelot

cycle season(4) = spring | summer | autumn | winter   decided d-0009
cycle moon(29)                                         decided d-0009
```

## Forks — exploring an undecided world

A `fork` supposes an answer and explores its consequences over an unchanged
store; nothing a fork does changes canon (koan 2). Its arms are `option`s, like
a question's.

```dli
scope camelot

fork what-does-the-word-cost
  option a-year-of-life
  option a-truth-told
```

This is the one place the tool sequence is worth learning in full, because it is
where the expert tier earns its keep. Open the workspace **on the fork**, mint a
hypothesis, hang a claim on it, then pin it and read what moved:

```json transcript
→ { "op": "author.open", "params": { "fork": "who-holds-excalibur" } }
← { "ok": true, "result": { "workspace": "ws-1", "workspace_digest": "jcs:sha256:b0e257cf…dca91093" } }

→ { "op": "author.add_hypothesis", "params": { "workspace": "ws-1", "id": "h-return",
      "statement": "Excalibur was always the Lake's to reclaim, never the king's to keep" } }
← { "ok": true, "result": { "findings": [] } }

→ { "op": "author.add_relation", "params": {
      "workspace": "ws-1", "relation": "keeps", "from": "the-lake", "to": "excalibur",
      "provenance": { "kind": "hypothesis", "hyp": "h-return" } } }
← { "ok": true, "result": { "findings": [],
      "rendered": ["keeps(the-lake, excalibur) proposed assumes h-return"] } }

→ { "op": "author.suppose", "params": { "workspace": "ws-1", "hypotheses": ["h-return"] } }
← { "ok": true, "result": { "findings": [ {
      "code": "consequence.changed", "severity": "warn", "grade": "checked",
      "message": "Under h-return, the derived-consequence set may move. See author.diff for the consequences." } ] } }
```

`hypothesis` is a third provenance kind alongside `proposed` and cited decisions
`cited`, and it is the one that makes exploration cheap. Now read the shift:

```json transcript
→ { "op": "author.diff", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": {
      "rows": [ {
        "atom": "keeps(the-lake, excalibur)",
        "after": { "status": "certain", "grade": "asserted", "support": [["+h-return"]] },
        "introduced": true, "retracted": false,
        "rendered": "keeps(the-lake, excalibur): dead (asserted) -> certain (asserted)" } ],
      "consequences": { "newly_certain": [ … ], "conflicts": [] } } }
```

The claim is now **certain** — and its support still reads `+h-return`. That is
the whole trick, and it is worth being precise about: no stored fact changed,
and no ruling was made. Supposing a hypothesis is a *valuation* over the same
saturated world, so a fork costs a re-read, not a re-derivation, and abandoning
one costs nothing at all. Certainty inside a fork is certainty *under an
assumption*, and the assumption never gets dropped from the record.

## Practices — magic systems as checked lore

A `practice` is a magic system: kinds, roles, resources, rules, conditions,
errors, and costs. Its interior is a **typed structure** in the stored fact
object, checked by its lane; nothing is dropped (koan 4). This is how it reads
once rendered — a shallow braced block, never a deep tree:

```dli
scope camelot

practice oath-binding  proposed {
  kind bond = fealty | troth | ransom
  role oathgiver : person   requires standing(oathgiver)
  resource boon   : linear conserved
  rule pledge(bond, oathgiver)
  rule give-and-take
  condition witnessed requires witness within earshot
  error unowed         refines L2 premise-not-available where absent
  error wrong-moon     refines L3 side-condition [timing] over cycle moon
  cost social | bodily | material | epistemic
  }
```

## Proofs — a scene's reasoning, step by step

A `proof` records a bounded piece of reasoning — a premise, the steps taken, a
spend, and what it closes — inside a practice.

```dli
scope camelot

proof lancelots-oath  speaker lancelot  in oath-binding {
  premise p1  standing(lancelot)  cited d-0012  cue "Lancelot comes to court"
  premise p2  consent(arthur)     cited d-0012
  premise p3  boon(1)             drawn-against p1
  step   s1  instantiate pledge  with bond = fealty, oathgiver = lancelot
  step   s2  chain from p1, s1 by give-and-take conclusion pledged(lancelot, fealty)
  spend  p3
  close  pledged(lancelot, fealty)  expect holds
  }
```

## Dialects and names — a constructed language

A `dialect` is a language variety with its inventory and name-parts; `name`
composes a proper name from parts; `term` and `banned` govern in-world
vocabulary; a `hypothesis` conditions speculative claims, and `ratify` records
the ruling that settles one.

```dli
scope camelot

dialect old-speech  "the old speech"  proposed {
  vowels a e i o u y
  name-part myr   "sea"        role given-proto     proposed
  }

name myrdin "Myrdin"  person.given in old-speech
  = given-proto myr + given-deutero din
  bearer the-enchanter   proposed

term the-drawing "the freeing of Excalibur from the stone"  decided d-0008
banned cannon   d-0010  "the Matter of Britain keeps to sword and spear — no gunpowder anachronism"

hypothesis h-old-loan "the old speech took a hero-name from the tongue over the water"

dialect over-the-water  proposed agent:factory/naming-run-0714  assumes h-old-loan {
  inventory placeholder
  }

ratify d-0011 { h-old-loan := true }
```

---

## What an expert declaration is, stored

Every form on this card lands in the store as an ordinary fact object with one
extra field. Here is the `hypothesis h-return` of the Camelot corpus, exactly as
the store holds it:

```json fact-object-v1
{
  "schema_version": 2,
  "authority": null,
  "claim_id": "sha256:ab05e37eea32e724fc758f77ccc9829dc274eb3172ef0d4316284818298bac9e",
  "atom": { "pred": "__decl", "args": ["Hypothesis", "h-return"] },
  "support": { "monomials": [[]] },
  "defeaters": { "monomials": [] },
  "grade": "asserted",
  "expert_body": {
    "kind": "hypothesis",
    "name": "h-return",
    "body": ["{\"Hypothesis\":{\"name\":{\"text\":\"h-return\",\"span\":{\"file\":0,\"start\":5299,\"end\":5307}},\"statement\":\"Excalibur was always the Lake's to reclaim, never the king's to keep\"}}"]
  }
}
```

Three things to read off it:

- The `atom` is a **declaration**, `__decl(Hypothesis, h-return)` — the world
  recording that this hypothesis exists, distinct from any claim resting on it.
- `support` is the **empty monomial** `[[]]`: unconditionally present. Declaring
  a hypothesis asserts nothing about whether it holds; that is what `suppose`
  and a ruling are for.
- `expert_body` carries the typed interior. A `practice`'s body carries its
  kinds, roles, resources, and errors the same way; a `proof`'s carries its
  premises, steps, and spend. The lane that owns the form reads that structure
  directly — nothing re-parses text to check a proof.

The same shape is why the expert tier survives the surface retirement intact:
the vocabulary lives in the store, and the braced blocks above are one way of
printing it.

---

## The one rule that still holds

Everything here obeys the first card's discipline: **a claim carries its
origin**, and nothing is true until a declared decision is cited in the consuming
ledger (koan 1, koan 3). The
expert vocabulary buys reach — checkable rules, cyclic time, forks, magic — but
not a way past provenance. When you materialize, all of it commits as fact
objects with their typed interiors; a reviewer reads the structured diff rows,
and the rendered block is offered alongside for eyes that want a sentence.
