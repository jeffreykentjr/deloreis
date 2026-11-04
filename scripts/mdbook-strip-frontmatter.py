#!/usr/bin/env python3
"""Strip repository status frontmatter after mdBook expands includes."""
import json
import sys

if len(sys.argv) > 1 and sys.argv[1] == "supports":
    sys.exit(0)

_context, book = json.load(sys.stdin)

def visit(items):
    for item in items:
        chapter = item.get("Chapter")
        if chapter is None:
            continue
        content = chapter["content"]
        if content.startswith("---\n"):
            end = content.find("\n---\n", 4)
            if end >= 0:
                chapter["content"] = content[end + 5:].lstrip("\n")
        content = chapter["content"]
        links = {
            "docs/quickstart.md": "quickstart.md",
            "docs/concepts.md": "concepts.md",
            "docs/guide/modelling.md": "guide-modelling.md",
            "docs/guide/your-world.md": "guide-your-world.md",
            "docs/casebook/01-watson-name.md": "casebook-01.md",
            "casebook/01-watson-name.md": "casebook-01.md",
            "../casebook/01-watson-name.md": "casebook-01.md",
            "../casebook/03-timeline.md": "casebook-03.md",
            "../../examples/holmes/sources.md": "holmes-sources.md",
            "../../examples/holmes/notes.md": "holmes-notes.md",
            "../../examples/holmes/build.py": "https://github.com/jeffreykentjr/deloreis/blob/main/examples/holmes/build.py",
            "../cards/syntax-reference.md": "syntax-reference.md",
            "../quickstart.md": "quickstart.md",
            "../concepts.md": "concepts.md",
            "../SKILL.md": "skill.md",
            "SKILL.md": "skill.md",
            "modelling.md": "guide-modelling.md",
            "your-world.md": "guide-your-world.md",
            "sources.md": "holmes-sources.md",
            "AGENTS.md": "https://github.com/jeffreykentjr/deloreis/blob/main/AGENTS.md",
            "release/HANDOVER.md": "https://github.com/jeffreykentjr/deloreis/blob/main/docs/release/HANDOVER.md",
            "../charter.md": "charter.md",
            "01-syntax.md": "spec-01.md",
            "02-semantics.md": "spec-02.md",
            "03-time.md": "spec-03.md",
            "04-practices.md": "spec-04.md",
            "05-language-layer.md": "spec-05.md",
            "07-verdicts-and-llm-boundary.md": "spec-07.md",
            "08-reasoning.md": "spec-08.md",
            "09-exports.md": "spec-09.md",
            "10-serving.md": "spec-10.md",
            "11-agent-authoring.md": "spec-11.md",
            "../cards/gentle-card.md": "https://github.com/jeffreykentjr/deloreis/blob/main/docs/cards/gentle-card.md",
            "../cards/probe-protocol.md": "https://github.com/jeffreykentjr/deloreis/blob/main/docs/cards/probe-protocol.md",
            "../design-notes/scale-and-distribution.md": "https://github.com/jeffreykentjr/deloreis/blob/main/docs/design-notes/scale-and-distribution.md",
            "../reference-generated/error-codes.md": "reference-error-codes.md",
            "../../examples/camelot.dli": "https://github.com/jeffreykentjr/deloreis/blob/main/examples/camelot.dli",
        }
        for source, target in links.items():
            content = content.replace("(" + source + ")", "(" + target + ")")
        if chapter.get("path") == "holmes-sources.md":
            import re
            content = re.sub(r"\| `(d-[0-9]+)` \|", r'| <a id="decision-\1"></a>`\1` |', content)
        chapter["content"] = content
        visit(chapter.get("sub_items", []))

visit(book.get("sections", book.get("items", [])))
json.dump(book, sys.stdout)
