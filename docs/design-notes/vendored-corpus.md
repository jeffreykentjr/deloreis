---
status: reference
title: "The vendored fantasy corpus and pinned oracles"
verdict: "fantasy owns the source; deloreis owns the pinned snapshot — cut once from a commit, hash-verified, and self-provided for the parity lanes."
---

# The vendored fantasy corpus and pinned oracles

De Lore Is is built against a real world, not a toy one. The consuming fantasy
project — *To Pleach the Hedgeway* (D-0143) — is the parity and dual-run input
for the later integration lanes (CHECK-1, LANG-2, MIG-1) and the seed material
for the detection corpus (PROOF-1). This note records what is vendored, the exact
pin, why the corpus is self-provided and grown at the edges, how to refresh it,
and where its status is tracked.

It is `status: reference`: settled procedure, reopened only by new ledger
evidence, never re-argued here.

## What is vendored

Three self-provided artifacts, all cut from **one pinned commit** of the fantasy
repository and living under `deloreis` ownership:

1. **The corpus snapshot** — `fixtures/vendor/fantasy-40f6679/`. A `git archive`
   of the pinned tree, carrying:
   - `canon/**` — the whole story bible (49 files: characters, geography, society,
     narrative, world, texts, and `canon/data/**` — `characters.yaml`,
     `facts.yaml`, `timeline.yaml`, `banned-vocabulary.yaml`, `lexicon.yaml`).
   - `episodes/**` — the per-episode continuity fixtures (3 files).
   - `src/fantasy/naming/data/**` — the ratified-name mirror, register/phonetic
     data, and the factory-side screens (ai-tics, collisions, homophones) (10 files).
   - `src/fantasy/propagate/data/killed_tokens.yaml` — the killed-token registry.

   63 files total. `MANIFEST.txt` in that directory carries the pinned SHA, a
   per-file sha256, and a top-level tree hash (sha256 over the sorted per-file
   lines). It verifies `sha256sum -c`–style: `cd fixtures/vendor/fantasy-40f6679
   && sha256sum -c MANIFEST.txt` (the `#` header lines are ignored by `sha256sum`).

2. **The pinned oracles** — `scripts/pinned-oracles.sh`. A lab-side, **non-gating**
   wrapper that provisions a pristine fantasy checkout at the same pinned SHA
   (via `git archive`, never the working tree), runs `uv sync` once, and exposes
   its `uv run fantasy …` oracles as subcommands (`validate`, `world-build`,
   `factcheck <draft>`). This is how the deloreis kernel is dual-run against the
   legacy engine on identical inputs. It is **never** part of `cargo xtask gate`:
   the deloreis gate is deterministic, offline, and model-free.

3. **The detection corpus** — `fixtures/mutation/{canon,draft,episode}.yaml`.
   The fantasy mutation-operator catalogue (33 operators) ported to DATA seeds for
   PROOF-1: each operator's seeded change plus the deloreis kernel check class that
   must flag it. Index and taxonomy in `fixtures/mutation/README.md`.

## The pin

```
pinned-sha:  40f66792229abf5d73d995a8167adc79152a64e7
pinned-ref:  origin/main   (resolved at vendor time)
source-repo: /home/jeff/Documents/scratch/fantasy
tree-hash:   92c343e72b408a8b149f105d0e7dd5fd1e02534dd7a38fe9f18830b5b70f5d1d
```

The pin is a **commit**, resolved once with `git -C <fantasy> rev-parse
origin/main` and recorded. The fantasy tree is a shared, live checkout that other
agents dirty; vendoring from the committed object at a pinned SHA — never from the
working tree — is what makes the snapshot reproducible. The same SHA is the
default in `scripts/pinned-oracles.sh` and in every `fixtures/mutation/*.yaml`
provenance block, so the corpus, the oracles, and the detection seeds all speak
about one identical world.

## Why self-provided and grown at the edges

The boundary rule (`decisions.md` DL-0010) is that three artifacts cross the
deloreis ↔ fantasy seam, each with one owner: the contract (deloreis owns), the
worked-scenario fixture (deloreis owns), and **the vendored corpus snapshots —
fantasy owns the source, deloreis owns the pinned snapshot.** Per the author's
ruling that the fantasy-side EXT artifacts are in scope for this sprint and that
the example world should **expand at the edges**, this lane self-provides the
snapshot rather than waiting on a cross-repo hand-off: deloreis cuts and owns the
pin, and `examples/**` grows the `.dli` surface world the later lanes test
against. Growing at the edges — new practices, a second register, an episodes
pack — exercises the parameterization (koan 10) without touching the pinned canon,
which stays a frozen reference.

Vendoring never writes the fantasy tree, and nothing here is a second writable
copy of fantasy's world: the snapshot is read-only reference input, and the
authoritative world remains the fantasy repo's.

## Refresh procedure

The pin is deliberate; refresh is a decision, not a drift. To advance it:

1. **Re-pin.** `git -C <fantasy> rev-parse origin/main` → the new SHA; record it.
2. **Re-archive.** `git -C <fantasy> archive <new-sha> canon episodes
   src/fantasy/naming/data src/fantasy/propagate/data/killed_tokens.yaml | tar -x
   -C fixtures/vendor/fantasy-<new-short>/`.
3. **Regenerate `MANIFEST.txt`** (pinned SHA + per-file sha256 + tree hash) and
   verify `sha256sum -c`.
4. **Re-port the detection corpus** if `src/fantasy/mutation/operators.py` or the
   gate-coverage matrix changed — every operator in the inventory must appear.
5. **Re-run the pinned oracle** at the new SHA (`scripts/pinned-oracles.sh
   validate`) to confirm the checkout still provisions and validates clean.
6. **Update this note and every `pinned-sha:` reference** (the oracle script
   default and the mutation provenance blocks), then re-run the docs-oracle over
   `examples/**`.

The old snapshot directory is retained, not deleted — history is the guardrail
corpus, here as elsewhere.

## Status tracking

The coordination-point (CP) rows on `docs/integration/STATUS.md` for the
vendored-corpus and pinned-oracle artifacts receive their **self-provided**
markers in **wave 4**, when the STATUS board is reconciled. Until then, this note
and the `MANIFEST.txt` are the authoritative record that the snapshot exists, at
which pin, and how to verify it.
