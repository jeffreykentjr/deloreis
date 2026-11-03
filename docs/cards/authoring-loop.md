---
status: live
verdict: "Follow a checked authoring session through The Hollow."
---

# The authoring loop

Start with a new world and make the first ruling that Wren lives in Thornfield,
following steps 1–6 of the [quickstart](../quickstart.md). Then use the calls
below to continue the session. The complete
[transcript](../../examples/hollow/transcript.md) is generated and checked by
the MCP driver using `--tools all`; use that profile for this walkthrough.
Each block below is the arguments object for a `tools/call`; replace
`WORKSPACE` with the ID returned by `author.open`.

## Read the existing ruling

Call `author.open` with `{"scope":"world"}`, then `author.query`:

```json
{"workspace":"WORKSPACE","goal_atom":{"pred":"home","args":[{"Sym":"wren"},{"Sym":"thornfield"}]}}
```

The checked transcript reports `home(wren, thornfield)` as `certain`, with a
claim ID and a support polynomial. `author.why` can explain the support.

## Stage a contradiction

Record `d-0002` with `author.record_decision`:

```json
{"workspace":"WORKSPACE","decision":"d-0002"}
```

Then call `author.add_fact`:

```json
{"workspace":"WORKSPACE","entity":"wren","key":"home","value":"ashby","provenance":{"kind":"cited","decisions":["d-0002"],"state":"ratified"}}
```

Call `author.check` with `{"workspace":"WORKSPACE"}`. The transcript's
finding names the two rulings and both homes:

```json
{"code":"continuity.fd-violation","severity":"error","citations":["d-0001","d-0002"],"data":{"pred":"home","slot_key":"home/wren","subject":"wren","values":["ashby","thornfield"]}}
```

This is an excerpt of the finding, omitting its grade and message. Calling
`author.materialize` returns `materialize-blocked`. Discard the staged
contradiction with `author.discard` and open a fresh workspace.

## Keep rival accounts possible

Open a question with `author.open_question`:

```json
{"workspace":"WORKSPACE","id":"miller-death","prompt":"How did the miller die?"}
```

Add these separately with `author.add_option` before materializing the question:

```json
{"workspace":"WORKSPACE","question":"miller-death","id":"witness-one","label":"witness-one","implies":{"pred":"death","args":[{"Sym":"miller"},{"Sym":"drowned"}]}}
```

```json
{"workspace":"WORKSPACE","question":"miller-death","id":"witness-two","label":"witness-two","implies":{"pred":"death","args":[{"Sym":"miller"},{"Sym":"poisoned"}]}}
```

Query each outcome with `author.query`, then ask `author.why` using the same
`goal_atom`. The captured results report each account as `possible` and name
its own witness in the explanation. Materialize, reopen, and the test checks
that both explanations survive.

## Retcon and explore

Record `d-0003`, add the drowning claim with that citation, and materialize it.
Query `death(miller, drowned)` with `layer: all` to get its claim ID. Then call
`author.defeat`, replacing `CLAIM_ID` with that ID:

```json
{"workspace":"WORKSPACE","claim_id":"CLAIM_ID","decision":"d-0004","defeated_decision":"d-0003"}
```

Add the poisoned account under `d-0004` and materialize it. Query
`death(miller, poisoned)` to get the new claim ID, then call `author.defeat`
again:

```json
{"workspace":"WORKSPACE","claim_id":"RETCON_CLAIM_ID","decision":"d-0005","defeated_decision":"d-0004"}
```

The drowning claim becomes certain again, and the rival account becomes
possible.

On main, call `author.add_rule` to say Wren is away from Thornfield if she has
not stayed:

```json
{"workspace":"WORKSPACE","name":"away-if-not-stayed","head":{"pred":"away","args":[{"Sym":"wren"},{"Sym":"thornfield"}]},"body":[{"atom":{"pred":"journey","args":[{"Sym":"wren"},{"Sym":"stayed"}]},"negated":true}],"provenance":{"kind":"cited","decisions":["d-0001"],"state":"ratified"}}
```

Materialize the rule, then create a branch with `author.branch`:

```json
{"workspace":"WORKSPACE","action":"create","from":"main","name":"never-left"}
```

Open it using `author.open` with `{"scope":"world","branch":"never-left"}`.
On the returned workspace, record decision `d-0006` with
`author.record_decision`:

```json
{"workspace":"BRANCH_WORKSPACE","decision":"d-0006"}
```

Then stage the alternative with `author.add_fact`:

```json
{"workspace":"BRANCH_WORKSPACE","entity":"wren","key":"journey","value":"stayed","provenance":{"kind":"cited","decisions":["d-0006"],"state":"ratified"}}
```

Add the welcome rule with `author.add_rule`:

```json
{"workspace":"BRANCH_WORKSPACE","name":"welcome-if-stayed","head":{"pred":"welcome","args":[{"Sym":"wren"},{"Sym":"thornfield"}]},"body":[{"atom":{"pred":"journey","args":[{"Sym":"wren"},{"Sym":"stayed"}]},"negated":false}],"provenance":{"kind":"cited","decisions":["d-0001"],"state":"ratified"}}
```

Call `author.diff` with `{"workspace":"BRANCH_WORKSPACE"}`. The diff shows
the welcome becoming certain and the away claim changing from certain to
possible. Main still reports the away claim as certain.

Finally, restart the server, open the main world, and call `author.query`:

```json
{"workspace":"WORKSPACE","goal_atom":{"pred":"death","args":[{"Sym":"miller"},{"Sym":"drowned"}]},"layer":"all"}
```

The original ruling is still certain. The transcript records each call and
its result.
