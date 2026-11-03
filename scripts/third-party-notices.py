#!/usr/bin/env python3
"""Write license notices for the dependencies in a release binary.

Adapted from Chelis-Lang/sail; see scripts/SAIL-LICENSE.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice|copyright|unlicense|authors)([-._].*)?$", re.I)
MIT_TEXT = """Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""


def dependency_closure(target):
    data = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target],
        cwd=ROOT,
    ))
    packages = {package["id"]: package for package in data["packages"]}
    nodes = {node["id"]: node for node in data["resolve"]["nodes"]}
    roots = [key for key, package in packages.items() if package["name"] == "deloreis" and package["source"] is None]
    if len(roots) != 1:
        raise ValueError("expected one deloreis package")
    seen, pending = set(), roots
    while pending:
        key = pending.pop()
        if key in seen:
            continue
        seen.add(key)
        for dependency in nodes[key]["deps"]:
            if any(kind["kind"] is None for kind in dependency["dep_kinds"]):
                pending.append(dependency["pkg"])
    return sorted((packages[key] for key in seen if packages[key]["source"] is not None),
                  key=lambda package: (package["name"], package["version"]))


def license_files(directory):
    result = []
    for path in sorted(directory.iterdir()):
        if path.is_file() and LICENSE_FILE.match(path.name):
            result.append(path)
        elif path.is_dir() and path.name.lower() in ("license", "licenses"):
            result.extend(sorted(item for item in path.rglob("*") if item.is_file()))
    return result


def render(target, packages):
    rows, texts = [], {}
    for package in packages:
        directory = Path(package["manifest_path"]).parent
        expression = package.get("license") or package.get("license_file") or "unspecified"
        files = [(str(path.relative_to(directory)), path.read_text(errors="replace"))
                 for path in license_files(directory)]
        if not files:
            if "MIT" not in expression:
                raise ValueError(f"no license file for {package['name']} {package['version']}")
            authors = ", ".join(package.get("authors") or [f"the {package['name']} authors"])
            files = [(f"MIT license, copyright {authors} (no license file in package)", MIT_TEXT)]
        refs = []
        for name, text in files:
            digest = hashlib.sha256(text.encode()).hexdigest()
            if digest not in texts:
                texts[digest] = (len(texts) + 1, text)
            refs.append(f"{name} [text {texts[digest][0]}]")
        rows.append((package, expression, refs))
    lines = [f"Third-party notices for De Lore Is ({target})", "=" * 50, "",
             "De Lore Is is MIT-licensed; see LICENSE. Its binary includes the",
             "crates listed below. License files are reproduced as supplied by",
             "their packages. Rust standard library notices are provided separately.", ""]
    for package, expression, refs in rows:
        repository = package.get("repository") or ""
        lines += [f"* {package['name']} {package['version']}: {expression} {repository}",
                  "  " + ", ".join(refs)]
    lines += ["", "License texts", "-------------", ""]
    for number, text in texts.values():
        lines += [f"[text {number}]", "", text.rstrip(), "", "-" * 72, ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.write_text(render(args.target, dependency_closure(args.target)))


if __name__ == "__main__":
    main()
