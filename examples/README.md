# Examples

Probe-compatible sketches in the gentle subset. The concrete grammar is
**syntax-pending**: these files exist so the docs-oracle has a real corpus and so
the shape of a world is legible before the parser lands. The AST
(`crates/dli-ast`) is the normative surface; when the parser arrives at SURF-1
these examples become its first golden inputs.

- `saltmarket.dli` — a small world: a pack, an era and event, places with
  adjacency, people and a thing, a fact, and an open question.
- `true-naming.dli` — a practice sketch: a register, a coined lexeme and its
  morphemes, a craft principle, and a fork.

Every line here passes `dli_syntax::probe_source`; the docs-oracle enforces it.
