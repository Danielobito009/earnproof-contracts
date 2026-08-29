# EarnProof Bindings Quick Start

## What are these bindings?

Typed TypeScript definitions and client for calling EarnProof Soroban contracts
without shell-outs to Stellar CLI. All parameters and return types are validated
at compile time.

## Installation (Backend)

```bash
npm install @stellar/stellar-sdk
```

## Configuration

Copy `.env.example` to `.env` and populate contract IDs, network passphrase, and RPC URL:

```bash
cp .env.example .env
# Edit .env with your deployment details
```

From `scripts/deployment-manifest.testnet.json`:
```json
{
  "contracts": {
    "protocolConfig": "CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A",
    "issuerRegistry": "CB73TVWVJIIVNTKLWSHZB5NL2UIF3B3EUL4YH4MUD6EYX6SFIHE77D2F",
    "proofRegistry": "CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK"
  }
}
```

## Basic Usage

```typescript
import { EarnProofClient } from './artifacts/bindings';

const client = new EarnProofClient({
  protocolConfigId: process.env.PROTOCOL_CONFIG_ID,
  issuerRegistryId: process.env.ISSUER_REGISTRY_ID,
  proofRegistryId: process.env.PROOF_REGISTRY_ID,
  networkPassphrase: process.env.NETWORK_PASSPHRASE,
  rpcUrl: process.env.SOROBAN_RPC_URL,
  secretKey: process.env.SIGNER_SECRET_KEY,
});

// Read contract state (no signing required)
const isProtocolPaused = await client.isPaused({});
console.log('Protocol paused:', isProtocolPaused);

// Register a proof (requires issuer authorization)
const proofIdHash = '0x' + sha256(proofId);
const commitmentHash = '0x' + sha256(payload);

await client.registerProof({
  proof_id_hash: proofIdHash,
  commitment_hash: commitmentHash,
  issuer_address: issuerStellarAddress,
  schema_version: 1,
  expires_at: BigInt(futureTimestamp),
});

// Query proof state
const proof = await client.getProof({ proof_id_hash: proofIdHash });
console.log('Proof status:', proof.status);
```

## Common Tasks

### Check if Proof is Valid

```typescript
const isValid = await client.isValidProof({
  proof_id_hash: proofIdHash,
});
```

### Get Issuer Record

```typescript
const issuer = await client.getIssuer({
  issuer_id_hash: issuerIdHash,
});
console.log('Issuer status:', issuer.status); // 'Active' | 'Suspended' | 'Revoked'
```

### Approve Schema Version (Admin Only)

```typescript
await client.approveSchemaVersion({
  version: 1,
});
```

### Suspend an Issuer (Admin Only)

```typescript
await client.suspendIssuer({
  issuer_id_hash: issuerIdHash,
});
```

## NestJS Service Example

```typescript
import { Injectable } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { EarnProofClient } from './artifacts/bindings';

@Injectable()
export class ContractService {
  private client: EarnProofClient;

  constructor(configService: ConfigService) {
    this.client = new EarnProofClient({
      protocolConfigId: configService.getOrThrow('PROTOCOL_CONFIG_ID'),
      issuerRegistryId: configService.getOrThrow('ISSUER_REGISTRY_ID'),
      proofRegistryId: configService.getOrThrow('PROOF_REGISTRY_ID'),
      networkPassphrase: configService.getOrThrow('NETWORK_PASSPHRASE'),
      rpcUrl: configService.getOrThrow('SOROBAN_RPC_URL'),
      secretKey: configService.getOrThrow('SIGNER_SECRET_KEY'),
    });
  }

  async verifyProof(proofIdHash: string): Promise<boolean> {
    return this.client.isValidProof({ proof_id_hash: proofIdHash });
  }

  async registerProof(params: RegisterProofParams) {
    return this.client.registerProof(params);
  }
}
```

## Error Handling

```typescript
import { ContractInvocationError } from './artifacts/bindings';

try {
  await client.registerProof(params);
} catch (err) {
  if (err instanceof ContractInvocationError) {
    console.error(`Failed to invoke ${err.method} on ${err.contractId}`);
    console.error(`Reason: ${err.message}`);
  }
}
```

