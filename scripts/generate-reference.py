#!/usr/bin/env python3
"""Generate deterministic API inventory artifacts from Rust public methods."""
from pathlib import Path
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
contracts = ["protocol-config", "issuer-registry", "proof-registry"]
entries = []
for name in contracts:
    source = ROOT / "contracts" / name / "src/lib.rs"
    text = source.read_text(encoding="utf-8")
    for match in re.finditer(r"pub fn (\w+)\((.*?)\)(?:\s*->\s*([^\{]+))?\s*\{", text, re.S):
        entries.append({"contract": name, "name": match.group(1), "parameters": " ".join(match.group(2).split()), "result": " ".join((match.group(3) or "()").split()), "authorization": "current admin" if match.group(1) not in {"get_admin", "get_issuer", "get_issuer_by_address", "is_active_address", "is_active_issuer", "is_paused", "is_schema_version_approved", "get_config_version", "get_proof", "get_issuer_registry", "get_protocol_config", "is_valid_proof", "is_revoked"} else "none", "storage_effect": "read-only" if match.group(1).startswith(("get_", "is_")) else "documented in lifecycle specification", "event_effect": "none unless documented in lifecycle specification", "failure_atomicity": "Soroban invocation rollback", "source": f"contracts/{name}/src/lib.rs"})
entries.sort(key=lambda item: (item["contract"], item["name"]))
events = []
for name in contracts:
    text = (ROOT / "contracts" / name / "src/lib.rs").read_text(encoding="utf-8")
    events.extend({"contract": name, "name": event} for event in re.findall(r"#\[contractevent\]\s*pub struct (\w+)", text))
shared = (ROOT / "packages/shared/src/lib.rs").read_text(encoding="utf-8")
errors = re.findall(r"pub enum (\w+Error) \{(.*?)\n\}", shared, re.S)
payload = {"format_version": 1, "contract_version": "0.1.0", "source_commit": "recorded-by-release", "toolchain": "rust-toolchain.toml", "entries": entries, "events": events, "errors": [{"type": name, "variants": re.findall(r"\b(\w+)\s*=", body)} for name, body in errors]}
reference = ROOT / "docs/reference"
json_text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
markdown = "# Generated Contract API\n\n<!-- BEGIN GENERATED: do not edit. -->\n\n"
for entry in entries:
    markdown += f"## {entry['contract']}::{entry['name']}\n\n- Parameters: `{entry['parameters']}`\n- Result: `{entry['result']}`\n- Authorization: {entry['authorization']}\n- Storage effect: {entry['storage_effect']}\n- Event effect: {entry['event_effect']}\n- Failure atomicity: {entry['failure_atomicity']}\n- Source: `{entry['source']}`\n\n"
markdown += "<!-- END GENERATED -->\n"
if "--check" in sys.argv:
    if (reference / "api.json").read_text(encoding="utf-8") != json_text or (reference / "api.md").read_text(encoding="utf-8") != markdown:
        print("generated API reference is stale", file=sys.stderr)
        sys.exit(1)
else:
    reference.mkdir(exist_ok=True)
    (reference / "api.json").write_text(json_text, encoding="utf-8")
    (reference / "api.md").write_text(markdown, encoding="utf-8")