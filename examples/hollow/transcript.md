---
status: reference
verdict: "A replayed MCP walkthrough of The Hollow."
---

# The Hollow: a replayed authoring session

These excerpts come from the checked MCP driver.

## 1. Wren's home is canon

```json
[
  {
    "atom": "home(wren, thornfield)",
    "bindings": {},
    "claim_ids": [
      "sha256:a7317a9e47824bdd8af5ca1c6f5afe7a10cedff55dc76564ac2b29640258d0da"
    ],
    "status": "certain",
    "verdict": {
      "grade": "asserted",
      "support": {
        "monomials": [
          [
            "+0"
          ]
        ]
      }
    }
  }
]
```

## 2. Conflicting canon is refused

```json
{
  "error": {
    "code": "materialize-blocked",
    "data": {
      "blocking": [
        "fact.contradiction",
        "continuity.fd-violation"
      ]
    },
    "message": "2 error-severity finding(s) are unresolved; resolve them or discard before materializing."
  },
  "finding": {
    "citations": [
      "d-0001",
      "d-0002"
    ],
    "code": "continuity.fd-violation",
    "data": {
      "pred": "home",
      "slot_key": "home/wren",
      "subject": "wren",
      "values": [
        "ashby",
        "thornfield"
      ]
    },
    "grade": "checked",
    "message": "canon breaks the functional dependency on 'home': wren carries 2 distinct certain values [ashby, thornfield]",
    "severity": "error"
  }
}
```

## 3. Two witnesses, two possible accounts

```json
{
  "drowned": {
    "status": "possible",
    "why": [
      "witness-one"
    ]
  },
  "poisoned": {
    "status": "possible",
    "why": [
      "witness-two"
    ]
  }
}
```

## 4. Retcon and restoration

```json
{
  "after_overturn": [
    {
      "atom": "death(miller, drowned)",
      "bindings": {},
      "claim_ids": [
        "sha256:2413010e72f14f211a2dfec41ed8e1a82d27b31287cb47defd3e76f67e169841"
      ],
      "status": "certain",
      "verdict": {
        "grade": "asserted",
        "support": {
          "monomials": [
            [
              "+1"
            ],
            [
              "+3"
            ]
          ]
        }
      }
    }
  ],
  "after_retcon": [
    {
      "atom": "death(miller, poisoned)",
      "bindings": {},
      "claim_ids": [
        "sha256:e86996be1770124deabb04ac7b574fd8a1971973bbb062aca94705388db21fd3"
      ],
      "status": "certain",
      "verdict": {
        "grade": "asserted",
        "support": {
          "monomials": [
            [
              "+2"
            ],
            [
              "+4"
            ]
          ]
        }
      }
    }
  ],
  "blocked_supports": [
    {
      "blocking_defeats": [
        {
          "defeated_decision": "d-0003",
          "defeating_decision": "d-0004"
        }
      ],
      "supporting_decision_path": [
        "d-0003"
      ]
    }
  ]
}
```

## 5. What if Wren stayed?

```json
[
  {
    "after": {
      "grade": "asserted",
      "status": "possible"
    },
    "atom": {
      "args": [
        "wren",
        "stayed"
      ],
      "pred": "journey"
    },
    "before": {
      "grade": "asserted",
      "status": "dead"
    },
    "rendered": "journey(wren, stayed): dead (asserted) -> possible (asserted)"
  },
  {
    "after": {
      "grade": "asserted",
      "status": "possible"
    },
    "atom": {
      "args": [
        "wren",
        "thornfield"
      ],
      "pred": "welcome"
    },
    "before": {
      "grade": "asserted",
      "status": "dead"
    },
    "rendered": "welcome(wren, thornfield): dead (asserted) -> possible (asserted)"
  }
]
```

## 6. Replay after restart

```json
[
  {
    "atom": "death(miller, drowned)",
    "bindings": {},
    "claim_ids": [
      "sha256:2413010e72f14f211a2dfec41ed8e1a82d27b31287cb47defd3e76f67e169841"
    ],
    "status": "certain",
    "verdict": {
      "grade": "asserted",
      "support": {
        "monomials": [
          [
            "+1"
          ],
          [
            "+3"
          ]
        ]
      }
    }
  }
]
```
