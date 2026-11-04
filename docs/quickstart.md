# Quickstart

Install De Lore Is with Cargo:

```sh
cargo install --git https://github.com/jeffreykentjr/deloreis --locked deloreis
```

Once a release is published, you can also download an archive from the [releases page](https://github.com/jeffreykentjr/deloreis/releases). Unpack it and put `bin/deloreis` somewhere on your `PATH`. Each archive has a SHA-256 checksum alongside it.

You'll also need an MCP client for this walkthrough; the example below uses
Claude Code. If you installed with Cargo, make sure Cargo's binary directory
is on your `PATH`.

Create a new project and register the server from that directory:

```sh
deloreis init my-story
cd my-story
claude mcp add --scope project deloreis -- "$(command -v deloreis)" mcp --project "$PWD"
```

`init` also suggests importing a `.dli` file; that is optional for this
walkthrough because the calls below build a blank world. Restart the client or
reload its MCP configuration. The project registration
writes `.mcp.json` in this directory. Other MCP clients can use the same
command and arguments; point `--project` at the directory containing
`deloreis.toml`. The server runs locally over stdio. `deloreis mcp --project
"$PWD" --tools all` exposes the extended tool set.

## Make the first ruling

Ask your client to make the following tool calls, in order. Substitute the
returned workspace ID for `WORKSPACE`. Read the findings returned by each call.
The blocks show tool arguments, not shell commands.

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

The [Hollow transcript](../examples/hollow/transcript.md) continues with
contested accounts, a retcon, a what-if branch, and replay. See the
[concepts](concepts.md) and [agent guide](../SKILL.md) for what the terms and
other tools mean.
