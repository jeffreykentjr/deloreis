# Bring your own notes

You do not need to rewrite a whole series bible before trying De Lore Is. Pick
a short section, start a local project, and ask your assistant to record the
parts you want to check. The assistant still needs your judgment about what is
canon and which constraints belong in your world.

1. Install De Lore Is and connect your MCP client using the [quickstart](../quickstart.md).
2. Give the assistant a page of notes and say which parts are settled and which
   are alternative accounts. Tell it what source names or decision numbers to
   use. Ask it to propose constraints instead of assuming them.
3. Have it open a workspace and use `author.*` to add entities, claims, and
   any rules you approve. Ask it to run `author.check`, read the findings, and
   show you the proposed changes and their sources.
4. Resolve blocking conflicts together. Then have it call `author.materialize`
   to commit the consistent staged changes. Ask `author.why` when you want to
   know what supports an answer.

For general rules and named branches, start the MCP server with `--tools all`
so the client sees those tools; the [agent skill](../../skills/deloreis/SKILL.md)
shows how to change the registration.

Some operations, including `author.record_decision` and `author.defeat`, update
the store immediately. Agree on those rulings before the assistant calls them;
`author.discard` only discards staged work.

The project store is read through MCP. To use CLI `check` or `query` on its
contents, start the server with `--tools all`, call `dli.corpus.export` with
`{"document":"world","format":"rendered"}`, and save each returned rendered
document as a `.dli` file. Pass those files to the CLI; see the
[quickstart](../quickstart.md) for client setup.

For example, you could say:

> Use these notes to start my world in De Lore Is. Record the settled claims
> with source references and leave the disputed accounts open. Suggest, but do
> not assume, any single-value rules. Show me the findings and proposed changes
> before committing them.

The [Holmes notes](../../examples/holmes/notes.md) show a small input. This
script replays a reviewed transcription of the name and wound notes through the
MCP tools in a temporary project:

```sh
python3 examples/holmes/notes_demo.py "$(command -v deloreis)"
```

It prints the queried results after committing them:

```text
first-name(watson, john): certain
reported-wound-location(watson, shoulder): possible
reported-wound-location(watson, leg): possible
```

The script is a fixed transcription, not a general-purpose prose importer, and
calls no model. The larger [casebook script](../../examples/holmes/build.py)
adds more sourced material and checks the other examples. An assistant can help
interpret your own notes, but review its proposed claims against your sources.

When you need to change an accepted account, inspect its support before recording
the new ruling. Use a branch to explore a possibility separately.
The [modelling guide](modelling.md) covers constraints, time, and decisions.

## Continue a chapter

At the start of a writing session, have the assistant open a fresh workspace
and query the characters, places, and events relevant to the next scene. Keep
the manuscript and style notes in their existing files. DLI supplies the
recorded lore; the assistant still reads and writes the prose.

After drafting, ask it to propose only the new factual claims. Use
`author.diff` to inspect staged additions and their derived consequences, then
`author.check` to read the findings. Compare the proposed claims with the scene
before materializing them. A clean check only covers the structured claims and
rules you supplied, so review the transcription as well as the result.

Brainstormed ideas need not become settled lore. Leave them proposed, or try a
separate branch for an alternative plot. Before continuing in another session,
commit the changes you want to keep. A newly opened workspace sees the current
committed lore; a workspace already open retains its base.

## Make a wiki

With your world in `world.dli`, export its accepted claims as linked Markdown:

```sh
deloreis export --profile static-wiki --corpus world.dli > wiki.json
python3 - <<'PY'
import json
from pathlib import Path

pages = json.loads(Path("wiki.json").read_text())["artifact"]["pages"]
out = Path("wiki")
out.mkdir(exist_ok=True)
for page in pages:
    name = page["path"]
    if Path(name).name != name or not name.endswith(".md"):
        raise ValueError(f"unexpected wiki page path: {name}")
    (out / name).write_text(page["content"], encoding="utf-8")
PY
```

Open `wiki/index.md` in a Markdown viewer, or publish the pages with a
Markdown site generator. The default export includes claims with `certain` status; the
Holmes casebook also shows an all-standing view for unresolved and revised claims.
