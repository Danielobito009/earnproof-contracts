# Testing Setup for Contract Bindings

This document describes the test suite for the TypeScript contract bindings.

## Test Files

### 1. `artifacts/bindings/__tests__/bindings.test.ts` (700+ lines)

**Fixture and compile-time tests for type safety.**

13 test suites covering:

- **Provenance Verification (6 tests)**
  - File exists and is valid JSON
  - Contains sourceCommit, generatedAt, stellarCliVersion
  - Has correct contract names and WASM hashes

- **Type Shapes (4 tests)**
  - EarnProofClientConfig accepts required fields
  - Type validation at compile time
  - Shared types present (IssuerStatus, ProofStatus, IssuerRecord, ProofRecord)

- **Protocol Config Fixtures (10 tests)**
  - Parameter and result types for all 10 functions

- **Issuer Registry Fixtures (12 tests)**
  - Parameter and result types for all 12 functions

- **Proof Registry Fixtures (10 tests)**
  - Parameter and result types for all 10 functions

- **Spec Files (3 tests)**
  - Spec file exists for each contract
  - Spec files are valid JSON

- **Binding Files (8 tests)**
  - types.ts, client.ts, index.ts exist
  - Have AUTO-GENERATED headers
  - Include regeneration instructions

- **API Surface Coverage (2 tests)**
  - 31 parameter interfaces exported
  - 31 result type aliases exported

- **Determinism (3 tests)**
  - Generation is deterministic
  - Provenance tracks source commit
  - WASM hashes are reproducible

- **Error Types (1 test)**
  - ContractInvocationError structure

- **Security (3 tests)**
  - No hardcoded contract IDs
  - No hardcoded secret keys
  - Configuration is runtime-based

- **Documentation Quality (3 tests)**
  - JSDoc comments present
  - Method documentation
  - Header comments

- **Type Export Completeness (2 tests)**
  - Types re-exported from client
  - Index exports properly

### 2. `artifacts/bindings/__tests__/client.integration.test.ts` (500+ lines)

**Integration tests for EarnProofClient class.**

7 test suites covering:

- **Client Construction (7 tests)**
  - Constructs with valid config
  - Validates contract ID format
  - Validates secret key format
  - Optional timeout parameter

- **Configuration Validation (7 tests)**
  - Rejects invalid contract addresses
  - Rejects invalid secret keys
  - Accepts valid Stellar addresses and keys

- **Method Signatures (31 tests)**
  - One test per public contract method
  - Verifies method exists and is callable
  - Total method count verification

- **Error Types (5 tests)**
  - ContractInvocationError structure
  - Includes method name and contract ID
  - Supports originalError cause
  - Proper error naming

- **Configuration Edge Cases (7 tests)**
  - Testnet configuration
  - Mainnet configuration
  - Custom timeout values

- **Type Safety Documentation (4 tests)**
  - Parameter type checking
  - Return type inference
  - Address format validation
  - BytesN<32> representation

- **Configuration Immutability (3 tests)**
  - Configuration is stored internally
  - Secret key not exposed
  - Multiple independent instances

## Running Tests

### Prerequisites

```bash
npm install --save-dev jest ts-jest @types/jest
```

### Test Scripts

Add to `package.json`:

```json
{
  "scripts": {
    "test:bindings": "jest --config jest.config.js",
    "test:bindings:watch": "jest --config jest.config.js --watch",
    "test:bindings:coverage": "jest --config jest.config.js --coverage",
    "test:bindings:verbose": "jest --config jest.config.js --verbose"
  }
}
```

### Running Tests

```bash
# Run all binding tests
npm run test:bindings

# Run in watch mode (re-run on file changes)
npm run test:bindings:watch

# Run with coverage report
npm run test:bindings:coverage

# Verbose output
npm run test:bindings:verbose
```

### Running Specific Test Suite

```bash
# Run only provenance tests
npm run test:bindings -- --testNamePattern="provenance"

# Run only type shape tests
npm run test:bindings -- --testNamePattern="type shapes"

# Run only client construction tests
npm run test:bindings -- --testNamePattern="client construction"
```

## Test Coverage

Expected coverage (from jest.config.js):

- Statements: 80%
- Branches: 80%
- Functions: 80%
- Lines: 80%

To view coverage report:

