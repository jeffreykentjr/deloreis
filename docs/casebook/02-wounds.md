# Where was Watson wounded?

In [*A Study in Scarlet*](https://www.gutenberg.org/files/244/244-h/244-h.htm), Watson says a Jezail bullet hit his shoulder. In [*The Sign of the Four*](https://www.gutenberg.org/files/2097/2097-h/2097-h.htm), he describes a Jezail wound in his leg.

Those passages are often treated as a discrepancy. They do not establish that
the two references describe the same injury. We can record the accounts without
choosing between them. Here `proposed` marks an account as unresolved; its
origin label identifies the story in our [source notes](../../examples/holmes/sources.md).

The authoring tools rendered the claims as:

```dli
watson.reported-wound-location         = shoulder                       proposed study-in-scarlet
watson.reported-wound-location         = leg                            proposed sign-of-four
```

A query reports:

```text
reported-wound-location(watson, shoulder): possible
reported-wound-location(watson, leg): possible
```

`possible` means an account remains open in this model. It does not mean the
account is false or that De Lore Is has decided the wounds are the same. If a
writer chooses a single-wound interpretation, they can add that rule and make
a ruling later.

## Run it yourself

From the repository root, with `deloreis` installed, run:

```sh
python3 examples/holmes/try_casebook.py wounds
```

This replays the casebook in a temporary project and prints:

```text
reported-wound-location(watson, shoulder): possible
reported-wound-location(watson, leg): possible
```

<details>
<summary>The calls</summary>

```json
[
  {
    "tool": "author.add_fact",
    "arguments": {
      "workspace": "ws-1",
      "entity": "watson",
      "key": "reported-wound-location",
      "value": "shoulder",
      "provenance": {
        "kind": "proposed",
        "origin": "study-in-scarlet"
      }
    }
  },
  {
    "tool": "author.add_fact",
    "arguments": {
      "workspace": "ws-1",
      "entity": "watson",
      "key": "reported-wound-location",
      "value": "leg",
      "provenance": {
        "kind": "proposed",
        "origin": "sign-of-four"
      }
    }
  },
  {
    "tool": "author.materialize",
    "arguments": {
      "workspace": "ws-1",
      "rendered": true
    }
  },
  {
    "tool": "author.query",
    "arguments": {
      "workspace": "ws-1",
      "goal_atom": {
        "pred": "reported-wound-location",
        "args": [
          {
            "Sym": "watson"
          },
          {
            "Sym": "shoulder"
          }
        ]
      },
      "layer": "all"
    }
  },
  {
    "tool": "author.query",
    "arguments": {
      "workspace": "ws-1",
      "goal_atom": {
        "pred": "reported-wound-location",
        "args": [
          {
            "Sym": "watson"
          },
          {
            "Sym": "leg"
          }
        ]
      },
      "layer": "all"
    }
  },
  {
    "tool": "author.why",
    "arguments": {
      "workspace": "ws-1",
      "goal_atom": {
        "pred": "reported-wound-location",
        "args": [
          {
            "Sym": "watson"
          },
          {
            "Sym": "shoulder"
          }
        ]
      }
    }
  },
  {
    "tool": "author.why",
    "arguments": {
      "workspace": "ws-1",
      "goal_atom": {
        "pred": "reported-wound-location",
        "args": [
          {
            "Sym": "watson"
          },
          {
            "Sym": "leg"
          }
        ]
      }
    }
  }
]
```

</details>
