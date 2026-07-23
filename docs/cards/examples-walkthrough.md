---
status: live
verdict: "Migrated teaching commentary from the committed example worlds (DL-0025). Each fenced block is gate-checked by the docs-oracle."
---

# Examples walkthrough

The example worlds under `examples/` ship in canonical (comment-free) form.
This document carries the teaching commentary that explains what each
demonstrates, with fenced `.dli` blocks that the docs-oracle gate-checks
against the real parser. The french-deck example (`examples/games/french-deck/`)
is a JSON ruleset corpus (profiles, rules, game variants); no `.dli` commentary
existed or was migrated.

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

### Chronology — a totally-ordered era spine

Events carry era membership; WITHIN an era only `before` orders them. A
declared intra-era order uses the `before X -> Y` syntax:

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
option may `implies` a consequent and `assumes` a hypothesis — the assumption
links the option to the hypothesis it would settle:

```dli
hypothesis h-return "Excalibur was always the Lake's to reclaim, never the king's to keep"
question who-holds-excalibur "Who holds Excalibur after Camlann?"
  option arthur   implies keeps(arthur, excalibur)
  option the-lake implies keeps(the-lake, excalibur) assumes h-return
```

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
