# NestJS Backend Contract Integration

## Overview

The NestJS backend replaces manual Stellar CLI shell-outs with typed bindings generated
from Soroban contract specs.

This document covers:
- Setup and configuration
- Typed client usage patterns
- NestJS service integration
- Error handling
- Security requirements

## Environment Variables

Never hardcode contract IDs, network IDs, or secret keys. Load all configuration from
environment variables or a secrets manager.

### Required Configuration

```bash
# Contract addresses (from deployment manifest)
PROTOCOL_CONFIG_ID=CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A
ISSUER_REGISTRY_ID=CB73TVWVJIIVNTKLWSHZB5NL2UIF3B3EUL4YH4MUD6EYX6SFIHE77D2F
PROOF_REGISTRY_ID=CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK

# Network configuration
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"  # or "Public Global Stellar Network ; September 2015"
SOROBAN_RPC_URL=https://soroban-testnet.stellar.org:443  # or mainnet RPC URL

# Signing key (load from secrets manager, NEVER commit)
SIGNER_SECRET_KEY=S...  # Keypair with signing permission
```

### Network Passphrases

```typescript
// Testnet
Networks.TESTNET_NETWORK_PASSPHRASE
// "Test SDF Network ; September 2015"

// Mainnet
Networks.PUBLIC_NETWORK_PASSPHRASE
// "Public Global Stellar Network ; September 2015"
```

### RPC Endpoints

```
Testnet:  https://soroban-testnet.stellar.org:443
Mainnet:  https://soroban-mainnet.stellar.org:443
```

## Installation

### 1. Add Stellar SDK dependency

```bash
npm install @stellar/stellar-sdk
npm install --save-dev @types/node  # if using TypeScript
```

### 2. Import bindings

```typescript
import {
  EarnProofClient,
  IssuerRecord,
  ProofRecord,
  RegisterProofParams,
  // ... other types as needed
} from '../artifacts/bindings/client';
```

## Usage Patterns

### Basic NestJS Service

```typescript
import { Injectable, OnModuleInit } from '@nestjs/common';
import { ConfigService } from '@nestjs/config';
import { EarnProofClient } from '../artifacts/bindings/client';
import type { RegisterProofParams } from '../artifacts/bindings/types';

@Injectable()
export class ContractService implements OnModuleInit {
  private client: EarnProofClient;

  constructor(private readonly configService: ConfigService) {}

  onModuleInit() {
    this.client = new EarnProofClient({
      protocolConfigId: this.configService.getOrThrow('PROTOCOL_CONFIG_ID'),
      issuerRegistryId: this.configService.getOrThrow('ISSUER_REGISTRY_ID'),
      proofRegistryId: this.configService.getOrThrow('PROOF_REGISTRY_ID'),
      networkPassphrase: this.configService.getOrThrow('NETWORK_PASSPHRASE'),
      rpcUrl: this.configService.getOrThrow('SOROBAN_RPC_URL'),
      secretKey: this.configService.getOrThrow('SIGNER_SECRET_KEY'),
      timeoutMs: this.configService.get('SOROBAN_TIMEOUT_MS', 30000),
    });
  }

  async registerProof(params: RegisterProofParams) {
    return this.client.registerProof(params);
  }

  async getProof(proofIdHash: string) {
    return this.client.getProof({ proof_id_hash: proofIdHash });
  }

  async isValidProof(proofIdHash: string) {
    return this.client.isValidProof({ proof_id_hash: proofIdHash });
  }
}
```

### Query-Only Service (Read-Only Operations)

For services that only read contract state and don't sign transactions,
create a simpler client that doesn't require a secret key:

```typescript
@Injectable()
export class ContractQueryService {
  private client: EarnProofClient;

  constructor(configService: ConfigService) {
    // Use a placeholder keypair for read-only operations
    // NestJS will fail if any write operation is attempted
    this.client = new EarnProofClient({
      protocolConfigId: configService.getOrThrow('PROTOCOL_CONFIG_ID'),
      issuerRegistryId: configService.getOrThrow('ISSUER_REGISTRY_ID'),
      proofRegistryId: configService.getOrThrow('PROOF_REGISTRY_ID'),
      networkPassphrase: configService.getOrThrow('NETWORK_PASSPHRASE'),
      rpcUrl: configService.getOrThrow('SOROBAN_RPC_URL'),
      secretKey: Keypair.random().secret(), // Safe: only for reads
    });
  }

  async checkProofValidity(proofIdHash: string): Promise<boolean> {
    return this.client.isValidProof({ proof_id_hash: proofIdHash });
  }

  async getProofRecord(proofIdHash: string) {
    return this.client.getProof({ proof_id_hash: proofIdHash });
  }
}
```

