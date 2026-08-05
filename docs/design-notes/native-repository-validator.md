---
status: reference
verdict: "VAL-1's written-up mapping: the legacy Python V-slices against their native successors (eleven codes, V1–V8 and V10–V12), V9's closure by absorption with its three honest grades, the detection corpus's per-code coverage, and the normative finding-shape compatibility promise — including the one line-numbering deviation that remains after DL-0060 corrected V8 to file-absolute lines — that the consuming project's shadow-validate harness compares against."
---

# The native repository validator

The ruling is DL-0057. This note is the evidence behind it: what each check
does, what the legacy validator did, why there is no V9, and exactly what a
consumer may rely on in the output.

The validator is `dli_world::validate_repository`
(`crates/dli-world/src/repository.rs`), served as `deloreis validate-repo
<canon-root>`. It reads a Markdown canon repository — authority documents with
YAML frontmatter, a decision ledger, question files, and structured data files —
and returns findings. It calls no model, reads no clock, and touches no network.

## 1. The V-slice mapping

Eleven codes. There is no V9; §2 is why.

| Code | What it checks | Native implementation | Grade |
|---|---|---|---|
| V1 | A frontmatter block is present, terminated, parses as YAML, and is a non-empty mapping | `check_frontmatter` | `tested` |
| V2 | The frontmatter conforms: closed field set; `id`/`title` strings; `type`/`status` in their enums; `id` kebab-case and equal to the filename stem; `tags`/`lexicon_exceptions` lists of strings; `source` string-or-null; `diegetic` boolean | `check_frontmatter` | `tested` |
| V3 | Every `## ` section opens with a well-formed `> status:` marker, and the marker's citations fit its status: `ratified`/`provisional`/`rejected` need ≥1 `D-` and no `Q-`; `open-question` needs exactly one `Q-` and no `D-`; `proposed` cites nothing. `### ` subsections may override, and a malformed override is a finding | `check_sections`, `check_marker_shape` | `tested` |
| V4 | Ledger integrity and citation resolution: no duplicate `D-` id in `decisions.md`; every cited `D-` resolves; every cited `Q-` resolves **and is open** | `load_ledger_ids`, `check_citations` | `tested` |
| V5 | Intra-canon id uniqueness across all non-exempt files, and every `[[wiki-link]]` resolving in the id index | `build_id_index`, `check_links` | `tested` |
| V6 | The structured data files: `data/timeline.yaml` (eras a list; events mappings with unique `id`, a declared `era`, a valid `status`) and `data/characters.yaml` (a list of mappings with unique `id`, string `name`/`role`, valid `status`). A file that will not parse is a finding | `check_structured_data`, `check_timeline`, `check_characters`, `read_yaml` | `tested` |
| V7 | The character registry and the character files name each other, both directions | `check_characters_sync` | `tested` |
| V8 | The banned-vocabulary scan over document bodies, honouring per-entry `scope` (`diegetic` vs `all`), `match` (`stem` vs `exact`), and per-file `lexicon_exceptions` | `check_lexicon` | `tested` |
| ~~V9~~ | *Absorbed — see §2* | `dli_core::derivation::Derivation::support_of`; `mig1_replay` | mixed; see §2 |
| V10 | Questions live in the file their status names: everything in `open-questions.md` is `open`, everything in `resolved-questions.md` is `resolved` | `check_question_placement` | `tested` |
| V11 | Section-granular provenance: a `ratified` or `provisional` section citing a **known** decision must be named by that decision's `- affects:` line, either whole-file (`[[doc]]`) or section-scoped (`[[doc#Heading]]`) | `check_affects_provenance` | `tested` |
| V12 | The in-world text corpus under `texts/`: each section declares a `> kind:` from the closed register list; every register except `note` carries at least one `*"…"*` passage; passages are non-empty and under 1200 characters; work ids (slugged titles) are unique across the corpus | `check_texts_corpus` | `tested` |

Grades are `tested` because each code's behaviour is asserted by the detection
corpus (§4) over committed inputs, not because a type forbids the violation. V9's
structural half is the one `checked` claim in the table, and it is `checked`
precisely because it is no longer a check.

