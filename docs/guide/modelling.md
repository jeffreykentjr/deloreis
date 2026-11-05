# Modelling a world

Start with a question you would like to answer, not a complete inventory. A
series bible might already say who lives where, when a character joins the crew,
and which accounts of an old disaster disagree. Pick a small part of it and
record enough to check one decision. You can add more as the story grows.

A claim names a relationship and its participants: `home(ada, lighthouse)`
says Ada's home is the lighthouse. Give people, places, and other important
things stable IDs; a display name can be more readable. Keep a note of where
each claim came from. The [concepts guide](../concepts.md) explains the terms
used below.

## Choose your constraints

A `functional` relationship has at most one value for each subject. Declare it
when two values should cause a conflict. A character's birth date might be
single-valued in your setting; their friends usually are not. Do not declare
`home` single-valued if your character can have two homes at once.

Values can also change. For a changing relationship, use a state and place its
changes in story time. A present home and a former home are not necessarily a
contradiction. Here Ada takes over the lighthouse after the storm:

```dli
scope lighthouse-story
ratify d-0001
thing lighthouse "Lighthouse" ratified d-0001
person wren "Wren" ratified d-0001
person ada "Ada" ratified d-0001
era before-storm < after-storm ratified d-0001
event storm during after-storm ratified d-0001
state lighthouse.keeper = wren until storm ratified d-0001
state lighthouse.keeper = ada since storm ratified d-0001
```

`since` and `until` name events, not eras. `during` a declared era starts a
state at that era; it does not end the state automatically when a later era
begins. Use an event as the boundary when one value replaces another. You can
also use `author.add_state` through MCP.

## Put events in time

Use an **event** for something that happens and an **era** to group a period.
Order what you know. Leave two events unordered if the story does not tell you
which came first: the timeline will preserve that uncertainty. An as-of query
needs enough ordering to place the relevant changes at the point you ask about.
The [casebook timeline](../casebook/03-timeline.md) shows an example where a
dating assumption matters.

## Record sources and rulings

A decision such as `d-0001` names a ruling you record. Keep a source index
beside your world so you can find the episode, chapter, or editorial note behind
it. A decision can cover a whole episode, a scene, or one editorial ruling;
choose a size that makes later revisions understandable. If you may need to
revise one conclusion without withdrawing every claim from an episode, give
that conclusion its own decision as well as a source reference. The casebook
uses separate decisions for the source stories and the Reichenbach ruling.

A **proposed** claim is an open possibility. A **cited** claim names its
supporting decisions; cited provenance can be provisional or ratified. These
labels record how the claim was supplied. Its evaluated status—certain,
possible, or dead—also depends on the current decisions and any defeats. A
citation must refer to a recorded decision. See [Concepts](../concepts.md) for
the distinction and [Sources for the Holmes model](../../examples/holmes/sources.md)
for one way to keep an index.

## Revise an account

Separate a change in the story from a change in your account of it. A character
moving house belongs on the timeline. Revising where the character always lived
changes which account you accept.

For a revision, ask `author.why` what supports the old answer. A later decision
can defeat the earlier ruling while preserving the claims and their sources.
Inspect the resulting answer and its explanation. `author.defeat` and
`author.record_decision` write directly to the store; they are not staged changes
that `author.discard` will undo. If you have not chosen the revision yet, explore
it on a separate branch first. The [Reichenbach case](../casebook/04-return.md)
and its [alternative branch](../casebook/05-branch.md) show both approaches.

Rules also have sources. If a rule derives a surprising conclusion, inspect the
rule's support as well as its premises before changing the facts. A rule supplied
as a proposal does not establish a conclusion merely because its premises are
accepted. The [Watson rule example](../casebook/06-rules.md) traces a conflict to
an interpretation the author can revise.

## Grow in small steps

Add a few entities and claims, run a check, and read the findings. If a check
reports a conflict, inspect the two values and their support before choosing
whether to revise a claim, relax a rule, or leave accounts unresolved. Commit a
consistent step, then move to the next part of the story. A branch lets you
try an alternative without changing the main line.

People can write and check `.dli` files directly. An assistant authoring a
world should use the `author.*` tools so it can inspect findings and commit the
structured changes. The [your-world guide](your-world.md) shows how to start
with existing notes.
