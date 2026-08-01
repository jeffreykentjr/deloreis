---
status: live
verdict: "Names and words are world knowledge; the vet splits on whose world the ground truth is about."
title: "The language layer: names and words as world knowledge"
---

# The language layer

Names of people, places, and things — and the world's language generally — are
world knowledge, and world knowledge lives in one fabric ([koan 10](../charter.md)).
A dialect is a claim about a linguistic zone of the world: its phonotactics,
its name grammars, its name-part stock, every element carrying a gloss and a
citation. A ratified name is canon. The language layer is therefore not a
sibling tool bolted to the side of the fabric — it is packs in the fabric, under
one semantics, statused and forkable like any other lore.

The consuming repo's naming oracle splits along a **principled seam** rather than
staying whole in the factory: the half whose ground truth is a fact about the
*story's* world re-homes into the kernel as language packs and compile checks;
the half whose ground truth is a fact about *ours* stays factory-side as a second
screen. §3 draws that seam check by check.

## 1. Registers as declarations

Everything a dialect holds per linguistic zone becomes a pack, one-to-one — a
residency change, not a redesign:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

dialect old-speech "the old speech"  proposed {
  basis attested "Old Welsh / Brythonic"       # an external-world claim — see §7
  vowels a e i o u y
  endings -a -oc -wen
  cluster-max 2
  stress penultimate
  name-kind person.given = given-proto given-deutero
  name-part myr "sea"           role given-proto    proposed
  name-part din "fastness"      role given-deutero  proposed
  }
```

Each name-part is world lore with provenance — a cited fact about a tongue,
statused and forkable like any fact. The phonotactic frame (vowels, endings,
foreign-cluster markers, cluster ceiling, stress) and the name-kind slot
grammars are world facts too: they are what the composed surface form is checked
against.

One thing does **not** migrate. The composition *weights* the consuming repo's
generator uses — how often to try a dithematic name versus a simple one — are
proposal policy, not world fact. They describe how a generator should sample the
space; they say nothing about what the tongue *is*. Weights stay factory-side
with the generator that samples them.

## 2. Names as typed derivations

A name declaration cites its dialect and its composition, and "dialect-true"
stops being a reviewer's impression and becomes a **compile verdict**:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

name myrdin "Myrdin"  person.given in old-speech
  = given-proto myr + given-deutero din
  bearer the-enchanter   proposed
  # checked: the slot sequence matches a declared name-kind pattern; each
  # name-part exists in the dialect under the claimed role; the composed
  # surface form passes the dialect's phonotactics
```

Three obligations compile together on every name:

- **Slot sequence.** The sequence of name-part roles must match one of the
  dialect's declared `name-kind` patterns. `given-proto given-deutero` is a
  legal `person.given`; a lone `given-deutero` is not.
- **Role membership.** Each cited name-part must exist in the named dialect
  *under the role the name claims for it*. A name-part declared `place-generic`
  cannot fill a `given-proto` slot.
- **Phonotactics.** The composed surface form must pass the dialect's frame —
  its vowel inventory, cluster ceiling, ending set, and stress rule.

A name that fails any of the three does not compile. Register-fit is no longer a
note a reviewer writes; it is a verdict the gate produces, and it speaks the
domain rather than the parser ([koan 9](../charter.md)):

```text
invalid: dialect-fit at name gwrthdin — cluster 'rthd' exceeds old-speech
         cluster-max 2; and 'given-proto given-deutero' takes exactly two
         name-parts, but three are cited. declared name-kinds for person.given:
         [given-proto given-deutero]
```

The failure names the dialect, the offending clusters, and the legal slot
patterns — a writer meets a diagnosis in the world's terms, not a stack trace.

## 3. The vet split, check by check

The criterion for what re-homes into the kernel is **not**
deterministic-versus-LLM — several checks that stay factory-side are pure
lookups. The criterion is whose world the ground truth is about:

