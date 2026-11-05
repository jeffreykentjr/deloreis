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

Record a rule's own cited provenance when it represents an accepted world rule.
The optional decision on `author.materialize` stamps facts, not rules or other
declarations. Do not infer that a successful commit makes every proposal certain.
The [agent skill](../../skills/deloreis/SKILL.md) describes the authoring workflow.

Your application controls who may accept lore and record rulings. Some calls,
including `author.record_decision` and `author.defeat`, mutate the store
immediately. Keep those behind the application's review policy; discarding a
workspace does not roll them back.

## Keep retrieval

Retrieval can still supply the passages, descriptions, and style examples a
model needs. DLI checks the structured claims your application extracts against
recorded lore and declared rules. It does not check every sentence of a scene or
determine what your author intended. You can also export a context pack for
generation; see [exports](../spec/09-exports.md).

For a character's changing allegiance or location, record event-anchored states
and query the scene's point in time. Missing chronology can leave that query
indeterminate. A world's established facts are not automatically what each
character knows. See [modelling a world](modelling.md) for the time model.

## Test the integration

Start with a disposable world and two hand-written additions: one compatible
with its rules and one that conflicts with an established single-valued claim.
Check that your application accepts the first, presents the second's findings,
and does not commit the staged conflicting claim after discarding it.
Test proposed alternatives separately: an open possibility need not block a
commit. The [first-session example](../../skills/deloreis/references/first-session.md)
provides a small conflicting-home case to reproduce.

Once those paths work, try generated scenes and review whether your extraction
captures the facts you intended to check. The engine calls no model; extraction
quality and the choice of world rules belong to the surrounding application.
