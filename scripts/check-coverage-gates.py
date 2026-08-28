#!/usr/bin/env python3
"""Fail CI if any contract's measured coverage drops below its gate (#66).

Reads the `cargo llvm-cov report --json` output and checks region coverage
per file against MINIMUMS below. See docs/testing.md for how these numbers
were chosen and what's deliberately excluded.
"""
import json
import sys
from pathlib import Path

# Region-coverage minimums, one per contract. Calibrated below the coverage
# measured when this gate was introduced (issuer-registry 98.77%,
# proof-registry 98.61%, protocol-config 97.51%) rather than at those exact
# figures, so an unrelated one-line change to an already-well-tested branch
# doesn't fail CI over noise — while still catching an actual coverage
# regression (a new code path added with no test reaching it).
MINIMUMS = {
    "contracts/issuer-registry/src/lib.rs": 90.0,
    "contracts/proof-registry/src/lib.rs": 90.0,
    "contracts/protocol-config/src/lib.rs": 90.0,
}

# packages/shared/src/lib.rs is intentionally excluded: it contains only
# #[contracterror]/#[contracttype] declarations and constants, zero `pub fn`
# or `impl` blocks (verified by inspection — grep for `pub fn|impl ` returns
# nothing). llvm-cov attributes 0% to it because there is no executable code
# of its own to instrument; the derive-macro-generated (de)serialization
# code it produces IS exercised, but only observably through the contracts
# that use these types, which are the entries in MINIMUMS above.


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: check-coverage-gates.py <coverage.json>", file=sys.stderr)
        return 2

    report_path = Path(sys.argv[1])
    data = json.loads(report_path.read_text())
    files = data["data"][0]["files"]

    by_suffix = {}
    for f in files:
        for suffix in MINIMUMS:
            if f["filename"].endswith(suffix):
                by_suffix[suffix] = f["summary"]["regions"]["percent"]

    failed = False
    for suffix, minimum in MINIMUMS.items():
        if suffix not in by_suffix:
            print(f"FAIL: no coverage data found for {suffix} (renamed or moved?)")
            failed = True
            continue
        actual = by_suffix[suffix]
        status = "PASS" if actual >= minimum else "FAIL"
        if status == "FAIL":
            failed = True
        print(f"{status}: {suffix} region coverage {actual:.2f}% (minimum {minimum:.1f}%)")

    if failed:
        return 1
    print("all critical-path coverage gates passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
