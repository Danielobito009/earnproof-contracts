# Implementation Plan

- [ ] 1. Write bug condition exploration test
  - **Property 1: Bug Condition** - Schema and manifestVersion Absent
  - **CRITICAL**: This test MUST FAIL on unfixed code — failure confirms the bug exists
  - **DO NOT attempt to fix the test or the code when it fails**
  - **NOTE**: This test encodes the expected behavior — it will validate the fix when it passes after implementation
  - **GOAL**: Surface counterexamples that demonstrate the bug on the current unmodified repo
  - **Scoped PBT Approach**: The bug is deterministic — scope the property to the concrete failing cases below
  - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d "scripts/*.json" --spec=draft7 --all-errors` before any changes
  - **Expected counterexample 1**: Command exits non-zero because `schemas/deployment-manifest.schema.json` does not exist
  - **Expected counterexample 2**: If a draft schema is placed with `manifestVersion` required, both manifest files fail with "must have required property 'manifestVersion'" because neither file contains the field
  - **Expected counterexample 3**: A manifest with `"seed": "SXXX"` passes a schema without `patternProperties`, confirming the secret-field guard is absent
  - **Expected counterexample 4**: Inspect `.github/workflows/ci.yml` — confirm `schema-validation` job is absent
  - Document all counterexamples found before proceeding to implementation
  - **EXPECTED OUTCOME**: Validation command fails (this is correct — it proves the bug exists)
  - Mark task complete when the test is run and each counterexample is documented
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [ ] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** - Existing Manifest Content and PowerShell Behavior
  - **IMPORTANT**: Follow observation-first methodology — observe on UNFIXED code first
  - Observe: `scripts/deployment-manifest.testnet.json` contains specific contract addresses, WASM hashes, and transaction URLs (record exact values)
  - Observe: `scripts/deployment-manifest.example.json` contains specific placeholder values (record exact values)
  - Observe: `verify-manifest.ps1 -Manifest scripts/deployment-manifest.testnet.json -AllowPlaceholders` prints "Deployment manifest is valid" — record exact output
  - Observe: `.github/workflows/ci.yml` `contracts` job steps are exactly: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `cargo build --workspace` — record exact step list
  - Write property-based preservation tests:
    - **Property 2a**: For all strings matching `^G[A-Z2-7]{55}$` (random valid G-addresses), the schema (once created) must accept them in `admin` and `initialIssuer.address`
    - **Property 2b**: For all strings matching `^C[A-Z2-7]{55}$` (random valid C-addresses), the schema must accept them in `contracts.*`
    - **Property 2c**: For all 64-character hex strings matching `^[a-fA-F0-9]{64}$`, the schema must accept them as `sha256` values
    - **Property 2d**: For all manifests with `manifestVersion` in `[1, 100]` and all other required fields valid, the schema must report valid
  - These tests cannot yet be run against the schema (it doesn't exist), but baseline observations must be recorded now
  - Run `verify-manifest.ps1 -Manifest scripts/deployment-manifest.testnet.json -AllowPlaceholders` on UNFIXED code
  - **EXPECTED OUTCOME**: PowerShell check passes and outputs "Deployment manifest is valid"; manifest file content is recorded for regression comparison
  - Mark task complete when observations are recorded and property tests are written
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [ ] 3. Create `schemas/deployment-manifest.schema.json`

  - [ ] 3.1 Author the JSON Schema (Draft-07) file
    - Create `schemas/deployment-manifest.schema.json` with `"$schema": "http://json-schema.org/draft-07/schema#"`
    - Set `"$id": "https://github.com/earnproof/earnproof-contracts/schemas/deployment-manifest.schema.json"`
    - Set `"type": "object"` and `"additionalProperties": false` at the top level
    - Declare `required: ["manifestVersion", "network", "contracts", "wasm", "admin", "initialIssuer", "schemaVersions", "deployedAt"]`
    - _Bug_Condition: isBugCondition returns true when `fileExists("schemas/deployment-manifest.schema.json")` is false_
    - _Expected_Behavior: schema file exists at well-known path and validates all required fields and constraints_
    - _Preservation: schema must not reject any field present in the existing manifest files (beyond the new `manifestVersion` field)_
    - _Requirements: 2.1, 2.12_

  - [ ] 3.2 Add `manifestVersion` field definition
    - Define `manifestVersion` as `{ "type": "integer", "minimum": 1 }`
    - _Bug_Condition: isBugCondition returns true when `manifestHasField(manifest, "manifestVersion")` is false_
    - _Expected_Behavior: validators reject manifests without `manifestVersion`; validators reject `manifestVersion: 0` and `manifestVersion: "1"` (wrong type)_
    - _Requirements: 2.2_

  - [ ] 3.3 Add `network`, `admin`, `deployedAt`, `source`, `schemaVersions` field definitions
    - Define `network` as `{ "type": "string", "enum": ["stellar-testnet", "testnet", "stellar-mainnet", "mainnet"] }`
    - Define `admin` as `{ "type": "string", "pattern": "^G[A-Z2-7]{55}$" }`
    - Define `deployedAt` as `{ "type": "string", "format": "date-time", "pattern": "^\\d{4}-\\d{2}-\\d{2}T" }`
    - Define `source` as `{ "type": "string" }` (optional, free-form)
    - Define `schemaVersions` as `{ "type": "array", "items": { "type": "integer" }, "minItems": 1 }`
    - _Expected_Behavior: schema rejects `"network": "mainnet-staging"` and malformed G-addresses in `admin`_
    - _Requirements: 2.6, 2.9_

  - [ ] 3.4 Add `contracts` object definition
    - Define `contracts` as an object with `additionalProperties: false`
    - Required properties: `protocolConfig`, `issuerRegistry`, `proofRegistry`
    - Each property: `{ "type": "string", "pattern": "^C[A-Z2-7]{55}$" }`
    - _Expected_Behavior: schema rejects malformed C-addresses in `contracts.*`_
    - _Requirements: 2.7_

  - [ ] 3.5 Add `wasm` object definition
    - Define `wasm` as an object with `additionalProperties: false`
    - Required properties: `protocolConfig`, `issuerRegistry`, `proofRegistry`
    - Each property: object with `path` (string) and `sha256` (string, `pattern: "^[a-fA-F0-9]{64}$"`) — `additionalProperties: false`
    - _Expected_Behavior: schema rejects malformed SHA-256 hex strings in `wasm.*.sha256`_
    - _Requirements: 2.8_

  - [ ] 3.6 Add `initialIssuer` object definition
    - Define `initialIssuer` as an object with `additionalProperties: false`
    - Required: `address`, `issuerIdHash`, `metadataHash`
    - `address`: `{ "type": "string", "pattern": "^G[A-Z2-7]{55}$" }`
    - `issuerIdHash`, `metadataHash`: `{ "type": "string", "pattern": "^[a-fA-F0-9]{64}$" }`
    - _Expected_Behavior: schema enforces G-address pattern on `initialIssuer.address` and SHA-256 patterns on hashes_
    - _Requirements: 2.6, 2.8_

  - [ ] 3.7 Add optional field definitions (`transactions`, `explorer`, `commands`, `notes`)
    - Define `transactions` as `{ "type": "object", "additionalProperties": { "type": "string", "format": "uri" } }` (optional)
    - Define `explorer` as `{ "type": "object", "additionalProperties": { "type": "string", "format": "uri" } }` (optional)
    - Define `commands` as `{ "type": "array", "items": { "type": "string" } }` (optional)
    - Define `notes` as `{ "type": "string" }` (optional)
    - _Preservation: these fields appear in existing manifests and must not be rejected_
    - _Requirements: 3.4, 3.5_

  - [ ] 3.8 Add forbidden secret-field guard via `patternProperties`
    - Add at top level: `"patternProperties": { "(?i)^(seed|mnemonic|secretKey|privateKey|secret)$": false }`
    - _Bug_Condition: isBugCondition returns true when `secretFieldsAreBlocked(manifest)` is false_
    - _Expected_Behavior: schema rejects any top-level key matching `seed`, `mnemonic`, `secretKey`, `privateKey`, or `secret` (case-insensitive); includes `SEED`, `Seed`, `SECRET`_
    - _Requirements: 2.4_

  - [ ] 3.9 Verify schema structure with a dry-run against the testnet manifest (before adding `manifestVersion`)
    - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d scripts/deployment-manifest.testnet.json --spec=draft7 --all-errors`
    - **EXPECTED OUTCOME**: Fails with "must have required property 'manifestVersion'" — confirms schema is wired correctly and the field absence is caught
    - _Requirements: 2.2, 2.3_

