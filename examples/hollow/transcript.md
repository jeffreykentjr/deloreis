---
status: reference
verdict: "A replayed MCP walkthrough of The Hollow."
---

# The Hollow: a replayed authoring session

These excerpts come from the checked MCP driver.

## 1. Wren's home is canon

```json
{
  "calls": [
    {
      "arguments": {
        "scope": "world"
      },
      "tool": "author.open"
    },
    {
      "arguments": {
        "decision": "d-0001",
        "workspace": "ws-1"
      },
      "tool": "author.record_decision"
    },
    {
      "arguments": {
        "id": "wren",
        "kind": "person",
        "name": "Wren",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-1"
      },
      "tool": "author.add_entity"
    },
    {
      "arguments": {
        "id": "thornfield",
        "kind": "place",
        "name": "Thornfield",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-1"
      },
      "tool": "author.add_entity"
    },
    {
      "arguments": {
        "id": "ashby",
        "kind": "place",
        "name": "Ashby",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-1"
      },
      "tool": "author.add_entity"
    },
    {
      "arguments": {
        "id": "miller",
        "kind": "person",
        "name": "The Miller",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-1"
      },
      "tool": "author.add_entity"
    },
    {
      "arguments": {
        "id": "before-rain",
        "kind": "era",
        "name": "Before the Rain",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-1"
      },
      "tool": "author.add_entity"
    },
    {
      "arguments": {
        "id": "after-rain",
        "kind": "era",
        "name": "After the Rain",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-1"
      },
      "tool": "author.add_entity"
    },
    {
      "arguments": {
        "pred": "home",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-1"
      },
      "tool": "author.declare_functional"
    },
    {
      "arguments": {
        "entity": "wren",
        "key": "home",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "value": "thornfield",
        "workspace": "ws-1"
      },
      "tool": "author.add_fact"
    },
    {
      "arguments": {
        "workspace": "ws-1"
      },
      "tool": "author.materialize"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "wren"
            },
            {
              "Sym": "thornfield"
            }
          ],
          "pred": "home"
        },
        "workspace": "ws-1"
      },
      "tool": "author.query"
    }
  ],
  "result": [
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
}
```

## 2. Conflicting canon is refused

```json
{
  "calls": [
    {
      "arguments": {
        "decision": "d-0002",
        "workspace": "ws-1"
      },
      "tool": "author.record_decision"
    },
    {
      "arguments": {
        "entity": "wren",
        "key": "home",
        "provenance": {
          "decisions": [
            "d-0002"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "value": "ashby",
        "workspace": "ws-1"
      },
      "tool": "author.add_fact"
    },
    {
      "arguments": {
        "workspace": "ws-1"
      },
      "tool": "author.check"
    },
    {
      "arguments": {
        "workspace": "ws-1"
      },
      "tool": "author.materialize"
    }
  ],
  "result": {
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
}
```

## 3. Two witnesses, two possible accounts

```json
{
  "calls": [
    {
      "arguments": {
        "workspace": "ws-1"
      },
      "tool": "author.discard"
    },
    {
      "arguments": {
        "scope": "world"
      },
      "tool": "author.open"
    },
    {
      "arguments": {
        "id": "miller-death",
        "prompt": "How did the miller die?",
        "workspace": "ws-2"
      },
      "tool": "author.open_question"
    },
    {
      "arguments": {
        "id": "witness-one",
        "implies": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "label": "witness-one",
        "question": "miller-death",
        "workspace": "ws-2"
      },
      "tool": "author.add_option"
    },
    {
      "arguments": {
        "id": "witness-two",
        "implies": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "poisoned"
            }
          ],
          "pred": "death"
        },
        "label": "witness-two",
        "question": "miller-death",
        "workspace": "ws-2"
      },
      "tool": "author.add_option"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-2"
      },
      "tool": "author.query"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "poisoned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-2"
      },
      "tool": "author.query"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "workspace": "ws-2"
      },
      "tool": "author.why"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "poisoned"
            }
          ],
          "pred": "death"
        },
        "workspace": "ws-2"
      },
      "tool": "author.why"
    }
  ],
  "result": {
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
}
```

## 4. Retcon and restoration

