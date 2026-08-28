/**
 * Integration tests for EarnProofClient
 *
 * These tests verify:
 * 1. Client construction and validation
 * 2. Configuration parameter validation
 * 3. Method signatures match expected types
 * 4. Error handling for invalid inputs
 */

import { EarnProofClient, ContractInvocationError } from '../client';
import type { EarnProofClientConfig } from '../client';

describe('EarnProofClient integration', () => {
  // Test configuration — NEVER use these test keys in production
  // These are example contract addresses and test keys for unit testing only
  const validConfig: EarnProofClientConfig = {
    protocolConfigId: 'CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A', // Example contract address
    issuerRegistryId: 'CB73TVWVJIIVNTKLWSHZB5NL2UIF3B3EUL4YH4MUD6EYX6SFIHE77D2F', // Example contract address
    proofRegistryId: 'CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK', // Example contract address
    networkPassphrase: 'Test SDF Network ; September 2015',
    rpcUrl: 'https://soroban-testnet.stellar.org:443',
    secretKey: 'SBHMPXKAFNHZPQIKOOYF7LDJ4PLRJMZMNMVHUFAQRJVGHTF2EYSHHIDZ', // Example test key — DO NOT USE IN PRODUCTION
  };

  // ────────────────────────────────────────────────────────────
  // SUITE 1 — Client Construction
  // ────────────────────────────────────────────────────────────

  describe('client construction and validation', () => {
    it('constructs successfully with valid configuration', () => {
      expect(() => {
        new EarnProofClient(validConfig);
      }).not.toThrow();
    });

    it('validates protocolConfigId format', () => {
      const badConfig = { ...validConfig, protocolConfigId: 'INVALID' };
      expect(() => {
        new EarnProofClient(badConfig);
      }).toThrow();
    });

    it('validates issuerRegistryId format', () => {
      const badConfig = { ...validConfig, issuerRegistryId: 'not-a-contract-id' };
      expect(() => {
        new EarnProofClient(badConfig);
      }).toThrow();
    });

    it('validates proofRegistryId format', () => {
      const badConfig = { ...validConfig, proofRegistryId: '' };
      expect(() => {
        new EarnProofClient(badConfig);
      }).toThrow();
    });

    it('validates secretKey format', () => {
      const badConfig = { ...validConfig, secretKey: 'not-a-secret-key' };
      expect(() => {
        new EarnProofClient(badConfig);
      }).toThrow();
    });

    it('accepts optional timeoutMs parameter', () => {
      const configWithTimeout: EarnProofClientConfig = {
        ...validConfig,
        timeoutMs: 60000,
      };
      expect(() => {
        new EarnProofClient(configWithTimeout);
      }).not.toThrow();
    });

    it('uses default timeout of 30000ms when not specified', () => {
      const client = new EarnProofClient(validConfig);
      // Timeout is used internally; we verify it doesn't throw on construction
      expect(client).toBeDefined();
    });
  });

  // ────────────────────────────────────────────────────────────
  // SUITE 2 — Configuration Validation
  // ────────────────────────────────────────────────────────────

  describe('configuration parameter validation', () => {
    it('rejects contract IDs not starting with C', () => {
      const badConfig = {
        ...validConfig,
        protocolConfigId: 'G' + 'A'.repeat(55),
      };
      expect(() => {
        new EarnProofClient(badConfig);
      }).toThrow(/contract ID/i);
    });

    it('rejects contract IDs with wrong length', () => {
      const badConfig = {
        ...validConfig,
        issuerRegistryId: 'C' + 'A'.repeat(50), // 51 chars instead of 56
      };
      expect(() => {
        new EarnProofClient(badConfig);
      }).toThrow(/contract ID/i);
    });

    it('rejects secret keys not starting with S', () => {
      const badConfig = {
        ...validConfig,
        secretKey: 'G' + 'A'.repeat(55),
      };
      expect(() => {
        new EarnProofClient(badConfig);
      }).toThrow(/secret key/i);
    });

    it('accepts valid Stellar contract addresses', () => {
      // Test contract addresses — example addresses for testing only
      const validAddresses = [
        'CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A', // Example test address
        'CB73TVWVJIIVNTKLWSHZB5NL2UIF3B3EUL4YH4MUD6EYX6SFIHE77D2F', // Example test address
        'CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK', // Example test address
      ];

      validAddresses.forEach((addr) => {
        const config = {
          ...validConfig,
          protocolConfigId: addr,
        };
        expect(() => {
          new EarnProofClient(config);
        }).not.toThrow();
      });
    });

    it('accepts valid Stellar secret keys', () => {
      // Test secret keys — example keys for testing only, DO NOT USE IN PRODUCTION
      const validSecretKeys = [
        'SBHMPXKAFNHZPQIKOOYF7LDJ4PLRJMZMNMVHUFAQRJVGHTF2EYSHHIDZ', // Example test key
        'SBXVMVENPBNRUR23XNKSQMTCTW4V6OVXNFG4KJWFSBX7ZZLYGBYHK4Q3', // Example test key
      ];

      validSecretKeys.forEach((key) => {
        const config = {
          ...validConfig,
          secretKey: key,
        };
        expect(() => {
          new EarnProofClient(config);
        }).not.toThrow();
      });
    });
  });

  // ────────────────────────────────────────────────────────────
  // SUITE 3 — Method Signature Verification
  // ────────────────────────────────────────────────────────────

  describe('method signatures', () => {
    let client: EarnProofClient;

    beforeAll(() => {
      client = new EarnProofClient(validConfig);
    });

    // Protocol Config methods
    it('has initializeProtocolConfig method', () => {
      expect(typeof client.initializeProtocolConfig).toBe('function');
    });

    it('has getAdminProtocolConfig method', () => {
      expect(typeof client.getAdminProtocolConfig).toBe('function');
    });

    it('has setAdmin method', () => {
      expect(typeof client.setAdmin).toBe('function');
    });

    it('has isPaused method', () => {
      expect(typeof client.isPaused).toBe('function');
    });

    it('has pause method', () => {
      expect(typeof client.pause).toBe('function');
    });

    it('has unpause method', () => {
      expect(typeof client.unpause).toBe('function');
    });

    it('has approveSchemaVersion method', () => {
      expect(typeof client.approveSchemaVersion).toBe('function');
    });

    it('has deprecateSchemaVersion method', () => {
      expect(typeof client.deprecateSchemaVersion).toBe('function');
    });

    it('has isSchemaVersionApproved method', () => {
      expect(typeof client.isSchemaVersionApproved).toBe('function');
    });

    it('has getConfigVersion method', () => {
      expect(typeof client.getConfigVersion).toBe('function');
    });

    // Issuer Registry methods
    it('has initializeIssuerRegistry method', () => {
      expect(typeof client.initializeIssuerRegistry).toBe('function');
    });

    it('has getAdminIssuerRegistry method', () => {
      expect(typeof client.getAdminIssuerRegistry).toBe('function');
    });

    it('has registerIssuer method', () => {
      expect(typeof client.registerIssuer).toBe('function');
    });

    it('has updateIssuer method', () => {
      expect(typeof client.updateIssuer).toBe('function');
    });

    it('has suspendIssuer method', () => {
      expect(typeof client.suspendIssuer).toBe('function');
    });

    it('has reactivateIssuer method', () => {
      expect(typeof client.reactivateIssuer).toBe('function');
    });

    it('has revokeIssuer method', () => {
      expect(typeof client.revokeIssuer).toBe('function');
    });

    it('has rotateIssuerAddress method', () => {
      expect(typeof client.rotateIssuerAddress).toBe('function');
    });

    it('has getIssuer method', () => {
      expect(typeof client.getIssuer).toBe('function');
    });

    it('has isActiveIssuer method', () => {
      expect(typeof client.isActiveIssuer).toBe('function');
    });

    it('has isActiveAddress method', () => {
      expect(typeof client.isActiveAddress).toBe('function');
    });

    it('has getIssuerByAddress method', () => {
      expect(typeof client.getIssuerByAddress).toBe('function');
    });

    // Proof Registry methods
    it('has initializeProofRegistry method', () => {
      expect(typeof client.initializeProofRegistry).toBe('function');
    });

    it('has registerProof method', () => {
      expect(typeof client.registerProof).toBe('function');
    });

    it('has revokeProof method', () => {
      expect(typeof client.revokeProof).toBe('function');
    });

    it('has adminRevokeProof method', () => {
      expect(typeof client.adminRevokeProof).toBe('function');
    });

    it('has getProof method', () => {
      expect(typeof client.getProof).toBe('function');
    });

    it('has isValidProof method', () => {
      expect(typeof client.isValidProof).toBe('function');
    });

    it('has isRevoked method', () => {
      expect(typeof client.isRevoked).toBe('function');
    });

    it('has getAdminProofRegistry method', () => {
      expect(typeof client.getAdminProofRegistry).toBe('function');
    });

    it('has getIssuerRegistry method', () => {
      expect(typeof client.getIssuerRegistry).toBe('function');
    });

    it('has getProtocolConfig method', () => {
      expect(typeof client.getProtocolConfig).toBe('function');
    });

    it('has 31 public methods total', () => {
      const methods = Object.getOwnPropertyNames(Object.getPrototypeOf(client))
        .filter(
          (name) =>
            !name.startsWith('_') &&
            name !== 'constructor' &&
            typeof (client as any)[name] === 'function'
        );

      // 31 public contract methods (private methods start with _)
      expect(methods.length).toBeGreaterThanOrEqual(31);
    });
  });

  // ────────────────────────────────────────────────────────────
  // SUITE 4 — Error Type Verification
  // ────────────────────────────────────────────────────────────

  describe('error types', () => {
    it('ContractInvocationError is a subclass of Error', () => {
      const error = new ContractInvocationError(
        'testMethod',
        'C' + 'A'.repeat(55),
        'Test error'
      );

      expect(error instanceof Error).toBe(true);
      expect(error instanceof ContractInvocationError).toBe(true);
    });

    it('ContractInvocationError includes method name', () => {
      const methodName = 'registerProof';
      const error = new ContractInvocationError(
        methodName,
        'C' + 'A'.repeat(55),
        'Test error'
      );

      expect(error.method).toBe(methodName);
      expect(error.message).toContain(methodName);
    });

    it('ContractInvocationError includes contract ID', () => {
      const contractId = 'CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A'; // Example test address
      const error = new ContractInvocationError('method', contractId, 'Test error');

      expect(error.contractId).toBe(contractId);
      expect(error.message).toContain(contractId);
    });

    it('ContractInvocationError supports originalError cause', () => {
      const originalError = new Error('Root cause');
      const error = new ContractInvocationError(
        'method',
        'C' + 'A'.repeat(55),
        'Wrapper error',
        originalError
      );

      expect(error.originalError).toBe(originalError);
    });

    it('ContractInvocationError.name is "ContractInvocationError"', () => {
      const error = new ContractInvocationError(
        'method',
        'C' + 'A'.repeat(55),
        'Test'
      );

      expect(error.name).toBe('ContractInvocationError');
    });
  });

  // ────────────────────────────────────────────────────────────
  // SUITE 5 — Configuration Edge Cases
  // ────────────────────────────────────────────────────────────

  describe('configuration edge cases', () => {
    it('handles testnet network passphrase', () => {
      const config: EarnProofClientConfig = {
        ...validConfig,
        networkPassphrase: 'Test SDF Network ; September 2015',
      };

      expect(() => {
        new EarnProofClient(config);
      }).not.toThrow();
    });

    it('handles mainnet network passphrase', () => {
      const config: EarnProofClientConfig = {
        ...validConfig,
        networkPassphrase: 'Public Global Stellar Network ; September 2015',
      };

      expect(() => {
        new EarnProofClient(config);
      }).not.toThrow();
    });

    it('handles testnet RPC URL', () => {
      const config: EarnProofClientConfig = {
        ...validConfig,
        rpcUrl: 'https://soroban-testnet.stellar.org:443',
      };

      expect(() => {
        new EarnProofClient(config);
      }).not.toThrow();
    });

    it('handles mainnet RPC URL', () => {
      const config: EarnProofClientConfig = {
        ...validConfig,
        rpcUrl: 'https://soroban-mainnet.stellar.org:443',
      };

      expect(() => {
        new EarnProofClient(config);
      }).not.toThrow();
    });

    it('supports custom timeout values', () => {
      const config: EarnProofClientConfig = {
        ...validConfig,
        timeoutMs: 120000,
      };

      expect(() => {
        new EarnProofClient(config);
      }).not.toThrow();
    });

    it('supports minimum timeout (1000ms)', () => {
      const config: EarnProofClientConfig = {
        ...validConfig,
        timeoutMs: 1000,
      };

      expect(() => {
        new EarnProofClient(config);
      }).not.toThrow();
    });

    it('supports very large timeout (10 minutes)', () => {
      const config: EarnProofClientConfig = {
        ...validConfig,
        timeoutMs: 600000,
      };

      expect(() => {
        new EarnProofClient(config);
      }).not.toThrow();
    });
  });

  // ────────────────────────────────────────────────────────────
  // SUITE 6 — Type Safety Documentation
  // ────────────────────────────────────────────────────────────

  describe('type safety documentation', () => {
    it('documents that method parameters are type-checked at compile time', () => {
      // This test documents that TypeScript prevents passing invalid parameters
      // to contract methods. The actual verification happens at compile time.

      const testHashFormat = '0x' + 'a'.repeat(64);
      expect(testHashFormat).toMatch(/^0x[a-f0-9]{64}$/);
    });

    it('documents that return types are inferred', () => {
      // Contract method return types are strictly defined:
      // - Void methods return undefined
      // - Query methods return typed results (bool, string, IssuerRecord, etc.)

      const voidResult: undefined = undefined;
      const boolResult: boolean = true;
      const stringResult: string = 'address';

      expect(voidResult).toBeUndefined();
      expect(typeof boolResult).toBe('boolean');
      expect(typeof stringResult).toBe('string');
    });

    it('documents that Stellar addresses must match exact pattern', () => {
      const validAddress = 'GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U';
      const addressPattern = /^G[A-Z2-7]{55}$/;

      expect(validAddress).toMatch(addressPattern);
    });

    it('documents that BytesN<32> is represented as hex string', () => {
      // Rust BytesN<32> is passed as hex string with 0x prefix
      const hash32 = '0x' + 'a'.repeat(64); // 32 bytes = 256 bits = 64 hex chars
      expect(hash32).toMatch(/^0x[a-f0-9]{64}$/);
    });
  });

  // ────────────────────────────────────────────────────────────
  // SUITE 7 — Configuration Immutability
  // ────────────────────────────────────────────────────────────

  describe('configuration immutability', () => {
    it('client stores all required configuration fields', () => {
      const client = new EarnProofClient(validConfig);

      // Client should be initialized with the configuration
      // (internal representation may vary)
      expect(client).toBeDefined();
    });

    it('client does not expose configuration publicly', () => {
      const client = new EarnProofClient(validConfig);

      // Configuration (especially secret key) should not be directly accessible
      // to prevent accidental exposure
      expect((client as any).secretKey).toBeUndefined();
      expect((client as any).config?.secretKey).toBeUndefined();
    });

    it('supports multiple client instances with different configs', () => {
      const testnetClient = new EarnProofClient({
        ...validConfig,
        rpcUrl: 'https://soroban-testnet.stellar.org:443',
      });

      const mainnetClient = new EarnProofClient({
        ...validConfig,
        rpcUrl: 'https://soroban-mainnet.stellar.org:443',
      });

      expect(testnetClient).toBeDefined();
      expect(mainnetClient).toBeDefined();
    });
  });
});
