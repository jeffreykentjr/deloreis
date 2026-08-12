---
name: deloreis
description: Author De Lore Is (.dli) lore through the tool surface — the deloreis server's author.* tools — by naming intents and reading the graded findings each call returns. You never write .dli by hand; you self-check with the read tools before you materialize; you supply attribution and decision state, and the engine records them and verifies every citation resolves.
status: live
verdict: "The v0 authoring manual: author through tools, self-check before materialize, read the graded feedback, obey provenance discipline — attribution is data you supply and citations are verified, not gated on who you are (DL-0054/DL-0058); raw .dli editing is a measured fallback and the gentle card is for reading your own rendered diffs."
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
2. **You supply attribution and decision state; the engine records them.** Every
   mutating call carries a provenance, and the engine writes down what you state
   rather than deciding what you are allowed to state. It does not ask who you
   are or whether you are entitled to a ruling: **every caller of this engine is
   a machine**, so a question about *who may rule* is a question about the
   consuming application's review pipeline, and it is answered there (DL-0054).
3. **The engine verifies what it can check: that a cited decision exists.** This
   is the one half of authority it keeps, and it is fail-closed. A `cited`
   provenance names decision ids in the consuming repo's ledger; each is checked
   against the decisions the corpus declares. An id that does not resolve draws
   `citation.unknown-decision` at **error** severity, which bars materialize, and
   the claim resting on it can never read `certain` — the engine leaves an
   unknown decision's variable open rather than believing you. So: cite a real
   ruling when a human made one, and **the engine will not invent one for you**.
   A human ruling reaches canon the ordinary way — you cite the ledger id that
   recorded it.
4. **The compile is the oracle, and it outranks this file.** The findings a tool
   returns are the authority on whether your lore is well-formed and consistent.
   If a finding disagrees with anything written here, the finding is right and
   this file is stale.

> **A version note, because the rule above reversed one.** Rules 2 and 3 hold
> from **contract 1.3.0** (DL-0058). Before it, the engine refused every `cited`
> provenance outright with the legacy cited-provenance refusal and staged nothing — it had
> encoded "only a human mints citations" as a wire refusal, which never
> distinguished a human's decision from an agent's and taught agents to route
> human rulings *around* the provenance system instead of through it. Every wire
> below 1.3.0 still serves that refusal, byte for byte, permanently. If you get
> the legacy cited-provenance refusal, you are speaking an older contract: negotiate
> 1.3.0 or above, or resubmit as `proposed`.

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
  → author.materialize               ACCEPT: commit the staged facts to the
                                     lorestore; get them back as JSON
  → review through your own pipeline the committed facts (or a rendering of
                                     them) go to whoever reviews your work
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
  returned only when asked (`rendered: true`, from contract 0.6.0 / DL-0046;
  the served contract is 1.6.0) and is **empty on a build with no formatter
  linked** — the default build links none
  (LS-7). It is never the authoring surface: you propose through the typed tool
  params, not by writing these lines.
- **`workspace_digest`** — the new fence.

Read the findings on every call. Do not guess and re-emit — the tool already told
you. An ignored finding does not go away: it reappears in the next call's set and
blocks materialize.

## 2. The tool surface (the 31 tools you author with)

The contract's §3.2 table lists 33 `author.*` tools. Two of them,
`author.branch` and `author.merge`, manage durable branches rather than stage
lore and sit outside the authoring loop below — reach for `author.branch` when
you want a durable, named checkpoint of the current state, one that survives
across sessions rather than living only in one workspace's staged layer,
either to come back to later or to compare against; reach for `author.merge`
when you are ready to fold a branch's fact-set back into another — it always
commits a merge, reporting valuation, exclusivity, and functional-dependency
conflicts rather than blocking on them, so a merge never leaves you stuck. Both
operate on the durable lorestore's refs, not the staged workspace layer, and
neither stages a declaration itself.

**The twenty-four mutating tools** (each returns `{findings, workspace_digest}`,
plus `rendered` when you ask for it) and **two read-only state ops** (status,
diff). `author.materialize` is a mutation — it is the accept decision, and
`author.discard` is the reject.

The **core nine** (`add_entity` … `suppose`) cover people, places, things,
eras, their attributes, relations, chronology, and tracked debt — most authoring
never leaves them. The **expert seven** below them are the language and
continuity lane, available from **contract 1.3.0** (DL-0058). The **standing
pair** (`set_standing`, `defeat`) and **`declare_functional`** are a
promotion/defeat pair from **contract 1.5.0** and a single-valuedness
declaration from **contract 1.6.0**; **`assert_same_as`** and **`add_rule`**
are the newest two — a coreference declaration and a datalog derivation rule,
both from **contract 1.7.0** (§3.10–§3.13 of spec `11-agent-authoring.md`
give their full semantics). Ask `dli.version` if you are unsure what your
negotiated wire carries — and negotiate the newest version your harness
supports: capabilities are floor-gated (a version advertises everything at or
below it), so an old negotiation is a permanent surface loss, not a
compatibility mode.

