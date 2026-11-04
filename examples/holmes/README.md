# The Holmes example

The [casebook](../../docs/casebook/01-watson-name.md) starts with Watson's
name and follows the model through disputed accounts, time, a retcon, a branch,
and a rule-derived conflict. [Sources](sources.md) maps the decisions to the
stories and explains where the model makes an interpretation.

`holmes.dli` is the consistent starting corpus. `build.py` recreates it through
the MCP authoring tools, then runs the staged conflicts and the later retcon in
a temporary project. Its JSON output includes the results and the all-standing
wiki after the retcon. The generated corpus represents the earlier starting
point; the later decisions and branch live in the replay, not in that text
snapshot.

The starting corpus checks cleanly. The replay stages the conflicts separately
so they cannot be committed by accident:

| Question | Replay result |
|---|---|
| John or James? | `continuity.fd-violation` after staging both as first names. |
| Shoulder or leg? | Both claims are `possible`; the texts do not establish that they describe one injury, so there is no automatic conflict finding. |
| Mary and the 1888 visit | `continuity.chronology-cycle` when the September 1888 interpretation and the marriage sequence are both staged; the baseline keeps the events unordered. |
| Reichenbach and the return | The newer ruling defeats the earlier one; the old support remains inspectable. This is a revision, not a checker error. |
| Holmes stays dead | A branch preserves the earlier ruling; this is an alternative, not a checker error. |
| A rule infers James | `continuity.derived-fd-violation` names the sources and the modelling rule. |

Run the replay check from the repository root:

```sh
cargo test -p deloreis --test holmes
```

To inspect the starting corpus:

```sh
deloreis check examples/holmes/holmes.dli
deloreis timeline show --corpus examples/holmes/holmes.dli --markdown
```

The test compares the captured tool results, formatted corpus, and generated
casebook chapters byte for byte. To update them after an intentional change,
run `DLI_BLESS_HOLMES=1 cargo test -p deloreis --test holmes`, then review the
diff. `notes.md` is a short
sample of the source notes; the script is a fixed transcription, not an
automatic prose importer.
