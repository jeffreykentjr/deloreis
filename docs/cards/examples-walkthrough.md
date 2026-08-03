---
status: live
verdict: "Migrated teaching commentary from the committed example worlds (DL-0025), reframed around the served fact objects: what the store holds, how to read it through the tools, and the .dli rendering of each shape. Each fenced block is gate-checked by the docs-oracle."
---

# Examples walkthrough

The bundled teaching worlds are **served from committed fact objects**. Each of
the three demo corpora — `camelot`, `round-table`, `warding-vows` — ships as a
`*.facts.json` bundle under `crates/dli-serve/demos/`: one commit paired with
the fact objects it names, in commit order. Nothing on that read path parses
text, and the bundles are re-derived from their committed sources on every gate
run, so drift is a hard failure rather than a silent one. The Camelot world
walked below is the `round-table` demo, whose source is `examples/camelot.dli`;
the Norse set (`examples/norse/`) is a corpus in the same shapes, gate-parsed
but not among the served demos.

So read the blocks below in the right register. The fenced `.dli` is the
**rendering** of what a world holds — kept here because ten readable lines
teach a shape that would take a hundred JSON objects to show. It is not a file
you write, and there is nowhere to type it. Where the canonical form teaches
something the rendering cannot, this document shows the fact object itself.

The french-deck example (`examples/games/french-deck/`) is a JSON ruleset
corpus (profiles, rules, game variants); no commentary existed or was migrated.

---

## What a served world actually is

The smallest bundled corpus, `camelot`, is one commit over eight facts. Here is
its head commit, whole:

```json commit-v1
{
  "schema_version": 2,
  "authority": null,
  "parent": "sha256:be78d4f6522725ad26e08555610344a380a3ef586a6301aa80420fe58288dce2",
  "facts": [
    "sha256:737556a9284ddd54c5084079d3e3754bd75ad2b1d5b3fc632a797a0c842833a9",
    "sha256:cf4e4b46d1bc36c0e7bd476713348375d3c9ad2190a23eeb815047604acb8f58",
    "sha256:e0021ae7d3c41a4a065ef84853115dbce2e4eb430762309f8de04e4051ca0dd8",
    "sha256:d0af634fed46f283a80a7b0d9980cedbc628f8b59c9d3e9895e26e533da5c80b",
    "sha256:cf7d37666af02c35db1faf3e499f67ede9341262e9a8ffa3ac71c067dc056522",
    "sha256:7c7422ff58612653e875a81b6bdd77229642e3f02dd0c62c84bbf99db7257a86",
    "sha256:c59dda706edf0f7ddd25581809f08516b09e0b998a4ce4869ae6afda3d245949",
    "sha256:b4df7fa3e46175e9cf542aac4435f30ddf74bdda09fbc9174fdeb08b0bf62e0d"
  ],
  "lineage": [],
  "evidence": [],
  "interner": ["D-0001", "D-0002", "h.camelot#0"],
  "decisions": ["D-0001", "D-0002"],
  "pack_name": "camelot"
}
```

A commit is a list of content addresses plus the table that decodes them. The
`interner` is the load-bearing part: every fact's support is written as indices
into it, so `+0` in this corpus means `D-0001` and `+2` means the proposal
bundle `h.camelot#0`. The `round-table` corpus below is the same object at
about a hundred facts and twenty-one interner entries.

## Reading one through the tools

You interrogate a committed corpus with `dli.query`, which stages nothing.
(`author.query` is its sibling for a workspace you have open — same rows, over
your staged work.) Ask for the goal as a typed atom; a `Var` argument
enumerates:

```json transcript
→ { "op": "dli.query", "params": { "document": "round-table", "why": true,
      "goal_atom": {
        "pred": { "text": "adjacent", "span": { "file": 0, "start": 0, "end": 0 } },
        "args": [ { "Sym": { "text": "camelot", "span": {…} } },
                  { "Var": { "text": "where",   "span": {…} } } ] } } }
← { "ok": true, "result": { "rows": [
      { "atom": "adjacent(camelot, camlann)", "bindings": { "where": "camlann" },
        "status": "certain", "grade": "asserted",
        "support": { "monomials": [["+5"]] },
        "citations": ["d-0004"],
        "why": { "antichain": [[5]], "rendered": ["d-0004"] } },
      { "atom": "adjacent(camelot, the-lake)", "…": "…" },
      { "atom": "adjacent(camelot, tintagel)", "…": "…" } ] } }
```

