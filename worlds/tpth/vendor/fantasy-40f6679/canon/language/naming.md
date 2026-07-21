---
id: naming
title: Language and Naming Conventions
type: meta
status: ratified
tags: [language, naming]
source: gemini-session-2026-07-b
---

# Language and Naming Conventions

## The signposting principle

> status: ratified (D-0079)

Regions must feel like different nations, and language is the primary signpost — "some
regions the language is more like greek, others more like latin" — alongside
clothing, food, and music. Register and cadence differences should be audible in
character voices even when everyone is heard in English.

## Proposed register map

> status: provisional (D-0081, D-0219)

The working scheme ([[regions]]): the rulers' tongue — clipped, precise,
Latin/High-German in texture, rich in words for debt and measure, poor in words for
mercy; the old tongue of the proofs — ancient, formal, never conversational (chanted,
quoted, woven); and the regional dialects — water-vowelled lowlands speech, guttural
crag speech, whistled-melodic coast speech. The apprentice's ear for switching between
them is part of her practical adaptability.

## Diegetic vocabulary discipline

> status: ratified (D-0030, D-0046)

No modern or technological register may surface in anything heard in-world. The
machine-word rejection is the precedent; `data/banned-vocabulary.yaml` is the
enforced list and grows as drafting reveals new leaks. Authors think "extraction
economy"; characters say "the tithe."

## Sourcing policy for names and terms

> status: proposed

Working convention, unratified: prefer real attested pre-Christian Northern European
material (wyrd, frith, geas, landvættir, ogham, skald — already in the lexicon) over
invented coinages; session coinages (Iron Ledger, Tithe of Minds, hedge-fair,
chalk-blooded…) stay provisional until ratified; proper names for people and places
are only proposed in clearly-marked batches for the author to rule on (Q-0002,
Q-0015).

## Everything still unnamed

> status: ratified (D-0154)

Q-0002 is closed (D-0154: naming has done enough for now; the oracle vets new
names as they arise). Remaining unnamed at closure: the languages themselves, the narrator's
order — still open; the series title is settled (D-0143). The single largest naming session still
ahead of us.

## The naming oracle workflow

> status: proposed

Working process, unratified. Every new proper name — person, place, people, polity,
organization, concept, term, title — passes through a naming oracle
(`src/fantasy/naming/`, `docs/reference/naming-workflow.md`) before it can become canon; none
enter from a raw generation. The pipeline is: need → oracle proposes vetted
candidate sheets (composed from real, glossed, cited elements of the register that
fits the thing's origin culture, optionally LLM-assisted) → six checks score each
candidate (AI-tic blocklist, role-aware pop-culture collision, intra-canon
uniqueness and aural distinctness, register phonotactic fit, banned-vocabulary reuse,
pronounceability for narration) → a human edits and proves the survivors out → Jeff
ratifies with a `D-nnnn`. The registers map to the four-biome frame plus the imperial
and old tongues ([[regions]]), anchored British-Isles-first and widening to Northern
Europe. The oracle proposes; only Jeff decides.

## Non-canonical status of existing names

> status: proposed

Working stance, unratified, consistent with the sourcing policy above and Q-0002 /
Q-0015. None of the names produced in the seed session are canonical; they are inputs
that must pass the oracle or be replaced. This explicitly includes the secondary-cast
sketches (Oona, Silas, Kaelen, Cerys, Jorin — Silas and Kaelen fail the AI-tic
check), the provisional framework name "the Iron Ledger" (Q-0001; it matches both a
verbatim AI tic and the "The [metal] [noun]" formula), the biome placeholders
(Chalk-Spines, Drowned Parishes, Slag-Peaks, Salt-Fringes) and earlier region
sketches (Glass Weald, Rust Wastes), and the empire, peoples, languages, and the
narrator's order. The first vetted proposal batch for the priority set lives at
`docs/rulings/RQ-001-names-batch-001.md`.

## The pragmatism layer (core cast)

> status: ratified (D-0095, D-0097)

Core-cast names carry the commercial surface of the show and get a stricter parse.
They must be easily sayable by an English speaker on first sight, voice-actable
(clear primary stress, no grapheme traps like "gyth" or Welsh ll/dd values), short
in everyday address — a longer formal name may exist, but the common address form
is what listeners live with (the "Vieno Aalto → Aalto" pattern) — and
gender-morphology-correct where elements are gendered (patronymic -sson is male,
-dottir female; the anchor is male, D-0096). Portent-cliché elements (doom-,
death-, shadow-class) fail as simplistic, and name elements that direct-hit major
fictional franchises fail regardless of legitimate etymology — "borg" reads as Star
Trek before it reads as Norse.

The author's calibration verdicts, kept as the layer's test set: Leofgyth and the
Old-English-king register (Aethelstan) — out for core cast; "Aalto" fine as address,
"Luotea" promising; Philodoros-class length — out; Ketildottir — out on morphology;
Ketilsson, Steinulf — acceptable shapes; doom-wight / doom-ward — out; borg-bearing
compounds — out.

## Naming institutions and orders

> status: ratified (D-0098, D-0099, D-0100)

Three further calibration rulings from the first proposal batch. The narrator needs
a name (even if rarely used — mostly cold-open attributions), but bare sacral
concept-nouns worn as names (Metod, Galdr) are rejected as poorly motivated; the
name should follow a believable convention of the narrator's own order. The
imperial framework's name needs the same commercial lens as the core cast — it is
spoken constantly, and Latin-blender fusions (Censordo, Pondelex) or stilted "Ordo
of X" shapes read silly, clunky, or both; "the Iron Ledger" itself remains a live
option (Q-0001). And the narrator's-order direction (Myndward / Runemoot /
"Assize of …") produced no winner — "Assize" rates only a tepid maybe; the next
batch needs a stronger motivation for how such an order would actually name itself.