> A check re-homes into the kernel **iff its ground truth is a fact about the
> story's world**. It stays in the factory **iff its ground truth is a fact
> about ours**.

**In-kernel — world-true:**

| Check | Why it is a fact about the story's world |
|---|---|
| Intra-canon uniqueness | a query over the name-space the kernel now owns; a colliding declaration fails `deloreis check` |
| Fixed-token reuse | the same query surface; reserved tokens are canon facts |
| Aural distinctness | the phonetic-key distance ported from the oracle's phonetics as a pure kernel primitive over the name-space |
| Set distinctness | the same primitive over a declared set of names |
| Register fit | it *is* the grammar check of §2 |
| Gender morphology | a join of the name's name-parts' gender metadata against the bearer's canon facts — impossible while names and characters lived in different systems |
| Sayability-tier caps | ruled world policy (d-0010) over the syllable structure the dialect already defines |
| Banned vocabulary in names | against the in-runtime ban list (§4) |

**Factory-side — world-safe:**

| Check | Why it is a fact about our world |
|---|---|
| AI-tics | a screen against machine-generation tells; changes on research's schedule |
| Pop / media / franchise collisions | knowledge of our culture, never cited to a ruling |
| Homophones in English | a fact about English, not the tongue |
| Anachronism | a screen against a modern-names list |
| Portent clichés | a taste judgment about our genre expectations |
| Court-card versus tarot | a fact about our tarot iconography |

Every factory-side screen carries no diegetic status, changes on research's
schedule rather than by ruling, can never be cited to a D-entry, and **must
never gate assertability**. It is a second screen the consuming repo runs, not a
condition on whether a name is canon.

**Aural and set distinctness** deserve a word, because they are the checks that
most clearly could not run before the merge. The phonetic-key distance primitive
ports from the oracle's phonetics module as a **pure kernel primitive**: it maps
a name to a phonetic key and measures distance between keys, so "these two names
sound too alike" becomes a query over the name-space rather than an ear's
impression. Aural distinctness runs a candidate against the whole name-space;
set distinctness runs it against a declared set (a scene cast, a family, a
region). The scene-scoped version is what the craft layer's `scene-name-clash`
rule joins against ([06-craft-and-payloads](06-craft-and-payloads.md) §3) — a
join possible only where naming data and cast lists share a kernel.

**Pronounceability splits down its middle.** Cluster and syllable structure
follow the dialect grammar and are checked in-kernel; the
can-an-English-narrator-voice-it heuristic is audio-production knowledge about
our world and stays factory-side.

**Two-layer verdict composition.** The oracle's verdict becomes two-layered by
design: a kernel verdict (world-true) and an external screen (world-safe). The
consuming repo's `fantasy name vet` composes them — it asks the kernel whether
the name is true to the world and asks its own screens whether the name is safe
in ours, and reports both. Neither layer can overrule the other's ground truth,
because they are ground truths about different worlds.

## 4. The lexicon

Diegetic terms are declarations with status and citations, and the allegory's
lexical bans are *cited lexicon facts*:

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

term round-table "the Round Table"  decided d-0004

banned cannon d-0010 "the Matter of Britain keeps to sword and spear — no gunpowder anachronism"
```

The prose scanner stays a factory-side tool — scanning markdown is not world
knowledge — but it consumes the *compiled* ban list instead of a private YAML,
so there is one authoritative statement of what is banned and why, cited to the
ruling that banned it. A ban is lore with provenance, not a config entry.

The language layer also feeds magic. A spoken-enchantment practice draws its
primitives straight from it — merlins-craft's true-names are dialect derivations
checked in the speaker's register ([04-practices](04-practices.md) §4): magic
consumes language through an ordinary reference rather than through a convention
that lives in two places. One fabric means the magic layer reads the language
layer, not a copy of it.

## 5. The agent-init flow

The data that seeds a dialect comes from LLM and agent interaction — and it is
world lore, so it enters like world lore: as a **proposal bundle under one
hypothesis variable**, every clause carrying agent provenance by inheritance.
Agents author through **the tool surface**, the semantic tools against the `deloreis`
server (DL-0014; [11-agent-authoring](11-agent-authoring.md)); the surface below
is the *materialized* rendering of what those tools produced, reviewed as
surface before it is committed. No agent hand-writes the AST, and no agent edits
canon — the bundle enters `proposed`, awaiting the author's ruling.

```dli
# grammar: v0 (unfrozen; AST normative)
scope camelot

