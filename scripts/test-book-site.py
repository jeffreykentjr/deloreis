#!/usr/bin/env python3
"""Check the rendered wiki's sources and the offline link checker's failures."""
import pathlib
import subprocess
import sys
import tempfile

root = pathlib.Path(__file__).resolve().parent.parent
site = pathlib.Path(sys.argv[1])
watson = (site / "wiki/subject-watson.html").read_text(encoding="utf-8")
for expected in ("h.study-in-scarlet", "h.sign-of-four", "decision-d-0001", "decision-d-0002"):
    if expected not in watson:
        raise SystemExit(f"Watson's wiki page is missing {expected}")
outcome = (site / "wiki/predicate-reichenbach-outcome.html").read_text(encoding="utf-8")
for expected in ("<strong>dead</strong>", "<strong>certain</strong>", "decision-d-0008", "decision-d-0009"):
    if expected not in outcome:
        raise SystemExit(f"the retcon wiki page is missing {expected}")
with tempfile.TemporaryDirectory(prefix="dli-link-test-") as temp:
    tmp = pathlib.Path(temp)
    (tmp / "index.html").write_text('<a href="missing.html">missing</a>', encoding="utf-8")
    command = [sys.executable, str(root / "scripts/check-book-links.py"), str(tmp)]
    if subprocess.run(command, capture_output=True).returncode == 0:
        raise SystemExit("link checker accepted a missing file")
    (tmp / "missing.html").write_text('<p id="target">found</p>', encoding="utf-8")
    subprocess.run(command, capture_output=True, check=True)
    (tmp / "index.html").write_text('<img src="missing.png">', encoding="utf-8")
    if subprocess.run(command, capture_output=True).returncode == 0:
        raise SystemExit("link checker accepted a missing image")
print("book site: source, retcon, and link-check tests passed")
