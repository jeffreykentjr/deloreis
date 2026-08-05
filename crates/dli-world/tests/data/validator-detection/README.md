# The repository validator's detection corpus (VAL-1)

Data for `crates/dli-world/tests/repository_detection.rs`. This directory is
DATA only — no Rust, no gate wiring. The written-up mapping, the finding-shape
compatibility promise, and V9's absorption argument live in
`docs/design-notes/native-repository-validator.md`; the ruling is DL-0057.

## Files

| Path | What |
|---|---|
| `baseline/` | A complete canon repository that validates to the **empty** finding list. |
| `operators.yaml` | 26 seeded-divergence operators, each naming every finding it must produce. |

## Why the baseline matters as much as the operators

A validator that reports nothing on a broken tree is useless, and one that
reports something on a clean tree is worse — it trains its readers to ignore it.
`baseline/` is the second half of that pair: a repository exercising every
positive requirement of all eleven checks at once, asserted to produce zero
findings. Every operator is measured as a delta from it, so an operator's
expected set is exactly what the seeded defect caused.

The baseline also carries the *negative* controls that would otherwise be
untested: an exempt `README.md` and `SCHEMA.md` with no frontmatter, an exempt
`sources/` tree, a `note`-register text with no passage, a `proposed` section
citing nothing, a whole-file `affects` target and a section-scoped one, and a
`### ` subsection override. Each of those is a rule that says *no finding here*,
and the pristine census is what proves the validator honours them.

## Coverage

Eleven codes, twenty-six operators, no honest misses. Every operator is a real
catch; nothing here is `xfail`.

| Code | Operators |
|---|---|
| V1 | `v1-missing-frontmatter`, `v1-unparsable-frontmatter` |
| V2 | `v2-invalid-type-enum`, `v2-id-stem-divergence` |
| V3 | `v3-status-marker-stripped` |
| V4 | `v4-malformed-decision-reference`, `v4-unknown-decision-cited`, `v4-closed-question-cited`, `v4-duplicate-ledger-id` |
| V5 | `v5-dangling-wiki-link`, `v5-duplicate-id` |
| V6 | `v6-unknown-era`, `v6-unparsable-yaml`, `v6-duplicate-character-id` |
| V7 | `v7-registry-row-without-file`, `v7-orphan-character-file` |
| V8 | `v8-banned-word-diegetic-scope`, `v8-banned-word-all-scope` |
| V10 | `v10-open-question-filed-as-resolved`, `v10-resolved-question-left-open` |
| V11 | `v11-forged-promotion`, `v11-affects-scope-stripped` |
| V12 | `v12-unknown-register`, `v12-missing-passage`, `v12-duplicate-work-id`, `v12-overlong-passage` |

**V9 has no operator and never will.** It closed by absorption, and its native
successors are a theorem in `dli-core` plus the MIG-1 replay legs — not a
mutation of this tree. The design note carries the argument and its three
grades.

## What the corpus cannot express

Two branches of the validator are reachable in production and not from committed
inputs, and they are named here rather than quietly uncovered:

- `check_frontmatter`'s `cannot read file` (V1) and `read_yaml`'s
  `cannot read YAML` (V6) both need a path that exists and cannot be read. Git
  cannot record that state portably, and seeding it at test time would mean
  either a `chmod` or a broken symlink — a platform-conditional operator, which
  is a worse trade than a named gap.

Neither is a detection hole: both branches only convert an I/O error into a
finding, and the property that matters — that they are findings and not panics —
is asserted by `v6-unparsable-yaml` and by the missing-root case in the
finding-shape promise.

## Adding an operator

1. Seed the divergence in `operators.yaml`. A `replace` seed's `from` must match
   exactly once; the harness asserts this, because a seed that silently matched
   nothing would test the baseline instead of the defect.
2. Run the suite and read the reported finding set. Record it — path, line, and
   a distinguishing message fragment — rather than the set you expected.
3. If the observed set surprises you, that is a finding about the validator.
   Write it down in the operator's `rationale` and in the design note. Do not
   reshape the seed until the surprise goes away.
