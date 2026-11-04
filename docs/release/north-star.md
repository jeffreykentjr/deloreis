---
status: live
verdict: "The release direction: help people understand De Lore Is and use it on their own fiction before publishing."
---

# What this release is for

De Lore Is helps writers and the tools they use keep track of fictional worlds.
It checks structured lore against the rules and rulings a writer gives it, shows
why an answer follows, and keeps earlier claims available when a story changes.
The checks run locally and call no model.

The release is on hold until a newcomer can understand the tool and use it on a
world of their own. The decision ledger records the direction in DL-0122; the
work is tracked by the understanding epic, #235.

## Who it helps

Writers and worldbuilders who maintain a series bible, teams working in a shared
setting, and people building tools that write or work with fiction. A person can
write and check `.dli` files; an assistant can use the authoring tools through a
local MCP server. The tool checks what has been recorded: it does not read a
manuscript and discover plot holes on its own.

## The casebook

The flagship example is the Sherlock Holmes canon. Build an original model from
the stories themselves, with short citations to the relevant texts and no copied
fan-wiki material. Check each claimed discrepancy before using it. If the text
does not establish a claimed contradiction, explain the uncertainty or leave
that claim out.

The casebook should show, in prose and with reproducible output, how to keep
rival accounts open, check a single-valued fact, order a disputed chronology,
revise a ruling without erasing it, and explore a what-if branch. A rule-derived
conflict should show why the tool helps with more than an obvious pair of facts.
Its evidence should make the difference between the text and the modeller's
choices clear.

A newcomer should be able to read the opening chapter, understand why DLI is
useful, browse a wiki of the model, and then use the guides to model their own
world. Explain terms before relying on them; the glossary is a reference, not a
prerequisite for every sentence. Show actual results next to the narrative and
keep machine-readable calls available without making them the main story.

## Where it lives

DLI's GitHub Pages site will carry the book, casebook, guides, and browsable
static wiki. Build and check the site during this work; publish it when the
release is cut. The release remains on hold until the understanding epic is
complete. Keep the existing Linux x86-64 and approved-RC-only macOS ARM release
arrangements.

Another agent owns the mesite site and post. This repository supplies a resource
pack with a brief, source-pinned excerpts and hashes, PNGs, and a blog outline;
it does not write or publish the post.

## Material to prepare

| Material | Purpose |
|---|---|
| README and quickstart | Explain the tool and get a new user to a first check. |
| Concepts and reference | Explain the words and operations used in the guides. |
| Holmes world and casebook | Show sourced, reproducible examples at a useful scale. |
| Modelling and your-world guides | Help readers turn their own notes into checked lore. |
| Static wiki and timeline | Let readers browse claims, standing, sources, and story order. |
| GitHub Pages book | Put the user path first and technical internals afterward. |
| Mesite resource pack | Give the separate site agent verified material to use later. |

## Later post outline

Start with a real issue in the Holmes texts, show what the cited accounts say,
and then show the result of modelling them. Follow a sourced retcon and a what-if
branch. Explain what DLI checks and what a writer must decide, then point readers
to the casebook and modelling guide. Do not write the post as part of this work.