## Round-two calibration (batch-002 verdicts)

> status: ratified (D-0108, D-0109, D-0111)

The second calibration pass, folded into the oracle's test set. The **Tiffany
problem** is now a named failure class: a historically authentic name that reads
modern to today's ear (Audrey) fails for core cast — core-cast names must also
*signal the genre*. For the protagonist: clearly feminine, never grandmotherly
(Edith/Hilda-class out), no -stan forms (they read male), nothing audio-ambiguous
(Wynne-class out) — and she carries a given name plus a family name from mismatched
registers, because she is a bastard of two societies (D-0109). The apprentice's
family name and common address is Aalto (given name still wanted); the anchor's
family name is Ketilson (given names to explore); the hound takes wed — or ward — as
suffix with prefixes to explore. The imperial framework's name wants **Orwellian
doublespeak** — benevolent words for extraction (the Gift, the Balance) — rather than
honest menace. And names for the narrator's order (or anything else) that drift
toward violence or maleness are against what the series stands for (D-0005): the
warden/warrior register is the wrong register. Ratified place names (per D-0111 the earlier region
thumbs-ups were approvals, not candidate-passes): Whitecombe, Corsbach, Swartby,
Halspetra — plus Gwern (starting valley) and Trefpen (hub). Calibration
aside: Halspetra shows short Greek elements pass when effortlessly sayable — the
D-0095 Philodoros rejection was about length and opacity, not origin.

## Round-three calibration (batch-003 verdicts)

> status: ratified (D-0129, D-0130, D-0131)

The third pass sharpened three things. First, the anachronism detector must be
systematic: Marcia slipped through after Audrey was caught, so a curated list is not
enough — the check needs usage-frequency data behind it. Second, breadth: two rounds
running, the given-name exploration for the protagonist and apprentice was too
narrow. The core four names are the hardest-working assets in the show; they carry
the market and audio craft bar (mutually distinct in the ear, memorable, ownable,
effortless to say), and the search must range far wider than a shortlist. Standing
results: Livia is a maybe as a protagonist family name; Sig leads for the anchor
(Sig Ketilson); the hound remains unnamed; Fennon is banked as the best
place-derived personal name. Third, closures and redirections: the narrator's name
is a placeholder, not a project (D-0130, [[narrator]]), and the imperial framework's
name must carry the singularity contrast — salvation in the mouth, obsolescence in
the world (D-0131, [[allegory]]).

## Round-four calibration: the cast is named

> status: ratified (D-0132, D-0133)

Batch-004 landed the cast. Enfys (protagonist given; Welsh, audio-crisp, genre-true);
Ranta Aalto (apprentice, called Aalto); Geir Ketilson (anchor — Geir over Sig);
Bramble (the hound's call-name, grown from the meeting itself, D-0133) with Frithwed
as treaty-formal; and the Plenty as the framework — doublespeak with the full
singularity contrast (D-0131). The quartet clears the ensemble bar: four distinct
initials, four distinct vowel cores. Still open on the core cast: the protagonist's
FAMILY name, which must pair against Enfys so the mismatch itself reads as the
bastard mark (D-0109) — the one remaining hard naming problem before writing.

## The series title

> status: ratified (D-0143)

The series is **To Pleach the Hedgeway** (D-0143, eight rounds: title-001..008).
The infinitive states the mission as a verb; pleach carries the poetry and Hedgeway
carries the search. The in-world hedge-road lore develops separately under its
allegory-test condition (D-0139, D-0140).
