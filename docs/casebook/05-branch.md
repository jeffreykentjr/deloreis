# What if Holmes stayed dead?

The `author.branch` call below needs the MCP server's `--tools all` profile.

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

Use the branch to explore a story in which Watson's conclusion was right. So
far it holds only the earlier ruling; the later stories that would follow are
yours to write.

## Run it yourself

From the repository root, with `deloreis` installed, run:

```sh
python3 examples/holmes/try_casebook.py branch
```

This replays the casebook in a temporary project and prints:

```text
reichenbach-outcome(holmes, died): certain
```

<details>
<summary>The calls</summary>

```json
[
  {
    "tool": "author.branch",
    "arguments": {
      "workspace": "ws-4",
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
      "workspace": "ws-5",
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
