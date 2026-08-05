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
| **CP-1** — contract freeze | v0 schemas + the closed fixture registry | the vendored copy + a parity run | provided | tag `contract/1.0.0` at `76888052`; `docs/integration/fantasy-contract.md` at 1.0.0 truth on that commit |
| **CP-2** — pinned oracles | the shadow-read harness slot | the pinned legacy gates (EXT-F1) | proposed | — |
| **CP-3** — vendored corpus | the dual-run consumer | canon snapshots at a pinned commit (EXT-F2) | proposed | — |
| **CP-4** — detection corpus | detection proven separately | the seeded divergence corpus (EXT-F5) | proposed | — |
| **CP-5** — core shadow-read | dual-run parity (validator/continuity/factcheck) | the pinned oracles + corpus | proposed | — |
| **CP-6** — naming shadow-read | the LANG-2 kernel-half view | the legacy `name vet` + external screens | proposed | — |
| **CP-7** — the batch RQ | the offset→before compiler + the draft RQ | the author's ruling | proposed | — |
| **CP-8** — source deletion | the conversion + the reviewed diff | the review + the deletion commit | proposed | — |
| **CP-9** — worker on the contract | `deloreis serve` + a tagged build | the fantasy worker on the vendored contract (EXT-F3) | provided | tag `contract/1.0.0` at `76888052` |
| **CP-10** — authoring-tools handshake | a tagged build with the `author.*` tools + the worked-scenario fixture | their agent exercises the scenario against the tagged build + posts the transcript (EXT-F4) | verified | tag `contract/1.0.0` at `76888052`; the AUTH-2 worked-scenario fixture under `worlds/tpth/auth2/`, replayed by `cargo test -p dli-serve --test auth2`. Acknowledged and verified together at the Stage-3 close: the consumer ran EXT-F4 against the tagged build and their posted transcript is byte-parity verified against that fixture (fantasy `e4a51fa2`) |

The source tree currently implements contract `1.3.0`, served **multi-version
dual-mode**: it accepts `0.5.1`, `0.6.0`–`0.6.4`, `1.0.0`, `1.1.0`, `1.2.0`, and
`1.3.0`, answering each caller in the version it declared, so a `0.5.1` request
still receives byte-identical `0.5.1`-shaped responses and every vendored v1
fixture remains in the registry as the standing compatibility suite (DL-0046,
DL-0051, DL-0053, DL-0055, DL-0056, DL-0058). All six export emission branches
are implemented.

Engineering readiness became delivery when the annotated tag `contract/1.0.0`
was cut on merge commit `76888052`, the commit whose tree carries the contract
doc at 1.0.0 truth. That tag is the deloreis deliverable named by CP-1, CP-9,
and CP-10, so those three rows stood at `provided`. Two of them still do:
`acknowledged` needs the consumer's vendored SHA and digest-check output, and
`verified` needs their replay artifact — neither is evidence deloreis can
produce for itself.

**CP-10 is now `verified`.** The consumer ran EXT-F4 against the tagged build and
posted the transcript; it is byte-parity verified against the AUTH-2 fixture this
side owns. The row moves from `provided` straight to `verified` because both
rungs were earned at once and are recorded together — the acknowledgement is in
the evidence cell rather than lost. Later tags (`contract/1.2.0`, `contract/1.3.0`)
do not restate CP-10: the fixture is deliberately frozen at `0.5.1` terms, so the
transcript that verified it is not re-run per tag.

When a point advances, deloreis updates its **State** and fills **Evidence** with
a link (a fixture path, a tagged build, a transcript). A point never advances
without its evidence.
