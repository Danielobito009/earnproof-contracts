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

The script:

- installs the `wasm32v1-none` target if needed;
- builds optimized release WASM artifacts with `stellar contract build`;
- deploys `protocol-config`, `issuer-registry`, and `proof-registry`;
- initializes each contract;
- approves schema version `1`;
- registers the backend issuer address before proof anchoring is enabled;
- writes a manifest with contract IDs, WASM hashes, admin address, schema versions, and CLI command evidence.

## Verify Manifest

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.testnet.json
```

For the checked-in example manifest:

```powershell
.\scripts\verify-manifest.ps1 -Manifest scripts\deployment-manifest.example.json -AllowPlaceholders
```

The verifier checks the manifest shape and rejects placeholder contract IDs unless `-AllowPlaceholders` is explicitly supplied.
