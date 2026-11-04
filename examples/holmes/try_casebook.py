#!/usr/bin/env python3
"""Run one casebook example against a fresh temporary project."""
import json
from pathlib import Path
import subprocess
import sys


def answer(result, chapter):
    if chapter == "name":
        finding = result["name_conflict"]
        return f'{finding["code"]}\nsupporting decisions: {", ".join(finding["citations"])}'
    if chapter == "wounds":
        rows = result["wound_accounts"]
        return "\n".join(f'{rows[part][0]["atom"]}: {rows[part][0]["status"]}' for part in ("shoulder", "leg"))
    if chapter == "timeline":
        finding = result["chronology_conflict"]
        return f'{finding["code"]}\nsupporting decisions: {", ".join(finding["citations"])}'
    if chapter == "return":
        why = result["retcon"]["why_old"]
        label = {"dead": "overturned", "possible": "unconfirmed", "certain": "canon"}[why["why_not"]["status"]]
        return f'earlier claim: {label}\nsupporting decisions: {", ".join(why["supporting_decisions"])}'
    if chapter == "branch":
        row = result["retcon"]["branch"][0]
        return f'{row["atom"]}: {row["status"]}'
    if chapter == "rule":
        finding = result["rule_conflict"]
        return f'{finding["code"]}\nsupporting decisions: {", ".join(finding["citations"])}'
    raise ValueError(f"unknown chapter: {chapter}")


def block(result, chapter):
    return f'''\n## Run it yourself

From the repository root, with `deloreis` installed, run:

```sh
python3 examples/holmes/try_casebook.py {chapter}
```

This replays the casebook in a temporary project and prints:

```text
{answer(result, chapter)}
```
'''


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3):
        raise SystemExit("usage: try_casebook.py CHAPTER [DELOREIS_BINARY]")
    binary = sys.argv[2] if len(sys.argv) == 3 else "deloreis"
    build = Path(__file__).with_name("build.py")
    output = subprocess.run([sys.executable, str(build), binary], check=True, capture_output=True, text=True)
    print(answer(json.loads(output.stdout), sys.argv[1]))
