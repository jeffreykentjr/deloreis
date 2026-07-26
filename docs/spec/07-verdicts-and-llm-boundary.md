---
status: live
verdict: "Four classes, one kernel surface each; the lattice ranks evidence, the ledger ranks authority."
title: "Verdicts and the LLM boundary"
---

# Verdicts and the LLM boundary

The wall is settled: the factory owns every model call, and the kernel is
deterministic and offline. This document types what happens to the *outputs* of
those calls. Model-sourced content enters the system in four roles with four
verification stories, and each role has exactly one kernel input surface. It
also fixes how a verdict carries its confidence — the four-grade lattice over
two orthogonal channels (DL-0006) — so that a model's judgment can be recorded
honestly without ever becoming canon.

## 1. The grade lattice

Every verdict carries a grade from a four-rung lattice:

```text
checked > tested > attested > asserted
```

- **checked** — re-derives exactly; anyone re-runs it and gets the same result.
- **tested** — re-derives statistically; a battery over a committed spec and
  seed (10⁶ games, 0 hits).
- **attested** — a stored model judgment; can be re-posed, contested, escalated,
  but not re-derived.
- **asserted** — grounded in will; there is nothing to re-run.

**Two orthogonal channels.** A verdict is carried over a **support** channel and
a **grade** channel, and they never entangle:

- The *support* channel is the Boolean expression over citations and assumptions
  — **what a conclusion rests on** ([02-semantics](02-semantics.md)).
- The *grade* channel is **how the procedure stacked on that ground was
  verified.**

Rollup is **weakest-grade, taken as the minimum over the procedure chain — never
over the citations.** This is what keeps the fourth grade insertable without
breaking anything, and it answers the collapse worry directly.

**The collapse worry, answered.** If `asserted` is the bottom and all canon
bottoms out in authorial fiat, does every verdict that touches canon roll down
to `asserted`? No. A proof-check over D-cited premises reports grade `checked`
even though those premises are ratified by fiat, because the fiat rides in the
*support* channel and the checking procedure rides in the *grade* channel.
`asserted` appears as a grade only when a claim is taken with **no procedure at
all** — "this invariant holds; author's word."

Worked, on one verdict — a proof-check whose premises are ratified by fiat but
whose checking procedure re-derives exactly:

```text
verdict: valid   grade: checked
  support: D-0113 ∧ D-0146            # what it rests on — ratified, i.e. fiat
  grade chain: project(checked) → account(checked) → close(checked) = checked
```

The support channel records that the conclusion rests on two rulings; the grade
channel records that every step of the *procedure* stacked on them re-derives
exactly. Weakest-grade rollup takes the minimum over `{project, account, close}`
— the procedure chain — and never descends into the citations, so the fiat under
D-0113 does not drag the verdict down to `asserted`. Swap the exact projection
for a Monte-Carlo metric and the `close` link becomes `tested`, so the rollup is
`tested`; the support channel is untouched.

**Asserted is the strongest word and the weakest evidence.** The objection
writes itself: the author outranks the model, so how can authorial fiat grade
below a model's opinion? Because the grade axis measures the *recheckability of
the establishing procedure*, not the *authority of the speaker*. Checked: anyone
re-derives it. Tested: re-derives it statistically. Attested: can be re-posed
and contested. Asserted: there is nothing to re-run — the claim is grounded in
will, which is not a defect, because canon is *supposed* to bottom out in fiat.
Authority lives in the valuation layer, and there the author is supreme: a
ruling can kill any attestation, and no attestation can touch canon. The
one-liner ([koan 6](../charter.md)): **the lattice ranks evidence; the ledger
ranks authority.**

**The lattice is the upgrade path read backwards.** `asserted → attested`: write
the question into the store and let a sweep answer it. `attested → tested`: build
the battery. `tested → checked`: prove it. An `asserted` grade is an invitation,
not an insult — and `deloreis report --min-grade tested` is how the author asks
what still rides on say-so.

## 2. The four classes

Every factory model call declares which of four classes its output is, and each
class has exactly one kernel input surface:

| Class | What it is | Example | Human review | Kernel surface |
|---|---|---|---|---|
| **Attestation** | a stored model judgment on a bounded question | "Is Myrdin a real Welsh given name?" | never, by design | the attestation store (read-only input to checks) |
| **Candidate** | model-generated content that wants to become canon | thirty hamlet names; a proposal bundle | always — sign-off *is* ratification | hypothesis declarations, via Door T |
| **Encoding** | model transcription of human content into formal form | cue → proof-ledger; draft → payload | of the rendered form, spot-wise; schema + fixtures always | declarations with `encoded-by` provenance |
| **Gloss** | model prose over structured output | the fork-diff sidebar summary | n/a | none — never stored |

