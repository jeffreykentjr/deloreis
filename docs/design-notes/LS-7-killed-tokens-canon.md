---
status: accepted
verdict: "Empirical record of how the killed-tokens canon domain is derived from store fact objects (DL-0064). Ground-truth: canon_export over /tmp/s5b byte-identifies to committed worlds/tpth/generated/parity.json. This is evidence behind DL-0064, not a re-litigation of it."
---

# LS-7 — killed-tokens canon projection

This note records the empirical derivation behind DL-0064, the design call that
resolves the killed-tokens blocker in `docs/integration/STATUS.md`. It is
evidence, not authority — it pins down *what the store actually holds* and *how
the projection reads it*, so a future engineer can replay the verdict.

## Source of truth

The killed-tokens domain is the first per-domain normalizer to land under the
lorestore pivot (DL-0041). It projects four maps from the folded fact objects at
`head`:

```
canon.killed_tokens.{killed, worker_projection, rejected_claims, excluded_ratified_surface_collisions}
```

These four maps are byte-identical to `compile_lore_dli.py`'s `killed_tokens()`
output, which `worlds/tpth/generated/parity.json` (committed) carries as
`killed-tokens.yaml`. The projection reads **only** the live store, via
`dli.corpus.export{document:"tpth",format:"canon"}` — no file reads, no clocks,
no models (DL-0007).

## Inputs read from the store

`canon_killed_tokens(serialized_objects, by_pred, interner)` consumes:

- **`serialized_objects`** — the `FactObject`s at `head`, in **commit order**
  (`store.fact_hashes_from(head)` walks the history in append order = `facts.dli`
  source order = the registry row order). This is the ordered Vec the domain
  lists iterate.
- **`by_pred`** — a `BTreeMap<String, Vec<Value>>` keyed by `atom.pred`. The Vec
  *within* each bucket preserves commit order; the bucket keys sort
  alphabetically. Used for predicate-bucketed lookups (e.g. `by_pred["killed"]`,
  `factcheck-reject-*` keys). `by_pred` is **not** used to order the projected
  lists — only the commit-order Vec is — because the guardrail cue sequence
  would otherwise be alphabetized and no longer byte-match `killed_tokens()`.
- **`interner`** — 0-based: `+N` → `interner[N]`. `+1` → `D-0108` (ALL 20 killed
  facts carry `support.monomials [["+1"]]` — the ratified batch-002 authoring
  decision). `+58` → `D-0009`; `+59`,`+60` → `D-0010`,`D-0033`; `+62` → `D-0002`;
  `+45` → `D-0104`.
- **`decisions`** — 68 ratified decision ids (for context; the projection resolves
  support monomials through the interner, not this census).

## The four maps, reconstructed

### `killed` (20) — the verbatim registry row

Each `killed` fact is `atom = {pred:"killed", args:["killed-token-registry", <json>]}`.
`args[1]` is a JSON **string** (parsed via `serde_json::from_str`) whose shape is
`{token, match, kind, scope, decision}`. The census emits this object **as-is**
(after the scope split below), preserving every key.

**`match` is data, not a function of the token.** It is read verbatim from the
stored fact (`token_obj["match"]`). The earlier projection derived it via
`token.contains(' ')` → `"phrase"`, which marked `doom-wight`/`doom-ward` as
`"word"` (no space) — wrong; the source registry declares them `"phrase"`
(hyphenated). Reading verbatim fixes both. `Salt and Rust` → `"phrase"`
(contains space); all others → `"word"`.

**Colophon scope split.** The source `killed_tokens.yaml` Colophon row is a YAML
flow-mapping:
`scope: series title — clunky, mismatched, suspected AI-literary-vocabulary tic`.
YAML parses this as `scope="series title — clunky"` + `mismatched=null` +
`suspected AI-literary-vocabulary tic=null` (a 7-key row; the only row with
commas in scope). The authored store fact flattens the scope back to a single
string. The projection reverses the flatten: it `split(", ")`s the scope; the
first piece is `scope`, the rest become sibling `null`-valued keys. The other 19
rows have no `, ` in scope and pass through as the 5-key `{token,match,kind,scope,decision}`.

### `worker_projection` (19) — killed minus ratified surfaces