| Tool | Intent |
|---|---|
| `author.open` | Open a workspace: `{scope}` to stage over a scope, `{fork}` for fork-scoped exploration. |
| `author.status` | The staged declarations + current digest (read-only). From 1.3.0 it also returns `staged`: one row per staged declaration with its structural `key`, its `kind`, and its rendered line where a formatter exists. That key is what `author.discard` takes. The key is a digest of the declaration's content, so it holds within a contract version — re-read `author.status` after negotiating a new one rather than carrying keys across. |
| `author.diff` | The full rendered block of everything staged (read-only). |
| `author.discard` | Drop one staged declaration or the whole workspace. From 1.3.0 a declaration is named by the **structural key** `author.status` lists for it (`jcs:sha256:…`), which exists on every build; below 1.3.0 it is named by its rendered `.dli` line, which a build with no formatter cannot produce — there, that form refuses by name. |
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
| `author.add_dialect` | **1.3.0.** Declare a naming register `{id, label?, body[]}` — its vowels, endings, foreign clusters, cluster/sayability ceilings, stress rule, and `name-kind` slot grammars. `body` is the typed interior; you send the items, not clause text. Provenance required. |
| `author.add_name_part` | **1.3.0.** Declare a morpheme `{id, gloss?, head[]}` — the raw material composed names draw on. `head` carries the role tokens, e.g. `["role","given-proto"]`. Provenance required. |
| `author.add_name` | **1.3.0.** Compose a name `{id, label?, head[], composition[], bearer?}` from morphemes, optionally binding it to the entity that bears it. Each composition step is `{role, morpheme}`. Provenance required. |
| `author.add_term` | **1.3.0.** Introduce a glossary term `{id, label?}`. Provenance required. |
| `author.add_banned` | **1.3.0.** Ban a word `{word, reason?}` from the lore surface. Provenance required. |
| `author.add_state` | **1.3.0.** Assert a time-varying fluent `{id, value?, time?}` — a state that holds over an interval, not forever. `id` may be dotted (`lancelot.oath-status`), and the subject before the dot must be a known entity. `time` is `{kind: after\|during\|since\|until, point}`. Provenance required. |
| `author.add_attest` | **1.3.0.** Request an attestation `{question}` against a question already open in this workspace. Provenance required. |
| `author.set_standing` | **1.5.0.** Promote a provisional stored claim `{claim_id, decision}` to ratified, by citing a decision that is declared and occurs in every support path. |
| `author.defeat` | **1.5.0.** Defeat a stored claim `{claim_id, decision, defeated_decision}` with a new decision, while preserving the claim's history and object. |
| `author.declare_functional` | **1.6.0.** Declare a predicate `{pred}` single-valued: two distinct certain values for one subject become a `continuity.fd-violation`, even for a first-seen predicate canon has no precedent for. Provenance required. |
| `author.assert_same_as` | **1.7.0.** Assert `{left, right}` name the same bearer: the world merges them symmetric-transitively, gated on this claim's own standing — defeating its citing decision un-merges every derived consequence. Refused over a declared era or event (`identity.temporal-unsupported`). Provenance required. |
| `author.add_rule` | **1.7.0.** Declare one datalog rule `{name, head, body}` — a typed `head :- body` clause; every fact it derives carries this rule's own provenance, so defeating its citing decision kills every derivation through it. One clause per call — an N-clause rule is N calls under distinct names, for finer-grained defeat. Refuses to stage on its own bad shape (`rule.unbound-variable`, `rule.duplicate`) or on a stratification failure (`rule.unstratifiable`) — a bad rule never reaches the workspace, because staging one would brick every later read. Provenance required. |

**The five self-check read tools** let you interrogate the staged world *before*
you materialize (§4). All are read-only and mutate nothing; each names a workspace
in its params (except `author.list_workspaces`):

| Tool | Params | Answers |
|---|---|---|
| `author.findings` | `{workspace}` | `{errors, warnings, materializable}` — your pre-flight: `errors` bar materialize, `warnings` pass, `materializable` is the go/no-go boolean. |
| `author.check` | `{workspace}` | `{ok, findings}` — the full check battery (continuity + factcheck + rejected-lore) over the staged world; `ok:true` means no error-severity finding remains. New proposed claims are legitimately unsupported, so only contradictions surface — not "unsupported" noise. |
| `author.query` | `{workspace, goal_atom, layer?, why?}` | `{rows}` — query the staged world. `goal_atom` is a **typed atom**, written `{"pred": "serves", "args": [{"Sym": "bedivere"}, {"Var": "who"}]}` — each name is plain text, and a `Var` argument is a variable that enumerates matches. A proposed claim reads `status:"possible"`, never `certain`. Unknown predicate → empty rows (not an error). The rendered string `goal` is the retired predecessor: a registered killed form, refused from contract 1.0 naming `goal_atom`, and still answered on a `0.5.1`/`0.6.x` wire. |
| `author.why` | `{workspace, atom}` | `{why_not, why?}` — why an atom can't be certain yet: the blocking rulings and the defeater. `known:false` means the world derives no such atom. |
| `author.list_workspaces` | `{}` | `{workspaces:[{workspace, base, mode, staged, workspace_digest}]}` — context recovery across your open workspaces. |