**The structural claim.** There is **no door through which an unclassified model
utterance can reach the kernel** — not as a matter of policy, but because no
other input surface exists. A model utterance is one of these four kinds, or it
does not enter. Agents that produce candidates and encodings do so through Door
T, the semantic tools against the `deloreis` server (DL-0014;
[11-agent-authoring](11-agent-authoring.md)); raw AST emission is a mechanism
inside those tools, not a door.

## 3. Attestations

An attestation is a model judgment that will **never receive human review — by
design, not neglect.** Author review time is the scarcest resource in the
system; questions with a fact-of-the-matter *outside* the repo — a register's
real basis ("is Myrdin Welsh?"), homophone plausibility, tone-fit — are exactly
the questions to spend model time on and zero author time. But "never reviewed"
does not mean "silently trusted." It means **stored, graded, and replayable**: an
attestation is a committed record, not a transient answer.

The project-backed worker stores accepted records as versioned RFC 8785 JCS
batches beneath
`.dli-evidence/attestations/<document>/batches/<sha256>.json`. Batches form one
content-addressed parent chain; records within them retain the bounded question,
typed yes/no answer, model and prompt identities, sampling policy, rationale
hash, staleness fingerprint, supersession link, and sweep provenance.

**The store lives with the corpus** as project-owned evidence, like simulation
manifests: machine-written, append-only per question, with the head record
answering it.

**The kernel never calls a model.** Project-backed `dli.check` stays offline and
deterministic and replays this durable evidence; standalone `deloreis check`
checks source files and has no project evidence input. A project check that needs
an attestation the store lacks reports
`attestation-missing` — **a finding, never a guess.** The factory answers demand
in **sweeps**: lab-run-shaped jobs that batch open questions, call models, and
commit records plus a manifest. Sweeps are factory work; the kernel only reads
what they commit.

**Why attested sits below tested.** Three reasons, each a property of the
evidence rather than of the speaker:

1. **Reproducibility.** `tested` evidence re-derives from a committed spec and
   seed — anyone re-runs the battery and gets the same numbers. An attestation
   replays the *question*, not the answer: API aliases drift, models deprecate,
   sampling wobbles. The record is replayable; the judgment is only weakly so.
2. **Calibration.** A battery states its evidence quantitatively (10⁶ games, 0
   hits); an attestation carries no distributional claim, and models are
   confidently wrong.
3. **The substitution test.** When a battery and a model disagree about the same
   claim, this house believes the battery — evidence that loses head-to-head is
   the weaker grade.

**No amount of model consensus lifts the grade.** Samples and agreement live in
the record (`samples: 3, agreement: "3/3"`), but the grade changes only when the
*procedure class* changes. Three models agreeing is still `attested`.

**Attestations are check-evidence, never support-polynomial variables.** This is
committed (DL-0006), for three reasons:

1. **Support variables are ratifiable-in-principle** — RQ options, proposals,
   things the author can rule on. An attestation is an observation, not a
   creative fork.
2. **If attestations entered support, certainty could be minted by model
   opinion.** "Certain modulo the model" violates *the machine never writes
   canon* ([koan 3](../charter.md)) at the semantic layer, not just the write
   layer. A fact can never become *certain* via attestation; a *check*
   (register-fit, homophone risk) can report grade `attested`.
