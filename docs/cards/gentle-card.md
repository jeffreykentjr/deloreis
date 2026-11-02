---
status: live
title: "A first world in .dli"
verdict: "A short tutorial for writing and checking fictional-world notes as .dli text or through the authoring tools."
---

# A first world in .dli

A `.dli` document records claims about a fictional world and where they came
from. The text is useful to read and edit; the lorestore is the canonical copy
once you import a project.

Start with a scope, two people or places, and a proposed fact:

```dli
scope story
person wren "Wren" proposed author
place ashby "Ashby" proposed author
wren.home = ashby proposed author
```

The identifier (`wren`) is how later claims refer to an entity; the quoted label
is for readers. A slot such as `wren.home` holds a detail. You can also write a
relation as `adjacent(ashby, thornfield) proposed author`. Proposed claims are
kept as possibilities rather than treated as settled canon.

Save the notes as a `.dli` file and run `deloreis check path/to/story.dli`.
The checker reports syntax and continuity findings; when a subject resembles a
declared entity, it can suggest the intended identifier. `deloreis fmt
path/to/story.dli` formats the file, and `deloreis query --help` explains how
to ask about claims.

To use a persistent project, run `deloreis init my-story`, then `deloreis
import path/to/story.dli --project my-story`, and start `deloreis mcp --project
my-story`. Import is for a new, empty project. In an authoring session, agents
use `author.open`, the relevant `author.add_*` tools, `author.check`, and
`author.materialize`; see [the authoring loop](authoring-loop.md).

A recorded decision can support a claim. A citation must refer to a decision
that exists in the world; the engine records the caller's decision state and
checks its references. It does not decide who is allowed to make the decision.
Questions and options let a writer keep rival possibilities open until a ruling.
