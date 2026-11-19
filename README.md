# De Lore Is

De Lore Is is a local continuity checker for fictional worlds. It keeps track
of the people, places, events, and rules you give it, checks them for
contradictions, and can explain which decisions support an answer.

It's for writers, worldbuilders, and people building tools that work with
fiction. Write lore in `.dli` files and check it from the command line, or
connect an MCP client, such as an AI assistant, to a local world. The checks
run locally and don't call a model.

You decide what to record and which rules apply. De Lore Is checks the lore you
give it, not your manuscript.

## Try it

Install from crates.io with Rust 1.96 or later:

```sh
cargo install deloreis --locked
```

Save the following as `story.dli`:

```dli
scope story
ratify d-0001
person arthur "Arthur" decided d-0001
place camelot "Camelot" decided d-0001
place avalon "Avalon" decided d-0001
functional home decided d-0001
fact arthur home camelot decided d-0001
fact arthur home avalon decided d-0001
```

`d-0001` is the decision behind each declaration and fact; `decided d-0001` cites it.
[Concepts](docs/concepts.md#sources-and-rulings) covers `decided`,
`ratified`, and `proposed`.

Then run:

```sh
deloreis check story.dli
```

The finding lists both values: `[avalon, camelot]`. The `functional` line says
Arthur can have only one home; without it, two homes are fine. The command
exits with status 1 because it found a problem.

To find matching homes, run `deloreis query 'home($person, camelot)' --corpus story.dli`.
`$person` is a variable; the quotes keep your shell from expanding
it. The query warns while the corpus still has errors.

## Work with a world

Create a project with `deloreis init my-story`. The
[quickstart](docs/quickstart.md) connects it to an MCP client such as Claude
Code, adds facts, and catches a contradiction.

A project keeps the decisions behind your claims, including claims you later
revise. You can leave accounts unresolved, ask why something is canon, and try
changes on a separate branch. It records the order in which claims and rulings
were added. It doesn't track what each character knew at a given point in the
story.

- [Holmes casebook](docs/casebook/01-watson-name.md) — follow a sourced example
  through conflicts, revisions, and alternatives.
- [Modelling guide](docs/guide/modelling.md) — choose rules and record sources.
- [Bring your own notes](docs/guide/your-world.md) — start from a series bible.
- [Concepts](docs/concepts.md) — the terms used by the tools.
- [Agent guide](skills/deloreis/SKILL.md) — authoring tools and session workflow.
- [Contributing](AGENTS.md) — working on the project.

Linux x86-64 and macOS ARM archives are available on the [releases page](https://github.com/jeffreykentjr/deloreis/releases). De Lore Is is MIT licensed.

## Use with a writing assistant

Install the agent skill:

```sh
npx skills add jeffreykentjr/deloreis --skill deloreis
```

The [skill](skills/deloreis/SKILL.md) teaches an assistant the MCP tools and
includes a first-session walkthrough. It doesn't install `deloreis` itself;
install that first, as above. How the skill is published and updated is in
[agent-skill-distribution](docs/agent-skill-distribution.md).
