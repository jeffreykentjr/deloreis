# The return from Reichenbach

At the end of [*The Final Problem*](https://www.gutenberg.org/files/834/834-h/834-h.htm), Watson concludes that Holmes died at Reichenbach. In [*The Empty House*](https://www.gutenberg.org/files/108/108-h/108-h), Holmes returns and explains that he escaped. We can revise the earlier ruling while keeping its history.

First, our editorial decision `d-0008` accepts Watson's conclusion, supported
by source `d-0004`. The authoring tool renders the claim as:

```dli
holmes.reichenbach-outcome = died ratified d-0004 d-0008
```

The query returns:

```text
reichenbach-outcome(holmes, died): certain
```

After the return, decision `d-0009` defeats `d-0008`, and a new claim cites
*The Empty House* as `d-0005`. The new claim is:

```dli
holmes.reichenbach-outcome = survived ratified d-0005 d-0009
```

The new query returns:

```text
reichenbach-outcome(holmes, survived): certain
```

When we ask why the old claim no longer holds, the explanation says:

```text
status: dead
support: d-0004, d-0008
d-0009 defeats d-0008
```

Here `dead` describes the old claim's support under the new rulings. The claim
and the earlier decision are still available to explain what changed. This is
a model of a revised account: the later story does not say Holmes died and
came back to life.

<details>
<summary>The calls</summary>

```json
[
  {
    "tool": "author.record_decision",
    "arguments": {
      "workspace": "ws-3",
      "decision": "d-0008"
    }
  },
  {
    "tool": "author.add_fact",
    "arguments": {
      "workspace": "ws-3",
      "entity": "holmes",
      "key": "reichenbach-outcome",
      "value": "died",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0004",
          "d-0008"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.materialize",
    "arguments": {
      "workspace": "ws-3"
    }
  },
  {
    "tool": "author.query",
    "arguments": {
      "workspace": "ws-3",
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
  },
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
    "tool": "author.defeat",
    "arguments": {
      "workspace": "ws-3",
      "claim_id": "sha256:970cd2741b4bfe5631c5514e3fe8a8b0906ea68905cba85d9d12baf2316fabbe",
      "decision": "d-0009",
      "defeated_decision": "d-0008"
    }
  },
  {
    "tool": "author.add_fact",
    "arguments": {
      "workspace": "ws-3",
      "entity": "holmes",
      "key": "reichenbach-outcome",
      "value": "survived",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0005",
          "d-0009"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.materialize",
    "arguments": {
      "workspace": "ws-3"
    }
  },
  {
    "tool": "author.query",
    "arguments": {
      "workspace": "ws-3",
      "goal_atom": {
        "pred": "reichenbach-outcome",
        "args": [
          {
            "Sym": "holmes"
          },
          {
            "Sym": "survived"
          }
        ]
      },
      "layer": "all"
    }
  },
  {
    "tool": "author.query",
    "arguments": {
      "workspace": "ws-3",
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
  },
  {
    "tool": "author.why",
    "arguments": {
      "workspace": "ws-3",
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
      }
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