Same as `killed` but filtered to tokens whose `token.to_lowercase()` is **not** in
`ratified_surfaces`, and rendered as the 4-key `{token, match, decision, scope}`
(`kind` dropped). Colophon appears here with `scope="series title — clunky"` (the
split short form). Count = 19 (Frankpledge excluded — it is a ratified surface).

### `excluded_ratified_surface_collisions` (1) — Frankpledge / D-0138

The single killed row whose `token.to_lowercase()` ∈ `ratified_surfaces`.
`frankpledge` is a real declared entity (a `person`/`thing` declaration surface),
casefolded to itself; `audrey` is not in any declaration surface so it stays in
the 19. Emitted as the full 7-key (re-split) row verbatim.

`ratified_surfaces` is derived from every `(actor, pred)` pair in the folded
buckets (entity ids + `factcheck-alias`/`name` second-args, casefolded). This is
sufficient for the `Frankpledge` collision — no separate entity census is needed.

### `rejected_claims` (5) — guardrail reject cue

Iterates `serialized_objects` (commit order) for atoms whose `pred` starts with
`factcheck-object-`; takes the suffix `key`; keeps only `key`s present in
`rejected_keys`. `rejected_keys` = the suffixes of `factcheck-reject-*` predicates
(the registry cue `status == "rejected"`). This is what drops `gender` (D-0096 is
ratified → only a `factcheck-assert-*` cue keys it, so `gender` never enters
`rejected_keys`).

Each surviving fact's `support.monomials[0]` literals (`+N`) are resolved through
the interner to recover the decision id; one row is emitted per literal:
`{predicate, subject, value, match, decision, scope}` where `scope = "rejected
{subject} {key}"` and `match` is derived from `value.contains(' ')` (NOT read
from the fact — the object fact carries no match field; `compile_lore_dli` derives
it the same way).

**Order:** `[D-0009 role-auditor, D-0110 antagonist, D-0033 antagonist, D-0002
mechanism, D-0104 ticking-clock]`. The `D-0110` row is present even though
D-0110 is not in the store's `decisions_at` census — the support monomial `+59`
resolves to `D-0110` via the interner (loaded from the imported batch).

**Two exclusions** (mirror `killed_tokens()`):
- `gender`/D-0096: ratified → not in `rejected_keys` → never emitted.
- `mortality`/D-0120 vs D-0089: `factcheck-object-mortality` has a support literal
  resolving to D-0120, BUT `(protagonist, mortality)` ∈ `asserted_slots` (a plain
  `fact protagonist mortality <value>` exists in `facts.yaml`), so the slot is
  filtered — the lore owns it, the guardrail cannot also claim it.

## Empirical verification

Over `/tmp/s5b` (exact copy of `worlds/tpth` at base hash + 20 authored `killed`
facts + the `killed-token-registry` declaration):

```
canon == parity: True   (json.dumps(..., sort_keys=True) equality)
killed:               20/20
worker_projection:    19/19
rejected_claims:       5/5
excluded:              1/1
by_pred: 223 keys (incl killed=20, factcheck-alias=11, 30 factcheck-reject-*,
          6 factcheck-assert-*, 6 factcheck-object-*, guardrail=1, +~50 lore)
interner: 68 symbols (0-based)
decisions: 68 ratified ids
```

`/tmp/s5b` tip root_hash after authoring: `sha256:6a165d818d2826b390326451d2c98cc366a5f6eaf366f4162c8f779beb71df4d`,
578 claim-bearing objects (556 base + 22), interner 68 symbols, decisions 68.
Unchanged — no re-authoring needed; the projection reconstructs the census form
from the store's flat-scope `killed` facts.

## Replay recipe

```
cargo build -p deloreis --features syntax --target-dir /home/jeff/tg-dm4fix
dm4fix mcp --project /tmp/s5b/worlds/tpth/lorestore-project.toml
dli.corpus.export{document:"tpth",format:"canon"}   # omit at:
```

Extract: `resp["result"]["content"][0]["text"]` → `json.loads` →
`["result"]["canon"]["killed_tokens"]`.

## Status

Closed. Gate `cargo xtask gate --ci` → 21/21 green. Independent red-team
(fresh-context reconstruction of the four maps from raw `objects` export + a
planted divergence) passes GREEN before the plant and RED after. See DL-0064.
