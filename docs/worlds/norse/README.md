---
status: reference
title: "Norse worldbook — the Nine Realms toward Ragnarok"
verdict: "A self-contained public-domain mythology example for the fiction-focused tools."
---

# The Nine Realms — a Norse world in dli

> **Worldbook.** This chapter tours the fiction tools through a
> mythology example. Where [`examples/camelot.dli`](../../../examples/camelot.dli) is
> one teaching file, this world is a *directory of packs* — the shape a real
> world takes — under [`examples/norse/`](../../../examples/norse/). It uses a
> different mythos from the original Hollow example.

The Norse example is three `.dli` packs, each pack a self-contained
scope that `deloreis check` passes clean, and together one corpus a proof checks
and a context pack exports over:

| Pack | Scope | What it teaches |
|---|---|---|
| [`nine-realms.dli`](../../../examples/norse/nine-realms.dli) | `nine-realms` | the gentle subset + chronology: eras, events, cycles, places, people, states, a **negation rule**, a fork, a question, a ratify block |
| [`seidr.dli`](../../../examples/norse/seidr.dli) | `seidr` | a **practice** (magic system) and a worked **proof that holds** |
| [`runes.dli`](../../../examples/norse/runes.dli) | `runes` | the **language layer**: a dialect, name-parts, names as typed derivations, an agent proposal bundle |

## The spine: ages running down toward the doom

A totally-ordered era spine carries the world from the golden age to the reborn
world; a `narration vantage` sits past the end, where the seeress sings the
doom in retrospect. Events in ordered eras inherit their eras' order when the
placements are certain. Within one era, an undeclared pair remains unordered
until a `before` ruling orders it.

```dli
scope nine-realms

era elder-age < the-binding-age < the-wolf-age < after-ragnarok   decided d-0001

event the-world-shaped      during elder-age        decided d-0002
event the-wall-raised       during elder-age        decided d-0002
event the-binding-of-fenrir during the-binding-age  decided d-0003
before the-binding-of-fenrir -> the-death-of-baldr   decided d-0003

narration vantage after-ragnarok
```

Cyclic time is orthogonal to the spine — the seasons and the great-year wheel of
the Norse eternal return (gold, strife, doom, reborn). A cycle carries
`succ`/`pred`/`across`/`distance` and refuses `<`, `sort`, and `max`: the wheel
refuses rank.

```dli
scope nine-realms

cycle season(4) = winter | spring | summer | autumn   decided d-0009
cycle great-year(4) = gold | strife | doom | reborn    decided d-0009
```

States acquire validity spans, so the sun crossing the sky *until* the wolf
takes it and *devoured since* hold over disjoint spans — an elder-age flashback
and the wolf-age doom both check clean, with no false contradiction between them.

## A rule with negation: a certain-at-risk ward

A realm's ward holds where its wall is raised and it has *not* been stormed. The
storming is merely `proposed` — a shadow — so it does not block the derivation,
but it *poisons*: `ward-holds(asgard)` is derived **certain** (resting on the
ratified `warded`) yet flagged **certain-at-risk**, defeasible should `stormed`
ever be ruled true (see [spec 02 §4](../../spec/02-semantics.md)). Had `stormed`
been certain, the certain-layer test would have blocked the rule outright.

```dli
scope nine-realms

warded(asgard)    ratified d-0011
stormed(asgard)   proposed

rule ward-holds   ratified d-0011
  ward-holds($r) :- warded($r), not stormed($r)
```

## Seidr: a practice with laws, and a proof that holds

Seidr is the fate-craft. As a **practice** it declares its own kinds, its
resources (a `linear conserved` fate-debt that must balance, and an
`affine bounded` bodily might), its roles, its laws, and its failure classes —
each a refinement of one of the seven structural classes L1..L7 — entirely as
data. The kernel supplies one generic checker; nothing here is Rust.

```dli
scope seidr

practice seidr  proposed {
  use chronology.cycles
  kind working = prophecy | weather-witch | shape-sending
  role seer       : person   requires standing(seer)
  role petitioner : person   requires consent(petitioner)
  resource orlog  : linear conserved
  resource megin  : affine bounded body-limit
  rule weave(working, seer)
  rule spend-and-settle
  error unowed      refines L2 premise-not-available where absent
  error wrong-moon  refines L3 side-condition [timing] over cycle moon
  error overdrawn   refines L6 resource-violation on megin signals collapse
  cost fate | bodily | social | epistemic
  }
```

A **proof** is one casting written out and checked against the practice and the
scene: the Wanderer (Odin in disguise) wakes the dead seeress and asks her the
doom. Her standing and his consent seat in the certain pool; a fate-debt is
drawn against her standing; `weave` is instantiated; a `spend-and-settle` chain
concludes the seeing; the fate-debt is settled so the conserved multiset balances
to zero; and the close **holds**.

```dli
scope seidr

proof the-seeress-answers  speaker the-seeress  in seidr {
  premise p1  standing(the-seeress)   cited d-0105  cue "the Wanderer wakes her from the mound"
  premise p2  consent(the-wanderer)   cited d-0105
  premise p3  orlog(1)                drawn-against p1
  step   s1  instantiate weave  with working = prophecy, seer = the-seeress
  step   s2  chain from p1, s1 by spend-and-settle conclusion foretold(the-seeress, doom)
  spend  p3
  close  foretold(the-seeress, doom)  expect holds
  }
```

```text
$ deloreis check-proof examples/norse/seidr.dli -c examples/norse
verdict: holds — the close follows from the accounted ledger
grade: checked
proof checks: the declared expectation was met
```

## The language layer: names as typed derivations

Old Norse is dithematic — a hero-name is a proto + deutero compound. The dialect
declares its phonotactic frame and a name grammar; a `name` is then a *typed
derivation*, checked at compile for its slot sequence, for each morpheme existing
under its claimed role, and for the composed surface passing the phonotactics.

```dli
scope runes

dialect old-norse "the Old Norse tongue"  decided d-0201 {
  vowels a e i o u y
  endings -r -i -a -unn
  cluster-max 2
  stress initial
  name-kind person.given = given-proto given-deutero
  name-part sig "victory"       role given-proto    decided d-0201
  name-part run "rune, secret"  role given-deutero  decided d-0201
  }

name sigrun "Sigrun"  person.given in old-norse
  = given-proto sig + given-deutero run
  bearer the-seeress   decided d-0202
```

Candidate lore enters like any other — `proposed`, under one hypothesis variable,
carrying agent provenance — and is ruled in one recorded act; see the loan-name
bundle in [`runes.dli`](../../../examples/norse/runes.dli).

## Verify the whole world

```text
$ deloreis check examples/norse                                     # clean
$ deloreis check-proof examples/norse/seidr.dli -c examples/norse   # holds
$ deloreis export --profile context-pack -c examples/norse          # exit 0
```

Nothing in this world is a component of the kernel: the eras, the wards, the
fate-debt and the dialect are all declarations. Point the same
machinery at Camelot or at the Nine Realms and it runs unchanged.
