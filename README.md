---
status: live
verdict: "De Lore Is checks and explains continuity in fictional worlds."
---

# De Lore Is

Wren lives in Thornfield. Three chapters later, someone says she lives in
Ashby. If both are meant to be true at the same time, De Lore Is can catch the
conflict before it becomes part of your story.

For example, here's part of a finding from the
[Hollow walkthrough](examples/hollow/transcript.md):

```json
{
  "citations": ["d-0001", "d-0002"],
  "code": "continuity.fd-violation",
  "data": {"pred": "home", "slot_key": "home/wren", "subject": "wren", "values": ["ashby", "thornfield"]},
  "grade": "checked",
  "message": "canon breaks the functional dependency on 'home': wren carries 2 distinct certain values [ashby, thornfield]",
  "severity": "error"
}
```

The commit is refused until the conflict is resolved or discarded.

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
cargo install --git https://github.com/jeffreykentjr/deloreis --locked deloreis
```

Release archives will be available on the [releases page](https://github.com/jeffreykentjr/deloreis/releases) once published.

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
