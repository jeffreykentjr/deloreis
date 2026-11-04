#!/usr/bin/env python3
"""Render casebook chapters from the captured authoring and timeline results."""
import json
import sys

result = json.load(sys.stdin)
chapter = sys.argv[1]
if chapter == "wounds":
    shoulder = result["wound_accounts"]["shoulder"][0]
    leg = result["wound_accounts"]["leg"][0]
    model = "\n".join(line for line in result["rendered"] if line.startswith("watson.reported-wound-location"))
    print(f'''# Where was Watson wounded?

In [*A Study in Scarlet*](https://www.gutenberg.org/files/244/244-h/244-h.htm), Watson says a Jezail bullet hit his shoulder. In [*The Sign of the Four*](https://www.gutenberg.org/files/2097/2097-h/2097-h.htm), he describes a Jezail wound in his leg.

Those passages are often treated as a discrepancy. They do not establish that
the two references describe the same injury. We can record the accounts without
choosing between them. Here `proposed` marks an account as unresolved; its
origin label identifies the story in our [source notes](../../examples/holmes/sources.md).

The authoring tools rendered the claims as:

```dli
{model}
```

A query reports:

```text
{shoulder["atom"]}: {shoulder["status"]}
{leg["atom"]}: {leg["status"]}
```

`possible` means an account remains open in this model. It does not mean the
account is false or that De Lore Is has decided the wounds are the same. If a
writer chooses a single-wound interpretation, they can add that rule and make
a ruling later.
''', end="")
elif chapter == "timeline":
    model = "\n".join(line for line in result["rendered"] if line.startswith(("era ", "event ", "before ")))
    print('''# Putting cases in order

[*A Scandal in Bohemia*](https://www.gutenberg.org/files/1661/1661-h/1661-h.htm#chap01) places Watson's married visit to Holmes on 20 March 1888. In [*The Sign of the Four*](https://www.gutenberg.org/files/2097/2097-h/2097-h.htm), Mary Morstan describes an advertisement from May 1882 as about six years earlier. Reading that approximation as a precise date can put her meeting with Watson after the March visit. The story does not give us a precise date for that meeting, so we leave the two events unordered.

The model puts both events in the 1880s, the Reichenbach encounter in 1891,
and Holmes's return in 1894. We record the order of those eras under an
editorial decision. The timeline follows the recorded order; it does not fill
in an order between the two 1880s cases.

The relevant model lines are:

```dli
''' + model + '''
```

The actual Markdown timeline is:

''' + result["timeline"] + '''
Each line states a known ordering. A pair missing from the list is unordered,
not necessarily simultaneous. A writer can add a more specific order if the
story calls for one.
''', end="")
elif chapter == "return":
    retcon = result["retcon"]
    before = retcon["before"][0]
    after = retcon["after"][0]
    why = retcon["why_old"]
    defeat = why["blocking_defeats"][0]
    print(f'''# The return from Reichenbach

At the end of [*The Final Problem*](https://www.gutenberg.org/files/834/834-h/834-h.htm), Watson concludes that Holmes died at Reichenbach. In [*The Empty House*](https://www.gutenberg.org/files/108/108-h/108-h), Holmes returns and explains that he escaped. We can revise the earlier ruling while keeping its history.

First, our editorial decision `d-0008` accepts Watson's conclusion, supported
by source `d-0004`. The authoring tool renders the claim as:

```dli
{retcon["death_rendered"][0]}
```

The query returns:

```text
{before["atom"]}: {before["status"]}
```

After the return, decision `d-0009` defeats `d-0008`, and a new claim cites
*The Empty House* as `d-0005`. The new claim is:

```dli
{retcon["survived_rendered"][0]}
```

The new query returns:

```text
{after["atom"]}: {after["status"]}
```

When we ask why the old claim no longer holds, the explanation says:

```text
status: {why["why_not"]["status"]}
support: {", ".join(why["supporting_decisions"])}
{defeat["defeating_decision"]} defeats {defeat["defeated_decision"]}
```

Here `dead` describes the old claim's support under the new rulings. The claim
and the earlier decision are still available to explain what changed. This is
a model of a revised account: the later story does not say Holmes died and
came back to life.
''', end="")
elif chapter == "branch":
    row = result["retcon"]["branch"][0]
    print(f'''# What if Holmes stayed dead?

The `author.branch` call below needs the MCP server's `--tools all` profile.

Before revising the Reichenbach ruling, we made a branch called
`holmes-stayed-dead`. A branch is a separate line of work. The main line can
accept Holmes's return while the branch keeps the earlier starting point.

The earlier claim was:

```dli
{result["retcon"]["death_rendered"][0]}
```

On that branch, the same query returns:

```text
{row["atom"]}: {row["status"]}
```

The branch is a place to explore the consequences of a story in which Watson's
conclusion was right. It does not change the main line. The branch shown here
keeps the earlier ruling; it does not yet invent the later stories that would
follow from it.
''', end="")
elif chapter == "rule":
    finding = result["rule_conflict"]
    rule = "\n".join(result["rule_rendered"])
    print(f'''# When a rule causes the conflict

The `author.add_rule` call below needs the MCP server's `--tools all` profile.

Suppose we interpret the name James used by Watson's wife as referring to
Watson, while the first chapter records John as his first name. Those two notes
can coexist. Now imagine a rule saying the name she uses must be his first name.
This is a deliberately questionable rule: a person might use a nickname.

The authoring tool renders that rule as:

```dli
{rule}
```

The check follows the rule and reports:

```text
{finding["code"]}: {finding["message"]}
citations: {", ".join(finding["citations"])}
```

`d-0001` and `d-0003` identify the two story sources, and `d-0006` identifies
our rule. The conflict did not require us to type the James first-name claim:
it followed from the rule and the note. In a real project, the right repair
might be to change the rule rather than change either source.
''', end="")
else:
    raise SystemExit(f"unknown chapter: {chapter}")

print("\n<details>\n<summary>The calls</summary>\n\n```json\n" + json.dumps(result["calls"][chapter], indent=2, ensure_ascii=False) + "\n```\n\n</details>")