```bash
npm run test:bindings:coverage
open coverage/index.html  # macOS/Linux
# or
start coverage\index.html # Windows
```

## Continuous Integration

Tests are run automatically in CI via `.github/workflows/bindings.yml`:

```yaml
typecheck-bindings:
  name: TypeScript Compile Check
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Setup Node.js
      uses: actions/setup-node@v4
    - name: Install dependencies
      run: npm ci
    - name: Run binding tests
      run: npm run test:bindings
```

## Test Categories

### Compile-Time Tests

Tests that verify TypeScript type safety. These use type annotations
to verify that invalid code would not compile:

```typescript
it('type safety example', () => {
  // This compiles if types are correct
  const config: EarnProofClientConfig = { /* ... */ };
  
  // This would NOT compile if types are wrong:
  // const badConfig: EarnProofClientConfig = { /* missing field */ };
  
  expect(config).toBeDefined();
});
```

### Fixture Tests

Tests that verify representative data structures match expected shapes:

```typescript
it('IssuerRecord has all required fields', () => {
  const record: IssuerRecord = {
    issuer_id_hash: '0x...',
    issuer_address: 'G...',
    metadata_hash: '0x...',
    status: 'Active',
    created_at: 1234567890n,
    updated_at: 1234567890n,
  };

  expect(record.issuer_id_hash).toBeDefined();
  // ... etc
});
```

### Integration Tests

Tests that verify client construction, configuration validation,
and method availability:

```typescript
it('client constructs with valid config', () => {
  expect(() => {
    new EarnProofClient(validConfig);
  }).not.toThrow();
});
```

### Security Tests

Tests that verify no hardcoded secrets:

```typescript
it('types.ts does not contain hardcoded contract IDs', () => {
  const content = fs.readFileSync(typesPath, 'utf8');
  const contractIdPattern = /C[A-Z2-7]{55}/g;
  expect(content.match(contractIdPattern) || []).toHaveLength(0);
});
```

## Debugging Tests

### Run Single Test File

```bash
npm run test:bindings -- bindings.test.ts
```

### Run Single Test Suite

```bash
npm run test:bindings -- -t "provenance"
```

### Debug Mode

```bash
node --inspect-brk node_modules/.bin/jest --runInBand
# Open chrome://inspect in Chrome DevTools
```

### Verbose Output

```bash
npm run test:bindings:verbose
```

## Test Maintenance

### After Modifying Types

If you change types in `artifacts/bindings/types.ts`:

1. Update relevant fixture tests in `bindings.test.ts`
2. Run `npm run test:bindings` to verify
3. Update documentation if API changes

### After Adding Contract Methods

If you add new functions to contracts and regenerate bindings:

1. A new parameter interface and result type are created
2. A new client method is added
3. Update `client.integration.test.ts` to test new method
4. Run `npm run test:bindings` to verify

### After Changing Generation Script

If you modify `scripts/generate-bindings.ps1`:

1. Provenance tests may change (e.g., new fields)
2. Update provenance tests in `bindings.test.ts`
3. Run `npm run test:bindings` to verify
4. Update `BINDINGS_IMPLEMENTATION.md` if needed

## Common Test Patterns

### Testing Async Methods

```typescript
it('method returns promise', async () => {
  const client = new EarnProofClient(validConfig);
  
  // Methods are async (return Promises)
  const result = client.isPaused({});
  expect(result instanceof Promise).toBe(true);
});
```

### Testing Error Conditions

```typescript
it('rejects invalid config', () => {
  const badConfig = { ...validConfig, secretKey: 'INVALID' };
  
  expect(() => {
    new EarnProofClient(badConfig);
  }).toThrow(/secret key/i);
});
```

### Testing Type Interfaces

```typescript
it('parameter interface is correct', () => {
  const params: RegisterProofParams = {
    proof_id_hash: '0x...',
    commitment_hash: '0x...',
    issuer_address: 'G...',
    schema_version: 1,
    expires_at: 1234567890n,
  };
  
  expect(params).toBeDefined();
  // Type checking happens at compile time
});
```

## References

- [Jest Documentation](https://jestjs.io/)
- [TypeScript Jest Setup](https://kulshekhar.github.io/ts-jest/)
- [Testing TypeScript](https://basarat.gitbook.io/typescript/type-system/testing)
