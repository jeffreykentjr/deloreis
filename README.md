---
status: live
verdict: "De Lore Is checks and explains continuity in fictional worlds."
---

# De Lore Is

Wren lives in Thornfield. Three chapters later, someone says she lives in
Ashby. If both are meant to be true at the same time, De Lore Is can catch the
conflict before it becomes part of your story.

For example, a check reports `continuity.fd-violation` and names both homes
and the rulings behind them. The [Hollow walkthrough](examples/hollow/transcript.md)
shows the actual finding and the refused commit.

De Lore Is keeps track of the people, places, events, and rules you give it.
It can tell you which claims are settled, which are still in dispute, and why.
When you change your mind, the old ruling stays in the record. You can explore
an alternative without rewriting the original.

It's useful for authors keeping a series bible and for people building tools
that write or work with fiction. It checks structured lore, not manuscripts:
you decide what to record and which rules matter in your world.
It records when you add and rule on claims; it does not model what each
character knew at a particular point in the story.

## Install

With Rust installed:

```sh
cargo install deloreis --locked
```

Or download a macOS or Linux archive from the [releases page](https://github.com/jeffreykentjr/deloreis/releases). Each archive has a checksum alongside it.

## Get started

Create a world:

```sh
deloreis init my-story
cd my-story
```

The [quickstart](docs/quickstart.md) walks through connecting an MCP client,
adding your first facts, and catching a contradiction. You can also read the
[Hollow walkthrough](examples/hollow/transcript.md) for a complete example of
a dispute, a retcon, and a what-if branch.

- [Concepts](docs/concepts.md) — the terms you'll meet as you go.
- [Agent guide](SKILL.md) — the authoring tools and how to use them.
- [Contributing](AGENTS.md) — how to work on the project.

The project is MIT licensed.
