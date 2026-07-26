---
status: live
verdict: "The deloreis-side coordination board for the fantasy integration. Single-writer (deloreis). Every coordination point starts at proposed; the paired fantasy-side file is the other half of the handshake."
---

# Integration STATUS — deloreis side

This is the **deloreis-side** coordination board for the fantasy integration. It
is **single-writer**: only deloreis writes this file. The fantasy repository
keeps its own mirror, written only by fantasy. Read together, the pair is the
handshake — neither side edits the other's file, so there is no write race.

**States**, in order:

- **proposed** — the coordination point is defined; no artifact exchanged yet.
- **provided** — deloreis has delivered its side (a tagged build, a fixture, a
  harness slot).
- **acknowledged** — fantasy has received and vendored/consumed it.
- **verified** — both sides' fixtures/transcripts agree; the point holds.
- **closed** — verified and no longer active work.

The coordination points and what each side owes are defined in `migration.md`
§7. This board tracks their state and links the evidence.

| CP | What deloreis provides | What fantasy provides | State | Evidence |
|---|---|---|---|---|
| **CP-1** — contract freeze | v0 schemas + the closed fixture registry | the vendored copy + a parity run | proposed | — |
| **CP-2** — pinned oracles | the shadow-read harness slot | the pinned legacy gates (EXT-F1) | proposed | — |
| **CP-3** — vendored corpus | the dual-run consumer | canon snapshots at a pinned commit (EXT-F2) | proposed | — |
| **CP-4** — detection corpus | detection proven separately | the seeded divergence corpus (EXT-F5) | proposed | — |
| **CP-5** — core shadow-read | dual-run parity (validator/continuity/factcheck) | the pinned oracles + corpus | proposed | — |
| **CP-6** — naming shadow-read | the LANG-2 kernel-half view | the legacy `name vet` + external screens | proposed | — |
| **CP-7** — the batch RQ | the offset→before compiler + the draft RQ | the author's ruling | proposed | — |
| **CP-8** — source deletion | the conversion + the reviewed diff | the review + the deletion commit | proposed | — |
| **CP-9** — worker on the contract | `deloreis serve` + a tagged build | the fantasy worker on the vendored contract (EXT-F3) | proposed | — |
| **CP-10** — authoring-tools handshake | a tagged build with the `author.*` tools + the worked-scenario fixture | their agent exercises the scenario against the tagged build + posts the transcript (EXT-F4) | proposed | — |

The source tree currently implements pre-v1 contract `0.5.0`, including all six
export emission branches. That is engineering readiness, not delivery: CP-1,
CP-9, and CP-10 remain `proposed` until a tagged build and the corresponding
consumer-owned evidence exist.

When a point advances, deloreis updates its **State** and fills **Evidence** with
a link (a fixture path, a tagged build, a transcript). A point never advances
without its evidence.