3. **Drift semantics.** When a sweep re-poses `q-3f9c21` to a new model and the
   answer flips yes→no, a superseding record commits, and the next project-backed
   `dli.check` flips the consuming check from pass(attested) to
   fail(attested) — a diffable finding whose closed outcome implies the new
   answer and carries the new record id, the superseded record id and prior
   typed answer, and both staleness fingerprints — **while nothing in canon
   moves.** The name is ratified by its
   D-entry; the flip generates at most an RQ ("the model no longer believes the
   basis; keep, re-derive, or rule it explicitly"). If attestations were support,
   model drift would churn derived canon. Instead the flag flips like any drift
   finding, and that is the entire drift story.

**Ratification-exit.** The author may review an attested question whenever they
please. The ruling converts the question from judgment to fact: support now
cites the D-entry, the consuming check becomes a lookup, and the grade becomes
`checked` — a lookup is exact. The attestation chain is retained as history (the
guardrail-corpus rule, applied to model opinions). This is the two-channel
semantics demonstrating itself: the fiat rides in support; the procedure — now a
lookup — rides in grade.

## 4. Candidates

A candidate is model-generated content that wants to become canon: names,
proposal bundles, rule variants, encodings offered as content. Candidates enter
as **hypothesis declarations** authored through Door T
([11-agent-authoring](11-agent-authoring.md)), carrying origin metadata the
kernel enforces at the declaration — `{generated-by: campaign/model/date,
gauntlet: manifest-ref}` — not a convention.

Candidates are **queryable but structurally unassertable**: the certain layer
cannot cite a hypothesis variable, so no candidate can leak into a context pack
or a proof premise by any path. **Sign-off is valuation**: the author's D-entry
flips the variable and the derived consequences move by evaluation, never by
edit. **Rejection writes guardrails** — a rejected candidate is dead *and* a
constraint (killed-token registration is the naming-layer instance of the
general rule). Candidates have **no drift story**: they do not change, they wait;
and a waiting candidate is an open question, which is a debt — filed as an RQ,
surfaced for the author, never held in memory ([koan 5](../charter.md)).

## 5. Encodings

An encoding is model transcription of *human* content into formal form:
premise/conclusion cues → a proof ledger ([04-practices](04-practices.md)), a
full draft → a payload ([06-craft-and-payloads](06-craft-and-payloads.md)), a
`decisions.md` clause → a ledger row. **The checked artifact is the encoding** —
the kernel verifies what the encoder produced — so encoding infidelity is *the*
risk of this class, and it gets three mitigations:

1. **Schema-constrained emission** with golden / negative / framing fixtures —
   the worker-protocol discipline ([10-serving](10-serving.md)).
2. **Human review of the rendered form**: the three-column surface — prose with
   cue lines highlighted | encoding | verdict. The author approves the encoding
   or corrects the cues; approved encodings commit with the episode.
3. **Quote anchoring** where the source is text: every extracted item carries a
   verbatim quote and line, and the kernel mechanically verifies the quote
   appears at the claimed position — **a hard error otherwise.** The encoder
   cannot hallucinate anchors, and every downstream finding renders a real quote
   from the real draft.

Provenance is an `encoded-by` record — the attestation record shape minus the
judgment: model, prompt hash, source content-hash. **Staleness is mechanical**:
the encoding cites the source's content hash; editing the source raises a
staleness finding (the same family as the propagation staleness linter) and
queues re-encoding.

## 6. Glosses

A gloss is pure presentation: model prose over structured output, produced at
render time, always labeled, and **never stored, never parsed back.** The
structured verdict is the only thing downstream logic reads; the gloss is a
convenience for a human reader and has **no kernel surface at all.** There is
nothing to drift because there is nothing retained.

## 7. The negotiation protocol

| | Attestation | Candidate | Encoding | Gloss |
|---|---|---|---|---|
| **Who initiates** | a check (`attestation-missing`) or a re-attestation policy | agents / author-directed campaigns | the factory, when a source artifact needs its shadow | the UI, at render |
| **Factory does** | batch questions, call models, commit records + sweep manifest | generate, run the gauntlet, bundle, `promote` → RQ | schema-constrained call, fixtures, review surface | the call, the label |
| **Kernel stores** | the record; consuming checks read it, grade `attested` | hypothesis declarations with origin metadata (Door T) | declarations with `encoded-by` + source hash; verifies quotes | nothing |
| **What can flip later** | the answer, on re-attestation (check verdict flips, flagged); exit via ruling | the variable, on ruling (ratify → certain; reject → dead + guardrail) | the encoding, on source edit (staleness → re-encode) or review correction | n/a |
| **Drift story** | first-class: sweeps are lab runs; flips are findings; canon never moves | none — waiting is a filed debt, not drift | hash-mismatch staleness findings | none by construction |

**The boundary in one sentence:** the factory may say anything; the kernel stores
only typed things with provenance; the author alone makes things true; and every
model utterance is one of four kinds, or it does not enter.

## A parked koan

The charter records a parked koan candidate that states this boundary as law —
*"the model's word is evidence about the work, never support for the world"*
([charter](../charter.md), DL-0006). It is not adopted yet: twelve koans landed,
and this one waits for its demonstrated need, per the budget. The mechanism it
would name is already enforced by §3's third reason and the two-channel
semantics of §1; the koan would only make the enforcement a first-class law.
