---
status: live
verdict: "The consumer skill is maintained here; registry exports derive from its merged package."
---

# Agent skill distribution

The canonical package is `skills/deloreis/`: `SKILL.md`,
`references/first-session.md`, and the MIT license. The root `SKILL.md` is a
pointer; the book includes the canonical instructions. The personal website
links upstream. Contributor rules remain separate in `AGENTS.md`.

```sh
npx skills add jeffreykentjr/deloreis --skill deloreis
```

The skill does not install the runtime. Its package version and registry wrapper
versions are separate from the runtime's 0.1.0 release. Export the merged folder,
including its reference and license, rather than editing a hosted copy. Record
its source commit and verify installed bytes after publication.

## Validation

On 5 November 2026 the local installer copied all three canonical files into a
clean Codex project. Their bytes match the source package. The reference's JSON
calls ran through the real MCP server in a fresh disposable project: an accepted
home queried as certain; why returned support; a conflicting home was refused
at materialization; discard and rendered export worked. The Holmes notes,
rule-derived conflict, retcon and branch replays also passed.

The existing full gate passed all 23 steps with output confined to this
worktree's target directory. After prose revisions, docs-oracle and teaching
coverage passed. The book built and its 267 HTML pages passed link checks.
Existing scans now cover the canonical entrypoint and packaged reference.

Fresh Bedrock devrel and technical readers checked the package. Accepted
findings improved setup, direct-mutation guidance, and the first-session path.
A truncated review was excluded. Raw receipts, test transcripts, installer
output, and individual dispositions stay in ignored local directories. These
are bounded package/task checks, not adoption or agent success-rate claims.

## Distribution audit

The 19-channel landscape used for Sail was rechecked on 5 November 2026. The
source is the package above; a submission is not a listing, and a listing is not
successful use. Initial submissions follow the canonical-package merge. The
results section will record returned URLs and blockers separately.

| Channel | Route / current constraint |
| --- | --- |
| ClawHub | Versioned package upload; existing Jeffrey Kent publisher account. https://docs.openclaw.ai/clawhub |
| Tessl | Public plugin in existing `jeffreykent` workspace. https://docs.tessl.io/ |
| Agent Skill Hub | Public GitHub import and source refresh. https://doc.agentskillhub.dev/guide/publishing |
| skills.sh | Public GitHub install; catalog discovery follows real aggregate usage. https://skills.sh/docs/faq |
| Skillstore | Anonymous source-folder submission and review. https://skillstore.io/submit |
| Skill.Fish / MCP Market | One repository-submission channel; not two independent submissions. https://github.com/knoxgraeme/skillfish |
| Agent Skills CLI | Public GitHub discovery and search. https://www.agentskills.in/ |
| Skills Directory | Submission redirects to sign-in. https://app.skillsdirectory.com/submit |
| SkillRouter | CLI submission with existing owner pairing; queued is not published. https://skillrouter.org/us/submit |
| SkillsLLM | Published eligibility rules include a star threshold; check eligibility before submission. https://skillsllm.com/llms.txt |
| SkillsMP | Source request challenged with HTTP 403; author-submission path unverified. https://skillsmp.com/about |
| Smithery | Current service concerns MCP servers; no verified skill-only route. https://www.smithery.ai/ |
| Playbooks | Live page; current skill-submission route unverified. https://playbooks.com/ |
| SkillHub.space | CLI documentation reachable; usable publishing flow still needs verification. https://docs.skillhub.space/docs/cli |
| SkillHub.club | Upload flow requires an account. https://www.skillhub.club/app/skills |
| skills.rest | Public GitHub folder indexing form. https://skills.rest/contribute |
| FindSkills | Public index; submission route unverified. https://findskills.org/ |
| Tencent SkillHub | Publication requires account/identity steps. https://skillhub.cn/dashboard/publish |
| LobeHub | Public GitHub import API, without a new GitHub OAuth grant. https://lobehub.com/skills |

Use existing authorized accounts. Do not create accounts, pay for promotion,
request expanded permissions, or manufacture installs, stars, reviews, or traffic.
The distribution record describes observed outcomes, not promised ranking.

## Results — 5 November 2026

