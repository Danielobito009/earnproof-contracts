#!/usr/bin/env python3
"""Fail on missing local Markdown links and stale code/test references."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
failed = False
for document in sorted((ROOT / "docs").rglob("*.md")):
    text = document.read_text(encoding="utf-8")
    for target in re.findall(r"\]\(([^)#]+)(?:#[^)]+)?\)", text):
        if target.startswith(("http:", "https:", "mailto:")):
            continue
        if not (document.parent / target).resolve().exists():
            print(f"missing link: {document.relative_to(ROOT)} -> {target}")
            failed = True
    for reference in re.findall(r"`((?:contracts|tests)/[^`:#]+(?:/[^`:#]+)*::[A-Za-z0-9_]+)`", text):
        path, symbol = reference.rsplit("::", 1)
        source = ROOT / path
        if not source.exists() or not re.search(rf"\b{re.escape(symbol)}\b", source.read_text(encoding="utf-8")):
            print(f"stale code reference: {document.relative_to(ROOT)} -> {reference}")
            failed = True
if failed:
    sys.exit(1)
print("documentation links and code references are current")