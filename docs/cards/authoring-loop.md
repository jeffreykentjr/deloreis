---
status: live
title: "The authoring loop — a worked playbook"
verdict: "One character, its facts, a relation, and an event, authored through the author.* tools and self-checked before materialize — every call's real request and response."
---

# The authoring loop — a worked playbook

This card walks one real Door T session end to end: open a workspace, add a
person with a trait, relate them to a second person, add an event, self-check,
and materialize. Every request and response below is the **engine-true** shape a
`deloreis` server returns — captured from the running worker, not invented. Read
`SKILL.md` first for the law; this is the muscle memory.

The frames below are the stdio form (`deloreis serve`): a real request line is a
single JSON object `{"dli":"0.4.2","id":"<n>","op":"<tool>","params":{…}}`, and a
mutating call may also carry `"expected_digest"` (the fence) and
`"idempotency_key"` (safe-retry). Over MCP (`deloreis mcp`) the identical tools are
reached with a JSON-RPC `tools/call` whose `{name, arguments}` carry the same op
and params, and the worker response rides back in the result content — same tools,
same findings, same rendered surface. Only `params` and the salient response are
shown here; the envelope's constant fields are elided for brevity.

---

## 1. Open a workspace

`author.open` stages over a scope. It returns the workspace id and the first
digest.

```json
→ { "op": "author.open", "params": { "scope": "camelot" } }
← { "ok": true, "result": {
      "workspace": "ws-1",
      "workspace_digest": "jcs:sha256:4b8f5ec2…1bf7703b" } }
```

Every later call names `"workspace": "ws-1"`. The digest is your fence: pass it as
`expected_digest` on a mutating call and a moved workspace is refused with
`stale-fence` before it stages anything.

## 2. Add a person — and read the finding

```json
→ { "op": "author.add_entity", "params": {
      "workspace": "ws-1", "kind": "person", "id": "bedivere",
      "name": "Bedivere",
      "provenance": { "kind": "proposed", "origin": "session-7c9" } } }
← { "ok": true, "result": {
      "rendered": ["person bedivere \"Bedivere\" proposed session-7c9"],
      "workspace_digest": "jcs:sha256:2d3650bb…6413ea0f",
      "findings": [ {
        "code": "name.unvetted", "severity": "warn", "grade": "asserted",
        "message": "The name \"bedivere\" has not passed the naming oracle; it enters proposed and unvetted.",
        "repair": { "kind": "run-check", "op": "dli.name.check", "params": { "name": "bedivere" } } } ] } }
```

`name.unvetted` is `warn`, so it does **not** block materialize — it is an
advisory that the name never passed the oracle. To clear it, run the repair's
`dli.name.check` (add your scope's `dialect` — the oracle requires it):

```json
→ { "op": "dli.name.check", "params": { "name": "bedivere", "dialect": "old-speech" } }
← { "ok": true, "result": { "ok": true, "findings": [] } }
```

## 3. Set a trait

```json
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

```json
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

```json
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

```json
→ { "op": "author.findings", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": {
      "errors":   [ { "code": "entity.unknown", "severity": "error", "repair": {…} } ],
      "warnings": [ { "code": "name.unvetted", "severity": "warn" } ],
      "materializable": false } }
```

`materializable:false` is the signal to fix before materializing — which step 5
did. You can prove a consequence with `author.query` (an argument written `?who`
enumerates; a proposed claim reads `status:"possible"`, never `certain`):

```json
→ { "op": "author.query", "params": { "workspace": "ws-1", "goal": "serves(bedivere, ?who)", "why": true } }
← { "ok": true, "result": { "rows": [ {
      "atom": "serves(bedivere, the-steward)",
      "bindings": { "who": "the-steward" },
      "status": "possible",
      "verdict": { "grade": "asserted", "support": {…} } } ] } }
```

and ask `author.why` why it is not yet certain — `why_not` names the blocking
rulings; `known:false` would mean the world derives no such atom at all:

```json
→ { "op": "author.why", "params": { "workspace": "ws-1", "atom": "serves(bedivere, the-steward)" } }
← { "ok": true, "result": { "why_not": {
      "atom": "serves(bedivere, the-steward)", "known": true, "status": "possible",
      "defeater": null, "rendered": ["awaits ruling: h.session-7c9"] } } }
```

With `the-steward` added, `author.check` runs the full battery clean and
`author.findings` flips the gate open:

```json
→ { "op": "author.check", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": { "ok": true, "findings": [ { "code": "name.unvetted", "severity": "warn" }, … ] } }

→ { "op": "author.findings", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": { "errors": [], "warnings": [ … ], "materializable": true } }
```

`ok:true` and `materializable:true` — only the advisory `name.unvetted` warnings
remain, and warnings do not block. Materialize will succeed. (`author.diff` and
`author.status` render the same staged block for a final eyeball; the whole loop
closes through `author.*` alone — no raw `.dli`, no `dli.*` op.)

## 7. Materialize — get the file bytes as data

```json
→ { "op": "author.materialize", "params": { "workspace": "ws-1" } }
← { "ok": true, "result": {
      "findings": [],
      "rendered": [
        "scope camelot", "",
        "person bedivere \"Bedivere\" proposed session-7c9",
        "bedivere.trade = knight proposed session-7c9",
        "serves(bedivere, the-steward) proposed session-7c9",
        "person the-steward \"The Steward\" proposed session-7c9" ] } }
```

The `rendered` lines are the canonical `.dli` file, including the `scope` header
and the blank line after it. The kernel writes nothing: **you** write these bytes
to your repo's staging path and open a pull request. This is the surface a human
reviews:

```dli
scope camelot

person bedivere "Bedivere" proposed session-7c9
bedivere.trade = knight proposed session-7c9
serves(bedivere, the-steward) proposed session-7c9
person the-steward "The Steward" proposed session-7c9
```

---

## Two moves you will reach for

**Drop a bad declaration** — name it by its rendered line (its first line, for a
multi-line declaration):

```json
→ { "op": "author.discard", "params": {
      "workspace": "ws-1", "decl": "place lyonesse \"Lyonesse\" proposed sess" } }
← { "ok": true, "result": { "discarded": true, "workspace_digest": "jcs:sha256:5988a2fe…" } }
```

Omit `decl` to discard the whole workspace. A `decl` that matches nothing is an
`unknown-doc` refusal — it never falls through to nuking the workspace.

**Open a question as tracked debt** — it asserts nothing, takes no provenance:

```json
→ { "op": "author.open_question", "params": {
      "workspace": "ws-1", "id": "who-holds-excalibur", "prompt": "Who holds Excalibur after Camlann?" } }
← { "ok": true, "result": { "rendered": ["question who-holds-excalibur \"Who holds Excalibur after Camlann?\""] } }

→ { "op": "author.add_option", "params": {
      "workspace": "ws-1", "question": "who-holds-excalibur", "id": "arthur", "label": "The King" } }
← { "ok": true, "result": { "rendered": ["  option arthur"] } }
```

The rendered surface a human reviews:

```dli
scope camelot

question who-holds-excalibur "Who holds Excalibur after Camlann?"
  option arthur
  option the-lake
```