### Exemptions

`SCHEMA.md`, `decisions.md`, `open-questions.md`, `resolved-questions.md`, and
`README.md` are exempt by basename anywhere in the tree; everything whose first
path component is `sources/` is exempt by prefix. Exempt files are skipped by
V1, V2, V3, V4 (as *subjects* — `decisions.md` is still read as authority), V5,
V8, and V11. They are not in the id index, so a `[[wiki-link]]` to one does not
resolve.

## 2. V9 closed by absorption

The legacy V9 forbade a ratified claim from resting on a proposed one — a
status leak. It has no native code, and that is a ruling (DL-0057 §1), not an
oversight. Its two halves landed in different places and **earn different
grades**; the weakest-grade rollup (DL-0006) makes the absorption claim as a
whole `attested`.

**The status-leak half is `checked`.** `Derivation::support_of`
(`crates/dli-core/src/derivation.rs:142`) computes a derived atom's support as
its rule's support conjoined with the supports of all its premises. A conclusion
therefore cannot be more certain than what it rests on, because its support
polynomial *is* the conjunction — there is no code path that assembles a certain
conclusion over a possible premise. `docs/spec/02-semantics.md:78` states it as
the theorem it is: "the rule that anything resting on a proposed entity is itself
merely possible (the legacy validator's V9) becomes a theorem". This is the
strongest grade in the ladder and it is the right one: the violation is
unrepresentable, not detected.

**The row-discipline half is `tested`.** The data-layer version of V9 — a stored
row whose recorded `status:` disagrees with the standing its own `decisions:`
support, which is how a status leak appears in a corpus rather than in a
derivation — is `crates/dli-world/tests/mig1_replay.rs`. Its pristine census
finds zero disagreements across the vendored corpus, and because a harness that
only ever reports agreement is a null oracle, three seeded legs prove it can
still see one: `detects_flip_proposed_to_ratified`,
`detects_forged_promotion_strip_decisions`, and `detects_break_status_marker`.

**Legacy equivalence is `attested`, and the caveat is the point.** The claim
"the native successors cover what V9 covered" rests on `02-semantics.md`'s
characterisation of what V9 was. The Python V9 input file was deleted by the
consuming project's D-0245 source flip, so **this is a characterisation and not a
replay**: there is no pinned legacy V9 to run and diff against. Saying so plainly
is what separates an absorption ruling from absorption by oral tradition. A
future reader who wants the replay would have to recover the deleted source from
the consumer's history first, and their boundary guard forbids returning it to
the tree.

The detection corpus has no V9 operator and asserts that it has none.

## 3. The finding-shape compatibility promise

This section is the stable surface. The consuming project's shadow-validate
harness compares against it, so a change here is a change to a published contract
and needs its own ledger entry.

### The object

```json
{
  "path": "characters/maren-holt.md",
  "line": 9,
  "code": "V11",
  "message": "section 'Standing' cites D-0002, but D-0002's affects does not name this section",
  "severity": "error"
}
```

- **`path`** — relative to the canon root, POSIX separators, no leading `./`.
  Backslashes are normalised, so a Windows run reports the same string.
- **`line`** — one-based. See the deviations below before comparing it.
- **`code`** — one of `V1`, `V2`, `V3`, `V4`, `V5`, `V6`, `V7`, `V8`, `V10`,
  `V11`, `V12`. Never `V9`. The set is closed by DL-0057, and the detection
  suite reads it out of the validator's own source so a new code cannot appear
  without an operator.
- **`message`** — human-readable. **Not** part of the promise: wording may
  change. Match on `code`, not on prose.
- **`severity`** — always the string `"error"`. There is no warning tier today.
  It is a field rather than a constant so that adding one later is additive, but
  a consumer may currently treat every finding as blocking.

`deloreis validate-repo` prints the whole list as one JSON array on stdout and
exits `0` for an empty list, `1` for a non-empty one, `2` for a usage error.

### Ordering

Findings are sorted by `(path, line, code, message)`, lexicographically on the
strings and numerically on the line. The order is total and stable: two runs over
identical bytes produce byte-identical output, on any platform. A consumer may
diff the arrays directly.

### Failure behaviour

The library never panics on input:

- **A missing or unreadable canon root** yields an **empty finding list**, not a
  finding and not an error. The directory walk simply finds nothing and every
  check skips its absent inputs. It is the CLI, not the library, that treats a
  missing root as a usage error (exit `2`) — so a consumer calling the library
  must check for the root itself, and a consumer shelling out to the CLI gets the
  error for free. This asymmetry is deliberate and is the single most likely
  thing for a shadow harness to get wrong.
- **A file that exists and cannot be read** becomes a `V1` finding carrying the
  I/O error in its message; the rest of the repository is still validated.
- **A structured data file that will not parse** becomes a `V6` finding; the
  other data file is still checked, and the Markdown checks are unaffected.

### One known line-numbering deviation

It is recorded as it behaves. It is not fixed, because the consuming project is
about to pin this output and a silent renumbering inside a parity-support change
would be exactly the kind of drift the promise exists to prevent. Changing it
needs its own entry.

- **V6 and V7 findings always carry `line: 1`.** The structural checks read a
  parsed YAML value that keeps no position information, so a finding about the
  ninth event in `timeline.yaml` reports line 1, and the registry↔file join (V7)
  is a whole-file presence check. The `message` names the offending id; the line
  does not. This is a file-level convention, not a bug — DL-0060 keeps it accepted
  and pins it — where the V8 body-relative numbering below *was* a bug. One honest
  caveat: the **invalid-YAML** V6 branch parses through `serde_yaml_ng`, whose
  error *does* carry a location, and that line is currently discarded and reported
  as 1 like the rest. Surfacing it is a tracked DL-0060 follow-up, not a claim that
  V6 never has a line; the convention is kept for now and pinned by a unit test
  over that branch specifically.

**V8 now reports file-absolute lines (DL-0060), and the fix removes spurious
scans rather than merely renumbering.** V8 once numbered lines within the document
body — a *byte* slice that began at the *closing* `---` of the frontmatter — so
for a document whose frontmatter occupies `F` lines including both delimiters, a
hit on file line `N` was reported as line `N − F + 1`. In the detection corpus a
hit on file line 15 of an eight-line-frontmatter document reported as line 8, and
a hit on file line 13 of a seven-line-frontmatter document reported as line 7.
That violated the `line` field's "file/absolute line" contract; it was held
recorded-not-fixed under DL-0057 only so the correction would land in its own
ruled entry ahead of the consumer pinning this output, and DL-0060 is that entry.
The scan now skips the frontmatter by *line count*, which corrects the line and
also retires two false findings the byte arithmetic produced: on a **CRLF** file
the old `len + 1` undercounted one byte per line and the slice drifted back into
the last frontmatter field, matching a banned word in the metadata; on
**unterminated** frontmatter the offset ran off into an arbitrary trailing slice.
So the output change is the corrected line plus two classes of false V8 finding
removed — no legitimate catch is dropped. Both corpus operators pin the absolute
lines (15 and 13), the CRLF and unterminated cases each have a unit-test pin, and
every check reports file-absolute lines.

## 4. The detection corpus

`crates/dli-world/tests/data/validator-detection/`, walked by
`crates/dli-world/tests/repository_detection.rs`. Its own README is the
operating manual; this is what it proves.

**A clean baseline.** `baseline/` is a complete canon repository — a ledger,
open and resolved question files, three structured data files, character, place,
concept, and in-world-text documents, an exempt `sources/` tree — that validates
to the **empty** finding list. It exercises every positive requirement of all
eleven checks at once, and it carries the negative controls no operator can: the
exempt files that need no frontmatter, the `note` register that may carry no
passage, the `proposed` section that must cite nothing, the whole-file and
section-scoped `affects` targets that must not raise V11, the `### ` subsection
override that must parse.

**Twenty-six operators, eleven codes, no honest miss.** Each operator copies the
baseline, applies one seeded change, and names every finding that must follow —
path, line, code, and a distinguishing message fragment. The assertion is set
equality, so an unexpected finding fails as loudly as a missing one. Nothing is
declared `xfail`: every emitted code has at least one real catch.

| Code | Operators |
|---|---|
| V1 | 2 — no frontmatter; unparsable frontmatter |
| V2 | 2 — invalid `type` enum; id/stem divergence |
| V3 | 1 — status marker stripped |
| V4 | 4 — malformed reference; unknown decision; closed question cited; duplicate ledger id |
| V5 | 2 — dangling wiki-link; duplicate id |
| V6 | 3 — unknown era; unparsable YAML; duplicate registry id |
| V7 | 2 — registry row with no file; file with no registry row |
| V8 | 2 — diegetic scope; `all` scope |
| V10 | 2 — open question filed as resolved; resolved question left open |
| V11 | 2 — forged promotion; affects scope stripped |
| V12 | 4 — unknown register; missing passage; duplicate work id; overlong passage |

**Why the message fragment is in the oracle.** Two different defects can land on
the same `(path, line, code)` triple — a malformed YAML append and a duplicate
registry id both read `data/characters.yaml:1 V6`. A triple-only oracle blessed
the wrong catch while this corpus was being written. The fragment is the repair,
and it is why the corpus is a stronger oracle than the unit tests in
`repository.rs`, which assert only that *some* finding matches a predicate and
therefore cannot see a check firing twice, firing against the wrong file, or
reporting the right code for the wrong reason.

**What it deliberately cannot express.** `check_frontmatter`'s `cannot read
file` and `read_yaml`'s `cannot read YAML` both need a path that exists and
cannot be read. Git cannot record that state portably; seeding it would mean a
`chmod` or a broken symlink and hence a platform-conditional operator, which is a
worse trade than a named gap. Both branches only convert an I/O error into a
finding, and the property that matters — findings, not panics — is proven by the
unparsable-YAML operator and by the missing-root behaviour in §3.

## 5. What the corpus found

Three things surfaced while it was being built. All three are recorded rather
than smoothed away; none is fixed here.

**The two marker parsers disagree.** `check_sections` parses a `> status:` line
strictly: a citation that is not `D-`/`Q-` plus four digits invalidates the whole
marker, and the section is reported as marker-less (V3). `check_citations` scans
the same line loosely — split on `status: `, take the parenthesised text, split
on `, ` — and never validates citation shape, so it does see a citation and
reports it unresolvable (V4). A section citing `D-0001-forged` draws both.
Both readings are defensible. The operator `v4-malformed-decision-reference`
pins the pair so a future unification of the two parsers has to argue for the
behaviour change instead of shipping it as a cleanup.

**The id is a three-way join key.** Changing one document's frontmatter `id`
raises V2 (the stem no longer matches), V5 at every inbound `[[wiki-link]]` (the
index now publishes a different name), and V11 (the decision's `affects` no
longer reaches the document). The operator `v2-id-stem-divergence` records all
four findings, because the cascade is the honest answer and a corpus that hid it
behind a narrower seed would misrepresent the blast radius of an id rename.

**Placement and citability are one fact seen twice.** Marking an
`open-questions.md` entry `resolved` raises V10 for the misfiling *and* V4 at
every section citing it, because the question is no longer open. The mirror
operator is quieter only because nothing in the baseline cites `Q-0002` — an
asymmetry of the corpus, not of the checks.

## 6. Ownership

Deloreis owns the validator, the corpus, the mapping table above, and the
finding-shape promise — together, the SP-4v delivery of the cutover campaign.

The consuming project owns its shadow-validate harness and its accrual, and it
owns the two DP-008 items that are sometimes mistaken for validator work: **X6**,
the direct-SQL lineage negatives, and **X8**, propagation and index parity. Both
need a database this repository does not have and must not grow. No node here
claims them. The `native-lineage-negatives` branch, opened to attempt X6 locally,
was a dead end and DL-0057 records it as one.