Canonical source: `40d230a79c0f4063f57ada0a2d10c41754948a3a`, merged in
https://github.com/jeffreykentjr/deloreis/pull/251. The package has three files.
Its SKILL.md SHA-256 is
`08f732380e1cc9523b3992ef62583a2f34a315ff299e592d203e35c15e62223f`.
Registry versions are independent of runtime versions.

Four hosted bundles are public and verified against the canonical bytes. A
fifth catalog returns the canonical source in search. Four more channels
accepted submissions but have not been verified as public listings.

| Channel | Observed result |
| --- | --- |
| ClawHub | Public version `1.0.0`; ordinary owner-scoped CLI install passed, without a scan bypass. All three files match source. https://clawhub.ai/jeffreykentjr/skills/deloreis |
| Tessl | Public wrapper `1.0.0`; lint, publication dry run and clean strict Codex install passed. Instructions and reference match source; `LICENSE.md` matches the canonical MIT text. https://tessl.io/registry/jeffreykent/deloreis |
| Agent Skill Hub | Public `2026.11.05`, pinned to source `40d230a`. All manifest Git blob hashes and installed files match source. https://agentskillhub.dev/u/jeffreykentjr/sk/deloreis |
| LobeHub | Public canonical package `1.0.1`; anonymous ZIP download contains exactly the three canonical files, byte-identical. https://lobehub.com/skills/jeffreykentjr-deloreis-deloreis |
| Agent Skills CLI | `agent-skills-cli` search returns the canonical GitHub folder and `skills/deloreis/SKILL.md`. Search also returns two documentation stubs; those are not additional usable skills. No bundle-install claim. |
| Skillstore | Public form accepted the canonical folder and created a pending audit submission, ID beginning `d5123048`. No published listing or review PR verified yet. |
| Skill.Fish / MCP Market | CLI `1.0.39` accepted the repository into its queue. Its acknowledgement selected the root path and also found the nested skill; canonical-package publication remains unverified. |
| SkillRouter | Existing authenticated CLI accepted `jeffreykentjr/deloreis`; response reports `state: queued` and `submitted_by_you: true`. No claim file or new account created. |
| skills.rest | Public form confirmed successful submission of the canonical folder and said indexing occurs within 24 hours. No listing/status URL returned. |
| skills.sh | The package installs through the public GitHub route. Owner-filtered search still returns no result; there is no verified manual-submission route. Test installs use `DO_NOT_TRACK=1`. |

The LobeHub repository import initially queued both the root pointer and the
canonical package. Only the nested canonical identifier above returned a public
skill and verified download. Do not promote or count the root pointer as a
separate package. Agent Skills CLI similarly indexes the root pointer and an
mdBook include; use the full canonical folder URL when precision matters.

The remaining nine services in the audit table remain account-gated, ineligible,
challenged, or without a verified publishing route. SkillsLLM's published
100-star minimum excludes this repository (zero stars when checked). Skills
Directory redirected to sign-in. SkillHub.club and Tencent require account or
identity steps. No new accounts, identity submissions, OAuth grants, fees, fake
installs, ratings, or stars were used.

## Updating the hosted copies

Edit and test `skills/deloreis` here first. Export its merged files for the
hosted registries, retaining its license, and record the new source commit.

- ClawHub: publish a new package version with source repo, commit, ref and path
  metadata. Wait for publication and test an ordinary owner-scoped install.
- Tessl: export outside a Git-ignored directory. Add the canonical license text
  as `LICENSE.md`, which Tessl retains, and repository/homepage/author metadata
  in the generated wrapper. Lint, dry-run and publish in workspace
  `jeffreykent`; use a strict install to verify the actual hosted version.
- Agent Skill Hub: use its documented source-refresh API for
  `jeffreykentjr/deloreis`; inspect the returned source commit and file manifest.
- LobeHub: use the public GitHub import API without a new GitHub OAuth grant.
  Check the canonical identifier and downloaded bytes; a queue acknowledgement
  alone does not establish an updated bundle.
- For indexing queues and forms, inspect the current listing before submitting
  again. Keep pending acknowledgements distinct from publication.

Keep raw receipts and validation downloads out of the repository. These checks
establish distribution and package integrity, not adoption or search ranking.
