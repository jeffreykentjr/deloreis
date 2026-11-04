---
status: reference
verdict: "Integration notes for the separate mesite agent."
---

# Notes for the mesite agent

Use mesite's current content model and its own review process. The repository
currently registers projects in `src/data/software.ts` and uses
`src/content/guides/<project>.md` for an overview, with answer pages under
`src/content/guides/<project>/`. A potential entry needs the project name,
repository URL, install or quickstart URL, agent skill URL, and Rust as the
implementation language. Set `sourceRevision` in the guide frontmatter to the
full commit recorded in this pack's manifest.

The manifest checks excerpt bytes against the pinned commit and rebuilds each
image source page from that commit. PNG hashes check file integrity; the visual
match between each screenshot and its page was reviewed separately.

The excerpts retain their source documents’ relative links; resolve those
against the pinned source paths rather than copying them unchanged into mesite.
The pack supplies evidence and outlines, not final site copy. Confirm release
and Pages URLs at publication time. Keep the site static, with no new client
JavaScript, iframes, or SVG. Mesite's own contributor guide governs
its editorial checks, visual review, and publication.

A draft registration, for the mesite agent to check against the release:

```ts
deloreis: {
  name: 'De Lore Is',
  repository: 'https://github.com/jeffreykentjr/deloreis',
  install: 'https://github.com/jeffreykentjr/deloreis/blob/main/docs/quickstart.md',
  skill: 'https://raw.githubusercontent.com/jeffreykentjr/deloreis/main/SKILL.md',
  programmingLanguage: 'Rust',
},
```