The whole loop closes through `author.*` alone: you never touch a `dli.*` batch op
or a raw `.dli` file to author *or* self-check. (`author.diff` and `author.status`
also return the full finding set and rendered surface, so any of them is a valid
pre-flight.)

A read-only `dli.*` family also lives on the same server — `dli.timeline.*`,
`dli.fork.*`, `dli.check-draft`, `dli.corpus.export`, `dli.name.check`, and a
few others — for inspection and export; authoring never needs it, but it is
there if your harness wants to read canon a different way.

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
missing field, unknown field, a stale fence) is refused *whole* as a typed **wire
error** (`schema-violation`, `stale-fence`, `materialize-blocked`,
`unknown-doc`, …) and **stages nothing**. An **error-severity finding** is
different: it stages the claim but bars materialize. Neither ever drops your
input silently.

An unresolvable citation is deliberately the *second* kind, not the first: the
claim stages so you can see it in the diff and repair the id, and
`citation.unknown-decision` bars materialize until you do.

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
  `status:"possible"` — expected until a resolvable decision citation makes it certain.
- **`author.why`** answers "why isn't this certain yet?" — `why_not` names the
  blocking rulings and the defeater; `known:false` means the world derives no
  such atom at all (you referenced something that isn't there).
- **`author.diff`** / **`author.status`** render the whole staged block (and, for
  `diff`, every open finding and the derived-consequence shift) — the surface a
  human will review.
- **`author.materialize`** is the last gate: it **refuses** with
  `materialize-blocked` if any error-severity finding is open, naming the blocking
  finding codes. A clean materialize commits the staged facts to the lorestore
  and returns them as JSON — `facts` and `root_hash` — with an empty `findings`
  array; `.dli` rendering is the `rendered: true` opt-in (§1, DL-0046).

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

- **Default to `proposed`, carrying your session origin.** A claim you are
  making on your own authority is a proposal, and a proposal reads `possible`,
  never `certain`, until a ruling lands. `hypothesis`-scoped is the fork-local
  form for exploration.
- **Cite only a ruling that exists.** `cited` is for cascading a decision the
  consuming repo's ledger already records — you name its id, and the engine
  records the attribution and checks the id resolves. It is not a way to make
  your own claim true: an id the corpus does not declare is refused
  (`citation.unknown-decision`), and inventing one is the single thing this
  interface is built to catch.
- **A rename is a supersession, never an in-place edit.** The new name is ruled
  in; the old name dies into lineage and the killed-form registry, with every
  claim's history intact. There is no edit that changes the past quietly.
- **An open question is a debt.** File it with `author.open_question`, surface it,
  and never carry it in your head — a waiting decision lives in the world, not in
  memory.

## 7. Reading a rendered diff — the gentle `.dli` card (reference)

Authoring is tools-only, but you **review your own output**, and one of the
views you can ask for is `.dli`: `author.diff` and `author.materialize` render
canonical `.dli` lines when you opt in (`rendered: true`) on a build with a
formatter linked — the canonical return is the JSON (§1). A human may review
that same rendering. So you must be able to *read* `.dli` even though you never
*write* it. Read it as notes, not English — **what parses is exactly what the surface
shows** (`docs/cards/gentle-card.md` is the full one-page card).

A materialized workspace — the exact lines a `rendered: true` materialize hands
back for a small cast:

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

- **Expert tier.** The language lane (dialects, name-parts, names, terms, bans)
  and the continuity/ledger pair (states, attestations) have tools from contract
  1.3.0 — they are in the §2 table, and you author them the same way as
  everything else. The standing pair (`set_standing`, `defeat`, 1.5.0) and
  `declare_functional` (1.6.0) are also in the §2 table. Rules, cycles,
  practices (magic systems), and proofs do
  **not** have tools yet: they are deliberately parked until a consumer needs
  them (DL-0058), so for those the reference is the expert card
  (`docs/cards/expert-card.md`) and the lane specs under `docs/spec/`. Fork
  authoring uses `author.open {fork}`, `author.add_hypothesis`, and
  `author.suppose`; read the consequence shift in `author.diff`.
- **Raw `.dli` editing is a measured fallback**, not a supported interface. It meets the same
  gate and the same graded findings — nothing is lost — but no tooling is built
  for it and no reliability claim is made for it. Prefer the tool surface. If you must edit
  surface by hand, keep to the gentle card and let the formatter canonicalize on
  save. There is one supported way in (koan 11, DL-0043); the fallback exists so
  nothing is silently dropped, never as a second interface.
