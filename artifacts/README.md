# EarnProof Artifacts

This directory contains generated build artifacts for the EarnProof contracts.

## Contents

### `bindings/`

**TypeScript/JavaScript type definitions and typed client for contract interaction.**

- `types.ts` — Complete type definitions for all contracts
  - Shared types (`IssuerStatus`, `ProofStatus`, `IssuerRecord`, `ProofRecord`)
  - Parameter types for every public contract function
  - Return types for every public contract function
  - Provenance metadata

- `client.ts` — Type-safe contract client
  - `EarnProofClient` class with methods for all contract functions
  - Automatic serialization/deserialization via Stellar SDK
  - Transaction simulation, signing, and submission
  - Error handling with `ContractInvocationError`

- `index.ts` — Convenience re-exports

- `provenance.json` — Build metadata
  - Source commit hash
  - Generation timestamp
  - Stellar CLI version
  - WASM file hashes for all contracts

- `*-spec.json` — Per-contract specification files
  - Generated from contract WASM via `stellar contract inspect`
  - Contains full contract interface metadata

## Usage

### In NestJS Backend

```typescript
import { EarnProofClient, RegisterProofParams } from '@earnproof/contracts/artifacts/bindings';

const client = new EarnProofClient({
  protocolConfigId: process.env.PROTOCOL_CONFIG_ID,
  issuerRegistryId: process.env.ISSUER_REGISTRY_ID,
  proofRegistryId: process.env.PROOF_REGISTRY_ID,
  networkPassphrase: process.env.NETWORK_PASSPHRASE,
  rpcUrl: process.env.SOROBAN_RPC_URL,
  secretKey: process.env.SIGNER_SECRET_KEY,
});

// Type-safe contract invocation
await client.registerProof({
  proof_id_hash: sha256(proofId),
  commitment_hash: sha256(payload),
  issuer_address: issuerAddress,
  schema_version: 1,
  expires_at: BigInt(expiresAt),
});
```

## Regenerating Bindings

After modifying contract interfaces:

```bash
# Windows PowerShell
./scripts/generate-bindings.ps1 -Network testnet

# macOS/Linux
pwsh ./scripts/generate-bindings.ps1 -Network testnet

# Commit changes
git add artifacts/bindings/
git commit -m "chore: regenerate contract bindings after interface change"
```

## CI Stale Detection

The `.github/workflows/bindings.yml` workflow automatically detects stale bindings:
- Fails if contract interfaces change without regenerating bindings
- Shows exact diff of what changed
- Requires bindings commit before PR merge

## Security

- Generated bindings should **never contain hardcoded secrets**
- Contract addresses are configurable via environment variables
- Secret keys are loaded at runtime, never embedded in code
- All files are automatically generated; manual edits will be overwritten

## Licensing

Generated bindings are licensed under the same license as this repository (see `LICENSE`).
They may be consumed by the NestJS backend and published with it.

## References

- [Bindings Integration Guide](../docs/bindings-integration.md)
- [Backend Integration Guide](../docs/backend-integration.md)
- [Storage Model Reference](../docs/storage-model.md)
