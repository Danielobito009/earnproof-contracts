# Deployment Manifest Schema Bugfix Design

## Overview

Deployment manifests are currently validated only by `scripts/verify-manifest.ps1`, a PowerShell
script that performs semantic checks procedurally. No portable, machine-readable contract exists
that CI, backend tooling, or migration scripts can rely on independently of PowerShell.

The fix introduces a JSON Schema (Draft-07) at `schemas/deployment-manifest.schema.json` that
encodes structural validation: required fields, field formats, forbidden secret-material names,
Stellar address patterns, and a `manifestVersion` integer. A new CI job (`schema-validation`) runs
`npx ajv-cli validate` against every `scripts/*.json` manifest on every push and pull request.
The PowerShell verifier is **not modified** — it remains the semantic/chain-state layer.

**Files affected:**

| File | Change |
|------|--------|
| `schemas/deployment-manifest.schema.json` | New — the JSON Schema |
| `tests/fixtures/valid-manifest.json` | New — positive fixture |
| `tests/fixtures/invalid-manifest.json` | New — negative fixture |
| `scripts/deployment-manifest.example.json` | Add `"manifestVersion": 1` |
| `scripts/deployment-manifest.testnet.json` | Add `"manifestVersion": 1` |
| `.github/workflows/ci.yml` | Add `schema-validation` job |
| `docs/schema-versioning.md` | New — versioning policy |

---

## Glossary

- **Bug_Condition (C)**: The condition that triggers the defect — any manifest file is written,
  consumed by tooling, or merged without a portable schema to validate against.
- **Property (P)**: The desired structural outcome — a JSON Schema enforces all required fields,
  patterns, forbidden keys, and `manifestVersion`, independently of PowerShell.
- **Preservation**: The existing `verify-manifest.ps1` semantic checks, the Rust CI pipeline, and
  the content of the two existing manifest files must remain unchanged after the fix.
- **`manifestVersion`**: An integer field added to every manifest file that signals the schema
  generation in use. Starts at `1`; incremented on every breaking schema change.