### Hashing Service Integration

The backend must hash all identifiers before passing them to contracts.
Create a helper service:

```typescript
import crypto from 'crypto';
import type { Hash32 } from '../artifacts/bindings/types';

@Injectable()
export class ContractHashingService {
  /**
   * Hash proof identifier to create proof_id_hash parameter
   */
  hashProofId(proofId: string): Hash32 {
    return this.sha256(proofId) as Hash32;
  }

  /**
   * Hash issuer identifier to create issuer_id_hash parameter
   */
  hashIssuerId(issuerId: string): Hash32 {
    return this.sha256(issuerId) as Hash32;
  }

  /**
   * Hash credential payload to create commitment_hash parameter
   */
  hashCommitment(canonicalPayload: string): Hash32 {
    return this.sha256(canonicalPayload) as Hash32;
  }

  /**
   * Hash issuer metadata to create metadata_hash parameter
   */
  hashMetadata(canonicalMetadata: string): Hash32 {
    return this.sha256(canonicalMetadata) as Hash32;
  }

  private sha256(value: string): string {
    return crypto.createHash('sha256').update(value, 'utf8').digest('hex');
  }
}
```

### Complete Example: Register Proof

```typescript
import { Controller, Post, Body } from '@nestjs/common';
import { ContractService } from './contract.service';
import { ContractHashingService } from './contract-hashing.service';
import type { RegisterProofParams } from '../artifacts/bindings/types';

@Controller('proofs')
export class ProofController {
  constructor(
    private readonly contractService: ContractService,
    private readonly hashingService: ContractHashingService,
  ) {}

  @Post('register')
  async registerProof(@Body() dto: {
    proofId: string;
    credentialPayload: string;
    issuerAddress: string;
    schemaVersion: number;
    expiresAt: bigint;
  }) {
    // Hash identifiers before sending to contract
    const params: RegisterProofParams = {
      proof_id_hash: this.hashingService.hashProofId(dto.proofId),
      commitment_hash: this.hashingService.hashCommitment(dto.credentialPayload),
      issuer_address: dto.issuerAddress,
      schema_version: dto.schemaVersion,
      expires_at: dto.expiresAt,
    };

    // Invoke contract with typed parameters
    await this.contractService.registerProof(params);

    return { success: true, proofId: dto.proofId };
  }
}
```

## Error Handling

The client throws `ContractInvocationError` for contract failures:

```typescript
import { ContractInvocationError } from '../artifacts/bindings/client';

try {
  await this.client.registerProof(params);
} catch (err) {
  if (err instanceof ContractInvocationError) {
    console.error(`Contract ${err.contractId} call failed:`, err.message);
    console.error(`Method: ${err.method}`);
    if (err.originalError) {
      console.error(`Root cause:`, err.originalError);
    }
    // Handle specific error types based on err.message
    if (err.message.includes('already registered')) {
      // Duplicate prevention
      throw new ConflictException('Proof already exists');
    }
    if (err.message.includes('not active')) {
      // Issuer status check failed
      throw new BadRequestException('Issuer is not active');
    }
    throw new InternalServerErrorException('Contract call failed');
  }
  throw err;
}
```

## Type Safety

All parameters and return values are typed at compile time:

```typescript
// ✅ Valid: matches RegisterProofParams
await client.registerProof({
  proof_id_hash: '0x1234...',
  commitment_hash: '0x5678...',
  issuer_address: 'GCATS5YOVB6...',
  schema_version: 1,
  expires_at: 1234567890n,
});

// ❌ Type error: missing required parameter
await client.registerProof({
  proof_id_hash: '0x1234...',
  // commitment_hash is required
});

// ❌ Type error: wrong type
await client.registerProof({
  proof_id_hash: 'not-a-hash',  // string, but expected hex format
  // ...
});

// ✅ Valid: return type is inferred
const record: ProofRecord = await client.getProof({
  proof_id_hash: '0x1234...',
});
console.log(record.status); // ✅ 'Active' or 'Revoked'
console.log(record.unknown); // ❌ Type error: no such field
```

## Testing

### Unit Tests (Mock Client)

