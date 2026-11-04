#!/usr/bin/env python3
"""Render the recorded decisions and MCP-produced declarations from stdin."""
import json
import sys

result = json.load(sys.stdin)
lines = result["rendered"]
print("\n".join(lines[:2] + ["ratify " + decision for decision in result["decisions"]] + [""] + lines[2:]))
