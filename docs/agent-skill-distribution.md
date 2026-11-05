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
