---
status: live
title: "TPtH worldbook — Language"
verdict: "The TPtH language layer behind spec 05: the cymric-fringe dialect, the name Enfys, the hedgeway/the-plenty lexicon and the banned machine/algorithm rationale, the cymric agent-init bundle, and the 'is Enfys Welsh?' attestation seam."
---

# Language — cymric-fringe and the lexicon

> **Worldbook.** This chapter faces [spec 05 · Language layer](../../spec/05-language-layer.md)
> and documents the pack data at
> [`worlds/tpth/packs/cymric-fringe.dli`](../../../worlds/tpth/packs/cymric-fringe.dli).
> The spec teaches the language layer on camelot's `old-speech` dialect and the
> name `myrdin`; the worked TPtH instances it used to carry live here. The
> lexicon's loan-word layer interlocks with [the Concordance](concordance.md) —
> `kind binder from lexicon.loan-words`.

The western fringe of the TPtH world speaks a Brythonic-shaped tongue,
cymric-fringe, and its names, its lexicon, and the allegory's lexical bans are all
world knowledge under one semantics.

## The dialect

Everything a dialect holds per linguistic zone becomes a pack, one-to-one:

```dli
# grammar: v0 (unfrozen; AST normative)
scope language

dialect cymric-fringe  "the fringe tongue"  provisional D-0181 {
  basis attested "Brythonic; medieval Welsh"   # an external-world claim — see §7
  vowels a e i o u w y
  endings -ys -wen -eth -air
  foreign qu x z tion                          # clusters that mark a form as not-ours
  cluster-max 2
  stress penultimate
  name-kind person.given = given-proto given-deutero | given-simple
  name-kind place        = place-qualifier place-generic
  name-part en   "spirit"        role given-proto     ratified D-0134
  name-part fys  "arc, bow"      role given-deutero   ratified D-0134
  name-part wern "alder-marsh"   role place-generic   ratified D-0111
  name-part cors "bog, fen"      role place-qualifier ratified D-0111
  }
```

## Enfys, a typed derivation

The protagonist's name cites its dialect and its composition, so "dialect-true"
is a compile verdict, not a reviewer's impression:

```dli
# grammar: v0 (unfrozen; AST normative)
scope language

name enfys "Enfys"  person.given in cymric-fringe
  = given-proto en + given-deutero fys
  bearer protagonist   ratified D-0134
  # checked: the slot sequence matches a declared name-kind pattern; each
  # name-part exists in the dialect under the claimed role; the composed
  # surface form passes the dialect's phonotactics
```

A name that fails any of the three obligations does not compile, and the failure
speaks the domain rather than the parser:

```text
invalid: dialect-fit at name gwyneth — cluster 'th' after 'wyn' exceeds
         cymric-fringe cluster-max 2; and 'given-simple' takes exactly one
         name-part, but two are cited. declared name-kinds for person.given:
         [given-proto given-deutero] | [given-simple]
```

## The lexicon and the banned-vocabulary rationale

Diegetic terms are declarations with status and citations, and the allegory's
lexical bans are *cited lexicon facts*:

```dli
# grammar: v0 (unfrozen; AST normative)
scope language

term hedgeway "the hedgeway"  ratified D-0143
term the-plenty "the Plenty"  ratified D-0132

banned machine   D-0056  "the allegory is structural, never lexical"
banned algorithm D-0056  "the allegory is structural, never lexical"
```

The allegory is structural, never lexical: no diegetic term or name may carry
"machine" or "algorithm." The prose scanner stays a factory-side tool, but it
consumes the *compiled* ban list, so there is one authoritative statement of what
is banned and why, cited to the ruling that banned it (D-0056). A ban is lore with
provenance, not a config entry. The lexicon also feeds magic: the spoken-proof
practice's `binder` kind draws from the lexicon's attested loan-word layer — `kind
binder from lexicon.loan-words` — so the binder layer *is* the loan-word layer, not
a copy of it ([the Concordance](concordance.md)).

## The cymric agent-init bundle

The data that seeds a dialect comes from LLM and agent interaction, and it enters
like world lore: as a proposal bundle under one hypothesis variable, every clause
carrying agent provenance by inheritance:

```dli
# grammar: v0 (unfrozen; AST normative)
scope language

hypothesis h-cymric "the western fringe speaks a Brythonic-shaped tongue"

dialect cymric-fringe  proposed agent:factory/naming-run-0712  assumes h-cymric {
  # inventory, patterns, name-parts — every clause inherits the bundle's
  # hypothesis and the agent provenance
  }

name gwenair "Gwenair"  person.given in cymric-fringe  proposed
  # support: H(h-cymric) — merely-possible: visible to shadow queries,
  # structurally excluded from every context pack and every assertion

ratify D-0195 { h-cymric := true }     # one valuation ratifies the bundle
```

One hypothesis variable per bundle means ratification is one authorial act; the
clause grain gives partial ratification for free — ratify the dialect but reject
two name-parts, and every name derived through them dies into lineage and lands in
the killed-token dialect.

## The attestation seam — "is Enfys Welsh?"

`basis attested "Brythonic; medieval Welsh"` is a claim about *our* world, not the
story's, and it must not enter the annotation polynomials as if it were a
citation. Asking "is Enfys Welsh?" is asking an attestation, not a support term:
attestation grades never feed assertability, and cymric-fringe is ratifiable with
its basis unverified. The author ratifies the *sound* — the phonotactics, the
name-parts, the names that derive true to them — never the scholarship. A name can
never become *certain* because a model believes its basis is authentic.