## Type Safety

All methods are fully typed:

```typescript
// ✅ TypeScript knows what parameters registerProof accepts
await client.registerProof({
  proof_id_hash: '0x...',
  commitment_hash: '0x...',
  issuer_address: 'G...',
  schema_version: 1,
  expires_at: 1234567890n,
});

// ❌ TypeScript error: missing parameter
await client.registerProof({
  proof_id_hash: '0x...',
  // commitment_hash is required!
});

// ❌ TypeScript error: wrong type
await client.registerProof({
  // ...
  schema_version: '1', // must be number, not string
});
```

## Hashing Before Contract Calls

**Important:** Hash all identifiers before passing to contracts.

```typescript
import crypto from 'crypto';

function sha256(value: string): string {
  return crypto.createHash('sha256').update(value, 'utf8').digest('hex');
}

const proofIdHash = '0x' + sha256(proofId);
const issuerIdHash = '0x' + sha256(issuerId);
const commitmentHash = '0x' + sha256(credentialPayload);
const metadataHash = '0x' + sha256(publicMetadata);
```

## Contract Methods Reference

### Protocol Config

- `initializeProtocolConfig` — Initialize (admin)
- `getAdminProtocolConfig` — Get admin address
- `setAdmin` — Change admin (admin)
- `isPaused` — Check if paused
- `pause` — Pause operations (admin)
- `unpause` — Resume operations (admin)
- `approveSchemaVersion` — Approve schema (admin)
- `deprecateSchemaVersion` — Deprecate schema (admin)
- `isSchemaVersionApproved` — Check if approved
- `getConfigVersion` — Get config version counter

### Issuer Registry

- `initializeIssuerRegistry` — Initialize (admin)
- `getAdminIssuerRegistry` — Get admin address
- `registerIssuer` — Register issuer (admin)
- `updateIssuer` — Update metadata (admin)
- `suspendIssuer` — Suspend issuer (admin)
- `reactivateIssuer` — Reactivate issuer (admin)
- `revokeIssuer` — Revoke issuer (admin)
- `rotateIssuerAddress` — Rotate address (admin)
- `getIssuer` — Get issuer by ID
- `isActiveIssuer` — Check if active
- `isActiveAddress` — Check if active by address
- `getIssuerByAddress` — Get issuer by address

### Proof Registry

- `initializeProofRegistry` — Initialize (admin)
- `registerProof` — Register proof (issuer)
- `revokeProof` — Revoke proof (issuer)
- `adminRevokeProof` — Admin revoke (admin)
- `getProof` — Get proof by ID
- `isValidProof` — Check if valid
- `isRevoked` — Check if revoked
- `getAdminProofRegistry` — Get admin address
- `getIssuerRegistry` — Get issuer registry contract address
- `getProtocolConfig` — Get protocol config contract address

## Full Documentation

See `docs/bindings-integration.md` for complete guide including:
- Advanced usage patterns
- Testing strategies
- Security best practices
- Deployment guides

## Regenerating Bindings

After contract changes:

```bash
./scripts/generate-bindings.ps1 -Network testnet
git add artifacts/bindings/
git commit -m "chore: regenerate bindings"
```

## Troubleshooting

### "Invalid contract ID"
Ensure contract addresses match format: `C` followed by 55 alphanumeric characters.

### "Invalid secret key"
Secret key must start with `S` and be a valid Stellar key.

### "Network passphrase does not match"
Use exact passphrase from network config:
- Testnet: `"Test SDF Network ; September 2015"`
- Mainnet: `"Public Global Stellar Network ; September 2015"`

### Type error: Parameter does not match
All parameters must match exact types. Use `0x` prefix for hex hashes, use `BigInt(...)` for timestamps.

## Support

- Integration guide: `docs/bindings-integration.md`
- Contract reference: `docs/backend-integration.md`
- Storage model: `docs/storage-model.md`
