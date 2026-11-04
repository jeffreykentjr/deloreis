# Concepts

| Engine term | Meaning for a fictional world |
|---|---|
| Scope | The world or document being worked on. A new project has scope `world`. |
| Workspace | A staging area over a particular committed world. |
| Claim | A structured statement about the world, with its support and history. |
| Decision | A named canon ruling. `author.record_decision` records it; cited claims can use it as support. |
| Certain / possible / dead | The status of a claim under the current rulings: settled, still possible, or defeated. A possible claim is not canon. |
| Functional | A predicate declared single-valued for each subject, such as a character's home. Contradiction checking depends on declaring this constraint. |
| Fluent | A value that can change over story time. |
| Materialize | Check and commit staged objects to the store. A conflicting commit is refused. |
| Defeat | Record a retcon that attacks a ruling. The earlier claim remains inspectable, and defeating the retcon can restore its support. |
| Branch / fork | A separate line of exploration. A diff shows its consequences without changing the main branch. |
| Provenance | Where a claim came from and, for a ruled claim, which decisions support it. |

For an authoring diff, use the structured `rows` to distinguish a missing
claim from a defeated one. The older `consequences` summary uses `dead` as a
placeholder for a missing side.

Name suggestions are warnings. They can help spot a misspelled subject, but
they don't catch every missing declaration.

The engine records the rulings and attribution supplied by its caller; review
and permission policy belong to the application using it. The record follows
production order, not story order: it does not generally model what each
character knew at a given moment or how an unreliable narrator understood it.
