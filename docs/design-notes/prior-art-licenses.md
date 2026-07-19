---
status: reference
title: "Prior art: what each cited work licenses"
verdict: "each cited line of published work licenses specific named decisions — and nothing more."
---

# Prior art and the licenses it grants

The surface of De Lore Is stands on published work, not on taste. This note
records what each cited line of prior art licenses — and, as carefully, what it
does not. It is `status: reference`. The provenance detail behind it is in the
fantasy repo, `docs/research/RR-018-lore-language-fifth-pass.md` §8.1 and §17.
The discipline of this note is the discipline the charter demands: a claim that
its surface is approachable is a hypothesis until measured, and the citations
below set the *direction* of the surface without validating it.

## Empirical PL and DSL methodology

**Stefik & Siebert (ACM TOCE, 2013) — word keywords over punctuation.** Their
randomized controlled trials found novices writing Java and Perl no more accurate
than novices writing Randomo, a control language with randomly chosen keywords,
while Ruby, Python, and Quorum — word-based and punctuation-light — did
significantly better. Quorum turned the method into evidence-oriented language
design. **Licenses:** the closed vocabulary of domain *words*, the near-absence
of operator punctuation, the refusal of sigils and clever brackets. **Does not
license:** anything about *this* grammar specifically — Stefik studied novice
humans writing general-purpose programs, not fiction writers writing
declarations, and not agents. It sets the surface's direction; it does not
validate the surface.

**Green & Petre (JVLC, 1996) — the tradeoff ledger.** The cognitive-dimensions
framework is a vocabulary for naming what a notation costs, not a scorecard to
pass. **Licenses:** the method itself — naming the wins (low viscosity, no hidden
dependencies, deferred premature commitment, close mapping, consistency,
structural error-proneness) *and* the two dimensions the design deliberately
pays: diffuseness (one claim per line is verbose, bought for per-line blame) and
a flat abstraction gradient (no user-defined abstractions — records, not macros).
**The rule it enforces:** a notation review that names no costs has not used the
framework.

**Mernik, Heering & Sloane (ACM Computing Surveys, 2005) — the named position.**
Their catalog places the language at a nameable point rather than a vibes-argued
one. **Licenses:** the *decision* patterns that justify a DSL at all — **notation**
(citations-as-syntax that no library API can carry), **AVOPT** (analysis and
verification are the whole kernel), and **task automation** (context packs and
export compilation); the *design* pattern **language invention with piggybacked
lexical conventions** (a new grammar whose lexical layer rides shapes writers
know — line-oriented records, `key = value` fields); and the *implementation*
pattern **own compiler, not embedding.** Their catalogued weakness of embedded
DSLs — host-language error reporting, compromised notation — is exactly what the
Ash baseline rediscovered empirically: the annotation machinery in JSONB with no
compile-time help *is* the embedding pattern's textbook failure mode (see
`baselines-why-not.md`).

**Myers, Pane & Ko (CACM, 2004) — the natural-programming footing.** Their
studies found that non-programmers describing computation produce event- and
rule-shaped declarations — "when X, then Y," aggregate statements over sets — and
almost never control flow. Lore *is* declarative rules over a finite world.
**Licenses:** the claim that the language notates the paradigm non-programmers
already speak rather than teaching a foreign one — the empirical footing under
the "structured notes" claim, which otherwise rests on analogy.

## The fiction and game lineage

The shipped prior art divides into two families. The **parser-IF family**
(Inform 6/7, TADS, Dialog) builds *world-model languages*: objects, scope, rules
— simulation of a world at play time. The **choice family** (Twine, Ink,
ChoiceScript) builds *flow languages*: choices, diverts, state flags — delivery
of a narrative at play time.

- **From Inform:** world-model seriousness, and the existence proof that
  non-programmer writers will adopt a real programming language when it models
  their domain. **Refused:** its natural-language surface, whose
  easy-to-read/hard-to-write asymmetry is documented from inside and outside the
  project.
