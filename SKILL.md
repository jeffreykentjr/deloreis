---
name: deloreis
description: Author De Lore Is (.dli) lore through the tool surface — the deloreis server's author.* tools — by naming intents and reading the graded findings each call returns. You never write .dli by hand; you self-check with the read tools before you materialize; nothing you supply mints canon.
status: live
verdict: "The v0 authoring manual: author through tools, self-check before materialize, read the graded feedback, obey provenance discipline; raw .dli editing is a measured fallback and the gentle card is for reading your own rendered diffs."
---

# Skill: authoring De Lore Is through the tool surface

## 0. The core law

You add lore to a De Lore Is world by **calling the `deloreis` server's `author.*`
tools** and reading the graded findings each call returns. Four rules govern
everything below, and they never bend:

1. **Author through tools, never text.** You name an intent — "add a person,"
   "set a trait," "order this event before that one" — and the tool lowers it to
   the world model, checks it, and renders it back. You do not type `.dli`
   surface, and you do not touch the AST. Hand-editing `.dli` is a *measured
   fallback* (§8), not a supported interface.
2. **Nothing you supply mints authority.** Every claim you add enters `proposed`
   or is `hypothesis`-scoped. A ruling — the act that makes a claim *true* — lives
   in the consuming repo's decision ledger and is a **human** act in review. You
   propose; the author confirms.
3. **You cannot cite.** The `cited` provenance form is human-only (B1). A
   `cited` provenance handed to any `author.*` tool is **refused** with the
   `provenance.cited-refused` finding — nothing is staged. Resubmit as `proposed`.
4. **The compile is the oracle, and it outranks this file.** The findings a tool
   returns are the authority on whether your lore is well-formed and consistent.
   If a finding disagrees with anything written here, the finding is right and
   this file is stale.

You never need to read Rust or write `.dli` to author or self-check a world. The
tools and their responses tell you everything: what you staged, what is wrong,
and exactly how to fix it.

## 1. The authoring loop, end to end

```
author.open                          open a workspace over a scope (or a fork)
  → author.add_* …                   stage intents; each answers with findings
  → self-verify                      author.diff / author.status  (+ the read
                                     tools of §4) — read findings BEFORE you commit
  → fix                              author.add_* the missing piece, or
                                     author.discard a bad declaration
  → author.materialize               get the .dli file bytes AS DATA
  → commit through your own review   write the bytes to your repo and open a PR
```

You reach the tools over a running `deloreis` server, on either of two
transports carrying the identical tool surface: **`deloreis mcp --project
<path>`** registers every tool for MCP (call them with JSON-RPC `tools/list` then
`tools/call`), and **`deloreis serve --project <path>`** speaks the same tools as
newline-delimited JSON over stdio. `<path>` is `deloreis.toml` or its containing
directory. Pick whichever your harness speaks; the tools, findings, and rendered
surface are the same on both. `--demo` is only for the bundled neutral fixtures.

Nothing you do touches disk or changes canon. All authoring happens in a
**workspace**: an in-memory staged layer over the compiled world. `author.open`
returns a workspace id (`ws-1`, `ws-2`, …) and a **workspace digest** (a
`jcs:sha256:…` fence over the staged state). Every later call names the workspace
in its params.

**Every mutating call returns three things, immediately:**

- **`findings`** — graded observations, each with a machine `code`, a
  domain-spoken `message`, and typed `repair` data (§3). They tell you exactly
  what is wrong and how to fix it.
- **`rendered`** — the **opt-in** `.dli` rendering of the result, a convenience
  view for a human reviewer. The canonical result is the structured JSON above
  (findings plus the staged fact objects / rows and the digest); `rendered` is
  returned only when asked (`rendered: true`, contract 0.6.0 / DL-0046) and is
  **empty on a build with no formatter linked** — the default build links none
  (LS-7). It is never the authoring surface: you propose through the typed tool
  params, not by writing these lines.
- **`workspace_digest`** — the new fence.

Read the findings on every call. Do not guess and re-emit — the tool already told
you. An ignored finding does not go away: it reappears in the next call's set and
blocks materialize.

## 2. The tool surface (19 tools; see contract §3.2 table)

**The eleven mutating tools** (each returns `{findings, rendered, workspace_digest}`) and
**three read-only state ops** (status, diff, materialize):