Every row carries its own reason. `support` is the raw polynomial, `why` is the
same thing resolved through the interner, and `status` is *derived* from them
under the corpus's valuation — never stored. That is the whole read model, and
the rest of this document is the vocabulary that feeds it.

---

## Camelot — the neutral teaching world

A recognizable, wholly public-domain setting (the Matter of Britain) that
exercises the full teaching surface end to end. It mirrors every shape the
later lanes check: a totally-ordered era spine, events with before-ordering,
a place adjacency graph, people, facts, states, a magic practice with a
graded-monotone resource, and an open question with a hypothesis.

### The world's ledger

Declaring a decision is what lets `decided d-000n` carry authority rather than
mint it (B1: a citation to an undeclared decision fails closed):

```dli
scope camelot

use language
use chronology
ratify d-0001
ratify d-0002
ratify d-0003
ratify d-0004
ratify d-0005
ratify d-0006
ratify d-0007
ratify d-0008
ratify d-0009
ratify d-0010
ratify d-0012
```

These are the corpus's `decisions` list, and the entries that populate its
interner — every `+n` in a support monomial resolves to one of them, or to a
hypothesis or option variable minted alongside them. A citation with no
matching declaration has no variable to point at, which is what "fails closed"
means concretely.

### Chronology — a totally-ordered era spine

Events carry era membership; WITHIN an era only `before` orders them. A
declared intra-era order renders as `before X -> Y`:

```dli
era uthers-time < arthurs-reign < after-camlann decided d-0001
event the-sword-drawn          during uthers-time decided d-0002
event the-round-table-founded  during arthurs-reign decided d-0002
event the-grail-quest          during arthurs-reign decided d-0003
event the-battle-of-camlann    during arthurs-reign proposed
before the-round-table-founded -> the-battle-of-camlann decided d-0003
narration vantage after-camlann
cycle moon(29) decided d-0009
cycle season(4) = spring | summer | autumn | winter decided d-0009
rule uthers-time-held-back over order decided d-0009
  : listener-learns(during uthers-time _) requires act > 1
```

### Places and the adjacency graph

```dli
place camelot        "Camelot"        decided d-0004
place the-lake       "The Lake"       decided d-0004
place avalon         "Avalon"         decided d-0004
place camlann        "Camlann"        decided d-0005
place tintagel       "Tintagel"       decided d-0005
place the-waste-land "The Waste Land" decided d-0005
adjacent(camelot, the-lake)       decided d-0004
adjacent(camelot, camlann)        decided d-0004
adjacent(camelot, tintagel)       decided d-0005
adjacent(the-lake, avalon)        decided d-0005
adjacent(camlann, the-waste-land) decided d-0005
adjacent(the-waste-land, avalon)  decided d-0005
adjacent(tintagel, the-lake)      proposed
```

### Entities — people, things, facts, and states

```dli
person arthur    "Arthur, the King" decided d-0006
person merlin    "Merlin"           decided d-0006
person guinevere "Guinevere"        decided d-0006
person lancelot  "Lancelot"         decided d-0007
person mordred   "Mordred"          decided d-0007
person morgan    "Morgan"           decided d-0007
thing excalibur "Excalibur" decided d-0008
fact arthur rules camelot   decided d-0006
fact merlin counsels arthur decided d-0006
fact lancelot serves arthur decided d-0007
standing(lancelot)          decided d-0012
consent(arthur)             decided d-0012
state excalibur.keeper = in-the-stone until the-sword-drawn decided d-0008
state excalibur.keeper = the-king since the-sword-drawn decided d-0008
```

### The language layer — a dialect with name derivations

