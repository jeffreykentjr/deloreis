# Canon Schema — the contract

This file defines how canon is encoded. The validator (`fantasy validate`) enforces it;
CI runs the validator on every push. **This file, `decisions.md`, `open-questions.md`,
`sources/**`, and any `README.md` are exempt from the section-marker rules below.**

## Why this exists

The canon was distilled from a long exploratory session that mixes four very different
kinds of material: decisions Jeff explicitly made, AI proposals he approved
directionally, AI proposals he never ruled on, and proposals he explicitly rejected.
Losing track of which is which would poison every downstream generation step. So every
claim in canon carries an explicit ratification status, and every ratified/rejected
claim traces to a quoted decision in the ledger.

## Ratification statuses

| Status | Meaning | May generators rely on it? |
|---|---|---|
| `ratified` | Jeff explicitly decided or approved | Yes |
| `provisional` | Jeff approved the direction; substance is AI-authored ("yeah, good") | Yes, flagged as mutable |
| `proposed` | AI suggestion never ruled on | No — must not be asserted, must not be contradicted |
| `rejected` | Jeff explicitly said no. Kept forever as a guardrail | Anti-canon: generators must not produce content consistent with it |
| `open-question` | Unresolved fork awaiting Jeff | No; must cite a Q-id |

## File frontmatter

Every `canon/**/*.md` file (except exempt files) opens with YAML frontmatter:

```yaml
---
id: magic-system            # required; kebab-case; globally unique; must equal filename stem
title: The Spoken Proof     # required
type: system                # required; one of: concept|system|character|place|faction|narrative|meta
status: ratified            # required; the five-status vocabulary above
tags: [magic, world]        # optional
source: gemini-session-2026-07   # optional pointer into sources/
diegetic: false             # optional (default false); true = text may appear in-world → banned-lexicon linted
lexicon_exceptions: []      # optional per-file escape hatch for the banned-vocabulary scan
---
```

Unknown keys are validation errors. File-level `status` answers "does this entity
belong in canon at all?" — a `ratified` character file means the character exists;
individual sections inside may still be `proposed`.

## Per-section status markers

The **first non-blank line after every `##` heading** must be a status line:

```
> status: ratified (D-0003)
> status: provisional (D-0007, D-0012)
> status: proposed
> status: rejected (D-0001)
> status: open-question (Q-0002)
```

Exact grammar (regex):

```
^> status: (ratified|provisional|proposed|rejected|open-question)( \((D-\d{4}(, D-\d{4})*|Q-\d{4})\))?$
```

