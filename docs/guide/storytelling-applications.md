---
status: live
verdict: "Application workflow for the released local authoring and query interfaces."
---

# Build a storytelling application

An AI storytelling application can use DLI to check new lore before accepting
it into a persistent world. Keep generation in your application. Run the local
engine over MCP, or use newline-delimited JSON over stdio with
`deloreis serve --project <path>`. The [serving reference](../spec/10-serving.md)
defines requests and responses. With MCP, use `--tools all` to advertise
general rule and named-branch tools.

## Check an addition

1. Open a workspace on the project's committed lore. Query the details relevant
   to the scene before generating it.
2. Extract factual additions from the generated prose. Stage entities, claims,
   and any author-approved rules through `author.*`, retaining source references.
3. Inspect `author.diff` and `author.check`. Return conflicts and their support
   to the generator or to a human editor for revision.
4. Materialize the accepted staged objects. Error findings block materialization;
   unresolved proposals may produce warnings without blocking it.

Give an accepted world rule its own cited provenance: the optional decision on
`author.materialize` stamps facts only, not rules or other declarations. A
proposal stays a proposal after a successful commit. The
[agent skill](../../skills/deloreis/SKILL.md) covers the full set of authoring calls.

Your application controls who may accept lore and record rulings. Some calls,
including `author.record_decision` and `author.defeat`, mutate the store
immediately. Keep those behind the application's review policy; discarding a
workspace does not roll them back.

## Keep retrieval

Keep your retrieval for passages, descriptions, and style examples. DLI checks
the claims your application extracts against recorded lore and declared rules,
so anything you don't extract goes unchecked. To give the generator structured
context, export a context pack; see [exports](../spec/09-exports.md).

For a character's changing allegiance or location, record event-anchored states
and query the scene's point in time. If the relevant events aren't ordered, the
query can come back indeterminate. Established facts are what is true in the
world, which may differ from what a given character knows.
[Modelling a world](modelling.md) covers the time model.

## Test the integration

Start with a disposable world and two hand-written additions: one compatible
with its rules and one that conflicts with an established single-valued claim.
Check that your application accepts the first, presents the second's findings,
and does not commit the staged conflicting claim after discarding it.
Then test a proposed alternative, which should commit with a warning at most.
The [first-session example](../../skills/deloreis/references/first-session.md)
provides a small conflicting-home case to reproduce.

Once those paths work, try generated scenes and check that your extraction
captures the facts you meant to check.