```dli
dialect old-speech "the old speech" proposed {
  basis attested "Old Welsh / Brythonic"
  vowels a e i o u y
  endings -a -oc -wen
  cluster-max 2
  stress penultimate
  name-kind person.given = given-proto given-deutero
  name-part myr "sea" role given-proto proposed
  name-part din "fastness" role given-deutero proposed
}
name myrdin "Myrdin" person.given in old-speech
  = given-proto myr + given-deutero din
  bearer the-enchanter proposed
```

### Forks, options, and hypotheses

A `question` models open uncertainty; a `fork` is the exploration frame. An
option may `implies` a consequent; the option itself gates that consequent:

```dli
hypothesis h-return "Excalibur was always the Lake's to reclaim, never the king's to keep"
question who-holds-excalibur "Who holds Excalibur after Camlann?"
  option arthur   implies keeps(arthur, excalibur)
  option the-lake
keeps(the-lake, excalibur) proposed assumes the-lake and h-return
```

When a separate hypothesis gates a claim, put the condition on that claim's
provenance. Here the Lake claim rests on both the selected option and
`h-return`; the retired `option ... assumes` form is refused rather than
silently discarded.

This is the shape most worth seeing in canonical form, because the rendering
compresses it. `assumes the-lake and h-return` is stored as a **conjunction of
two support variables in one monomial**:

```json fact-object-v1
{
  "schema_version": 2,
  "authority": null,
  "claim_id": "sha256:1e79c4e40d3a672db3973f050c5800431e42c5157fa33c100a061371da3f3534",
  "atom": { "pred": "keeps", "args": ["the-lake", "excalibur"] },
  "support": { "monomials": [["+17", "+19"]] },
  "defeaters": { "monomials": [] },
  "grade": "asserted"
}
```

In this corpus's interner, `17` is `h-return` and `19` is the option variable
`the-lake` — so the claim holds exactly when *both* the option is taken and the
hypothesis is ruled true. The sibling claim `keeps(arthur, excalibur)` carries
the single monomial `["+18"]`: the `arthur` option alone, because no separate
hypothesis gates it. Query them together and the engine says so:

```json transcript
→ { "op": "dli.query", "params": { "document": "round-table", "why": true,
      "goal_atom": { "pred": { "text": "keeps", "span": {…} },
        "args": [ { "Var": { "text": "who", "span": {…} } },
                  { "Sym": { "text": "excalibur", "span": {…} } } ] } } }
← { "ok": true, "result": { "rows": [
      { "atom": "keeps(arthur, excalibur)",   "status": "possible",
        "support": { "monomials": [["+18"]] },
        "why": { "antichain": [[18]], "rendered": ["arthur"] } },
      { "atom": "keeps(the-lake, excalibur)", "status": "possible",
        "support": { "monomials": [["+17", "+19"]] },
        "why": { "antichain": [[17, 19]], "rendered": ["h-return and the-lake"] } } ] } }
```

Both are `possible`, neither is `certain`, and the question stays open — which
is the point. An open question is a debt the world carries in the open, and
nothing about it resolves until a human rules.

A `fork` works similarly but in speculative frame — no question requires an
answer, and it explores cost:

```dli
fork what-does-the-word-cost
  option a-year-of-life
  option a-truth-told
```

### Practices and proofs

A `practice` declares a repeatable procedure with named resources and rules.
A `proof` block demonstrates one application — checked as data:

```dli
practice oath-binding proposed {
  use chronology.cycles
  kind bond = fealty | troth | ransom
  role oathgiver : person requires standing(oathgiver)
  role oathholder : person requires consent(oathholder)
  resource boon : linear conserved
  resource strain : affine bounded body-limit
  rule pledge(bond, oathgiver)
  rule give-and-take
  condition witnessed requires witness within earshot
  error unowed refines L2 premise-not-available where absent
  error forsworn refines L2 premise-not-available where contradicted signals dishonour
  error wrong-moon refines L3 side-condition [timing] over cycle moon
  error overpledged refines L6 resource-violation on strain signals collapse
  error hollow-vow refines L2 where available-in(licensed-pool) and contradicted-in(certain)
  cost social | bodily | material | epistemic
}
proof lancelots-oath speaker lancelot in oath-binding {
  premise p1 standing(lancelot) cited d-0012 cue "Lancelot comes to court"
  premise p2 consent(arthur) cited d-0012
  premise p3 boon(1) drawn-against p1
  step s1 instantiate pledge with bond = fealty, oathgiver = lancelot
  step s2 chain from p1, s1 by give-and-take conclusion pledged(lancelot, fealty)
  spend p3
  close pledged(lancelot, fealty) expect holds
}
```

