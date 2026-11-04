# When a rule causes the conflict

The `author.add_rule` call below needs the MCP server's `--tools all` profile.

Suppose we interpret the name James used by Watson's wife as referring to
Watson, while the first chapter records John as his first name. Those two notes
can coexist. Now imagine a rule saying the name she uses must be his first name.
This is a deliberately questionable rule: a person might use a nickname.

The authoring tool renders that rule as:

```dli
rule wife-name-is-first-name ratified d-0006
  first-name(watson, james) :- name-used-by-wife(watson, james)
```

The check follows the rule and reports:

```text
continuity.derived-fd-violation: the evaluated world derives a functional-dependency break on 'first-name': watson carries 2 distinct certain values [james, john] once rules fire
citations: d-0001, d-0003, d-0006
```

`d-0001` and `d-0003` identify the two story sources, and `d-0006` identifies
our rule. The conflict did not require us to type the James first-name claim:
it followed from the rule and the note. In a real project, the right repair
might be to change the rule rather than change either source.

<details>
<summary>The calls</summary>

```json
[
  {
    "tool": "author.add_rule",
    "arguments": {
      "workspace": "ws-2",
      "name": "wife-name-is-first-name",
      "head": {
        "pred": "first-name",
        "args": [
          {
            "Sym": "watson"
          },
          {
            "Sym": "james"
          }
        ]
      },
      "body": [
        {
          "atom": {
            "pred": "name-used-by-wife",
            "args": [
              {
                "Sym": "watson"
              },
              {
                "Sym": "james"
              }
            ]
          },
          "negated": false
        }
      ],
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0006"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.check",
    "arguments": {
      "workspace": "ws-2"
    }
  }
]
```

</details>