hypothesis h-old-loan "the old speech took a hero-name from the tongue over the water"

name-part tal "brow, high"  role given-proto  proposed agent:factory/naming-run-0714  assumes h-old-loan
  # inventory, patterns, name-parts — every clause inherits the bundle's
  # hypothesis and the agent provenance

name taldin "Taldin"  person.given in old-speech
  = given-proto tal + given-deutero din
  proposed agent:factory/naming-run-0714  assumes h-old-loan
  # support: H(h-old-loan) — merely-possible: visible to shadow queries,
  # structurally excluded from every context pack and every assertion

ratify d-0011 { h-old-loan := true }     # one valuation ratifies the bundle
```

**One hypothesis variable per bundle** means ratification is one authorial act.
The clause grain the annotation algebra already carries
([02-semantics](02-semantics.md)) gives **partial ratification for free**: ratify
the dialect but reject two name-parts, and their variables valuate false — every
name derived through them dies into lineage and lands in the killed-token
dialect, per the standing kill discipline. Names composed from an unratified
dialect are merely-possible *by construction* — no special case, just the same
status-gating every proposed entity gets. A merely-possible name is visible to
shadow queries and structurally excluded from every context pack and every
assertion.

## 6. The mirror dies

The consuming repo's `ratified_names.yaml` is a hand-synced mirror of world
knowledge — the very drift class this fabric exists to kill. It is **deleted as a
source**. The compiled world *is* the name-space; anything a legacy consumer
still needs comes from a query — `deloreis query name --layer certain --json` —
which is a generated view of the compiled world, never a second writable copy of
it.

Calling this "one fewer propagation step" understates it: it is the removal of a
standing context-poison vector that had a filename. The deletion is executed
under the migration protocol ([integration/migration.md](../integration/migration.md)),
at the naming-mirror checkpoint, after the shadow-read has proven the kernel
reproduces the legacy vet's world-half verdicts.

## 7. The attestation seam

`basis attested "…"` and per-name-part source URLs are claims about *our* world —
"this pattern is Welsh-like," "this root is on this reference page" — not about
the story's. They must not enter the annotation polynomials as if they were
citations. Two rules bind at this seam, and
[07-verdicts-and-llm-boundary](07-verdicts-and-llm-boundary.md) designs the
machinery that enforces them:

- **Attestation grades never feed assertability.** A dialect's real-basis is an
  attestation ([07](07-verdicts-and-llm-boundary.md) §3): stored, graded,
  replayable, and side-band on the checks that consume it — never a variable in a
  support polynomial. A name can never become *certain* because a model believes
  its basis is authentic.
- **A dialect is ratifiable with its basis unverified.** The author ratifies the
  *sound* — the phonotactics, the name-parts, the names that derive true to them —
  not the scholarship. `basis attested` may be unanswered, or answered `no`, and
  the dialect may still be ratified by a ruling. The scholarship is our-world
  evidence about the work; it is never support for the world.

This is the seam that keeps the language layer honest about which world each of
its claims is about — the same distinction §3 draws for the vet, applied to the
dialect's own provenance.

---

**Worldbook.** The TPtH language layer — the cymric-fringe dialect, the name
Enfys, the hedgeway/the-plenty lexicon and the banned machine/algorithm rationale,
the cymric agent-init bundle, and the "is Enfys Welsh?" attestation seam — lives
in [the TPtH worldbook · Language](../worlds/tpth/language.md).
