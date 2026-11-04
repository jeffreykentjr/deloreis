# De Lore Is

De Lore Is is a local continuity checker for fictional worlds. It keeps track
of the people, places, events, and rules you give it, checks them for
contradictions, and can explain which decisions support an answer.

It's for writers, worldbuilders, and people building tools that work with
fiction. You can write lore in `.dli` files and check it from the command line,
or connect an MCP client, such as an AI assistant, to work with a local world.
The checks don't call a model.

You decide what to record and which rules apply. De Lore Is checks structured
lore; it doesn't read a manuscript and find plot holes on its own.

## Try it

Install from this repository with Rust 1.96 or later:

```sh
cargo install --git https://github.com/jeffreykentjr/deloreis --locked deloreis
```

Save the following as `story.dli`:

```dli
scope story
ratify d-1
person arthur "Arthur" decided d-1
place camelot "Camelot" decided d-1
place avalon "Avalon" decided d-1
functional home decided d-1
fact arthur home camelot decided d-1
fact arthur home avalon decided d-1
```

Here `d-1` names the decision supporting the declarations and facts.

Then run:

```sh
deloreis check story.dli
```

The finding lists both values: `[avalon, camelot]`. The `functional` line tells
De Lore Is that Arthur can have only one home. Without that rule, the two
statements aren't automatically a conflict. The command exits with status 1
because it found a problem.

## Work with a world

Create a project with `deloreis init my-story`. To work with it through an MCP
client such as Claude Code, follow the [quickstart](docs/quickstart.md) to
connect the local server, add facts, and catch a contradiction. The walkthrough
shows how to ask the client to make the tool calls.

In a project, De Lore Is keeps the decisions behind your claims, including
claims you later revise. You can keep accounts unresolved, inspect why
something is considered canon, and explore changes on a separate branch. The
project keeps the order in which claims and rulings were added; it does not
model what each character knew at a particular point in the story.

- [Hollow walkthrough](examples/hollow/transcript.md) — a dispute, a retcon,
  and a what-if branch, with the actual calls and results.
- [Concepts](docs/concepts.md) — the terms used by the tools.
- [Agent guide](SKILL.md) — authoring tools and session workflow.
- [Contributing](AGENTS.md) — working on the project.

Release archives will be available on the [releases page](https://github.com/jeffreykentjr/deloreis/releases) once published. De Lore Is is MIT licensed.