- **From Ink:** the success shape — minimal plain-text markup mirroring the
  manuscript writers already produce, and a runtime shipped as an embeddable
  library (the shape that carried it into a long commercial tail; Yarn Spinner
  proves it twice). That is the shape of `deloreis serve` and the export layer.
- **From ChoiceScript:** the demonstration that a tiny closed command vocabulary
  sustains an entire professional catalog of prose-writer-authored games.
- **From Dialog** (the engine this repo adopted): the proof that flat predicates
  suffice for a rich world model, and their agent-legible terseness.

**The structural claim the lineage yields — the lore language is neither
family.** Both existing families are *runtime* languages for the player-facing
moment; neither has an authority layer. Inform's world model is mutable
simulation state, not canon-with-provenance; Ink's variables are flags, not
facts; nothing in either can answer "is this true, since when, on whose ruling,
and what breaks if it changes?" That missing layer is the third thing:

| Family | Models | Runs at | Exemplars |
|---|---|---|---|
| Simulation | world state | play | Inform, TADS, Dialog |
| Flow | discourse and choice | play | Ink, Twine, ChoiceScript |
| **Knowledge** | **what is true, on whose say-so** | **authoring** | articy (commercial, no provenance semantics); **De Lore Is** |

Positioning against both lineages at once is the load-bearing claim of the export
layer (`spec/09-exports.md`): a knowledge layer that is *not* a runtime is
exactly what can feed many runtimes.

## SyntaxBudget, demoted honestly

The in-house SyntaxBudget probe is **not a basis.** It is a small, single-model
experiment whose direction agrees with the published record — grammar regularity
and canonical form as the reproducible levers, task complexity dominating failure
variance. The load-bearing citations for the surface are Stefik & Siebert, Green
& Petre, and Mernik et al.; SyntaxBudget informs the *benchmark's* design, not
the grammar's defense. Its one honest residual value: nothing published measures
*agent* emission of a novel grammar, so the probe remains the only data point on
that specific question — a data point, not a basis.

**What remains genuinely unvalidated.** No empirical study of *this* grammar
exists, for humans or for agents. The regrounding narrows the bet; it does not
discharge it. Two instruments discharge it: the authoring benchmark (the
agent-side instrument, pass defined by the real gate) and a small human-writer
probe before the surface freezes (the gentle subset, Stefik-style comprehension
and transcription tasks, a cognitive-dimensions walkthrough as interview
scaffold). Until both report, every approachability claim is a hypothesis with
good priors, written as one.

## Full citation list

**Empirical PL and DSL methodology.** Stefik & Siebert, "An Empirical
Investigation into Programming Language Syntax," ACM TOCE 13(4), 2013, and
Quorum's evidence-oriented practice; Green & Petre, "Usability Analysis of Visual
Programming Environments: A Cognitive Dimensions Framework," JVLC 7(2), 1996;
Mernik, Heering & Sloane, "When and How to Develop Domain-Specific Languages,"
ACM Computing Surveys 37(4), 2005; Myers, Pane & Ko, "Natural Programming
Languages and Environments," CACM 47(9), 2004.

**Fiction and game lineage.** Ink (inklestudios.com); Yarn Spinner
(yarnspinner.dev); ChoiceScript (choiceofgames.com); Inform 7 (assessed);
articy:draft X (articy.com); CastleDB (castledb.org); Font, Mahlmann, Manrique &
Togelius, "A Card Game Description Language" (EvoApplications, 2013); Ludii
(ludii.games); GDL (Stanford GGP, ggp.stanford.edu).

**In-repo and sibling evidence.** The fantasy naming stack
(`src/fantasy/naming/{vet,phonetics,compose}.py`, `data/registers.yaml`,
`data/ratified_names.yaml`); the craft stack
(`src/fantasy/craft/{outline,check,config}.py`, `data/craft_kb.yaml`);
`docs/proposals/DP-002-hard-magic-dramatic-function.md`;
`canon/world/{magic-system,the-turning}.md`; the SyntaxBudget probe (demoted); the
Chelis agent-authorship record and FlukeBall bust rates (see
`sibling-lessons.md`).