Citation rules:
- `ratified`, `provisional`, `rejected` **require ≥1 `D-nnnn`** reference.
- `open-question` **requires exactly one `Q-nnnn`** reference.
- `proposed` takes **no** reference (there is no decision to cite — that's the point).
- `###` subsections inherit the parent `##` status; they may override with their own line.
- A file with no `##` headings is governed entirely by its frontmatter status.

## The decision ledger — `decisions.md`

Append-only. One `##` block per decision:

```markdown
## D-0003 — Magic is spoken, rigorous logical proof
- status: ratified
- date: 2026-07-05
- affects: [[magic-system#Core law]]
- quote: "the spells should be literal rigorous logical proofs (e.g. encodable in lean)"
```

Rules: `D-nnnn` ids are unique, never reused, never deleted. Required keys: `status`,
`date`, `quote` (Jeff's actual words; for structural decisions made outside the source
session, a short paraphrase marked `(paraphrase)`). Optional: `affects`, `note`,
`supersedes`, `superseded-by`. A reversal is a **new** decision with
`supersedes: D-nnnn`; the old entry gains `superseded-by: D-mmmm` and keeps its text.

### `affects` — section-granular provenance (V11)

`affects` is a comma-separated list of `[[wikilinks]]` naming **where the ruling
actually landed**, at two granularities:

- `[[file-id#Section Heading]]` — the ruling lands in one specific `##` (or overriding
  `###`) section. The text after `#` is the section's **exact heading text**.
- `[[file-id]]` — the ruling governs the **whole file** (a legal fallback: any section
  in that file may cite the decision).

**Matching semantics.** A `[[file#H]]` reference matches a section whose heading, after
**casefolding and collapsing internal whitespace to a single space**, equals `H`
normalized the same way. Heading text is unique within a file, so a reference is
unambiguous; punctuation (`'`, `"`, `,`, `:`, `/`, `-`, `—`, parentheses) is kept
verbatim and compared literally (only case and whitespace are normalized).

**The provenance rule (V11).** A `ratified`/`provisional` section that cites `D-x` is a
violation unless `D-x`'s `affects` covers it — i.e. lists that exact `file#section`, or
lists the whole file `[[file]]`. This closes the *forged-promotion* hole: flipping a
`proposed` section to `ratified` with a real-but-unauthorized `D-id` now fails, because
that decision's `affects` does not name the forged section. `affects` is thus the
**author's record of intent**, section by section — cite a decision on a section only if
the decision's `affects` was authored to include it. A decision cited by no prose
section (a purely process/rejected ruling) may carry a whole-file `affects` or none.

## Open questions

`open-questions.md` holds LIVE questions only — it is the author's ruling queue and
must stay readable at a glance. When a question resolves (a `D-nnnn` lands and
`resolved-by:` is set), its whole `## Q-nnnn` block MOVES to `resolved-questions.md`
(same block format; context/options preserved as history). Validator rule **V10**
enforces placement: a resolved question in the open file, an unresolved question in
the archive, or the same id in both files is a violation.

### Question-block format — `open-questions.md`

```markdown
## Q-0002 — What is the series title?
- status: open
- raised: 2026-07-05
- context: "The Iron Ledger" was invented by the Gemini agent for a pitch doc; never ratified.
- options:
  - keep The Iron Ledger
  - something drawn from Wyrd/Frith lore
```

`status` is `open` or `resolved`. Resolving adds `- resolved-by: D-nnnn` and the canon
sections that cited the Q get rewritten with the new D reference. A section may not
cite a resolved question.

## Cross-links

Wiki-style `[[id]]` (alias form `[[id|display text]]`). Targets resolve against the
global id index: **canon file ids + `data/timeline.yaml` event ids**. Ids are one flat
namespace; uniqueness is enforced across those two sources. Entries in
`data/characters.yaml` do **not** introduce new ids — a registry row's `id` is a
reference to the character's canon file id (`characters/<id>.md`), and the two are
required to stay in bidirectional sync.

## Structured data — `canon/data/`

Only four files; everything with narrative texture stays prose.

- `timeline.yaml` — `eras:` (ordered list of era ids) and `events:` (id, name, era,
  offset, status, decision, summary, links). `era` must exist in `eras`.
- `lexicon.yaml` — `terms:` (term, gloss, status, decision, notes?). Diegetic glossary.
- `banned-vocabulary.yaml` — `banned:` (word, scope: `diegetic|all`, reason,
  match: `stem|exact` [default stem], alternatives?). Words that must never surface in
  diegetic text (modern/technological register).
- `characters.yaml` — `characters:` (id, name, aliases?, role, status, decision).
  Registry basics only; prose lives in `characters/<id>.md`. Every registry row has a
  matching file and vice versa.

In YAML entries the same citation rule applies: `ratified`/`provisional`/`rejected`
entries carry a `decision:` D-id; `proposed`/`open-question` entries may omit it.

## Authoring discipline

- New lore enters as `proposed`. Promotion to `ratified`/`provisional` is Jeff's act,
  recorded as a ledger entry.
- Rejected material is never deleted — it is the guardrail corpus.
- Files marked `diegetic: true` (and any generated in-world text) must pass the
  banned-vocabulary scan: the technology allegory stays beneath the floorboards.
