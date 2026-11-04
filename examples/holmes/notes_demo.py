#!/usr/bin/env python3
"""Replay a small, reviewed transcription of notes.md through MCP."""
import json
import pathlib
import subprocess
import sys
import tempfile

binary = sys.argv[1] if len(sys.argv) > 1 else "deloreis"
with tempfile.TemporaryDirectory(prefix="dli-notes-") as temp:
    project = pathlib.Path(temp) / "project"
    subprocess.run([binary, "init", str(project)], check=True, stdout=subprocess.DEVNULL)
    server = subprocess.Popen([binary, "mcp", "--project", str(project)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    sequence = 0
    def call(name, arguments):
        global sequence
        sequence += 1
        server.stdin.write(json.dumps({"jsonrpc": "2.0", "id": sequence, "method": "tools/call", "params": {"name": name, "arguments": arguments}}) + "\n")
        server.stdin.flush()
        frame = json.loads(server.stdout.readline())
        reply = json.loads(frame["result"]["content"][0]["text"])
        if not reply["ok"]:
            raise RuntimeError(f"{name}: {reply}")
        result = reply["result"]
        if any(finding.get("severity") == "error" for finding in result.get("findings", [])):
            raise RuntimeError(f"{name}: {result['findings']}")
        return result
    try:
        ws = call("author.open", {"scope": "world"})["workspace"]
        call("author.record_decision", {"workspace": ws, "decision": "d-0001"})
        provenance = {"kind": "cited", "decisions": ["d-0001"], "state": "ratified"}
        call("author.add_entity", {"workspace": ws, "kind": "person", "id": "watson", "name": "John Watson", "provenance": provenance})
        call("author.add_fact", {"workspace": ws, "entity": "watson", "key": "first-name", "value": "john", "provenance": provenance})
        for value, origin in [("shoulder", "study-in-scarlet"), ("leg", "sign-of-four")]:
            call("author.add_fact", {"workspace": ws, "entity": "watson", "key": "reported-wound-location", "value": value, "provenance": {"kind": "proposed", "origin": origin}})
        call("author.check", {"workspace": ws})
        call("author.materialize", {"workspace": ws})
        for predicate, value in [("first-name", "john"), ("reported-wound-location", "shoulder"), ("reported-wound-location", "leg")]:
            atom = {"pred": predicate, "args": [{"Sym": "watson"}, {"Sym": value}]}
            row = call("author.query", {"workspace": ws, "goal_atom": atom, "layer": "all"})["rows"][0]
            print(f"{row['atom']}: {row['status']}")
    finally:
        server.terminate()
        server.wait()
