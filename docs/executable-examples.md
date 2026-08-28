# Executable Contract Invocation Examples

This guide explains how to run and understand the executable documentation examples that demonstrate contract invocation patterns for all three EarnProof contracts.

## Overview

The EarnProof contracts repository includes comprehensive documentation examples that are **executable and validated**. These examples:

- Demonstrate real-world contract invocation patterns
- Use synthetic identifiers (e.g., `"test-issuer-123"`, `"proof-id-456"`)
- Run in a local Soroban sandbox environment
- Are automatically validated in CI to prevent documentation drift
- Fail if method signatures or expected behaviors change

## Running Examples Locally

### Run All Documentation Examples

```bash
cargo test --doc --workspace
```

This command runs all examples across all contracts and displays:
- Number of examples found
- Pass/fail status for each example
- Execution time

### Run Examples for a Specific Contract

Run examples for the Protocol Config contract:
```bash
cargo test --doc protocol_config
```

Run examples for the Issuer Registry contract:
```bash
cargo test --doc issuer_registry
```

Run examples for the Proof Registry contract:
```bash
cargo test --doc proof_registry
```

Run cross-contract integration examples:
```bash
cargo test --doc integration
```

### Run a Single Example by Name

```bash
cargo test --doc example_initialize_protocol
cargo test --doc example_register_issuer
cargo test --doc example_end_to_end_workflow
```

### Run Examples with Output

To see println! output and more details:

```bash
cargo test --doc --workspace -- --nocapture
```

## Example Organization

All examples are located in `tests/doc-examples/`:

- **`protocol_config.rs`** — Protocol-level operations (initialization, schema management, pause controls, admin changes)
- **`issuer_registry.rs`** — Issuer lifecycle management (registration, status transitions, metadata updates, address rotation)
- **`proof_registry.rs`** — Proof lifecycle management (registration, revocation, validity checks)
- **`integration.rs`** — Cross-contract workflows and error scenarios

Each file contains markdown documentation with embedded Rust examples that:
1. Set up a test environment with `Env::default()`
2. Mock all authentication with `env.mock_all_auths()`
3. Register contracts and create clients
4. Invoke contract methods with synthetic data
5. Assert expected outcomes

## Key Patterns in Examples

### Synthetic Identifiers

All examples use synthetic, clearly-named identifiers that represent hashes:

```rust
// Protocol config admin - represents a Stellar account
let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR");

// Issuer ID hash - represents sha256("test-issuer-123")
let issuer_id_hash = BytesN::from_array(&env, &[1u8; 32]);

// Proof ID hash - represents sha256("proof-id-456")
let proof_id_hash = BytesN::from_array(&env, &[10u8; 32]);

// Commitment hash - represents sha256(credential_payload)
let commitment_hash = BytesN::from_array(&env, &[11u8; 32]);
```

### Authorization Patterns

All examples use `env.mock_all_auths()` to simulate authorization without requiring real Stellar signatures:

```rust
let env = Env::default();
env.mock_all_auths(); // Allows all addresses to pass require_auth() checks

// Now all contract calls requiring authorization will succeed
client.initialize(&admin);
client.pause();
client.register_issuer(&issuer_id_hash, &issuer_address, &metadata_hash);
```

### Lifecycle Patterns

Examples demonstrate state transitions:

**Issuer Lifecycle:**
```rust
// Register issuer in Active state
client.register_issuer(&issuer_id_hash, &issuer_address, &metadata_hash);
assert!(client.is_active_issuer(&issuer_id_hash));

// Suspend issuer
client.suspend_issuer(&issuer_id_hash);
assert!(!client.is_active_issuer(&issuer_id_hash));

// Reactivate issuer
client.reactivate_issuer(&issuer_id_hash);
assert!(client.is_active_issuer(&issuer_id_hash));

// Revoke issuer (terminal state)
client.revoke_issuer(&issuer_id_hash);
assert!(!client.is_active_issuer(&issuer_id_hash));
```

**Proof Lifecycle:**
```rust
// Register proof in Active state
client.register_proof(&proof_id_hash, &commitment_hash, &issuer_address, &schema_version, &expires_at);
assert!(client.is_valid_proof(&proof_id_hash));

// Revoke proof (terminal state)
client.revoke_proof(&proof_id_hash);
assert!(client.is_revoked(&proof_id_hash));
assert!(!client.is_valid_proof(&proof_id_hash));
```

## Example Highlights

### Protocol Config Examples

1. **Initialization** — Set up protocol with admin and default state
2. **Schema Approval** — Approve schema versions for proof registration
3. **Pause Protocol** — Pause and unpause to control proof registration
4. **Change Admin** — Transfer admin responsibilities
5. **Deprecate Schema** — Mark schema versions as deprecated

### Issuer Registry Examples

1. **Initialization** — Set up issuer registry with admin
2. **Register Issuer** — Register an issuer with ID hash, address, and metadata
3. **Suspend/Reactivate** — Demonstrate temporary suspension and reactivation
4. **Revoke Issuer** — Revoke issuer (terminal state)
5. **Update Metadata** — Update issuer's metadata hash
6. **Rotate Address** — Change issuer's signing address

### Proof Registry Examples

1. **Initialization** — Set up proof registry with cross-contract references
2. **Register Proof** — Register proof with validation checks
3. **Issuer Revocation** — Issuer revokes their own proof
4. **Admin Revocation** — Admin revokes a proof for compliance
5. **Validity Checks** — Check proof status and expiration

### Integration Examples

1. **End-to-End Workflow** — Complete flow from initialization through proof registration
2. **Paused Protocol Blocks Registration** — Demonstrates error when protocol is paused
3. **Suspended Issuer Blocks Registration** — Demonstrates error when issuer is inactive
4. **Unapproved Schema Blocks Registration** — Demonstrates error with unapproved schema version

## Error Cases

Examples demonstrate that CI fails if:

- Method signatures change
- Return values differ from expected
- Status transitions are broken
- Authorization requirements change
- Cross-contract validations are removed

All error-case examples use `#[should_panic(expected = "...")]` to verify that operations fail with expected error messages.

## CI Integration

Documentation examples run automatically in CI via:

```bash
cargo test --doc --workspace
```

This ensures:
- Drift detection: If a contract method signature or behavior changes, related examples fail immediately
- Documentation accuracy: Examples are guaranteed to work as written
- Regression prevention: New contributors cannot accidentally break documented patterns

## Best Practices for Running Examples

1. **Run all examples before committing:**
   ```bash
   cargo test --doc --workspace
   ```

2. **Run specific contract examples when working on that contract:**
   ```bash
   cargo test --doc issuer_registry
   ```

3. **Use `--nocapture` to debug example behavior:**
   ```bash
   cargo test --doc example_register_issuer -- --nocapture
   ```

4. **Update examples when changing contract APIs:**
   - If you change a method signature, update the corresponding examples
   - If you change expected behavior, update assertions
   - If you add a new method, add a corresponding example

## Related Documentation

- [Backend Integration Guide](./backend-integration.md) — Contract method signatures and parameter types
- [Storage Model Reference](./storage-model.md) — Data storage, TTL policies, and lifecycle events
- [Main README](../README.md) — Project overview and quick setup
