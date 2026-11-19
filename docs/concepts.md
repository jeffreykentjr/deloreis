# Concepts

De Lore Is checks the statements you record about a fictional world. You choose
which constraints apply and which accounts to accept, and the tool explains its
answers from those inputs.

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

A **rule** derives a claim from other claims. Rules are modelling choices you
make, so they carry provenance like claims do. **Materialize** checks and
commits a workspace to the store; a blocking conflict prevents the commit.

## Sources and rulings

**Provenance** records how a claim was supplied and, for a cited claim, the
**decisions** supporting it. A decision is a named ruling, such as `d-0001`;
the writer can use one per story, per scene, or per editorial ruling.
`author.record_decision` records a decision. In `.dli` text, `ratify d-0001`
declares it. Referenced decisions must exist.

A **proposed** claim is an unresolved possibility. A **cited** claim carries
one or more decision references. A citation's state can be **provisional** or
**ratified**; these are recorded attributes of the claim's provenance. A
**decided** claim is supported by a recorded decision. These labels are the
claim's **standing**: how it was supplied.

In text, `decided d-0001` and `ratified d-0001` both refer to a recorded
decision; the latter also records ratified provenance. `proposed` leaves the
claim open. In a wiki, **canon** means certain under the current rulings,
**unconfirmed** means possible, and **overturned** means the recorded support
no longer holds.

A `ratify` block's hypothesis assignments currently apply to language names,
not to ordinary world claims. If a world claim assumes that hypothesis, the
check reports an error instead of silently treating the assignment as a ruling.
Leave the claim proposed, or record the accepted claim with a decision citation.
An MCP corpus import preserves the supplied evidence and reports this error in
its findings; inspect them and run a check before accepting the world.

A claim's evaluated status is what follows from its standing under the
current rulings, so a later decision can change it:

- **Certain:** supported as an accepted answer under those rulings.
- **Possible:** neither settled as certain nor ruled out.
- **Dead:** the claim has no admissible support under the current model. An
  absent claim is one that was never recorded, so there is nothing to evaluate.

A **defeat** records a decision that attacks an earlier decision's support. It
can represent a retcon without deleting the old claim. If a later decision
defeats the retcon, the original support can return. Use `author.why` to inspect
support and `author.diff` to compare changes. The structured diff's `rows`
distinguish an absent claim from a dead one; the older `consequences` summary
uses `dead` as a placeholder for a missing side.

## Exploring and checking

A **branch** is a separate line of exploration; the CLI calls it a **fork**.
Its changes stay off main unless you adopt them. The **certain layer** contains
claims that are certain under the chosen decisions; the **shadow layer** holds
the possible claims that aren't. An **as-of** query asks what holds at a chosen
story-time point, provided the relevant events are ordered.

A **pack** is a group of source documents loaded together. In an explanation,
an **antichain** is the set of distinct, minimal combinations of support. A
**functional dependency** is the one-value rule from a functional predicate.
The query grade **asserted** means a claim was taken as given without a
verifying procedure; it says nothing about whether the claim cites a ruling.
**Inertial** states persist until a later value replaces them. **Saturated**
means the rule engine has derived everything it can from the supplied facts and
rules.

Name suggestions are warnings. They help spot a misspelled subject, but they
won't catch every missing declaration. The engine records whatever attribution
and rulings its caller supplies; review and permission policies belong to the
application using it.

The project records when claims and rulings were added. To ask what a
character or the audience knows at a point in the story, record that knowledge
in an episode file; `timeline knows` reads it in production order.
