# Deployment Scripts

These scripts provide a reproducible Stellar testnet deployment path for the EarnProof Soroban contracts.

## Prerequisites

- Rust toolchain from `rust-toolchain.toml`
- `wasm32v1-none` target support from Rust `1.84.0` or newer
- Stellar CLI available as `stellar`
- A funded Stellar testnet identity configured in Stellar CLI
- No secret keys committed to the repository

## Build and Deploy

```powershell
.\scripts\deploy-testnet.ps1 -Source deployer -Admin G... -IssuerAddress G... -Output scripts\deployment-manifest.testnet.json
```

By default, Stellar CLI deploy and invoke calls are retried up to five times for transient RPC transport failures such as connection resets, send failures, timeouts, temporary unavailability, and sequence races. Override this with `-MaxRetries` when needed:

```powershell
.\scripts\deploy-testnet.ps1 -Source deployer -Admin G... -IssuerAddress G... -MaxRetries 8
```

The script:

- installs the `wasm32v1-none` target if needed;
- builds optimized release WASM artifacts with `stellar contract build`;
- deploys `protocol-config`, `issuer-registry`, and `proof-registry`;
- initializes each contract;
- approves schema version `1`;
- registers the backend issuer address before proof anchoring is enabled;
- writes a manifest with contract IDs, WASM hashes, admin address, schema versions, and CLI command evidence.

## Local Sandbox

```powershell
pwsh -File scripts/local-sandbox/run-sandbox.ps1
```

Deploys all three contracts to a local Soroban sandbox and exercises a synthetic proof lifecycle: issuer registration, proof registration, verification, revocation, and pause behaviour. Each step asserts its result, so a run that completes is evidence rather than output.

Requires PowerShell 7 and a running local network (`stellar container start local`). The harness refuses any network other than `local`, reads no credentials, prints no secret, and writes a gitignored disposable manifest.

Smoke test — runs without Docker:

```powershell
pwsh -File scripts/local-sandbox/run-sandbox.tests.ps1
```

Full guide: [`docs/local-development.md`](../docs/local-development.md).

## Verify Manifest

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.testnet.json
```

For the checked-in example manifest:

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.example.json -AllowPlaceholders
```

The verifier checks the manifest shape and rejects placeholder contract IDs unless `-AllowPlaceholders` is explicitly supplied.

## Verify a Release Note

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.testnet.json -Release docs\releases\v0.1.0.md
```

With `-Release`, the verifier additionally reconciles a release note against the manifest. It checks that every required section and field is present, that the contract IDs and WASM hashes the note declares are the ones actually deployed, that no hash appears in the note which is absent from the manifest, and that no credential-shaped material has crept in.

Recording a hash is not the point — recording the *deployed* hash is. A note that lists an artifact which was never deployed is worse than no note, because it reads as evidence.

Breaking releases carry an extra requirement: the note must name an approving maintainer and provide substantive migration, rollback, and containment sections. See [`docs/compatibility.md`](../docs/compatibility.md).

## Live On-Chain Verification

Add `-Live` to perform read-only Stellar CLI checks against deployed contracts:

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.testnet.json -Live
```

This confirms admin addresses, pause state, config version, schema approvals, and cross-contract references without requiring a secret key or signing action.

Options:
- `-CliPath` — path to `stellar` CLI (default: `stellar`)
- `-TimeoutSeconds` — per-call timeout (default: 30)
- `-MaxRetries` — retries on transient RPC failures (default: 3)
- `-Network` — override manifest network

## Running Tests

```powershell
pwsh -NonInteractive -File scripts\verify-manifest.tests.ps1
```

Tests cover offline validation, live happy-path, admin mismatches, paused state,
schema approval failures, malformed CLI output, timeouts, and transient RPC retries.
No real network calls are made — all live checks use mock functions.
