#!/usr/bin/env python3
"""Check local links in the built book without network access."""
import pathlib
import sys
from html.parser import HTMLParser
from urllib.parse import unquote, urlsplit

root = pathlib.Path(sys.argv[1]).resolve()
class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.targets = []
        self.ids = set()
    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if attrs.get("id"):
            self.ids.add(attrs["id"])
        if tag == "a" and attrs.get("name"):
            self.ids.add(attrs["name"])
        if tag in {"a", "link"} and attrs.get("href"):
            self.targets.append(attrs["href"])
        if tag in {"img", "script", "source", "audio", "video"} and attrs.get("src"):
            self.targets.append(attrs["src"])

files = sorted(root.rglob("*.html"))
parsed = {}
for path in files:
    parser = Links()
    parser.feed(path.read_text(encoding="utf-8"))
    parsed[path] = parser
errors = []
for path, parser in parsed.items():
    for href in parser.targets:
        link = urlsplit(href)
        if link.scheme or link.netloc or href.startswith("//"):
            continue
        target = (path.parent / unquote(link.path)).resolve() if link.path else path
        if target == root or root not in target.parents:
            errors.append((path, href, "outside site"))
            continue
        if target.is_dir():
            target /= "index.html"
        if not target.is_file():
            errors.append((path, href, "missing file"))
        elif link.fragment and target in parsed and unquote(link.fragment) not in parsed[target].ids:
            errors.append((path, href, "missing anchor"))
if errors:
    for path, href, message in errors:
        print(f"{path.relative_to(root)}: {href}: {message}", file=sys.stderr)
    raise SystemExit(f"book links: {len(errors)} error(s)")
print(f"book links: checked {len(files)} HTML pages")
