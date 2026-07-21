---
status: live
title: "TPtH worldbook — Craft"
verdict: "The TPtH craft configuration behind spec 06: the D-0128 pillars that win over generic advice, the D-0166 hedge-register tension, the ten-reads-as-battle lexicon, and the episode-01 payload (Enfys / the door-ward)."
---

# Craft — the pillars and the hedge register

> **Worldbook.** This chapter faces [spec 06 · Craft and payloads](../../spec/06-craft-and-payloads.md)
> and documents the craft policy data at
> [`worlds/tpth/craft/world-principles.toml`](../../../worlds/tpth/craft/world-principles.toml)
> and [`worlds/tpth/craft/thresholds.toml`](../../../worlds/tpth/craft/thresholds.toml).
> The spec teaches the craft layer keeping the published craft-doctrine names
> (Sanderson, kishōtenketsu, MICE) but on camelot's decisions and register; the
> TPtH-specific pillar configuration and the hedge-register tension live here.

The published craft doctrine — try/fail before triumph, MICE nesting,
kishōtenketsu turns — is Tier-2 craft, legal to name and shared with the generic
manual. What is TPtH's own is the *configuration*: which pillars win, and the
particular register tension the mission adapts to.

## The principle configuration

The knowledge base ports as `principle` declarations, and the **pillars win**
(D-0128) where a mission-specific pillar overrides generic advice. TPtH's worked
principle set, with its own citations and its own register tension:

```dli
# grammar: v0 (unfrozen; AST normative)
scope craft

principle tryfail-before-triumph  family plot  mission pass
  source sanderson  cites D-0128
  checkable deterministic -> accomplishment-earned

rule accomplishment-earned over season severity error
  : beat $b kind accomplishment magnitude >= major
    requires beat $f  thread($f) = thread($b),
             outcome($f) in {yes-but, no-but, no-and},
             produced-before($f, $b)
  # season-wide join over the beat ledger; the old checker saw one outline

principle register-fit  family prose  mission adapted  cites D-0166
  tension "generic 'strong voice' advice; the hedge register wins"
  checkable judge -> attest register-fit
  # consuming checks report grade attested (07-verdicts-and-llm-boundary §3)

rule scene-name-clash over season severity error
  : scene $s casts $a, $b  where $a != $b
    requires aural-distance($a, $b) >= floor(scene)
  # joins the naming layer's confusability table with scene cast lists —
  # only checkable where name vetting and casts share a kernel
```

## The hedge-register tension (D-0166)

The `register-fit` principle carries a recorded tension: generic "strong voice"
advice pulls one way, and **the hedge register wins** (D-0166). TPtH's prose is
adapted to the hedgeway's register — the quiet, understated, thing-naming voice of
the enclosure country — so the standard maximal-voice advice is a screen the world
overrides by ruling, not by taste. This is why the principle is `mission adapted`
rather than `mission pass`: the resolution is recorded, not left as a live
argument.

## The episode-01 payload

The draft's structured shadow — TPtH's opening episode, the door-ward scene, with
its cast, beats, promises, and proof ledger:

```yaml
# episodes/s01/episode-01.payload.yaml — the draft's structured shadow
meta:
  episode: e01
  source: {file: episode-01.md, sha256: "9f3a…", words: 5210}
  extracted: 2026-07-19
  extractors:
    schema: payload/v1
    beats:  {model: claude-fable-5, prompt: "sha256:b41c…"}
    claims: {model: claude-fable-5, prompt: "sha256:77e0…"}
claims:                                # factcheck atoms
  - {atom: "holds(enfys, consent(household, door))",
     quote: "the house had said yes to her, in its way", line: 112}
deltas:                                # knowledge / possession / location / injury
  - {kind: knowledge, who: apprentice, fact: ward-needs-consent, scene: s04, line: 129}
scenes:
  - {id: s04, cast: [enfys, apprentice], location: gwern-threshold,
     phases: {tide: slack}, off_mic: [anchor]}
beats:
  - {id: b7, scene: s04, kind: accomplishment, thread: ward-the-door,
     magnitude: minor, by: enfys, assisted: false,
     quote: "the withy held", line: 131}
promises:
  opened: [{id: p-hound, kind: mystery, quote: "something four-legged kept…", line: 44}]
  closed: []
proof_ledgers:
  - scene: s04
    ledger: {premises: ["…"], steps: ["…"], close: {atom: "holds(ward(door))", expect: holds}}
names: {introduced: []}
lexicon: {banned_hits: [], register_tokens: 61}   # deterministic scans ride along
```

The lexicon and banned-vocabulary rows are deterministic scans, no model involved
— they ride in the payload so the report is one artifact. When a ruling lands,
this committed payload re-checks for free: a new D-entry that breaks the door-ward
proof ledger surfaces in the next `deloreis check`, not in an author reread.
