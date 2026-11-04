# Putting cases in order

[*A Scandal in Bohemia*](https://www.gutenberg.org/files/1661/1661-h/1661-h.htm#chap01) places Watson's married visit to Holmes on 20 March 1888. [*The Sign of the Four*](https://www.gutenberg.org/files/2097/2097-h/2097-h.htm#chap03) puts his meeting with Mary Morstan on a September evening. Three clues point toward 1888: an advertisement of 4 May 1882 was about six years before her visit, her father's disappearance in December 1878 was nearly ten years earlier, and she has received six yearly pearls.

Those clues make September 1888 a strong interpretation, though the story
does not give the year outright. If Watson's wife in the earlier story is Mary,
he is married to her before he meets her. We record both interpretations as
ordering claims: Mary meets Watson before the married visit, and the March
visit comes before their September meeting. The latter is our dating ruling,
not a date printed in the story. De Lore Is reports:

```text
continuity.chronology-cycle: the chronology contradicts itself: an ordering cycle over 2 edge(s) — scandal -> mary-case, mary-case -> scandal
supporting decisions: d-0002, d-0006, d-0007
```

The conflicting claims are staged for the check and not committed. If a writer
does not accept the 1888 interpretation or the identification of Watson's wife,
the two events can remain unordered instead.

The model puts both events in the 1880s, the Reichenbach encounter in 1891,
and Holmes's return in 1894. We record the order of those eras under an
editorial decision. The timeline follows the recorded order; it does not fill
in an order between the two 1880s cases.

The relevant model lines are:

```dli
era years-1880s "The 1880s" ratified d-0006
era year-1891   "1891"      ratified d-0006
era year-1894   "1894"      ratified d-0006
event scandal               "A Scandal in Bohemia"         during years-1880s ratified d-0007
event mary-case             "Mary Morstan consults Holmes" during years-1880s ratified d-0002
event reichenbach-encounter "The Reichenbach encounter"    during year-1891 ratified d-0004
event return                "Holmes returns"               during year-1894 ratified d-0005
before years-1880s -> year-1891 ratified d-0006
before year-1891 -> year-1894 ratified d-0006
```

The committed model's Markdown timeline is:

## Known order

These are the nearest known orderings. Follow the arrows for later points; points without a connecting path are unordered.

Points: `mary-case`, `reichenbach-encounter`, `return`, `scandal`, `year-1891`, `year-1894`, `years-1880s`

- `mary-case` → `reichenbach-encounter`
- `mary-case` → `year-1891`
- `reichenbach-encounter` → `return`
- `reichenbach-encounter` → `year-1894`
- `scandal` → `reichenbach-encounter`
- `scandal` → `year-1891`
- `year-1891` → `return`
- `year-1891` → `year-1894`
- `years-1880s` → `reichenbach-encounter`
- `years-1880s` → `year-1891`

Follow the arrows to see the known order. Points with no connecting path are
unordered, not necessarily simultaneous.

## Run it yourself

From the repository root, with `deloreis` installed, run:

```sh
python3 examples/holmes/try_casebook.py timeline
```

This replays the casebook in a temporary project and prints:

```text
continuity.chronology-cycle
supporting decisions: d-0002, d-0006, d-0007
```

<details>
<summary>The calls</summary>

```json
[
  {
    "tool": "author.add_event",
    "arguments": {
      "workspace": "ws-1",
      "id": "scandal",
      "name": "A Scandal in Bohemia",
      "era": "years-1880s",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0007"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.add_event",
    "arguments": {
      "workspace": "ws-1",
      "id": "mary-case",
      "name": "Mary Morstan consults Holmes",
      "era": "years-1880s",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0002"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.add_event",
    "arguments": {
      "workspace": "ws-1",
      "id": "reichenbach-encounter",
      "name": "The Reichenbach encounter",
      "era": "year-1891",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0004"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.add_event",
    "arguments": {
      "workspace": "ws-1",
      "id": "return",
      "name": "Holmes returns",
      "era": "year-1894",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0005"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.add_before",
    "arguments": {
      "workspace": "ws-1",
      "earlier": "years-1880s",
      "later": "year-1891",
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
    "tool": "author.add_before",
    "arguments": {
      "workspace": "ws-1",
      "earlier": "year-1891",
      "later": "year-1894",
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
    "tool": "author.add_before",
    "arguments": {
      "workspace": "ws-1",
      "earlier": "mary-case",
      "later": "scandal",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0002"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.add_before",
    "arguments": {
      "workspace": "ws-1",
      "earlier": "scandal",
      "later": "mary-case",
      "provenance": {
        "kind": "cited",
        "decisions": [
          "d-0002",
          "d-0006",
          "d-0007"
        ],
        "state": "ratified"
      }
    }
  },
  {
    "tool": "author.check",
    "arguments": {
      "workspace": "ws-1"
    }
  }
]
```

</details>
