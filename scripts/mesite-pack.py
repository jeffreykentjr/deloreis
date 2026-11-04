#!/usr/bin/env python3
"""Prepare or verify the source-pinned mesite resource pack."""
import argparse
from contextlib import contextmanager
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tempfile

root = pathlib.Path(__file__).resolve().parent.parent
pack = root / "docs/release/mesite-pack"
selections = {
    "01-watson-name.md": "docs/casebook/01-watson-name.md",
    "02-wounds.md": "docs/casebook/02-wounds.md",
    "03-timeline.md": "docs/casebook/03-timeline.md",
    "04-return.md": "docs/casebook/04-return.md",
    "06-rules.md": "docs/casebook/06-rules.md",
}
images = {
    "images/holmes-retcon.png": "wiki/predicate-reichenbach-outcome.html",
    "images/watson-wounds.png": "wiki/predicate-reported-wound-location.html",
}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def source(revision, path):
    return subprocess.run(["git", "show", f"{revision}:{path}"], cwd=root, check=True, capture_output=True).stdout

def excerpt(data):
    marker = b"\n<details>\n"
    if marker not in data:
        raise ValueError("source chapter has no calls appendix")
    return data.split(marker, 1)[0].rstrip() + b"\n"

@contextmanager
def source_wiki(revision):
    """Render wiki pages using only files read from the pinned commit."""
    paths = ["scripts/build-holmes-wiki.py", "examples/holmes/opening.json", "examples/holmes/sources.md", "examples/holmes/holmes.dli"]
    with tempfile.TemporaryDirectory(prefix="dli-pinned-wiki-") as temp:
        checkout = pathlib.Path(temp) / "source"
        for path in paths:
            dest = checkout / path
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(source(revision, path))
        site = pathlib.Path(temp) / "site"
        subprocess.run([sys.executable, str(checkout / paths[0]), str(site)], check=True, capture_output=True)
        yield site

def prepare(revision, site):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("source revision must be a full lowercase Git commit ID")
    subprocess.run(["git", "merge-base", "--is-ancestor", revision, "origin/main"], cwd=root, check=True)
    subprocess.run(["git", "merge-base", "--is-ancestor", revision, "HEAD"], cwd=root, check=True)
    manifest = {"sourceRevision": revision, "excerpts": [], "images": []}
    pending = []
    for name, path in selections.items():
        data = source(revision, path)
        extracted = excerpt(data)
        pending.append((name, extracted))
        manifest["excerpts"].append({"file": f"excerpts/{name}", "source": path, "sourceSha256": sha(data), "sha256": sha(extracted), "selection": "chapter before the calls appendix"})
    with source_wiki(revision) as pinned_site:
        for name, page in images.items():
            image = (pack / name).read_bytes()
            if not image.startswith(b"\x89PNG\r\n\x1a\n"):
                raise ValueError(f"{name} is not a PNG")
            page_bytes = (pinned_site / page).read_bytes()
            if (site / page).read_bytes() != page_bytes:
                raise ValueError(f"rendered page does not match pinned source: {page}")
            manifest["images"].append({"file": name, "sha256": sha(image), "sourcePage": page, "sourcePageSha256": sha(page_bytes), "capture": "Firefox screenshot, visually checked"})
    (pack / "excerpts").mkdir(parents=True, exist_ok=True)
    for name, data in pending:
        (pack / "excerpts" / name).write_bytes(data)
    (pack / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    verify(site)

def verify(site):
    manifest = json.loads((pack / "manifest.json").read_text())
    revision = manifest["sourceRevision"]
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("invalid source revision")
    subprocess.run(["git", "merge-base", "--is-ancestor", revision, "HEAD"], cwd=root, check=True)
    subprocess.run(["git", "merge-base", "--is-ancestor", revision, "origin/main"], cwd=root, check=True)
    if len(manifest["excerpts"]) != len(selections) or {item["source"] for item in manifest["excerpts"]} != set(selections.values()):
        raise ValueError("unexpected source selection")
    if len(manifest["images"]) != len(images) or {item["file"] for item in manifest["images"]} != set(images):
        raise ValueError("unexpected image selection")
    for item in manifest["excerpts"]:
        expected_name = next(name for name, path in selections.items() if path == item["source"])
        if item["file"] != f"excerpts/{expected_name}":
            raise ValueError("excerpt path does not match its source")
        full = source(revision, item["source"])
        data = (pack / item["file"]).read_bytes()
        if sha(full) != item["sourceSha256"] or data != excerpt(full) or sha(data) != item["sha256"]:
            raise ValueError(f"excerpt mismatch: {item['file']}")
    with source_wiki(revision) as pinned_site:
        for item in manifest["images"]:
            data = (pack / item["file"]).read_bytes()
            if not data.startswith(b"\x89PNG\r\n\x1a\n") or sha(data) != item["sha256"] or item["sourcePage"] != images[item["file"]]:
                raise ValueError(f"image mismatch: {item['file']}")
            pinned = (pinned_site / item["sourcePage"]).read_bytes()
            if sha(pinned) != item["sourcePageSha256"]:
                raise ValueError(f"pinned image source page changed: {item['sourcePage']}")
            if site is not None and (site / item["sourcePage"]).read_bytes() != pinned:
                raise ValueError(f"image source page changed: {item['sourcePage']}")
    print(f"mesite pack: verified {len(manifest['excerpts'])} excerpts and {len(manifest['images'])} PNGs at {revision}")

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("mode", choices=["prepare", "check"])
parser.add_argument("--revision")
parser.add_argument("--site-dir", type=pathlib.Path)
args = parser.parse_args()
if args.mode == "prepare":
    if not args.revision or args.site_dir is None:
        parser.error("prepare needs --revision and --site-dir")
    prepare(args.revision, args.site_dir)
else:
    verify(args.site_dir)
