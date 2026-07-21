---
status: live
title: "The TPtH worldbook"
verdict: "The worked TPtH instances extracted from the engine specs — the Concordance, cymric-fringe/Enfys, the Turning export, the era spine, the banned-vocab rationale — as diegetic prose plus the fenced blocks lifted verbatim from the specs. The specs teach the generic engine on the shipped generic worlds; this book carries the TPtH depth."
---

# The TPtH worldbook

*To Pleach the Hedgeway* (TPtH, D-0143) is one world authored in De Lore Is. The
engine specs under [`docs/spec/`](../../spec/00-overview.md) teach the generic
tool on the shipped generic worlds — [`examples/camelot.dli`](../../../examples/camelot.dli)
(the Matter of Britain) and [`examples/games/french-deck/`](../../../examples/games/french-deck/)
(a standard 52-card game). This companion carries the worked TPtH instances the
specs used to illustrate with, extracted here so a non-TPtH reader meets the
manual without a private glossary, and a TPtH author finds the depth in one place.

Each chapter faces one spec chapter and documents the `worlds/tpth/` data it
draws on:

| Chapter | Faces | Documents |
|---|---|---|
| [Time](time.md) | [spec 03 · Story-time](../../spec/03-time.md) | the era spine, the cycles + the Turning's leaf(29), the water/enclosure fluent, the D-0104 narration frame — [`worlds/tpth/time/cycles.dli`](../../../worlds/tpth/time/cycles.dli) |
| [The Concordance](concordance.md) | [spec 04 · Practices](../../spec/04-practices.md) | the spoken-proof practice (linear-conserved debt, the nine errors, licensed-ignorance / the rigged-deck reading), the e01-s04 proof ledger, Frith conservation — [`worlds/tpth/practices/spoken-proof.dli`](../../../worlds/tpth/practices/spoken-proof.dli) |
| [Language](language.md) | [spec 05 · Language layer](../../spec/05-language-layer.md) | cymric-fringe, Enfys, the hedgeway/the-plenty lexicon, the banned machine/algorithm rationale, the cymric agent-init bundle — [`worlds/tpth/packs/cymric-fringe.dli`](../../../worlds/tpth/packs/cymric-fringe.dli) |
| [Craft](craft.md) | [spec 06 · Craft and payloads](../../spec/06-craft-and-payloads.md) | the D-0128 pillars, the hedge-register tension, TPtH's principle configuration, the ep-01 payload — [`worlds/tpth/craft/world-principles.toml`](../../../worlds/tpth/craft/world-principles.toml) |
| [The Turning export](turning-export.md) | [spec 09 · Exports](../../spec/09-exports.md) | the Turning deck anatomy (D-0148), the cards/lowlands profile, keep-stamped/salt-fringe variants, the-debt-square, the ludii differential — [`worlds/tpth/turning/the-turning-rules.json`](../../../worlds/tpth/turning/the-turning-rules.json) |

Everything here is diegetic TPtH lore. The authority of record for that lore is
the fantasy repository's `canon/` and its `D-nnnn` ledger; the engine ledger
([`../../decisions.md`](../../decisions.md), `DL-nnnn`) governs only the tool.
Where the two ever disagree about TPtH content, canon wins.

The fenced blocks below are lifted **verbatim** from the specs they were extracted
from — they parse exactly as they did in place, and the docs oracle re-checks them
here. What the specs keep is the generic machinery; what moved here is the TPtH
instance that machinery was illustrated on.
