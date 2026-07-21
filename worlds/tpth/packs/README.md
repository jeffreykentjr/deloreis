# Examples

The shared `.dli` test corpus every later lane validates against. The `v0`
grammar is unfrozen but the parser has **landed**: every `.dli` file here parses
with **zero diagnostics** under `dli_syntax::parse_document`, and the docs-oracle
enforces it (a rejected file fails the gate). The AST (`crates/dli-ast`) is the
normative surface; these examples are its golden inputs.

Each `.dli` file opens with the `# grammar: v0 (unfrozen; AST normative)` marker,
matching the docs' `dli` blocks. Every claim carries an origin word — `proposed`,
or `decided`/`ratified`/`provisional d-nnnn` — or the parser diagnoses it. Block
bodies and closing braces are indented, matching the spec's
practice/proof/dialect examples.

**Episodes ride as payload data, not `.dli` surface, this wave.** The `.dli`
chronology surface commits era/event/before/narration/cycle and `rule … over order` only; the
EPI-1 boundary ruling made episodes serde data payloads (the DraftPayload
precedent), and their surface form arrives with TIME-3. So the episodes example
is `saltmarket-episodes.yaml`, a serialization of `dli_world::episode` types, and
is not probed (the docs-oracle probes `.dli` files only).

- `saltmarket.dli` — the base world: a totally-ordered **era spine** (≥3 eras),
  events with a `before`-fact and a **deliberately unordered pair**, an
  **adjacency graph** (7 places, ≥6 edges), people and a thing with facts,
  **states** with `since`/`until` (the two-era water-status case), and an open
  **question** with options under a **hypothesis**. Provenance is the world's own
  decision list (`decided d-0001…`), per 01-syntax §3.
- `true-naming.dli` — the arbitrariness proof of 04-practices §4, completed: a
  practice whose resource is **graded-monotone decay**, whose names are
  **dialect-derivations**, and whose premises are **knowledge states**. Three
  layers (magic, language, story-time) in one pack, zero kernel code.
- `second-practice.dli` — **the Turning**, a divination practice structurally
  **disjoint** from both proof-magic and true-naming (affine-bounded spread,
  cycle-phase timing failures, figures laid on a cyclic kind). The second
  arbitrariness proof: a wholly unrelated art hosting for free (04-practices §4).
- `cymric-fringe.dli` — the language-layer **dialect pack** of 05-language-layer
  §1–§5: dialects, name-parts, a word and terms, and names as typed
  derivations, carrying **both** provenances — cited (D-ref) canon and proposed
  (agent) candidates under a hypothesis bundle — with the attestation seam (§7).
- `saltmarket-episodes.yaml` — three episodes over the saltmarket world, as a
  serialization of `dli_world::episode` types (the `Vec<Episode>` the fold
  consumes). Production `production_order` and story `story_anchor` are bound
  separately, one **flashback scene** carries its own `scene_anchor` into an
  earlier era, and **knowledge deltas** move facts to the listener and cast
  (03-time §4, §6). Payload data until TIME-3 lands the episode `.dli` surface.

The docs-oracle (`cargo run -p xtask -- docs-oracle`) enforces probe-compatibility
across the `.dli` corpus; it upgrades to a full compile when the parser exists.
The `.yaml` episodes payload is validated by `dli-world`'s own serde round-trip,
not the probe.
