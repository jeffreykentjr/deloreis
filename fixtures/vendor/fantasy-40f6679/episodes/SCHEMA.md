# Episode-state schema — the continuity contract

An `EpisodeState` is one episode's **delta** over the world model. It records the
facts an episode settles and the ones it revises, tracks who learned what, follows
items and injuries as they move between characters, marks where each character ends
the episode, and logs any promise or debt opened or repaid. The checker
(`fantasy.world.continuity.check`) validates a delta against the assertable world
(`canon` compiled by `fantasy.world.compile`) and the run of prior episodes.

This schema is data, not prose. It is authored as YAML and loaded with
`EpisodeState.model_validate`. Unknown keys are rejected.

## Fields

| Field | Shape | Meaning |
|---|---|---|
| `episode` | str (required) | Stable episode id, e.g. `ep-0`. |
| `title` | str | Human label. |
| `order` | int | Sequence position; prior states are sorted by it. |
| `facts_established` | list of `{entity, key, value}` | Facts the episode presents as settled. |
| `facts_changed` | list of `{entity, key, from_value, to_value}` | State transitions. |
| `knowledge` | list of `{who, entity, key, value}` | Who learned which fact (the information economy). |
| `possessions` | list of `{who, item}` | Who holds what at episode end. |
| `injuries` | list of `{who, condition}` | Bodily state at episode end. |
| `locations` | list of `{who, place}` | Where each character ends. |
| `promises` | list of `{id, kind, from_party, to_party, opened, closed, note}` | Promises/debts opened or settled. |
| `references` | list of str | Entity ids the episode leans on (checked for existence). |

Values are lowercase-hyphen tokens (`male`, `cu-sith-moor-spirit`, `5-to-9`), the
same normalization the world model uses, so an episode fact compares directly to a
canon fact.

## What the checker catches

Six violation classes, one code each:

- **`WORLD_CONTRADICTION`** — an established or changed fact whose value disagrees
  with an *assertable* canon fact for the same `entity.key`.
- **`PROPOSED_AS_FACT`** — an established or changed fact whose only canon basis is
  `proposed`/`open-question` (shadow) lore — including any fact whose *entity* is a
  shadow-only (proposed) entity. Canon says such lore "must not be asserted, must not
  be contradicted"; stating it as settled breaks the first half.
- **`UNKNOWN_ENTITY`** — a referenced entity id canon does not define in either
  layer. New entities enter through canon and the naming oracle, not an episode.
- **`PRIOR_CONTRADICTION`** — a `from_value` that disagrees with the last known
  value, an established fact that reverses an earlier episode, or a promise/debt
  closed that was never opened.
- **`INTRA_EPISODE_CONTRADICTION`** — the same `entity.key` is set to two different
  values within one episode (self-contradiction, which the prior-state checks miss).
- **`REJECTED_LORE`** — the delta realizes a machine-checkable negative constraint
  (a rejected concept/name, banned vocabulary, a banned structure): `check()` consults
  `world.constraints()` so rejected lore cannot pass continuity.

A clean delta yields zero violations. See `docs/reference/lore-engine-hardening.md` for the
red-team pass that added the last two classes and the shadow-entity broadening.

## The Dialog-backbone mapping

An episode delta is representable as **Dialog predicate deltas** over the same flat
triple store the world emitter uses (`fantasy.world.continuity.to_predicate_deltas`).
This keeps the door open to replaying an episode against a compiled world and letting
the engine prove consistency; the pure-Python checker is that contract realized today.

| Delta element | Dialog predicate line |
|---|---|
| `facts_established` | `(now) (fact-of @entity @key @value)` |
| `facts_changed` | `(now) ~(fact-of @entity @key @from)` then `(now) (fact-of @entity @key @to)` |
| `knowledge` | `(now) (knows @who @entity @key @value)` |
| `possessions` | `(now) (#who holds #item)` |
| `locations` | `(now) (#who is #in #place)` |
| `promises` | `(now) (promise-open @id)` / `(now) (promise-closed @id)` |

## Fixture

`fixtures/episode-0.yaml` is a synthetic episode-zero built to trip every violation
class at once (checked against the real compiled world, with no prior episodes). It
is a test artifact, never a real episode.