| Tool | Intent |
|---|---|
| `author.open` | Open a workspace: `{scope}` to stage over a scope, `{fork}` for fork-scoped exploration. |
| `author.status` | The staged declarations + current digest (read-only). |
| `author.diff` | The full rendered block of everything staged (read-only). |
| `author.discard` | Drop one staged declaration (by its rendered line) or the whole workspace. |
| `author.materialize` | Commit the staged fact objects to the lorestore and return them as JSON (`facts` + `root_hash`); `.dli` rendering is the `rendered: true` opt-in (DL-0046). Refuses if any error-severity finding is open. |
| `author.add_entity` | Add a `person` / `place` / `thing` / `era`. Provenance required. |
| `author.add_fact` | Set `entity.key = value`. Provenance required. |
| `author.add_relation` | Assert `relation(from, to)`. Provenance required. |
| `author.add_event` | Add a chronology event (an event is **not** an entity). Provenance required. |
| `author.add_before` | Order `earlier` before `later`. Provenance required. |
| `author.open_question` | Open a question — a tracked debt. `{id, prompt}`; no provenance (opening asserts nothing). |
| `author.add_option` | Add an option `{question, id, label}` to a question already open here. |
| `author.add_hypothesis` | Mint a hyp variable `{id, statement}` — fork workspaces only, one per bundle. |
| `author.suppose` | Pin staged hypotheses true `{hypotheses:[…]}` and read the consequence shift. |

**The five self-check read tools** let you interrogate the staged world *before*
you materialize (§4). All are read-only and mutate nothing; each names a workspace
in its params (except `author.list_workspaces`):

| Tool | Params | Answers |
|---|---|---|
| `author.findings` | `{workspace}` | `{errors, warnings, materializable}` — your pre-flight: `errors` bar materialize, `warnings` pass, `materializable` is the go/no-go boolean. |
| `author.check` | `{workspace}` | `{ok, findings}` — the full check battery (continuity + factcheck + rejected-lore) over the staged world; `ok:true` means no error-severity finding remains. New proposed claims are legitimately unsupported, so only contradictions surface — not "unsupported" noise. |
| `author.query` | `{workspace, goal, layer?, why?}` | `{rows}` — query the staged world; `goal` is a rendered atom, and an argument written `?` or `?who` is a variable that enumerates matches. A proposed claim reads `status:"possible"`, never `certain`. Unknown predicate → empty rows (not an error). |
| `author.why` | `{workspace, atom}` | `{why_not, why?}` — why an atom can't be certain yet: the blocking rulings and the defeater. `known:false` means the world derives no such atom. |
| `author.list_workspaces` | `{}` | `{workspaces:[{workspace, base, mode, staged, workspace_digest}]}` — context recovery across your open workspaces. |

The whole loop closes through `author.*` alone: you never touch a `dli.*` batch op
or a raw `.dli` file to author *or* self-check. (`author.diff` and `author.status`
also return the full finding set and rendered surface, so any of them is a valid
pre-flight.)

## 3. Reading the graded feedback

Each finding has two independent axes. Do not conflate them.

- **`severity` gates materialize.** `error` blocks `author.materialize`; `warn`
  passes through. This is the only gate. An `error` finding still *stages* the
  claim — nothing is dropped — but you cannot materialize until the world is
  consistent again.
- **`grade` is evidence strength, never a gate.** The lattice is
  `checked > tested > attested > asserted`. A `checked` finding is machine-derived
  against the world model and authoritative; an `asserted` finding (e.g.
  `name.unvetted`) is a bare policy note with nothing to re-run. No amount of
  model agreement lifts a grade.

**Repair data is typed — act on it without parsing prose.** A finding that has a
mechanical next step carries it as structured data:

