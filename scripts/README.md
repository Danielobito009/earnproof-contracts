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

## Proof Lifecycle Smoke Test

`scripts/smoke-proof-lifecycle.ps1` exercises the full on-chain proof lifecycle
against the deployed testnet contracts: preflight checks, proof registration,
field lookup, revocation, and post-revoke validity check.

This script is **opt-in and manual**. It is not run by CI on pull requests.

### Credentials and funding

The script requires two funded testnet identities stored in the Stellar CLI
credential store (`~/.config/stellar` or equivalent, gitignored):

| Identity | Role | Requirement |
|---|---|---|
| `Source` (e.g. `earnproof-admin`) | Admin — signs `admin_revoke_proof` when `-AdminRevoke` is used | Must match the `admin` field in the manifest and on-chain |
| `IssuerSource` (e.g. `earnproof-issuer`) | Issuer — signs `register_proof` and `revoke_proof` | Address must be registered and active in issuer-registry |

If both roles share the same account, omit `-IssuerSource` and the script uses
`-Source` for both.

Fund both accounts from the Stellar testnet friendbot:

```bash
curl "https://friendbot.stellar.org?addr=<your-testnet-public-key>"
```

Each run submits at most three transactions (register, revoke, one preflight
read). On Stellar testnet the fee per transaction is typically 100 stroops
(0.00001 XLM). A funded account with 10 XLM covers thousands of runs.

### Preflight only (no transactions)

```powershell
pwsh -File scripts/smoke-proof-lifecycle.ps1 `
  -Source earnproof-admin `
  -IssuerSource earnproof-issuer `
  -PreflightOnly
```

The preflight mode confirms:

- Stellar CLI is installed and reachable
- Manifest contract IDs are valid and load correctly
- The on-chain admin address matches the manifest
- The issuer identity address is registered and active in issuer-registry
- Schema version 1 is approved in protocol-config
- The protocol is not paused

No transactions are submitted. Exit code 0 means the deployment is healthy.

### Full lifecycle — issuer revocation path (default)

```powershell
pwsh -File scripts/smoke-proof-lifecycle.ps1 `
  -Source earnproof-admin `
  -IssuerSource earnproof-issuer
```

Submits two transactions: `register_proof` (signed by issuer), then
`revoke_proof` (signed by issuer). Verifies valid status after registration and
invalid/revoked status after revocation.

### Full lifecycle — admin revocation path

```powershell
pwsh -File scripts/smoke-proof-lifecycle.ps1 `
  -Source earnproof-admin `
  -IssuerSource earnproof-issuer `
  -AdminRevoke
```

Same as above but uses `admin_revoke_proof` (signed by the admin identity)
instead of `revoke_proof`.

### Using a custom manifest

```powershell
pwsh -File scripts/smoke-proof-lifecycle.ps1 `
  -Manifest scripts/deployment-manifest.testnet.json `
  -Source earnproof-admin `
  -IssuerSource earnproof-issuer
```

### Result artifact

The script writes a secret-free result artifact to
`scripts/smoke-proof-lifecycle-result.json` (gitignored). The artifact records:

- Run ID (timestamp-based), network, source/issuer identity names
- Deployed contract IDs and Stellar Expert explorer links
- Transaction hashes and explorer links for every submitted transaction
- Synthetic proof values (derived from the run ID, not real data)
- `outcome`: `"PASS"` or `"FAIL"` with the error message on failure

A failure artifact is always written, even on error, so the exact failing step
is preserved for diagnosis.

### Cleanup

Proofs registered by this script are permanently stored on testnet (Stellar
has no on-chain delete). Each run generates a unique proof ID from its
timestamp so reruns cannot conflict. Revoked proofs remain in storage but are
invalid; they do not affect other proofs or operational state.

To minimise on-chain footprint, do not run the script repeatedly with no
diagnostic purpose.

### CI safety

The smoke script is intentionally excluded from CI. Running live testnet
transactions on pull requests from untrusted forks would expose admin/issuer
credentials. The CI workflow (`ci.yml`) runs only `cargo fmt`, `cargo clippy`,
`cargo test`, and `cargo build` — no live network calls.

## Running Tests

```powershell
pwsh -NonInteractive -File scripts\verify-manifest.tests.ps1
```

Tests cover offline validation, live happy-path, admin mismatches, paused state,
schema approval failures, malformed CLI output, timeouts, and transient RPC retries.
No real network calls are made — all live checks use mock functions.
