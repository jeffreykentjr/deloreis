---
status: live
title: "The authoring loop — a worked playbook"
verdict: "One character, its facts, and a relation, authored through the author.* tools and self-checked before materialize — every call's real request and response, and the fact objects the commit stores."
---

# The authoring loop — a worked playbook

This card walks one real authoring session end to end: open a workspace, add a
person with a trait, relate them to a second person, self-check, and
materialize. Every request and response below is the **engine-true** shape a
`deloreis` server returns — captured from the running worker at contract
`0.6.4`, not invented. Read `SKILL.md` first for the law; this is the muscle
memory.

The frames below are the stdio form (`deloreis serve`): a real request line is a
single JSON object `{"dli":"0.6.4","id":"<n>","op":"<tool>","params":{…}}`, and a
mutating call may also carry `"expected_digest"` (the fence) and
`"idempotency_key"` (safe-retry). Over MCP (`deloreis mcp`) the identical tools are
reached with a JSON-RPC `tools/call` whose `{name, arguments}` carry the same op
and params, and the worker response rides back in the result content — same tools,
same findings, same stored facts. Only `params` and the salient response are
shown here; the envelope's constant fields are elided for brevity.

**The answer is JSON; `.dli` is a rendering of it.** A mutating call returns
`findings`, a `workspace_digest`, and — only where a `.dli` renderer is linked —
`rendered`. The default build links no formatter at all (`dli_syntax` is
feature-gated out, roadmap LS-7), so `rendered` comes back empty and the fact
objects are the whole answer. The `rendered` arrays in this card are from a
`syntax`-enabled build: read them as a convenience for human eyes, never as
something you authored or could have typed instead.

---

## 1. Open a workspace

`author.open` stages over a scope. It returns the workspace id and the first
digest.

```json transcript
→ { "op": "author.open", "params": { "scope": "camelot" } }
← { "ok": true, "result": {
      "workspace": "ws-1",
      "workspace_digest": "jcs:sha256:9c49883e…a3b17a84" } }
```

Every later call names `"workspace": "ws-1"`. The digest is your fence: pass it as
`expected_digest` on a mutating call and a moved workspace is refused with
`stale-fence` before it stages anything.

Opening also **pins** the lorestore commit this session is relative to. It is
not the moving global head: whatever else lands while you work, your diff and
your commit are measured against the commit pinned here, and it comes back as
`root_hash` on `author.diff` and `author.status`. Two digest regimes ride side
by side and never share a field — `workspace_digest` (`jcs:sha256:…`) fences
*your staged state*; `root_hash` (`sha256:…`) names a *commit in the store*.

## 2. Add a person — and read the finding

You name an *intent* — "a person called Bedivere, proposed in this session" —
as typed params. There is no line of text to get right, and no `.dli` in the
request; the tool lowers the intent to fact objects, checks it, and answers.

```json transcript
→ { "op": "author.add_entity", "params": {
      "workspace": "ws-1", "kind": "person", "id": "bedivere",
      "name": "Bedivere",
      "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
← { "ok": true, "result": {
      "rendered": ["person bedivere \"Bedivere\" proposed session-7c9"],
      "workspace_digest": "jcs:sha256:e429b87c…1d939f68",
      "findings": [ {
        "code": "name.unvetted", "severity": "warn", "grade": "asserted",
        "message": "The name \"bedivere\" has not passed the naming oracle; it enters proposed and unvetted.",
        "repair": { "kind": "run-check", "op": "dli.name.check", "params": { "name": "bedivere" } } } ] } }
```

