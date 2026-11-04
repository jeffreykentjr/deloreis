# What if Holmes stayed dead?

Before revising the Reichenbach ruling, we made a branch called
`holmes-stayed-dead`. A branch is a separate line of work. The main line can
accept Holmes's return while the branch keeps the earlier starting point.

The earlier claim was:

```dli
holmes.reichenbach-outcome = died ratified d-0004 d-0008
```

On that branch, the same query returns:

```text
reichenbach-outcome(holmes, died): certain
```

The branch is a place to explore the consequences of a story in which Watson's
conclusion was right. It does not change the main line. The branch shown here
keeps the earlier ruling; it does not yet invent the later stories that would
follow from it.

<details>
<summary>The calls</summary>

```json
[
  {
    "tool": "author.branch",
    "arguments": {
      "workspace": "ws-3",
      "action": "create",
      "name": "holmes-stayed-dead",
      "from": "main"
    }
  },
  {
    "tool": "author.open",
    "arguments": {
      "scope": "world",
      "branch": "holmes-stayed-dead"
    }
  },
  {
    "tool": "author.query",
    "arguments": {
      "workspace": "ws-4",
      "goal_atom": {
        "pred": "reichenbach-outcome",
        "args": [
          {
            "Sym": "holmes"
          },
          {
            "Sym": "died"
          }
        ]
      },
      "layer": "all"
    }
  }
]
```

</details>
