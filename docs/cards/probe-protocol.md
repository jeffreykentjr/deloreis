---
status: live
title: "The human-writer probe — protocol"
verdict: "Protocol only: the comprehension probe of the gentle rendering. The freeze it was written to gate (SURF-4) is retired by DL-0041; what survives is whether a human can read a rendered diff correctly. The probe itself is the author's act and stays open until run."
---

# The human-writer probe — protocol

This document specifies the **human-writer probe**: the instrument that tests
whether the gentle subset is actually legible to the writers it is for. It is
one of the two instruments named in the syntax spec (`../spec/01-syntax.md` §8);
the other is the P0 authoring benchmark, which tested the agent route and was
specified with the retired two-door semantic (spec §6; DL-0041, DL-0043).

> **Standing (DL-0041, DL-0043).** The freeze this protocol was written to gate
> — SURF-4 — is **retired, not deferred**. The text surface closed, `.dli`
> became an opt-in rendering rather than the canonical form, and there is no
> grammar contract left to freeze. What the probe measures is therefore
> narrower, and still worth measuring: not whether a writer can *produce* the
> surface, but whether they can *read* it. Humans meet a De Lore Is world
> through rendered diffs and structured review rows (`SKILL.md` §7), so a
> rendering that readers misconstrue is a real defect with a real cost — a
> wrong review, not a syntax error. The comprehension half (§3A) survives
> whole. The transcription half (§3B) tested an authoring route that no longer
> exists; it is kept below as the record of what was asked, not as a live task.

**This is protocol only.** The probe itself — recruiting writers, running the
sessions, recording what happened — is the author's act. Until it has run, every
legibility claim about the rendering is a hypothesis with good priors (spec §8),
and this protocol stays **open**.

---

## 1. Why it exists, and where it sits

No study of *this* rendering exists (spec §8). The prior art sets the direction —
word keywords, structured notes, the cognitive-dimensions method — but it does
not validate this particular surface. The probe is how the bet is narrowed
before it becomes expensive to unwind.

It was scheduled at **G1**, the last milestone at which a surface change was
cheap, because after the freeze any change would have been a
canonical-to-canonical migration (spec §1, §7). That calculus changed with
DL-0041: a rendering carries no frozen contract, so a keyword rename now rides
the killed-form registry like any other retirement, and the probe is no longer
racing a deadline. What it still buys is evidence — a claim that the rendering
is readable, held to the same standard as any other verdict (koan 6).

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
timeline, questions, provenance). Nothing from the expert card appears. Task A
is the live one; task B is recorded history (see Standing, above).

**A. Comprehension — the live task.** The writer is shown a small world as it
renders and asked, in their own words, what it says — which claims are certain,
which are proposed, what question is open, what depends on what. Success is a
faithful read with no coaching. This is exactly the act a reviewer performs on
a rendered diff, which is why it is the half that survived. Example stimulus:

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

**B. Transcription — retired, recorded.** As specified, the writer was given a
fact of their world in prose and asked to write it as a claim; the measured
thing was whether they reached a form the parser accepts *and* whether they
attached an origin unprompted. The target they should have been able to produce:

```dli
person kay "Kay the Seneschal"   proposed
  kay.trade = seneschal          proposed
```

This was the sharp task, because the spec's central bet was *anti-Inform-7* —
that what parses is exactly what the card shows, so a writer is never lured into
an unparseable phrasing (spec §4). It is retired because the question it asks no
longer arises: a writer never transcribes prose into `.dli`, and the equivalent
act — naming an intent to a tool — is answered by graded findings with typed
repair rather than by a parser's silence. What the bet was protecting still
matters, and it moved: the finding a writer gets back must be one they can act
on. That is measured in §5's *graded ladder in use*, under task A.

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
- **Viscosity** — when they wanted one claim changed, could they see what
  changing it would take?
- **Error-proneness** — where did they misread, and did the surface's shape
  prevent the misreading or merely make it recoverable?
- **Hidden dependencies** — could they see, or ask for, what a claim rests on?
- **Consistency** — did the second world they read feel predictable from the
  first?

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
- **The graded ladder in use** — when the system answered (the ladder of
  `../cards/…` speaks the domain, charter koan 9), did the writer understand the
  finding and repair from it, or stall?
- **Walkthrough notes** — the dimension-by-dimension observations above, kept as
  qualitative record, not a score.
- *(retired)* **Transcription success** — did they produce a form the real parser
  accepts, and did they attach an origin without prompting? Recorded for the
  history of the instrument; not measured, since §3B is retired.

The probe reports observations, not a pass number. Its output is evidence the
author reads, in the register the writers wrote in.

---

## 6. What counts as failure

The probe **fails** — and names specific changes to the rendering — when any of
these appear across participants, not as a single stumble:

- readers routinely **misread provenance** (take a proposed claim as settled, or
  miss an open question);
- a **gentle keyword's name misleads** — readers consistently expect it to mean
  something it does not;
- the graded ladder is **read as noise** rather than as help (a koan-9 failure at
  the surface).

A failure is not a verdict to abandon the rendering; it is a named, located
change to make to how a world prints. Because the rendering carries no frozen
contract (DL-0041), such a change is a renderer edit plus a killed-form entry
for anything retired — not a corpus migration.

---

## 7. What it is evidence for

The freeze it once gated (SURF-4) is retired. What the probe now underwrites is
the standing claim that **the rendering is readable** — the claim `SKILL.md` §7
leans on when it tells an agent to review its own rendered output, and the one a
human reviewer leans on when reading a rendered diff instead of the structured
rows. Concretely:

1. Until the probe has run, "a human can read this correctly" is a hypothesis
   with good priors, not a verdict (spec §8). Say it that way.
2. Any §6 failure is a named change to the renderer, made and recorded; nothing
   about it is expensive to unwind now.
3. Whatever ruling ever rests on the rendering's legibility names this probe as
   the evidence it rests on (charter koan 6: a verdict never outranks its
   evidence).

Nothing is blocked on the probe any more — the milestone that was is gone. This
protocol stays **open** for a plainer reason: the claim it would settle is still
unsettled, and saying so is cheaper than pretending otherwise.
