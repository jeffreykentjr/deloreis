---
status: live
verdict: "De Lore Is checks and explains continuity in fictional worlds."
---

# De Lore Is

De Lore Is is a continuity engine for fictional worlds. Authors and their agents
record structured claims about people, places, events, and rules. It checks
those claims against ruled canon, explains its answers, and keeps the history
of retcons. The same committed inputs produce the same verdict without a model
call.

For example, if `home` is declared single-valued and Wren's ruled home is
Thornfield, an attempt to rule that Wren lives in Ashby produces:

```text
continuity.fd-violation: wren has two certain values for home
  values: ashby, thornfield
  rulings: d-0001, d-0002
materialize-blocked
```

This is a shortened version of the checked [Hollow transcript](examples/hollow/transcript.md).
DLI checks structured lore you give it; it does not read a manuscript and find
plot holes. You choose what to rule, which slots are single-valued, and who is
allowed to make those rulings.

[Get started](docs/quickstart.md) · [Concepts](docs/concepts.md) ·
[Agent authoring guide](SKILL.md) · [The Hollow walkthrough](examples/hollow/transcript.md)

Contributions are checked with `cargo xtask gate`; see [AGENTS.md](AGENTS.md)
and the [charter](docs/charter.md).
