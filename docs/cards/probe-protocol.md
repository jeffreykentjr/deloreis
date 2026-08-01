---
status: live
title: "The human-writer probe — protocol"
verdict: "Protocol only: the G1 comprehension-and-transcription probe of the gentle subset that gates the surface freeze. The probe itself is the author's act and stays open until run."
---

# The human-writer probe — protocol

This document specifies the **human-writer probe**: the G1 instrument that tests
whether the gentle subset is actually approachable for the writers it is for.
It is one of the two instruments named in the syntax spec (`../spec/01-syntax.md`
§8); the other is the P0 authoring benchmark, which tested the agent route and was
specified with the retired two-door semantic (spec §6; DL-0041, DL-0043).

**This is protocol only.** The probe itself — recruiting writers, running the
sessions, recording what happened — is the author's act. Until it has run, every
approachability claim about the surface is a hypothesis with good priors (spec
§8), and this protocol stays **open**.

---

## 1. Why it exists, and where it sits

No study of *this* grammar exists (spec §8). The prior art sets the direction —
word keywords, structured notes, the cognitive-dimensions method — but it does
not validate this particular surface. The probe is how the bet is narrowed
before it becomes expensive to unwind.

It is taken at **G1**, the last milestone at which a surface change is cheap.
Before G1, changing a keyword or a line shape is an edit. After G1 the grammar
version is frozen and any change is a canonical-to-canonical migration (spec §1,
§7). So the probe runs *before* the freeze, on purpose.

---

## 2. Participants

A handful — roughly five to eight — of **non-programmer writers**: fiction
authors, tabletop designers, worldbuilders. The population under test is the one
the gentle subset is for, not engineers who would forgive a rough surface. No
participant needs prior exposure to De Lore Is; the gentle card
(`gentle-card.md`) is the only material they are given.

---

## 3. The task set

Two kinds of task, both confined to the **gentle subset** (entities, facts,
timeline, questions, provenance). Nothing from the expert card appears.

**A. Comprehension.** The writer is shown a small world in surface text and
asked, in their own words, what it says — which claims are certain, which are
proposed, what question is open, what depends on what. Success is a faithful
read with no coaching. Example stimulus:

```dli
scope camelot

place camelot   "Camelot"   decided d-0004
place avalon    "Avalon"     proposed

person arthur "Arthur, the King"    decided d-0006
  arthur.blade = excalibur          proposed

question who-holds-excalibur "Who holds Excalibur after Camlann?"
  option arthur   implies keeps(arthur, excalibur)
  option the-lake implies keeps(the-lake, excalibur)
```

A comprehending reader should say, unprompted, that Camelot and Arthur are
settled, that Avalon and Arthur's blade are only proposed, and that who holds
Excalibur is an open question with two candidates.

**B. Transcription.** The writer is given a fact of their world in prose and
asked to write it as a claim. The measured thing is whether they reach a form
the parser accepts *and* whether they attach an origin without being told to.
Example target the writer should be able to produce:

```dli
person kay "Kay the Seneschal"   proposed
  kay.trade = seneschal          proposed
```

The transcription task is the sharp one: the spec's central bet is
*anti-Inform-7* — that what parses is exactly what the card shows, so a writer is
never lured into an unparseable phrasing (spec §4). Transcription is where that
bet is won or lost.

---

## 4. The interview scaffold — the cognitive-dimensions walkthrough

Each session is structured as a **cognitive-dimensions walkthrough** (spec §5),
used as an interview scaffold, not a questionnaire. The dimensions are prompts
for what to watch and ask about:

- **Closeness of mapping** — did the notation sit one line from the writer's
  mental object, or did they have to translate?
- **Premature commitment** — were they ever forced to decide something (an
  origin, an order) before they were ready? Proposed-first is meant to remove
  this; the probe checks that it does.
- **Viscosity** — when they wanted to change one claim, did it stay one edit?
- **Error-proneness** — where did they slip, and did the surface's shape prevent
  the slip or merely catch it?
- **Hidden dependencies** — could they see, or ask for, what a claim rests on?
- **Consistency** — did their second task feel predictable from their first?

The two paid dimensions are asked about directly, because a walkthrough that
names no cost has not used the framework (spec §5):

- **Diffuseness** — did one-claim-per-line feel verbose enough to resent, and
  was per-line blame worth the words to them?
- **Abstraction gradient** — did they miss a way to abstract or reuse, or did
  records-not-macros fit how they think about lore?

---

## 5. What is measured

Per participant, per task:

- **Comprehension accuracy** — did they read the certain / proposed / open
  distinctions correctly and unprompted?
- **Transcription success** — did they produce a form the real parser accepts,
  and did they attach an origin without prompting?
- **The graded ladder in use** — when the parser answered (the ladder of
  `../cards/…` speaks the domain, charter koan 9), did the writer understand the
  finding and repair from it, or stall?
- **Walkthrough notes** — the dimension-by-dimension observations above, kept as
  qualitative record, not a score.

The probe reports observations, not a pass number. Its output is evidence the
author reads, in the register the writers wrote in.

---

## 6. What counts as failure

The probe **fails** — and sends specific surface changes back while they are
still cheap — when any of these appear across participants, not as a single
stumble:

- writers routinely **misread provenance** (take a proposed claim as settled, or
  miss an open question);
- writers routinely reach for a phrasing the parser rejects and **cannot recover
  from the finding** — the anti-Inform-7 bet not holding;
- a **gentle keyword's name misleads** — writers consistently expect it to mean
  something it does not;
- the graded ladder is **read as noise** rather than as help (a koan-9 failure at
  the surface).

A failure is not a verdict to abandon the surface; it is a named, located
surface change to make **before** G1, when it is still an edit.

---

## 7. How it gates the freeze (SURF-4)

The surface freeze is recorded as a decision when it lands, and that decision is
**gated on this probe having run first** (spec §8). Concretely:

1. The freeze may not be proposed until the probe has run and its observations
   are recorded (as evidence the ruling cites).
2. Any §6 failure is resolved — by a surface change made pre-G1 — before the
   freeze decision is written. Changing the surface after the freeze is a
   versioned migration, not an edit; the whole point of the gate is to spend
   those changes while they are cheap.
3. The freeze decision names the probe as the evidence it rests on, so a later
   reader can see the surface was frozen on measurement, not on assumption
   (charter koan 6: a verdict never outranks its evidence).

Until the probe runs, SURF-4 stays blocked and this protocol stays open.