- `{"kind":"run-check","op":"dli.name.check","params":{…}}` — the exact worker
  call that clears the finding. (Note: `dli.name.check` also needs a `dialect`;
  supply your scope's dialect if the repair omits it.)
- `{"kind":"resolve-entity","candidates":[…]}` — the ids an unknown reference most
  plausibly meant. An empty list means no near match: add the entity, or fix the
  id.
- `{"kind":"add-provenance","required":"proposed"}` — the provenance form to
  resubmit with (you get this when you tried to `cite`).
- `{"kind":"remove-control-chars","field":"…"}` — strip a control/bidi character
  smuggled into that field.

**The `workspace_digest` is your fence.** If you pass an `expected_digest` that
does not match the workspace's current state, the call is refused with
`stale-fence` (it names both digests and tells you to read `author.status`). Your
view was stale; re-read before continuing.

**Errors vs. findings — two different channels.** A malformed call (bad enum,
missing field, unknown field, a `cited` provenance, a stale fence) is refused
*whole* as a typed **wire error** (`schema-violation`, `stale-fence`,
`materialize-blocked`, `unknown-doc`, …) and **stages nothing**. An
**error-severity finding** is different: it stages the claim but bars materialize.
Neither ever drops your input silently.

## 4. Self-check BEFORE you materialize

This is the discipline that keeps review cheap: never materialize a workspace you
have not interrogated. The read path costs nothing and mutates nothing.

- **`author.findings`** is your go/no-go pre-flight: `{errors, warnings,
  materializable}`. If `materializable` is `true`, materialize will succeed;
  otherwise the `errors` array is exactly what to fix, each with typed repair.
- **`author.check`** runs the full battery over the staged world and returns
  `{ok, findings}` — use it to catch a *contradiction* against the base (a
  proposed fact that fights a certain one), not just local reference errors.
- **`author.query`** proves a consequence before you commit to it: ask
  `serves(lancelot, ?who)` and read the rows. A proposed claim comes back
  `status:"possible"` — expected, since only a human ruling makes it certain.
- **`author.why`** answers "why isn't this certain yet?" — `why_not` names the
  blocking rulings and the defeater; `known:false` means the world derives no
  such atom at all (you referenced something that isn't there).
- **`author.diff`** / **`author.status`** render the whole staged block (and, for
  `diff`, every open finding and the derived-consequence shift) — the surface a
  human will review.
- **`author.materialize`** is the last gate: it **refuses** with
  `materialize-blocked` if any error-severity finding is open, naming the blocking
  finding codes. A clean materialize returns the `.dli` bytes and an empty
  `findings` array.

## 5. When you are stuck, the tools answer

You never have to guess or reach outside the tool surface:

- **A finding carries its own repair.** Unknown entity → `resolve-entity`
  candidates (or add it). Unvetted name → the `dli.name.check` call to run.
  Missing/wrong provenance → the form to resubmit with. Control character → the
  field to clean.
- **`author.why`** explains why an atom is or is not derivable, and why a finding
  fires — the blockers, named.
- **A killed form names its successor.** A retired op, keyword, or name is refused
  with the replacement in the message (for example `dli.suppose` is killed in
  favor of `dli.fork.suppose`). Use the named successor; never reintroduce the old
  form.
- **A wire error names the fix in plain words.** `stale-fence` tells you to read
  `author.status`; `materialize-blocked` lists the blocking codes; a bad enum
  lists the valid values.

## 6. Provenance discipline

- **Everything you add enters `proposed` or is `hypothesis`-scoped**, carrying
  your session origin. Only the author's ruling makes a thing true.
- **A rename is a supersession, never an in-place edit.** The new name is ruled
  in; the old name dies into lineage and the killed-form registry, with every
  claim's history intact. There is no edit that changes the past quietly.
- **An open question is a debt.** File it with `author.open_question`, surface it,
  and never carry it in your head — a waiting decision lives in the world, not in
  memory.

## 7. Reading a rendered diff — the gentle `.dli` card (reference)

Authoring is tools-only, but you **review your own rendered output**: `author.diff`
and `author.materialize` return canonical `.dli` lines, and a human reviews the
same surface. So you must be able to *read* `.dli` even though you never *write*
it. Read it as notes, not English — **what parses is exactly what the surface
shows** (`docs/cards/gentle-card.md` is the full one-page card).

A materialized workspace — the exact bytes `author.materialize` hands back for a
small cast:

```dli
scope camelot

person lancelot "Lancelot of the Lake" proposed session-7c9
lancelot.trade = knight proposed session-7c9
serves(lancelot, the-king) proposed session-7c9
person the-king "The King" proposed session-7c9
```

Read every claim's tail: `proposed session-7c9` says a proposal made in that
session stands behind it — out of canon until a human rules. A question renders
with its options indented one level, and asserts nothing:

```dli
scope camelot

question who-drew-the-sword "Who drew Excalibur from the stone?"
  option arthur
  option mordred
```

Time is eras, events, and orderings — every line still ending in its origin:

```dli
scope camelot

era arthurs-reign "Arthur's Reign" proposed session-7c9
event the-sword-drawn "The Sword Drawn" proposed session-7c9
event the-battle-of-camlann "The Battle of Camlann" proposed session-7c9
before the-sword-drawn -> the-battle-of-camlann proposed session-7c9
```

The worked call-by-call playbook that produces surfaces like these — every real
request and response — is `docs/cards/authoring-loop.md`.

## 8. The expert tier and the measured fallback

- **Expert tier.** Rules, forks with suppositions, cycles, practices (magic
  systems), proofs, dialects, and attestations are a second reference card
  (`docs/cards/expert-card.md`) and their lane specs under `docs/spec/`. Fork
  authoring uses `author.open {fork}`, `author.add_hypothesis`, and
  `author.suppose`; read the consequence shift in `author.diff`.
- **Raw `.dli` editing is a measured fallback**, not a supported interface. It meets the same
  gate and the same graded findings — nothing is lost — but no tooling is built
  for it and no reliability claim is made for it. Prefer the tool surface. If you must edit
  surface by hand, keep to the gentle card and let the formatter canonicalize on
  save. There is one supported way in (koan 11, DL-0043); the fallback exists so
  nothing is silently dropped, never as a second interface.
