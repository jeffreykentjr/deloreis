# Quickstart

Install De Lore Is with Cargo:

```sh
cargo install deloreis --locked
```

You can also download a Linux x86-64 or macOS ARM archive from the [releases page](https://github.com/jeffreykentjr/deloreis/releases). Unpack it and put `bin/deloreis` somewhere on your `PATH`. Each archive has a SHA-256 checksum alongside it.

If you installed with Cargo, make sure Cargo's binary directory is on your
`PATH`. You also need an MCP client; these steps use Claude Code.

Create a project and register the server from its directory:

```sh
deloreis init my-story
cd my-story
claude mcp add --scope project deloreis -- "$(command -v deloreis)" mcp --project "$PWD"
```

`init` prints a suggestion to import a `.dli` file; ignore it, since these
steps start from a blank world. The registration writes `.mcp.json` in this
directory. Restart the client or reload its MCP configuration.

Other MCP clients can run the same command and arguments, with `--project`
pointing at the directory that contains `deloreis.toml`. The server runs
locally over stdio. Add `--tools all` to the server command to advertise the
branch, rule, and corpus import/export tools; the casebook uses them.

The project's world lives in its store, which you reach through MCP. The CLI
`check` and `query` commands read `.dli` files instead. To check the store from
the CLI, start the server with `--tools all`, call `dli.corpus.export` with
`{"document":"world","format":"rendered"}`, and save the returned `rendered`
documents as `.dli` files.

## Make the first ruling

Ask your client to make these tool calls in order, replacing `WORKSPACE` with
the workspace ID that `author.open` returns. Each block is a tool's arguments.
Read the findings each call returns.

1. Call `author.open` with `{"scope":"world"}`.
2. Call `author.add_entity` three times, using `WORKSPACE`:

   ```json
   {"workspace":"WORKSPACE","kind":"person","id":"wren","name":"Wren","provenance":{"kind":"proposed","origin":"quickstart"}}
   ```

   ```json
   {"workspace":"WORKSPACE","kind":"place","id":"thornfield","name":"Thornfield","provenance":{"kind":"proposed","origin":"quickstart"}}
   ```

   ```json
   {"workspace":"WORKSPACE","kind":"place","id":"ashby","name":"Ashby","provenance":{"kind":"proposed","origin":"quickstart"}}
   ```

3. Declare the slot single-valued with `author.declare_functional`:

   ```json
   {"workspace":"WORKSPACE","pred":"home","provenance":{"kind":"proposed","origin":"quickstart"}}
   ```

4. Call `author.record_decision` with `{"workspace":"WORKSPACE","decision":"d-0001"}`.
5. Call `author.add_fact`:

   ```json
   {"workspace":"WORKSPACE","entity":"wren","key":"home","value":"thornfield","provenance":{"kind":"cited","decisions":["d-0001"],"state":"ratified"}}
   ```

6. Call `author.materialize` with `{"workspace":"WORKSPACE"}`. This commits
   the first ruling and the staged declarations.

## Catch a contradiction

7. Call `author.record_decision` with `{"workspace":"WORKSPACE","decision":"d-0002"}`.
8. Call `author.add_fact`:

   ```json
   {"workspace":"WORKSPACE","entity":"wren","key":"home","value":"ashby","provenance":{"kind":"cited","decisions":["d-0002"],"state":"ratified"}}
   ```

   This call can already report `fact.contradiction`; leave the conflicting
   fact staged for the next check.

9. Call `author.check` with `{"workspace":"WORKSPACE"}`. Its findings include
   `continuity.fd-violation`, both values, and rulings `d-0001` and `d-0002`.
   `author.materialize` refuses this conflicting staged fact. Use
   `author.discard` to abandon the staged changes. Decisions are recorded
   immediately, so discarding a workspace does not undo them.

Next, the [Holmes casebook](casebook/01-watson-name.md) works through contested
accounts, a retcon, a what-if branch, and replay. Restart the server with
`--tools all` first. [Concepts](concepts.md#sources-and-rulings) defines the
terms, and the [agent guide](../SKILL.md) lists the other tools.
