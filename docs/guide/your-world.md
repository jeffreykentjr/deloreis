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
   any rules you approve. Ask it to read every finding, run `author.check`, and
   show you the proposed changes and their sources.
4. Resolve blocking conflicts together. Then have it call `author.materialize`
   to commit the consistent staged changes. Ask `author.why` when you want to
   know what supports an answer.

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

When you need to change an accepted account, record the new ruling and inspect
its effect before committing. Use a branch to explore a possibility separately.
The [modelling guide](modelling.md) covers constraints, time, and decisions.
