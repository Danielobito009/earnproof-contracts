# Bugfix Requirements Document

## Introduction

Deployment manifests are currently validated only by `scripts/verify-manifest.ps1`, a PowerShell script that checks fields procedurally. This means there is no portable, machine-readable contract that CI, backend tooling, or future migration scripts can rely on independently of PowerShell. The bug is the absence of a versioned JSON Schema: any consumer other than the PS script must duplicate the validation logic, and no mechanism enforces a `manifestVersion` field that would allow breaking changes to be detected automatically.

This document captures what is currently broken (no schema exists, no `manifestVersion` field), what must be true after the fix (a JSON Schema is published, all existing manifests pass it, a negative fixture fails it, and CI enforces it), and what existing behavior must remain unchanged (the PowerShell semantic checks, the on-chain address patterns, and the testnet manifest content).

---

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN any tooling other than `verify-manifest.ps1` needs to validate a deployment manifest THEN the system provides no portable schema, forcing consumers to re-implement validation logic.

1.2 WHEN a deployment manifest file is written or updated THEN the system does not require a `manifestVersion` field, so breaking schema changes cannot be detected by automated tooling.

1.3 WHEN CI runs THEN the system does not validate `scripts/*.json` manifest files against any schema, allowing structurally invalid or incomplete manifests to be merged.

1.4 WHEN a manifest contains a field named `seed`, `mnemonic`, `secretKey`, `privateKey`, or `secret` THEN the system provides no schema-level guard to prevent secret material from being committed to the repository.

1.5 WHEN a new major schema revision is introduced THEN the system has no versioning convention or migration notes to guide consumers through the breaking change.

### Expected Behavior (Correct)

2.1 WHEN any tooling validates a deployment manifest THEN the system SHALL provide a JSON Schema at `schemas/deployment-manifest.schema.json` that can be used independently of PowerShell.

2.2 WHEN a deployment manifest file is written or updated THEN the system SHALL require a `manifestVersion` integer field so that breaking schema changes can be detected automatically.

2.3 WHEN CI runs THEN the system SHALL validate all `scripts/*.json` manifest files against `schemas/deployment-manifest.schema.json` and fail the build if any file is invalid.

2.4 WHEN the JSON Schema is evaluated THEN the system SHALL forbid top-level properties named `seed`, `mnemonic`, `secretKey`, `privateKey`, or `secret` using schema-level prohibition.

2.5 WHEN a new major schema revision is introduced THEN the system SHALL require a new `manifestVersion` value and SHALL document migration notes in `docs/schema-versioning.md`.

2.6 WHEN the JSON Schema validates the `admin` or `initialIssuer.address` fields THEN the system SHALL enforce the pattern `^G[A-Z2-7]{55}$`.

2.7 WHEN the JSON Schema validates a contract ID in `contracts.*` THEN the system SHALL enforce the pattern `^C[A-Z2-7]{55}$`.

2.8 WHEN the JSON Schema validates a `sha256` field THEN the system SHALL enforce the pattern `^[a-fA-F0-9]{64}$`.

2.9 WHEN the JSON Schema validates the `network` field THEN the system SHALL restrict the value to the enum `["stellar-testnet", "testnet", "stellar-mainnet", "mainnet"]`.

2.10 WHEN `scripts/deployment-manifest.example.json` and `scripts/deployment-manifest.testnet.json` are validated against the schema THEN the system SHALL report both files as valid (after `manifestVersion` is added to each file).

2.11 WHEN a test fixture at `tests/fixtures/invalid-manifest.json` is validated against the schema THEN the system SHALL report it as invalid.

2.12 WHEN the JSON Schema is evaluated THEN the system SHALL require the fields: `manifestVersion`, `network`, `contracts`, `wasm`, `admin`, `initialIssuer`, `schemaVersions`, and `deployedAt`.

### Unchanged Behavior (Regression Prevention)

3.1 WHEN `verify-manifest.ps1` is run against a valid manifest THEN the system SHALL CONTINUE TO perform semantic on-chain checks (placeholder detection, address format enforcement) as it does today.

3.2 WHEN `verify-manifest.ps1` is run against a manifest with placeholder contract IDs THEN the system SHALL CONTINUE TO reject it unless `-AllowPlaceholders` is passed.

3.3 WHEN CI runs the Rust job THEN the system SHALL CONTINUE TO execute `cargo fmt`, `cargo clippy`, `cargo test`, and `cargo build` without modification.

3.4 WHEN the `scripts/deployment-manifest.testnet.json` file is read THEN the system SHALL CONTINUE TO contain the same contract addresses, WASM hashes, and transaction URLs that are present today (only `manifestVersion` is added).

3.5 WHEN the `scripts/deployment-manifest.example.json` file is read THEN the system SHALL CONTINUE TO contain the same placeholder values that are present today (only `manifestVersion` is added).

3.6 WHEN `deploy-testnet.ps1` runs and writes a manifest THEN the system SHALL CONTINUE TO write valid JSON that satisfies the schema (the script's manifest structure already matches all required fields).