```json
{
  "calls": [
    {
      "arguments": {
        "workspace": "ws-2"
      },
      "tool": "author.materialize"
    },
    {
      "arguments": {
        "scope": "world"
      },
      "tool": "author.open"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-3"
      },
      "tool": "author.query"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "workspace": "ws-3"
      },
      "tool": "author.why"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "poisoned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-3"
      },
      "tool": "author.query"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "poisoned"
            }
          ],
          "pred": "death"
        },
        "workspace": "ws-3"
      },
      "tool": "author.why"
    },
    {
      "arguments": {
        "decision": "d-0003",
        "workspace": "ws-2"
      },
      "tool": "author.record_decision"
    },
    {
      "arguments": {
        "entity": "miller",
        "key": "death",
        "provenance": {
          "decisions": [
            "d-0003"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "value": "drowned",
        "workspace": "ws-2"
      },
      "tool": "author.add_fact"
    },
    {
      "arguments": {
        "workspace": "ws-2"
      },
      "tool": "author.materialize"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-2"
      },
      "tool": "author.query"
    },
    {
      "arguments": {
        "claim_id": "sha256:2413010e72f14f211a2dfec41ed8e1a82d27b31287cb47defd3e76f67e169841",
        "decision": "d-0004",
        "defeated_decision": "d-0003",
        "workspace": "ws-2"
      },
      "tool": "author.defeat"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "workspace": "ws-2"
      },
      "tool": "author.why"
    },
    {
      "arguments": {
        "entity": "miller",
        "key": "death",
        "provenance": {
          "decisions": [
            "d-0004"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "value": "poisoned",
        "workspace": "ws-2"
      },
      "tool": "author.add_fact"
    },
    {
      "arguments": {
        "workspace": "ws-2"
      },
      "tool": "author.materialize"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "poisoned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-2"
      },
      "tool": "author.query"
    },
    {
      "arguments": {
        "claim_id": "sha256:e86996be1770124deabb04ac7b574fd8a1971973bbb062aca94705388db21fd3",
        "decision": "d-0005",
        "defeated_decision": "d-0004",
        "workspace": "ws-2"
      },
      "tool": "author.defeat"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-2"
      },
      "tool": "author.query"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "poisoned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-2"
      },
      "tool": "author.query"
    }
  ],
  "result": {
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
    ],
    "overturned_account": [
      {
        "atom": "death(miller, poisoned)",
        "bindings": {},
        "claim_ids": [
          "sha256:e86996be1770124deabb04ac7b574fd8a1971973bbb062aca94705388db21fd3"
        ],
        "status": "possible",
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
    ]
  }
}
```

## 5. What if Wren stayed?

```json
{
  "calls": [
    {
      "arguments": {
        "body": [
          {
            "atom": {
              "args": [
                {
                  "Sym": "wren"
                },
                {
                  "Sym": "stayed"
                }
              ],
              "pred": "journey"
            },
            "negated": true
          }
        ],
        "head": {
          "args": [
            {
              "Sym": "wren"
            },
            {
              "Sym": "thornfield"
            }
          ],
          "pred": "away"
        },
        "name": "away-if-not-stayed",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-2"
      },
      "tool": "author.add_rule"
    },
    {
      "arguments": {
        "workspace": "ws-2"
      },
      "tool": "author.materialize"
    },
    {
      "arguments": {
        "action": "create",
        "from": "main",
        "name": "never-left",
        "workspace": "ws-2"
      },
      "tool": "author.branch"
    },
    {
      "arguments": {
        "branch": "never-left",
        "scope": "world"
      },
      "tool": "author.open"
    },
    {
      "arguments": {
        "decision": "d-0006",
        "workspace": "ws-4"
      },
      "tool": "author.record_decision"
    },
    {
      "arguments": {
        "entity": "wren",
        "key": "journey",
        "provenance": {
          "decisions": [
            "d-0006"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "value": "stayed",
        "workspace": "ws-4"
      },
      "tool": "author.add_fact"
    },
    {
      "arguments": {
        "body": [
          {
            "atom": {
              "args": [
                {
                  "Sym": "wren"
                },
                {
                  "Sym": "stayed"
                }
              ],
              "pred": "journey"
            },
            "negated": false
          }
        ],
        "head": {
          "args": [
            {
              "Sym": "wren"
            },
            {
              "Sym": "thornfield"
            }
          ],
          "pred": "welcome"
        },
        "name": "welcome-if-stayed",
        "provenance": {
          "decisions": [
            "d-0001"
          ],
          "kind": "cited",
          "state": "ratified"
        },
        "workspace": "ws-4"
      },
      "tool": "author.add_rule"
    },
    {
      "arguments": {
        "workspace": "ws-4"
      },
      "tool": "author.diff"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "wren"
            },
            {
              "Sym": "thornfield"
            }
          ],
          "pred": "away"
        },
        "workspace": "ws-2"
      },
      "tool": "author.query"
    }
  ],
  "result": [
    {
      "after": {
        "grade": "asserted",
        "status": "possible",
        "support": [
          [
            "+d-0001"
          ]
        ]
      },
      "atom": "away(wren, thornfield)",
      "before": {
        "grade": "asserted",
        "status": "certain",
        "support": [
          [
            "+d-0001"
          ]
        ]
      },
      "introduced": false,
      "rendered": "away(wren, thornfield): certain (asserted) -> possible (asserted)",
      "retracted": false
    },
    {
      "after": {
        "grade": "asserted",
        "status": "certain",
        "support": [
          [
            "+d-0006"
          ]
        ]
      },
      "atom": "journey(wren, stayed)",
      "introduced": true,
      "rendered": "journey(wren, stayed): absent -> certain (asserted)",
      "retracted": false
    },
    {
      "after": {
        "grade": "asserted",
        "status": "certain",
        "support": [
          [
            "+d-0001",
            "+d-0006"
          ]
        ]
      },
      "atom": "welcome(wren, thornfield)",
      "introduced": true,
      "rendered": "welcome(wren, thornfield): absent -> certain (asserted)",
      "retracted": false
    }
  ]
}
```

## 6. Replay after restart

```json
{
  "calls": [
    {
      "arguments": {
        "scope": "world"
      },
      "tool": "author.open"
    },
    {
      "arguments": {
        "goal_atom": {
          "args": [
            {
              "Sym": "miller"
            },
            {
              "Sym": "drowned"
            }
          ],
          "pred": "death"
        },
        "layer": "all",
        "workspace": "ws-1"
      },
      "tool": "author.query"
    }
  ],
  "result": [
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
}
```
