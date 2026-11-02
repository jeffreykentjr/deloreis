---
status: live
verdict: "Follow a checked authoring session through The Hollow."
---

# The authoring loop

This session starts from a copy of `examples/hollow`, whose first ruling says
Wren lives in Thornfield. The complete [transcript](../../examples/hollow/transcript.md)
is generated and checked by the MCP driver. Each block below is the arguments
object for a `tools/call`; replace `WORKSPACE` with the ID returned by
`author.open`.

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

The [remaining transcript](../../examples/hollow/transcript.md) shows the
actual output after recording the drowning ruling, defeating its support with
`d-0004`, and defeating that retcon with `d-0005`. It also shows a branch diff
and the answer after a server restart. The driver in
`crates/deloreis/tests/hollow.rs` contains the complete calls, including the
claim IDs and branch operations.
