---
status: live
verdict: "Explore the checked example worlds and their supported entry points."
---

# Example worlds

[The Hollow](../../examples/hollow/) is the release walkthrough. Its
[transcript](../../examples/hollow/transcript.md) is generated from an MCP
session and checked by the gate. The project includes both a content-addressed
store and a readable `.dli` export. Copy the directory before experimenting so
that your changes do not alter the checked-in seed.

The bundled demonstration stores contain 8 fact objects in `camelot`, 96 in
`round-table`, and 7 in `warding-vows`; each count matches the facts named by
its commit. These bundles are checked from their sources by the gate.

Camelot and the Norse examples are additional text corpora. Check one with:

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