- [ ] 4. Add `manifestVersion: 1` to `scripts/deployment-manifest.example.json`
  - Insert `"manifestVersion": 1` as the first field in the JSON object
  - All other fields and values must remain byte-for-byte identical to the current file
  - _Preservation: Preservation Requirements — file content unchanged except for the new field_
  - _Requirements: 2.10, 3.5_

- [ ] 5. Add `manifestVersion: 1` to `scripts/deployment-manifest.testnet.json`
  - Insert `"manifestVersion": 1` as the first field in the JSON object
  - All other fields and values must remain byte-for-byte identical to the current file (contract addresses, WASM hashes, transaction URLs, explorer links, commands, notes)
  - _Preservation: Preservation Requirements — file content unchanged except for the new field_
  - _Requirements: 2.10, 3.4_

- [ ] 6. Create `tests/fixtures/valid-manifest.json` (positive fixture)
  - Create directory `tests/fixtures/` if it does not exist
  - Copy `scripts/deployment-manifest.testnet.json` (with `manifestVersion: 1` already added in task 5) as the positive fixture
  - Purpose: confirms the real testnet manifest passes the schema end-to-end
  - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d tests/fixtures/valid-manifest.json --spec=draft7 --all-errors`
  - **EXPECTED OUTCOME**: Exits 0 — file is valid
  - _Requirements: 2.10, 2.11_

- [ ] 7. Create `tests/fixtures/invalid-manifest.json` (negative fixture)
  - Create a minimal JSON object that intentionally violates the schema in two ways:
    1. Omit the `contracts` object entirely (violates `required`)
    2. Set `admin` to a malformed address, e.g. `"BADADDRESS"` (violates G-address pattern)
  - Include `manifestVersion: 1` and `network: "stellar-testnet"` so only the two intentional violations trigger
  - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d tests/fixtures/invalid-manifest.json --spec=draft7 --all-errors`
  - **EXPECTED OUTCOME**: Exits non-zero — file is invalid with errors referencing missing `contracts` and malformed `admin`
  - _Requirements: 2.11_