```typescript
import { Test, TestingModule } from '@nestjs/testing';
import { ContractService } from './contract.service';
import { EarnProofClient } from '../artifacts/bindings/client';

describe('ContractService', () => {
  let service: ContractService;
  let mockClient: jest.Mocked<EarnProofClient>;

  beforeEach(async () => {
    mockClient = {
      registerProof: jest.fn(),
      getProof: jest.fn(),
      isValidProof: jest.fn(),
      // ... mock other methods as needed
    } as any;

    const module: TestingModule = await Test.createTestingModule({
      providers: [
        ContractService,
        {
          provide: EarnProofClient,
          useValue: mockClient,
        },
      ],
    }).compile();

    service = module.get<ContractService>(ContractService);
  });

  it('should register a proof', async () => {
    mockClient.registerProof.mockResolvedValue(undefined);

    await service.registerProof({
      proof_id_hash: '0x1234...',
      commitment_hash: '0x5678...',
      issuer_address: 'GCATS5YOVB6...',
      schema_version: 1,
      expires_at: 1234567890n,
    });

    expect(mockClient.registerProof).toHaveBeenCalledWith(
      expect.objectContaining({
        proof_id_hash: '0x1234...',
      })
    );
  });
});
```

### Integration Tests (Against Testnet)

```typescript
describe('ContractService (Integration)', () => {
  let service: ContractService;

  beforeAll(async () => {
    const configService = {
      getOrThrow: (key: string) => {
        const config = {
          PROTOCOL_CONFIG_ID: process.env.PROTOCOL_CONFIG_ID,
          ISSUER_REGISTRY_ID: process.env.ISSUER_REGISTRY_ID,
          PROOF_REGISTRY_ID: process.env.PROOF_REGISTRY_ID,
          NETWORK_PASSPHRASE: process.env.NETWORK_PASSPHRASE,
          SOROBAN_RPC_URL: process.env.SOROBAN_RPC_URL,
          SIGNER_SECRET_KEY: process.env.SIGNER_SECRET_KEY,
        };
        if (!config[key]) throw new Error(`Missing ${key}`);
        return config[key];
      },
    } as any;

    service = new ContractService(configService);
    service.onModuleInit();
  });

  it('should read protocol config state', async () => {
    const isPaused = await service.client.isPaused({});
    expect(typeof isPaused).toBe('boolean');
  });

  it.skip('should register proof (requires testnet setup)', async () => {
    // Integration test that requires deployed contracts and funded account
  });
});
```

## Regenerating Bindings

When contract interfaces change:

```bash
# Windows PowerShell
./scripts/generate-bindings.ps1 -Network testnet

# macOS/Linux
pwsh ./scripts/generate-bindings.ps1 -Network testnet

# Then commit
git add artifacts/bindings/
git commit -m "chore: regenerate contract bindings"
```

The CI workflow (`bindings.yml`) automatically detects stale bindings
and fails if contract interfaces change without regenerating bindings.

## Security Best Practices

1. **Never hardcode contract IDs, passphrases, or secret keys**
   - Always load from environment variables
   - Use a secrets manager in production

2. **Secret key management**
   - Never commit secret keys to git
   - Rotate keys regularly
   - Use separate keys for testnet and mainnet
   - Restrict key file permissions (chmod 600)

3. **Validation and sanitization**
   - All contract addresses are validated on client construction
   - Hash values must be valid 32-byte hex strings
   - Stellar addresses must match `G[A-Z2-7]{55}` pattern

4. **Transaction security**
   - Each invocation is simulated before submission
   - Signatures are cryptographically verified
   - Timeout protection against hung transactions

5. **Error handling**
   - Never log raw contract responses (may contain sensitive data)
   - Sanitize error messages before exposing to API consumers
   - Log invocation details for audit trails

## Deployment

### Testnet

```bash
export PROTOCOL_CONFIG_ID=CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A
export ISSUER_REGISTRY_ID=CB73TVWVJIIVNTKLWSHZB5NL2UIF3B3EUL4YH4MUD6EYX6SFIHE77D2F
export PROOF_REGISTRY_ID=CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK
export NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org:443
export SIGNER_SECRET_KEY=S...  # from secrets manager

npm run start
```

### Mainnet

```bash
# After independent audit review
export PROTOCOL_CONFIG_ID=C...  # mainnet contract ID
export ISSUER_REGISTRY_ID=C...  # mainnet contract ID
export PROOF_REGISTRY_ID=C...   # mainnet contract ID
export NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
export SOROBAN_RPC_URL=https://soroban-mainnet.stellar.org:443
export SIGNER_SECRET_KEY=S...  # mainnet key from HSM or secrets manager

npm run start
```

## References

- [Stellar SDK Documentation](https://developers.stellar.org/docs/build/smart-contracts)
- [Soroban RPC API](https://developers.stellar.org/docs/build/smart-contracts/rpc)
- [Contract Integration Guide](./backend-integration.md)
- [Storage Model Reference](./storage-model.md)