- **G-address**: A Stellar public key — matches `^G[A-Z2-7]{55}$`.
- **C-address**: A Stellar contract ID — matches `^C[A-Z2-7]{55}$`.
- **SHA-256 hex**: A 64-character lowercase or uppercase hex string — matches `^[a-fA-F0-9]{64}$`.
- **ajv-cli**: The CLI for the [Ajv](https://ajv.js.org/) JSON Schema validator; invoked via
  `npx ajv-cli@5` so no explicit install step is required in CI.
- **Draft-07**: JSON Schema specification version used for the schema, chosen for widest tooling
  support and Ajv default compatibility.

---

## Bug Details

### Bug Condition

The bug manifests whenever a deployment manifest is written or consumed by any tooling other than
`verify-manifest.ps1`. Because no JSON Schema exists, structural validation must be re-implemented
by every consumer, and there is no automated mechanism to detect breaking field changes.

**Formal Specification:**

```
FUNCTION isBugCondition(manifest)
  INPUT:  manifest — a JSON file at scripts/*.json or any deployment manifest path
  OUTPUT: boolean

  RETURN NOT fileExists("schemas/deployment-manifest.schema.json")
         OR NOT manifestHasField(manifest, "manifestVersion")
         OR NOT ciJobExists("schema-validation")
         OR NOT secretFieldsAreBlocked(manifest)
END FUNCTION
```

### Examples

- **Missing schema**: A backend service wants to validate a manifest before processing it. No
  schema file exists, so it either skips validation or duplicates PowerShell logic in another
  language. → **Expected**: schema exists at a well-known path.

- **Missing `manifestVersion`**: A script writes a manifest after a breaking field rename. No
  tool detects the version mismatch, and consumers silently read stale field names. → **Expected**:
  `manifestVersion` is required and validators reject manifests without it.

- **No CI enforcement**: A PR removes the `contracts` object from a manifest. The Rust CI job
  passes because it doesn't touch manifest files. The broken manifest is merged. → **Expected**:
  `schema-validation` CI job catches the omission and blocks the merge.

- **Secret committed by mistake**: A developer adds `"seed": "SXXXXX..."` to a manifest. No
  schema-level guard exists. → **Expected**: schema rejects any top-level property matching
  `seed`, `mnemonic`, `secretKey`, `privateKey`, or `secret` (case-insensitive).

---

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**

- `verify-manifest.ps1` continues to perform all existing semantic checks (placeholder detection,
  address format enforcement, hash format enforcement, network restriction to testnet values).
- `verify-manifest.ps1` continues to reject manifests with placeholder contract IDs unless
  `-AllowPlaceholders` is passed.
- The Rust CI job continues to execute `cargo fmt`, `cargo clippy`, `cargo test`, and
  `cargo build` without any modification.
- `scripts/deployment-manifest.testnet.json` retains all existing field values — only
  `"manifestVersion": 1` is added.
- `scripts/deployment-manifest.example.json` retains all existing placeholder values — only
  `"manifestVersion": 1` is added.
- `deploy-testnet.ps1` (if present) continues to write manifests that are structurally valid;
  the new schema requirement is satisfied by the existing field structure.

**Scope:**

All manifest files that already conform to the field structure described by the requirements
(i.e., all inputs where the bug condition does NOT hold) must continue to be accepted after the
fix. The schema must not reject any currently valid manifest except for the addition of the
`manifestVersion` field.

---

## Hypothesized Root Cause

Based on the bug description, the absence of a schema is not a code defect in the traditional
sense — it is a missing artifact. The root causes of each defect clause are:

1. **No schema file exists (1.1)**: The project relied on a PowerShell script as the sole
   validator. No JSON Schema was ever authored. Fix: create
   `schemas/deployment-manifest.schema.json`.

2. **No `manifestVersion` field (1.2)**: The manifest format was never formally versioned.
   The existing `schemaVersions` array tracks on-chain schema IDs, not the manifest format
   version. Fix: add `manifestVersion` as a required integer field to both the schema and the
   two existing manifest files.

3. **No CI schema validation (1.3)**: The `.github/workflows/ci.yml` only has a `contracts` job
   that runs Rust tooling. No step validates JSON manifest files. Fix: add a
   `schema-validation` job that invokes `npx ajv-cli@5 validate`.

4. **No secret-field guard (1.4)**: The schema doesn't exist, so there is nowhere to express a
   prohibition. Fix: use `patternProperties` with a case-insensitive regex to set the schema
   for any key matching `seed|mnemonic|secretKey|privateKey|secret` to `false`.

5. **No versioning convention (1.5)**: No documentation describes how `manifestVersion` is
   incremented or what constitutes a breaking change. Fix: create `docs/schema-versioning.md`.

---

## Correctness Properties

Property 1: Bug Condition — Schema Validates All Required Fields and Constraints

_For any_ manifest file where the bug condition holds (no schema exists, or `manifestVersion` is
absent, or CI has no schema job, or secret fields are not blocked), the fixed system SHALL provide
`schemas/deployment-manifest.schema.json` that requires `manifestVersion`, `network`, `contracts`,
`wasm`, `admin`, `initialIssuer`, `schemaVersions`, and `deployedAt`; enforces G-address, C-address,
SHA-256, and network enum constraints; and forbids top-level keys matching
`seed|mnemonic|secretKey|privateKey|secret`. The CI job `schema-validation` SHALL reject any
`scripts/*.json` file that does not satisfy the schema.

**Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.6, 2.7, 2.8, 2.9, 2.11, 2.12**

Property 2: Preservation — Existing Valid Manifests Continue to Pass

_For any_ manifest file where the bug condition does NOT hold (i.e., a structurally complete
manifest that already contains all required fields with correct formats, plus the new
`manifestVersion: 1` field), the fixed system SHALL report that file as valid when validated
against `schemas/deployment-manifest.schema.json`, and SHALL NOT alter the semantic behavior of
`verify-manifest.ps1`, the Rust CI job, or the content of the two existing manifest files beyond
the addition of `"manifestVersion": 1`.

**Validates: Requirements 2.10, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6**

---

## Fix Implementation

### Changes Required

#### 1. `schemas/deployment-manifest.schema.json` (new file)

**Schema declaration:**
- `"$schema": "http://json-schema.org/draft-07/schema#"`
- `"$id": "https://github.com/earnproof/earnproof-contracts/schemas/deployment-manifest.schema.json"`
- `"type": "object"`
- `"additionalProperties": false` at the top level (allows only declared properties)

**Required fields:**
```
["manifestVersion", "network", "contracts", "wasm", "admin",
 "initialIssuer", "schemaVersions", "deployedAt"]
```

**Field specifications:**

| Field | Type | Constraint |
|-------|------|------------|
| `manifestVersion` | `integer`, minimum `1` | Starts at 1, incremented on breaking changes |
| `network` | `string` | `enum: ["stellar-testnet", "testnet", "stellar-mainnet", "mainnet"]` |
| `admin` | `string` | `pattern: ^G[A-Z2-7]{55}$` |
| `deployedAt` | `string` | `format: date-time` + `pattern: ^\d{4}-\d{2}-\d{2}T` |
| `source` | `string` | Optional, free-form |
| `schemaVersions` | `array` of `integer`, `minItems: 1` | Tracks on-chain schema IDs |
| `contracts` | object — see below | Required |
| `wasm` | object — see below | Required |
| `initialIssuer` | object — see below | Required |
| `transactions` | object — optional | `additionalProperties: { type: "string", format: "uri" }` |
| `explorer` | object — optional | `additionalProperties: { type: "string", format: "uri" }` |
| `commands` | `array` of `string` | Optional |
| `notes` | `string` | Optional |

**`contracts` object:**
- Required: `protocolConfig`, `issuerRegistry`, `proofRegistry`
- Each: `string`, `pattern: ^C[A-Z2-7]{55}$`
- `additionalProperties: false`

**`wasm` object:**
- Required: `protocolConfig`, `issuerRegistry`, `proofRegistry`
- Each: object with `path` (string) and `sha256` (string, `pattern: ^[a-fA-F0-9]{64}$`)
- `additionalProperties: false` on the wasm sub-objects

**`initialIssuer` object:**
- Required: `address`, `issuerIdHash`, `metadataHash`
- `address`: `string`, `pattern: ^G[A-Z2-7]{55}$`
- `issuerIdHash`, `metadataHash`: `string`, `pattern: ^[a-fA-F0-9]{64}$`
- `additionalProperties: false`

**Forbidden fields via `patternProperties`:**
```json
"patternProperties": {
  "(?i)^(seed|mnemonic|secretKey|privateKey|secret)$": false
}
```
This blocks any top-level key whose name matches the pattern case-insensitively.

---

#### 2. `scripts/deployment-manifest.example.json` (modify)

Add `"manifestVersion": 1` as the first field. All other content unchanged.

---

#### 3. `scripts/deployment-manifest.testnet.json` (modify)

Add `"manifestVersion": 1` as the first field. All other content unchanged.

---

#### 4. `tests/fixtures/valid-manifest.json` (new file)

A copy of `scripts/deployment-manifest.testnet.json` with `"manifestVersion": 1` added.
Purpose: positive fixture confirming the real testnet manifest passes the schema.

---

#### 5. `tests/fixtures/invalid-manifest.json` (new file)

A minimal manifest that intentionally violates the schema in two ways:
- The `contracts` object is omitted (violates the `required` constraint).
- The `admin` field contains a malformed address (violates the G-address pattern).

Purpose: negative fixture confirming the schema rejects invalid manifests.

---

#### 6. `.github/workflows/ci.yml` (modify — add job)

Add a new job after the existing `contracts` job:

```yaml
schema-validation:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Validate manifests against JSON Schema
      run: |
        npx ajv-cli@5 validate \
          -s schemas/deployment-manifest.schema.json \
          -d "scripts/*.json" \
          --spec=draft7 \
          --all-errors
```

No `needs:` dependency on `contracts` — the two jobs run in parallel.
`npx` is available on `ubuntu-latest` without any setup step.

---

#### 7. `docs/schema-versioning.md` (new file)

Covers:
- What `manifestVersion` represents and its initial value (1).
- Definition of a **breaking change** (removing a required field, narrowing a pattern, adding
  a new required field, changing a field's type).
- Definition of an **additive change** (adding an optional field, widening a pattern) — does
  not require a version bump.
- Migration procedure: bump `manifestVersion`, update the schema, update all manifest files,
  update CI fixtures, add a migration note to the doc.
- Version history table starting with version 1.

---

## Testing Strategy

### Validation Approach

Testing follows a two-phase approach: first, confirm the bug is real by running the schema
validator on the current (unfixed) manifests and observing failures; then, verify the fix works
and that no existing behavior is broken.

---

### Exploratory Bug Condition Checking

**Goal**: Surface counterexamples that demonstrate the bug on the unfixed codebase. Confirm or
refute the root cause analysis.

**Test Plan**: Attempt to validate `scripts/deployment-manifest.testnet.json` and
`scripts/deployment-manifest.example.json` with `npx ajv-cli@5 validate` before any changes are
made. Observe that:
1. The command fails because `schemas/deployment-manifest.schema.json` does not exist.
2. Even if a schema were drafted without `manifestVersion`, both files would fail validation
   because the field is absent.

**Test Cases:**

1. **Schema file absent**: Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d scripts/deployment-manifest.testnet.json` on the unmodified repo. Will fail because the schema file does not exist.
2. **`manifestVersion` absent**: Draft the schema with `manifestVersion` required, then validate either manifest file before adding the field. Will fail with "must have required property 'manifestVersion'".
3. **Secret field not blocked**: Add `"seed": "SXXX"` to a manifest and validate against a schema without `patternProperties`. Will pass (incorrectly). Confirms the guard is needed.
4. **No CI job**: Inspect `.github/workflows/ci.yml` before changes. Confirms `schema-validation` job is absent.

**Expected Counterexamples:**
- `ajv-cli` exits non-zero because the schema file is missing.
- Manifests missing `manifestVersion` fail with a required-field error once a draft schema is in place.

---

### Fix Checking

**Goal**: Verify that for all inputs where the bug condition holds, the fixed system produces the
expected behavior.

**Pseudocode:**
```
FOR ALL manifest WHERE isBugCondition(manifest) DO
  result := validateAgainstSchema(manifest, "schemas/deployment-manifest.schema.json")
  ASSERT result.isValid = true   // for well-formed manifests after adding manifestVersion
END FOR

ASSERT fileExists("schemas/deployment-manifest.schema.json")
ASSERT ciJobExists("schema-validation")
ASSERT validateAgainstSchema("tests/fixtures/invalid-manifest.json").isValid = false
```

---

### Preservation Checking

**Goal**: Verify that for all inputs where the bug condition does NOT hold, the fixed system
produces the same results as the original.

**Pseudocode:**
```
FOR ALL manifest WHERE NOT isBugCondition(manifest) DO
  ASSERT verifyManifestPS1(manifest) = verifyManifestPS1_original(manifest)
  ASSERT validateAgainstSchema(manifest) = valid
END FOR
```

**Testing Approach**: Property-based testing is recommended for preservation checking because
the space of well-formed Stellar addresses and SHA-256 hashes is large, and manual unit tests
cover only a handful of examples. Generating random valid addresses and hashes confirms the
patterns accept the full valid domain.

**Test Cases:**

1. **Valid manifest passes schema**: Validate `tests/fixtures/valid-manifest.json` (testnet
   manifest + `manifestVersion`) — must report valid.
2. **Invalid manifest fails schema**: Validate `tests/fixtures/invalid-manifest.json` — must
   report invalid.
3. **Example manifest passes schema**: Validate `scripts/deployment-manifest.example.json`
   after adding `manifestVersion` — must report valid (placeholder C-addresses must be
   accepted because the schema only checks format, not placeholder detection).
4. **PowerShell verifier unchanged**: Run `verify-manifest.ps1 -Manifest scripts/deployment-manifest.testnet.json -AllowPlaceholders` before and after changes — output must be identical.
5. **Rust CI unchanged**: `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo build` all
   continue to pass.

---

### Unit Tests

- Validate a manifest with every required field present and correct → expect valid.
- Validate a manifest missing each required field in turn → expect invalid with a meaningful error.
- Validate a manifest with a malformed G-address in `admin` → expect invalid.
- Validate a manifest with a malformed C-address in `contracts.protocolConfig` → expect invalid.
- Validate a manifest with a malformed SHA-256 in `wasm.protocolConfig.sha256` → expect invalid.
- Validate a manifest with `"network": "mainnet-staging"` (not in enum) → expect invalid.
- Validate a manifest with `"manifestVersion": 0` (below minimum) → expect invalid.
- Validate a manifest with `"manifestVersion": "1"` (wrong type) → expect invalid.
- Validate a manifest containing `"seed": "SXXX"` → expect invalid (forbidden field).
- Validate a manifest containing `"SECRET": "..."` → expect invalid (case-insensitive match).
- Validate a manifest containing `"mnemonic": "word1 word2"` → expect invalid.
- Validate `tests/fixtures/valid-manifest.json` → expect valid.
- Validate `tests/fixtures/invalid-manifest.json` → expect invalid.

---

### Property-Based Tests

- Generate random strings matching `^G[A-Z2-7]{55}$` and verify the schema accepts them in
  `admin` and `initialIssuer.address`.
- Generate random strings matching `^C[A-Z2-7]{55}$` and verify the schema accepts them in
  `contracts.*`.
- Generate random 64-character hex strings and verify the schema accepts them as `sha256` values.
- Generate random strings that do NOT match `^G[A-Z2-7]{55}$` (wrong prefix, wrong length, or
  invalid characters) and verify the schema rejects them.
- Generate random well-formed manifests with `manifestVersion` in `[1, 100]` and verify they
  are all accepted.
- Generate manifests with random top-level keys from
  `["seed", "SEED", "Seed", "mnemonic", "secretKey", "privateKey", "secret"]` and verify each
  is rejected.

---

### Integration Tests

- Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d "scripts/*.json" --spec=draft7 --all-errors` end-to-end in a shell — must exit 0 after both manifest files have `manifestVersion` added.
- Simulate the full CI `schema-validation` job locally using the same `npx` command — must pass.
- Run `verify-manifest.ps1 -Manifest scripts/deployment-manifest.testnet.json` after all changes — must still print "Deployment manifest is valid" (semantic checks unaffected).
- Verify that the `contracts` Rust CI job steps are syntactically unchanged in the final `ci.yml`.