- [ ] 8. Add `schema-validation` job to `.github/workflows/ci.yml`
  - Append a new top-level job `schema-validation` after the existing `contracts` job
  - Job definition:
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
  - No `needs:` dependency — job runs in parallel with `contracts`
  - Do NOT modify the existing `contracts` job steps
  - _Bug_Condition: isBugCondition returns true when `ciJobExists("schema-validation")` is false_
  - _Expected_Behavior: CI rejects any `scripts/*.json` that does not satisfy the schema_
  - _Preservation: `contracts` job steps (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`) are syntactically unchanged_
  - _Requirements: 2.3, 3.3_

- [ ] 9. Create `docs/schema-versioning.md`
  - Create directory `docs/` if it does not exist
  - Document what `manifestVersion` represents and its initial value (`1`)
  - Define **breaking changes** (removing a required field, narrowing a pattern, adding a new required field, changing a field's type) — require a version bump
  - Define **additive changes** (adding an optional field, widening a pattern) — no version bump required
  - Document the migration procedure: bump `manifestVersion`, update the schema, update all manifest files, update CI fixtures, add a migration note
  - Include a version history table with version 1 as the initial entry
  - _Bug_Condition: isBugCondition returns true when no versioning convention exists_
  - _Expected_Behavior: document exists at `docs/schema-versioning.md` and describes the convention_
  - _Requirements: 2.5_

- [ ] 10. Fix validation — run full end-to-end integration check

  - [ ] 10.1 Verify bug condition exploration test now passes
    - **Property 1: Expected Behavior** - Schema Validates All Required Fields and Constraints
    - **IMPORTANT**: Re-run the SAME command from task 1 — do NOT write a new test
    - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d "scripts/*.json" --spec=draft7 --all-errors`
    - **EXPECTED OUTCOME**: Exits 0 — both `scripts/*.json` files are valid (confirms bug is fixed)
    - Also verify: `tests/fixtures/invalid-manifest.json` exits non-zero (schema rejects bad input)
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.6, 2.7, 2.8, 2.9, 2.11, 2.12_

  - [ ] 10.2 Verify preservation property tests still pass
    - **Property 2: Preservation** - Existing Valid Manifests Continue to Pass
    - **IMPORTANT**: Re-run the SAME observations from task 2 — do NOT write new tests
    - Run `verify-manifest.ps1 -Manifest scripts/deployment-manifest.testnet.json -AllowPlaceholders` — must still print "Deployment manifest is valid"
    - Confirm `scripts/deployment-manifest.testnet.json` field values match the baseline recorded in task 2 (only `manifestVersion: 1` is new)
    - Confirm `scripts/deployment-manifest.example.json` field values match the baseline recorded in task 2 (only `manifestVersion: 1` is new)
    - Confirm `.github/workflows/ci.yml` `contracts` job steps are identical to the baseline recorded in task 2
    - Run property-based checks from task 2: random valid G-addresses, C-addresses, SHA-256 hex strings, and `manifestVersion` values in `[1, 100]` all accepted
    - **EXPECTED OUTCOME**: All preservation checks pass — no regressions
    - _Requirements: 2.10, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [ ] 11. Checkpoint — Ensure all tests pass
  - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d "scripts/*.json" --spec=draft7 --all-errors` — must exit 0
  - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d tests/fixtures/valid-manifest.json --spec=draft7 --all-errors` — must exit 0
  - Run `npx ajv-cli@5 validate -s schemas/deployment-manifest.schema.json -d tests/fixtures/invalid-manifest.json --spec=draft7 --all-errors` — must exit non-zero
  - Run `verify-manifest.ps1 -Manifest scripts/deployment-manifest.testnet.json -AllowPlaceholders` — must print "Deployment manifest is valid"
  - Confirm all 7 files listed in the design are present: `schemas/deployment-manifest.schema.json`, `tests/fixtures/valid-manifest.json`, `tests/fixtures/invalid-manifest.json`, `scripts/deployment-manifest.example.json` (updated), `scripts/deployment-manifest.testnet.json` (updated), `.github/workflows/ci.yml` (updated), `docs/schema-versioning.md`
  - Ask the user if any questions arise before closing out
