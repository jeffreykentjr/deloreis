# Example worlds

[The Hollow](../../examples/hollow/) is a small world with a dispute, a retcon,
and a what-if branch. Its [transcript](../../examples/hollow/transcript.md)
shows the MCP calls and responses. The directory contains a project store and
a readable `.dli` export. Copy the directory before making changes.

The repository also includes Camelot and Norse text examples. From a checkout,
check Camelot with:

```sh
deloreis check examples/camelot.dli
```

To create a separate store-backed project from a text document, use
`deloreis import --help` for its arguments. Import initializes a new world;
it does not overwrite an existing world. `deloreis fmt`, `query --why`, and
`check-proof` provide further text tools. Run each command with `--help` for
its supported arguments.

The store is the canonical form for an authoring project. Agents author
through MCP and can export `.dli` for reading; people can also use the text
tools directly. See the [quickstart](../quickstart.md) and
[concepts](../concepts.md).
