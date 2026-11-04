#!/usr/bin/env python3
"""Render the opening chapter from the captured authoring results on stdin."""
import json
import sys
sys.dont_write_bytecode = True
from try_casebook import block

result = json.load(sys.stdin)
rendered = result["rendered"]
functional = next(line.strip() for line in rendered if line.startswith("functional first-name"))
first_name = next(line.strip() for line in rendered if line.startswith("watson.first-name"))
staged = result["name_staged"][0]
finding = result["name_conflict"]
citations = ", ".join(finding["citations"])
print(f'''# What should we call Watson?

To follow the tool calls throughout the casebook, start the MCP server with
`deloreis mcp --project <directory> --tools all`.

The part I title of [*A Study in Scarlet*](https://www.gutenberg.org/files/244/244-h/244-h.htm) names its narrator John H. Watson. In [*The Man with the Twisted Lip*](https://www.gutenberg.org/files/1661/1661-h/1661-h.htm#chap06), his wife offers to send “James” to bed while Watson is present.

Is James another name for Watson, a slip, or something else? The story does
not settle that question. For this example, we will make a deliberately strict
choice: treat John and James as competing first names, and say a person can
have only one. That choice belongs to us, not to Conan Doyle's text.

De Lore Is records the rule and the John claim with the decisions that support
them. A decision is a named ruling: `d-0006` is our modelling rule, and
`d-0001` points to *A Study in Scarlet*. `ratified` records that we accepted
the claim; later rulings can still change what follows. `functional` means at
most one value for each person. [Concepts](../concepts.md#sources-and-rulings) explains the terms.
These are the lines returned by the authoring tools:

```dli
{functional}
{first_name}
```

Now we stage the alternative, using `d-0003` for *The Man with the Twisted Lip*:

```dli
{staged}
```

The check returns this finding (with its other metadata omitted):

```text
{finding["code"]}: {finding["message"]}
supporting decisions: {citations}
```

That result does not decide which name a writer should use. It identifies the
choice that needs attention and names the sources behind it. A writer could
keep both as names people use instead of treating them as mutually exclusive
first names. The [source notes](../../examples/holmes/sources.md) explain the
decision numbers and the limits of the evidence.
''', end="")
print(block(result, "name"), end="")
print("\n<details>\n<summary>The calls</summary>\n\n```json\n" + json.dumps(result["calls"]["name"], indent=2, ensure_ascii=False) + "\n```\n\n</details>")
