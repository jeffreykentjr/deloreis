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
        content = content.replace("../examples/hollow/transcript.md", "hollow.md")
        content = content.replace("../SKILL.md", "skill.md")
        content = content.replace("docs/quickstart.md", "quickstart.md")
        content = content.replace("docs/concepts.md", "concepts.md")
        content = content.replace("examples/hollow/transcript.md", "hollow.md")
        chapter["content"] = content
        visit(chapter.get("sub_items", []))

visit(book.get("sections", book.get("items", [])))
json.dump(book, sys.stdout)
