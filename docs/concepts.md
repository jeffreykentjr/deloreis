# Concepts

De Lore Is checks the statements you record about a fictional world. You choose
which constraints apply and which accounts to accept. The tool can explain an
answer from those inputs; it does not read the story and decide canon for you.

## Building a world

A **scope** is the part of a project you are working in. A new project has a
`world` scope. A **workspace** is a staging area for changes to a scope. A
**claim** is a statement such as `home(watson, london)`. A **predicate** is the
name of the relationship, here `home`; an entity is a named person, place, or
other thing in the world.

A **functional** predicate permits one value for a subject at a time. Declare
`home` functional only if two simultaneous homes should be a conflict in your
model. A **state** (also called a fluent) represents a value that can change
over story time. An **event** is something that happens; an **era** groups a
period of the story. Ordering events and eras lets the timeline and as-of
queries reason about their sequence.

A **rule** derives a claim from other claims. It expresses a modelling choice,
not a fact automatically discovered in the text. **Materialize** checks and
commits a workspace to the store; a blocking conflict prevents the commit.

## Sources and rulings

**Provenance** records how a claim was supplied and, for a cited claim, the
**decisions** supporting it. A decision is a named ruling, such as `d-0001`;
the writer can use it for a story, an editorial ruling, or another useful unit.
`author.record_decision` records a decision. In `.dli` text, `ratify d-0001`
declares it. Referenced decisions must exist.

A **proposed** claim is an unresolved possibility. A **cited** claim carries
one or more decision references. A citation's state can be **provisional** or
**ratified**; these are recorded attributes of the claim's provenance. A
**decided** claim is supported by a recorded decision. **Standing** describes
such authoring metadata; it is different from the claim's evaluated status.
Neither the word ratified nor a citation by itself promises that a claim will
remain certain after later decisions.

The evaluated statuses describe what follows under the current rulings:

- **Certain:** supported as an accepted answer under those rulings.
- **Possible:** not settled as certain or ruled out. Possible does not mean
  false, and it does not make the claim canon.
- **Dead:** the claim has no admissible support under the current model. An
  absent claim is different: no claim is present to evaluate.

A **defeat** records a decision that attacks an earlier decision's support. It
can represent a retcon without deleting the old claim. If a later decision
defeats the retcon, the original support can return. Use `author.why` to inspect
support and `author.diff` to compare changes. The structured diff's `rows`
distinguish an absent claim from a dead one; the older `consequences` summary
uses `dead` as a placeholder for a missing side.

## Exploring and checking

A **branch** or fork is a separate line of exploration. Its changes do not alter
main unless you explicitly adopt them. The **certain layer** contains claims
that are certain under the chosen decisions; other layers can show unresolved
claims too. An **as-of** query asks what holds at a chosen story-time point,
provided the relevant events are ordered.

Name suggestions are warnings: they can help spot a misspelled subject but do
not catch every missing declaration. The engine records the attribution and
rulings supplied by its caller; review and permission policies belong to the
application using it.

The project records when claims and rulings were added. `timeline knows` reads
knowledge supplied for characters or the audience in an episode file, in
production order. It does not infer a character's beliefs or
knowledge at a particular story moment from the lore store.
