#!/usr/bin/env python3
"""Render the checked Holmes export as a browsable static wiki."""
import html
import json
import pathlib
import re
import sys
from urllib.parse import quote

root = pathlib.Path(__file__).resolve().parent.parent
out = pathlib.Path(sys.argv[1]) / "wiki"
out.mkdir(parents=True, exist_ok=True)
fixture = json.loads((root / "examples/holmes/opening.json").read_text())
pages = fixture["wiki"]["emission"]["artifact"]["pages"]
sources = (root / "examples/holmes/sources.md").read_text()
source_names = {
    "d-0001": "A Study in Scarlet, part I, chapter 1",
    "d-0002": "The Sign of the Four",
    "d-0003": "The Man with the Twisted Lip",
    "d-0004": "The Final Problem",
    "d-0005": "The Empty House",
    "d-0006": "Editorial modelling choices",
    "d-0007": "A Scandal in Bohemia",
    "d-0008": "Earlier editorial ruling",
    "d-0009": "Later editorial ruling",
}
origins = {"h.study-in-scarlet": "d-0001", "h.sign-of-four": "d-0002"}
for key in (*source_names, *origins):
    if f"`{key}`" not in sources:
        raise SystemExit(f"missing source index entry: {key}")

style = "body{max-width:68rem;margin:2rem auto;padding:0 1rem;font:17px/1.55 system-ui,sans-serif;color:#262626}a{color:#174f84}code{font-size:.94em}li{margin:.45rem 0}.index{columns:3 16rem}.meta{color:#555}nav{margin-bottom:2rem}h1{line-height:1.2}@media(max-width:40rem){.index{columns:1}}"
def page(title, content):
    title = html.escape(title)
    return f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title} — Holmes wiki</title><style>{style}</style></head>
<body><nav><a href="../index.html">De Lore Is book</a> · <a href="index.html">Wiki index</a> · <a href="../holmes-sources.html">Sources</a></nav><main>{content}</main></body></html>\n'''

def html_path(path):
    if pathlib.PurePosixPath(path).name != path or not path.startswith("predicate-") or not path.endswith(".md"):
        raise ValueError(f"unexpected wiki path {path!r}")
    return path[:-3] + ".html"

def link_path(path):
    return quote(path, safe="-_.")

def source_link(decision):
    if decision not in source_names:
        raise ValueError(f"unknown source decision {decision}")
    return f'<a href="../holmes-sources.html#decision-{html.escape(decision)}">{html.escape(decision)}: {html.escape(source_names[decision])}</a>'

rows_by_subject = {}
index_entries = []
for item in pages[1:]:
    filename = html_path(item["path"])
    title = item["title"]
    index_entries.append((title, filename))
    rows = []
    for line in item["content"].splitlines():
        if line.startswith("- ") and not line.startswith("- `"):
            raise ValueError(f"unrecognized wiki row: {line!r}")
        if not line.startswith("- `"):
            continue
        match = re.fullmatch(r"- `(.+?)` — \*\*(certain|possible|dead)\*\*(.*)", line)
        if not match:
            raise ValueError(f"unrecognized wiki row: {line!r}")
        claim, status, metadata = match.groups()
        claim = html.unescape(claim)
        detail = [f'<code>{html.escape(claim)}</code> — <strong>{status}</strong>']
        provenance = re.search(r"\(provenance: ([^)]+)\)", metadata)
        if provenance:
            detail.append(f'provenance: {html.escape(provenance.group(1))}')
        decisions = re.search(r"\(decisions: ([^)]+)\)", metadata)
        if decisions:
            detail.append("sources: " + ", ".join(source_link(d.strip()) for d in decisions.group(1).split(",")))
        support = re.search(r"\(support: (.+)\)(?: \(decisions:|$)", metadata)
        if support:
            expression = support.group(1)
            detail.append(f'support: <code>{html.escape(expression)}</code>')
            named_support = set(re.findall(r"h\.[A-Za-z0-9_.-]+", expression))
            for origin, decision in origins.items():
                if origin in named_support:
                    detail.append("account: " + source_link(decision))
        row = "<li>" + " <span class=\"meta\">·</span> ".join(detail) + "</li>"
        rows.append(row)
        subject = re.match(r"[^()]+\(([^,()]+)", claim)
        if subject:
            rows_by_subject.setdefault(subject.group(1).strip(), []).append(row)
    body = f"<h1>{html.escape(title.replace('-', ' '))}</h1><ul>" + "\n".join(rows) + "</ul>"
    (out / filename).write_text(page(title, body), encoding="utf-8")

entities = {}
pattern = re.compile(r'^(?:person|place|thing|era)\s+(\S+)\s+"([^"]+)"')
for line in (root / "examples/holmes/holmes.dli").read_text().splitlines():
    match = pattern.match(line)
    if match:
        entities[match.group(1)] = match.group(2)
subject_entries = []
for identifier, label in sorted(entities.items(), key=lambda item: item[1].casefold()):
    if identifier not in rows_by_subject:
        continue
    filename = "subject-" + quote(identifier, safe="-_") + ".html"
    subject_entries.append((label, filename))
    body = f"<h1>{html.escape(label)}</h1><p>Recorded claims about {html.escape(label)}. Possible claims remain open; dead support is retained for inspection.</p><ul>" + "\n".join(rows_by_subject[identifier]) + "</ul>"
    (out / filename).write_text(page(label, body), encoding="utf-8")

def index_list(entries):
    return '<ul class="index">' + "\n".join(f'<li><a href="{link_path(path)}">{html.escape(label)}</a></li>' for label, path in entries) + "</ul>"

body = '''<h1>The Holmes wiki</h1><p>Browse the claims in the casebook model. Each row shows its evaluated status and, where recorded, its provenance and source. The <a href="../holmes-sources.html">source notes</a> explain the decisions and modelling choices.</p><h2>People, places, and things</h2>''' + index_list(subject_entries) + "<h2>Relationships</h2>" + index_list(index_entries)
(out / "index.html").write_text(page("The Holmes wiki", body), encoding="utf-8")
print(f"wiki: wrote {len(index_entries)} relationship pages and {len(subject_entries)} subject pages")