`name.unvetted` is `warn`, so it does **not** block materialize — it is an
advisory that the name never passed the oracle. To clear it, run the repair's
`dli.name.check` (add your scope's `dialect` — the oracle requires it):

```json transcript
→ { "op": "dli.name.check", "params": { "name": "bedivere", "dialect": "old-speech" } }
← { "ok": true, "result": { "ok": true, "findings": [] } }
```

## 3. Set a trait

```json transcript
→ { "op": "author.add_fact", "params": {
      "workspace": "ws-1", "entity": "bedivere", "key": "trade", "value": "knight",
      "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
← { "ok": true, "result": {
      "rendered": ["bedivere.trade = knight proposed session-7c9"],
      "findings": [] } }
```

## 4. Relate to an entity that is not there yet — an error-severity finding

`author.add_relation` stages the relation, but a reference to an entity not in
scope is an **`error`** finding: staged, yet it will bar materialize until the
reference resolves.

```json transcript
→ { "op": "author.add_relation", "params": {
      "workspace": "ws-1", "relation": "serves", "from": "bedivere", "to": "the-steward",
      "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
← { "ok": true, "result": {
      "rendered": ["serves(bedivere, the-steward) proposed session-7c9"],
      "findings": [ {
        "code": "entity.unknown", "severity": "error", "grade": "checked",
        "message": "No entity \"the-steward\" is in scope. The claim is staged but cannot materialize until it exists.",
        "repair": { "kind": "resolve-entity", "candidates": [] } } ] } }
```

The `resolve-entity` repair's `candidates` is empty here — there is no near-miss
id to correct to, so the fix is to **add the entity**. (When you fat-finger an
existing id, `candidates` lists the ids you most plausibly meant.)

## 5. Resolve it — add the missing person

```json transcript
→ { "op": "author.add_entity", "params": {
      "workspace": "ws-1", "kind": "person", "id": "the-steward", "name": "The Steward",
      "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
← { "ok": true, "result": {
      "rendered": ["person the-steward \"The Steward\" proposed session-7c9"],
      "findings": [ { "code": "name.unvetted", "severity": "warn", "…": "…" } ] } }
```

The relation from step 4 now resolves — the `entity.unknown` finding is gone from
the workspace because every declaration is re-checked against the *full* staged
state, not just the moment it was added.

## 6. Self-check before you materialize

The five read tools interrogate the staged world without mutating it. Start with
`author.findings` — your go/no-go pre-flight. Back at step 4, with the relation's
`entity.unknown` still open, it reported the gate closed:

```json transcript
→ { "op": "author.findings", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": {
      "errors":   [ { "code": "entity.unknown", "severity": "error", "repair": {…} } ],
      "warnings": [ { "code": "name.unvetted", "severity": "warn" } ],
      "materializable": false } }
```

`materializable:false` is the signal to fix before materializing — which step 5
did. You can prove a consequence with `author.query`. Ask for the goal as a
**typed atom** — `goal_atom` is the 0.6.3 successor to the rendered `goal`
string, carried structurally instead of re-parsed from text; a `Var` argument
is what the string form spelled `?who`. A proposed claim reads
`status:"possible"`, never `certain`:

```json transcript
→ { "op": "author.query", "params": { "workspace": "ws-1", "why": true,
      "goal_atom": {
        "pred": { "text": "serves", "span": { "file": 0, "start": 0, "end": 0 } },
        "args": [ { "Sym": { "text": "bedivere", "span": {…} } },
                  { "Var": { "text": "who",      "span": {…} } } ] } } }
← { "ok": true, "result": {
      "root_hash": "jcs:sha256:1659361c…2bbfddca",
      "rows": [ {
        "atom": "serves(bedivere, the-steward)",
        "bindings": { "who": "the-steward" },
        "status": "possible",
        "verdict": { "grade": "asserted", "support": { "monomials": [["+0"]] } },
        "why": { "antichain": [[0]], "rendered": ["h.session-7c9"] } } ] } }
```

The string `goal` still works and still means the same thing — it is a
registered field-level killed form, accepted through 0.6.x and retired at
contract 1.0. When both are present, `goal_atom` wins and `goal` is never
parsed. Prefer the typed form in new code.

Ask `author.why` why the row is not yet certain — `why_not` names the blocking
rulings; `known:false` would mean the world derives no such atom at all:

```json transcript
→ { "op": "author.why", "params": { "workspace": "ws-1", "atom": "serves(bedivere, the-steward)" } }
← { "ok": true, "result": { "why_not": {
      "atom": "serves(bedivere, the-steward)", "known": true, "status": "possible",
      "blocking": [0], "defeater": null,
      "rendered": ["awaits ruling: h.session-7c9"] } } }
```

`author.diff` is the **review** surface — the one supervision reads. Its `rows`
are structured per-fact changes with before/after standing; the consumer renders
them however it likes, and deloreis ships no UI (LS-5). A row absent `before` is
a fact that did not exist in the base at all:

```json transcript
→ { "op": "author.diff", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": {
      "root_hash": "sha256:1c1afc1e…daf45021",
      "rows": [
        { "atom": "serves(bedivere, the-steward)",
          "after": { "status": "possible", "grade": "asserted",
                     "support": [["+h.session-7c9"]] },
          "introduced": true, "retracted": false,
          "rendered": "serves(bedivere, the-steward): dead (asserted) -> possible (asserted)" },
        { "atom": "trade(bedivere, knight)", "…": "…" } ],
      "consequences": { "newly_possible": [ … ], "conflicts": [] },
      "workspace_digest": "jcs:sha256:1659361c…2bbfddca" } }
```

Read `support` literally: `+h.session-7c9` is the proposal-bundle variable your
session minted. The claim is *possible* precisely because that variable is
unruled — nothing you supplied made it certain, and nothing you can supply will.

With `the-steward` added, `author.check` runs the full battery clean and
`author.findings` flips the gate open:

```json transcript
→ { "op": "author.check", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": { "ok": true, "findings": [ { "code": "name.unvetted", "severity": "warn" }, … ] } }

→ { "op": "author.findings", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": { "errors": [], "warnings": [ … ], "materializable": true } }
```

`ok:true` and `materializable:true` — only the advisory `name.unvetted` warnings
remain, and warnings do not block. Materialize will succeed. (`author.status`
carries the same finding set and the same `diff_rows`; any of the read tools is a
valid pre-flight. The whole loop closes through `author.*` alone — no raw `.dli`,
no `dli.*` op.)

## 7. Materialize — commit the fact objects

`author.materialize` writes the staged facts to the lorestore as a commit and
advances your branch's ref to it. What comes back is **JSON first**: the
committed fact objects and the new commit hash. `rendered` is empty unless you
ask for it (contract 0.6.0, DL-0046).

```json transcript
→ { "op": "author.materialize", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": {
      "findings": [],
      "rendered": [],
      "root_hash": "sha256:ac0624cf…f68ed6b0",
      "workspace_digest": "jcs:sha256:1659361c…2bbfddca",
      "facts": [
        { "atom": { "pred": "__decl", "args": ["Entity", "bedivere"] }, "…": "…" },
        { "atom": { "pred": "person", "args": ["bedivere"] },           "…": "…" },
        { "atom": { "pred": "trade",  "args": ["bedivere", "knight"] }, "…": "…" },
        { "atom": { "pred": "serves", "args": ["bedivere", "the-steward"] }, "…": "…" },
        { "atom": { "pred": "__decl", "args": ["Entity", "the-steward"] }, "…": "…" },
        { "atom": { "pred": "person", "args": ["the-steward"] },        "…": "…" } ] } }
```

Six facts from four calls: each entity contributes both its typed declaration
(`__decl`, carrying the label and provenance in its `expert_body`) and the plain
atom the rules reason over. Nothing was dropped and nothing was invented.

### What one stored fact actually is

This is the fourth entry of that `facts` array in full — the canonical form of
your relation, and the thing whose JCS byte serialization is hashed to give it a
name:

```json fact-object-v1
{
  "schema_version": 2,
  "authority": null,
  "claim_id": "sha256:ea0c222b30c84ca5d23509d461c8e77dfb66ff8d63a488a7bd25f1bf97ce7b92",
  "atom": { "pred": "serves", "args": ["bedivere", "the-steward"] },
  "support": { "monomials": [["+0"]] },
  "defeaters": { "monomials": [] },
  "grade": "asserted"
}
```

Read it field by field. `atom` is the claim. `support` is a positive-Boolean
polynomial over ledger variables — one inner list per alternative derivation —
and here it is the single monomial `+0`. `defeaters` is empty: nothing argues
against it yet. `grade` is `asserted`, the weakest evidence and the strongest
word: your fiat, with nothing to re-run. **Standing is nowhere in the object.**
Certain / possible / dead is *derived* from support under a valuation, never
stored, which is why one saturation serves every fork and every what-if.

Note also what is *not* here: no `parent`, no commit pointer, no timestamp. A
fact is identified by its content alone, so the same claim made twice anywhere
is one object.

### The commit that names them

```json commit-v1
{
  "schema_version": 2,
  "authority": null,
  "parent": "sha256:1c1afc1e0025aa009123ba09de0f09a52672555156e0d68f86606f31daf45021",
  "facts": [
    "sha256:831eaec865f7268b3b6c71b441dc28a4f197217228b8b3369a8a6ef449f81f78",
    "sha256:056a2db800878f58335d4468fdcfe0147caa1e1ac52f92c997170d5c9c1fef91",
    "sha256:38451bf53df17159d2c4e2a97ae8d5bae7cf6794427297050d7c03417b0384b6",
    "sha256:4c13431ad3453808339ddd0fd1d1299f5b7b2c581fcb06f9555bbeee9b3ce1cd",
    "sha256:fa497031852d825097931fba50043cc33720a4e16a78dc2dab5be564d87644ad",
    "sha256:09d7e532480decc2754efb5f9eeb62ecb4eb5823fcfc8559af59583c2bb2049a"
  ],
  "lineage": [],
  "evidence": [],
  "interner": ["h.session-7c9"],
  "decisions": ["d-0001"],
  "pack_name": "camelot"
}
```

The `parent` is the `root_hash` `author.diff` reported — the base you were
pinned to at `author.open`, so the commit says exactly what it was built on.
And the `interner` is the key that decodes the fact: index `0` is
`h.session-7c9`, so that fact's `+0` support **is** your proposal bundle. The
claim holds only if a human rules that bundle in. Provenance is not a comment
on the claim; it is the claim's truth condition, and it is structural.

The ref your branch now points at:

```json ref-v1
{
  "schema_version": 2,
  "authority": null,
  "name": "main",
  "commit": "sha256:ac0624cfecc2835a6ce92fcfa51512fd0b617c95eb4ef147c35a0a88f68ed6b0",
  "valuation": {}
}
```

### The rendering, if you want it

Pass `rendered: true` and the same call returns the `.dli` view alongside the
facts — same commit, same `root_hash`, one extra array. It is a pretty-print
for human eyes, produced only by a build that links the formatter:

```json transcript
→ { "op": "author.materialize", "params": { "workspace": "ws-1", "rendered": true } }
← { "ok": true, "result": {
      "root_hash": "sha256:ac0624cf…f68ed6b0",
      "facts": [ … ],
      "rendered": [
        "scope camelot", "",
        "person bedivere \"Bedivere\" proposed session-7c9",
        "bedivere.trade = knight proposed session-7c9",
        "serves(bedivere, the-steward) proposed session-7c9",
        "person the-steward \"The Steward\" proposed session-7c9" ] } }
```

Which reads as:

```dli
scope camelot

person bedivere "Bedivere" proposed session-7c9
bedivere.trade = knight proposed session-7c9
serves(bedivere, the-steward) proposed session-7c9
person the-steward "The Steward" proposed session-7c9
```

Four readable lines against six fact objects — that is why the rendering is kept
at all. Read the tail of every line: `proposed session-7c9` is the surface form
of the `+0` support monomial you just decoded. It is the same fact, said shorter.
Reach for it when a human needs to *read* the change; reach for the fact objects
and the diff rows when anything needs to *check* it.

---

## Two moves you will reach for

**Drop a bad declaration** — name it by its rendered line (its first line, for a
multi-line declaration):

```json transcript
→ { "op": "author.discard", "params": {
      "workspace": "ws-1", "decl": "place lyonesse \"Lyonesse\" proposed sess" } }
← { "ok": true, "result": { "discarded": true, "workspace_digest": "jcs:sha256:5988a2fe…" } }
```

Omit `decl` to discard the whole workspace. A `decl` that matches nothing is an
`unknown-doc` refusal — it never falls through to nuking the workspace.

**Open a question as tracked debt** — it asserts nothing, takes no provenance:

```json transcript
→ { "op": "author.open_question", "params": {
      "workspace": "ws-1", "id": "who-holds-excalibur", "prompt": "Who holds Excalibur after Camlann?" } }
← { "ok": true, "result": { "rendered": ["question who-holds-excalibur \"Who holds Excalibur after Camlann?\""] } }

→ { "op": "author.add_option", "params": {
      "workspace": "ws-1", "question": "who-holds-excalibur", "id": "arthur", "label": "The King" } }
← { "ok": true, "result": { "rendered": ["  option arthur \"The King\""] } }
```

Which renders, for a human reading the change, as:

```dli
scope camelot

question who-holds-excalibur "Who holds Excalibur after Camlann?"
  option arthur "The King"
  option the-lake
```

A question asserts nothing, so no row of it ever reaches `certain` on its own —
it is a debt the world carries in the open, in place of a decision made in
someone's head.
