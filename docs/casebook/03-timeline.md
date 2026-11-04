# Putting cases in order

[*A Scandal in Bohemia*](https://www.gutenberg.org/files/1661/1661-h/1661-h.htm#chap01) places Watson's married visit to Holmes on 20 March 1888. In [*The Sign of the Four*](https://www.gutenberg.org/files/2097/2097-h/2097-h.htm), Mary Morstan describes an advertisement from May 1882 as about six years earlier. Reading that approximation as a precise date can put her meeting with Watson after the March visit. The story does not give us a precise date for that meeting, so we leave the two events unordered.

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

The actual Markdown timeline is:

# Chronology

These are the known orderings. Pairs not listed are unordered.

Points: `mary-case`, `reichenbach-encounter`, `return`, `scandal`, `year-1891`, `year-1894`, `years-1880s`

- `mary-case` before `reichenbach-encounter`
- `mary-case` before `return`
- `mary-case` before `year-1891`
- `mary-case` before `year-1894`
- `reichenbach-encounter` before `return`
- `reichenbach-encounter` before `year-1894`
- `scandal` before `reichenbach-encounter`
- `scandal` before `return`
- `scandal` before `year-1891`
- `scandal` before `year-1894`
- `year-1891` before `return`
- `year-1891` before `year-1894`
- `years-1880s` before `reichenbach-encounter`
- `years-1880s` before `return`
- `years-1880s` before `year-1891`
- `years-1880s` before `year-1894`

Each line states a known ordering. A pair missing from the list is unordered,
not necessarily simultaneous. A writer can add a more specific order if the
story calls for one.

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
  }
]
```

</details>
