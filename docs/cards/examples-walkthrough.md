---
status: live
verdict: "Migrated teaching commentary from the committed example worlds (DL-0025). Each fenced block is gate-checked by the docs-oracle."
---

# Examples walkthrough

The example worlds under `examples/` ship in canonical (comment-free) form.
This document carries the teaching commentary that explains what each
demonstrates, with fenced `.dli` blocks that the docs-oracle gate-checks
against the real parser.

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

ratify d-0001
ratify d-0002
ratify d-0003
```

### Chronology — a totally-ordered era spine

Events carry era membership; WITHIN an era only `before` orders them. A
declared intra-era order uses the `before X -> Y` syntax:

```dli
scope camelot

era pre-arthur < arthurs-reign < post-arthur

event the-sword-drawn  during pre-arthur    decided d-0001
event camelot-founded  during arthurs-reign decided d-0001
before the-sword-drawn -> camelot-founded   decided d-0001
```

### Entities — people, places, things

```dli
scope camelot

person arthur     decided d-0001
person guinevere  decided d-0001
person lancelot   decided d-0001
place  camelot    decided d-0001
place  the-lake   decided d-0001
thing  excalibur  decided d-0002
```

### Facts and states

Facts are ground subject-relation-object triples; states carry temporal
scoping (`since`/`until`):

```dli
scope camelot

fact arthur rules camelot         decided d-0002
fact arthur holds excalibur       decided d-0002
state excalibur.keeper = arthur   since pre-arthur  decided d-0002
```

### Forks, options, and hypotheses

A `question` models open uncertainty; a `fork` is the exploration frame. An
option may `implies` a consequent and `assumes` a hypothesis — the assumption
links the option to the hypothesis it would settle:

```dli
scope camelot

hypothesis h-return "Excalibur returns to the lake at the end"

question excalibur-fate  decided d-0002
  option arthur   implies keeps(arthur, excalibur)
  option the-lake implies keeps(the-lake, excalibur) assumes h-return
```

### Practices and proofs

A `practice` declares a repeatable procedure with named resources and rules.
A `proof` block demonstrates one application:

```dli
scope camelot

practice knighting "The accolade"  decided d-0001 {
  role sovereign : person    requires holds(sovereign, excalibur)
  role candidate : person
  rule sword-touch(sovereign, candidate)  decided d-0001 :- ...
  }
```

## Norse — a second public-domain teaching world

The Norse example set (nine-realms, seidr, runes) exercises the same shapes
in Old Norse mythology, demonstrating world-agnosticism: same grammar, same
checks, different content.

### Nine realms — era spine and cosmology

```dli
scope nine-realms

era the-binding-age < ragnarok < the-renewal

place asgard     decided d-0001
place midgard    decided d-0001
place niflheim   decided d-0001

person odin    decided d-0001
person thor    decided d-0001
person baldr   decided d-0001
```

### Seidr — a magic practice

```dli
scope seidr

practice galdr "runic invocation"  decided d-0001 {
  role caster : person    requires knows(caster, rune-lore)
  resource will : linear conserved
  rule invoke(caster, rune)  decided d-0001 :- ...
  }
```

### Runes — the language layer

```dli
scope runes

dialect elder-futhark  decided d-0001 {
  vowels a e i o u
  }
```