## Norse — a second public-domain teaching world

The Norse example set (nine-realms, seidr, runes) exercises the same shapes
in Old Norse mythology, demonstrating world-agnosticism: same grammar, same
checks, different content.

### Nine realms — era spine and cosmology

A four-era spine with nine places and a warded/stormed question:

```dli
scope nine-realms

ratify d-0001
ratify d-0002
ratify d-0003
ratify d-0004
ratify d-0005
ratify d-0006
ratify d-0007
ratify d-0008
ratify d-0009
ratify d-0010
ratify d-0011
ratify d-0012
ratify d-0013
ratify d-0014
era elder-age < the-binding-age < the-wolf-age < after-ragnarok decided d-0001
event the-world-shaped       during elder-age decided d-0002
event the-wall-raised        during elder-age decided d-0002
event the-binding-of-fenrir  during the-binding-age decided d-0003
event the-death-of-baldr     during the-binding-age decided d-0003
event fimbulwinter-falls     during the-wolf-age proposed
event the-last-battle        during the-wolf-age decided d-0004
before the-binding-of-fenrir -> the-death-of-baldr decided d-0003
narration vantage after-ragnarok
```

### Seidr — a magic practice

The seidr practice exercises the full practice structure with a proof:

```dli
scope seidr

use chronology
ratify d-0101
ratify d-0104
ratify d-0105
cycle moon(29) decided d-0101
person the-seeress  "The Seeress"  decided d-0104
person the-wanderer "The Wanderer" decided d-0104
standing(the-seeress) decided d-0105
consent(the-wanderer) decided d-0105
practice seidr proposed {
  use chronology.cycles
  kind working = prophecy | weather-witch | shape-sending
  role seer : person requires standing(seer)
  role petitioner : person requires consent(petitioner)
  resource orlog : linear conserved
  resource megin : affine bounded body-limit
  rule weave(working, seer)
  rule spend-and-settle
}
```

### Runes — the language layer

The runes file demonstrates a full dialect declaration with name derivations:

```dli
scope runes

ratify d-0201
ratify d-0202
ratify d-0203
ratify d-0205
dialect old-norse "the Old Norse tongue" decided d-0201 {
  basis attested "Old Norse / Old West Norse"
  vowels a e i o u y
  endings -r -i -a -unn
  cluster-max 2
  stress initial
  name-kind person.given = given-proto given-deutero
  name-part sig "victory" role given-proto decided d-0201
  name-part thor "the thunder-god" role given-proto decided d-0201
}
```

---

## Adding to one of these worlds

Everything above is a world being *read*. Adding to one is four kinds of call,
and the shapes are the same whichever corpus you open:

```json transcript
→ { "op": "author.open", "params": { "scope": "camelot" } }
← { "ok": true, "result": { "workspace": "ws-1", "workspace_digest": "jcs:sha256:9c49883e…a3b17a84" } }

→ { "op": "author.add_entity", "params": {
      "workspace": "ws-1", "kind": "person", "id": "bedivere", "name": "Bedivere",
      "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
← { "ok": true, "result": { "findings": [ { "code": "name.unvetted", "severity": "warn" } ] } }

→ { "op": "author.diff", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": { "root_hash": "sha256:1c1afc1e…daf45021", "rows": [ … ] } }

→ { "op": "author.materialize", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": { "findings": [], "root_hash": "sha256:ac0624cf…f68ed6b0",
      "facts": [ … ] } }
```

Open, propose, review the structured diff, commit. Every claim you add enters
`proposed` and carries your session's bundle variable in its support, so it
reads `possible` and never `certain` — the same two-monomial arithmetic the
`keeps` rows above showed, now with your session as the unruled variable.
`authoring-loop.md` is this loop worked call by call, with every response in
full.
