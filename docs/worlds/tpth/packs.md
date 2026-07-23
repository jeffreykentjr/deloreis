---
status: live
verdict: "TPtH pack commentary migrated from baseline .dli comments (DL-0025). Fenced blocks are verbatim excerpts, oracle-checked."
---

# TPtH packs — recovered commentary

The pack files under `worlds/tpth/` ship in canonical comment-free form. This
document preserves the teaching commentary that was stripped during the
comment-removal migration, recovered from the baseline commit (268fe41). Each
fenced `.dli` block below is a verbatim excerpt from the current committed
pack file — the docs-oracle gate-checks them against the real parser.

## saltmarket

The saltmarket world is the shared test corpus every later lane checks against.
It carries a gentle subset plus a chronology spine: an era spine (≥3 eras),
events with a before-fact and a deliberately UNORDERED pair, states with
since/until (the two-era water-status case), an adjacency graph (≥6 edges),
and a question with options under a hypothesis. Provenance is this world's OWN
decision list (d-0001..d-0009) — a solo writer's ledger, per 01-syntax §3.

The common/enclosed state pair demonstrates that a pre-enclosure flashback
checks clean AND the present checks clean, because the two claims hold over
disjoint spans (03-time §2). No false contradiction between them.

```dli
scope saltmarket

ratify d-0001
ratify d-0002
ratify d-0003
ratify d-0004
ratify d-0005
ratify d-0006
ratify d-0007
ratify d-0008
ratify d-0009
era first-tides < the-silting < the-drought-years < the-ledger-days decided d-0001
event the-first-weir       during first-tides decided d-0002
event the-silting-begins   during the-silting decided d-0002
event the-long-drought     during the-drought-years decided d-0003
```

## cymric-fringe

The language-layer dialect pack of 05-language-layer §1–§5. Names and words are
world knowledge, so a dialect lives in the fabric like any other lore: statused
and forkable. This pack shows BOTH provenances the layer carries — cited (D-ref)
canon and proposed (agent) candidates under a hypothesis bundle — and the
attestation seam that keeps them apart (§7).

The `basis attested` claim is about OUR world (is this pattern Welsh-like?),
never support for the story's. It is graded and replayable but NEVER a variable
in a support polynomial: a name can never become certain because a model
believes its basis is authentic. The dialect is ratifiable with its basis
unverified — the author ratifies the SOUND, not the scholarship.

```dli
scope language

dialect cymric-fringe "the fringe tongue" provisional D-0181 {
  basis attested "Brythonic; medieval Welsh"
  vowels a e i o u w y
  endings -ys -wen -eth -air
  foreign qu x z tion
  cluster-max 2
  sayability-max 6
  stress penultimate
  name-kind person.given = given-proto given-deutero | given-simple
  name-kind place = place-qualifier place-generic
  name-part en "spirit" role given-proto ratified D-0134
  name-part fys "arc, bow" role given-deutero ratified D-0134
  name-part wern "alder-marsh" role place-generic ratified D-0111
}
```

## true-naming

The arbitrariness proof of 04-practices §4, completed: a practice whose
resource is graded-monotone DECAY (nothing spent, knowing only wears), whose
names are DIALECT-DERIVATIONS, and whose premises are KNOWLEDGE STATES. Three
layers — magic, language, and story-time — meet in one pack, zero kernel code.
Standing role: the workbench's acceptance test (04 §4).

Disjointness ledger (04 §4): the one honest cost this fixture exacts is adding
`graded monotone` to the resource algebra. Had it needed a second new algebra
member, the parameterization would have failed — which is what makes the fixture
load-bearing. Nothing here resembles proof-magic's conserved debt.

```dli
scope magic

use language
use chronology
dialect fen-tongue "the fen tongue" proposed {
  vowels a e i o u y
  endings -a -en -eth
  cluster-max 2
  stress penultimate
  name-kind person.given = given-proto given-deutero
  name-part sae "still-water" role given-proto proposed
  name-part wen "one-who-tends" role given-deutero proposed
}
name saewen "Saewen" person.given in fen-tongue
  = given-proto sae + given-deutero wen
```

## second-practice

The Turning — a divination game on the moon's twenty-nine leaves, and the
SECOND arbitrariness proof (04-practices §4). It is structurally disjoint from
BOTH proof-magic (no conserved debt) AND true-naming (no graded decay, no
knowledge states): its resource is affine-bounded, its failures are cycle-phase
timing and spread overflow, and its premises are figures laid on a cyclic kind.
Three practices, one generic checker, zero per-system kernel code.

The ring of twenty-nine leaves uses the SAME cyclic kind the timing checker
shares. succ/pred/across/distance exist; `<`, sort, and max are type errors —
the wheel refuses rank (koan 8).

```dli
scope magic

use chronology
cycle turning-leaf(29) proposed
practice the-turning proposed {
  use chronology.cycles
  kind figure = the-tide | the-weir | the-drowned | the-keeper | the-empty
  role reader : entity requires standing(reader)
  role querent : entity requires consent(querent)
  resource spread : affine bounded nine-leaves
  rule lay(figure, leaf) requires phase(turning-leaf) = leaf
  rule read(figure) requires lay(figure, _), seated-reading
  condition seated-reading requires phase(turning-leaf) within declared-span
  error mislaid-leaf refines L4 bad-instantiation
  error overspread refines L6 resource-violation on spread signals scatter
}
```

## spoken-proof

The Concordance, the proof-discipline magic of To Pleach the Hedgeway: a
casting is a spoken argument that either checks or fails at a named structural
class (docs/spec/04-practices.md §3, §5). It is the THIRD arbitrariness proof
alongside true-naming and the-turning, and structurally disjoint from both: its
resource algebra carries a LINEAR CONSERVED debt (not graded decay, not an
affine-only bound), its premises are DERIVED FACTS (not names known, not figures
laid), and its failures span the whole L2..L6 ladder.

Since the engine seats no ambient cycles (DL-0017), the practice's
`over cycle tide` resolves against the document-declared cycle and nothing
baked in.

```dli
scope magic

use chronology
cycle tide(4) = ebb | slack | flood | high proposed
practice spoken-proof proposed {
  kind ground = door | field | ore
  resource debt : linear conserved
  resource strain : affine bounded body-limit
  rule ward-form(ground, fibre)
  rule give-and-take
  error fizzle refines L2 premise-not-available where absent
  error rebound refines L2 premise-not-available where contradicted signals bodily-hit
  error wrong-hour refines L3 side-condition [timing] over cycle tide
  error overload refines L6 resource-violation on strain signals collapse
  cost bodily | material
}
```

## cycles

The cyclic-time vocabulary of To Pleach the Hedgeway (03-time §1.iv). These
were once seated in the engine as ambient "kernel cycles"; DL-0017 moved them
out — the engine seats no world's cycles of its own, so a world declares the
cycles its practices seat timing on. Tide presumes a coastal world; that is
TPtH canon, not kernel truth. succ/pred/across/distance exist on each; `<`,
sort, and max are type errors — a cycle refuses rank (charter koan 8).

```dli
scope chronology

cycle tide(4) = ebb | slack | flood | high ratified D-0146
cycle moon(29) ratified D-0148
cycle season proposed
```
