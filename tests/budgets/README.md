# Resource Budget Regression Tests

This directory contains resource budget regression tests for EarnProof Soroban contracts.

## Purpose

These tests measure CPU instructions, memory usage, and WASM sizes for representative operations to detect performance regressions that could:
- Make mainnet operations unexpectedly expensive
- Exceed Soroban resource limits
- Degrade user experience with slow transactions

## Test Coverage

### Protocol Config Contract
- `initialize` - Initial contract setup
- `pause` - Protocol pause operation
- `approve_schema_version` - Schema approval with persistent storage

### Issuer Registry Contract
- `initialize` - Initial contract setup
- `register_issuer` - New issuer registration (worst case: dual index writes)
- `get_issuer` - Issuer lookup with TTL extension
- `update_issuer` - Metadata update operation
- `suspend_issuer` - Status transition to Suspended
- `revoke_issuer` - Status transition to Revoked (terminal)
- `rotate_issuer_address` - Address rotation with index updates

### Proof Registry Contract
- `initialize` - Initial contract setup with cross-contract references
- `register_proof` - Proof registration with cross-contract validation (worst case)
- `get_proof` - Proof lookup with TTL extension
- `revoke_proof` - Proof revocation
- `is_valid_proof` - Validity check including expiration

## Resource Metrics

### CPU Instructions
Measures computational cost of operations. Soroban charges fees based on CPU usage.

### Memory Bytes
Measures memory footprint during operation execution. High memory usage increases costs.

### WASM Size
Measured by `scripts/measure-resources.ps1`. Larger WASM size:
- Increases deployment costs
- May hit size limits
- Suggests unnecessary dependencies or code bloat

## Thresholds

All thresholds include headroom above current baseline measurements:
- **CPU/Memory**: ~20% headroom for normal variance
- **WASM size**: ~10% headroom to allow reasonable growth

Thresholds are intentionally conservative to force explicit review of resource changes.

## Baseline Environment

Measurements taken on:
- **Soroban SDK**: v27.0.0
- **Rust toolchain**: As pinned in `rust-toolchain.toml` (1.83.0)
- **Target**: wasm32-unknown-unknown
- **Build profile**: release with overflow-checks=true

## Running Tests

### Run budget tests only:
```bash
cargo test -p resource-budget-tests
```

### Run with output to see actual measurements:
```bash
cargo test -p resource-budget-tests -- --nocapture
```

### Measure WASM sizes:
```pwsh
./scripts/measure-resources.ps1
```

**Note**: WASM size measurement requires building contracts for the `wasm32-unknown-unknown` target in release mode. Due to Soroban SDK v27.0.0 requirements, this may require specific Rust toolchain versions. The script is provided for local testing but is not currently enforced in CI.

### Verbose WASM measurement:
```pwsh
./scripts/measure-resources.ps1 -Verbose
```

## Updating Baselines

When tests fail after intentional changes:

1. **Review the change**: Understand why resource usage increased
2. **Justify the change**: Document the reason (new feature, security fix, etc.)
3. **Update thresholds**: 
   - For CPU/memory: Update constants in `tests/budgets/src/lib.rs`
   - For WASM size: Update constants in `scripts/measure-resources.ps1`
4. **Document in PR**: Explain the resource impact and justification

### Example: Adding a new feature

If adding a new validation check increases `register_proof` CPU from 650K to 750K:

```rust
// In tests/budgets/src/lib.rs
- const PROOF_REGISTER_CPU_MAX: u64 = 800_000;  // Old (650K + 20% headroom)
+ const PROOF_REGISTER_CPU_MAX: u64 = 900_000;  // New (750K + 20% headroom)
```

PR description should include:
```markdown
## Resource Impact

- `proof_registry.register_proof` CPU increased from 650K to 750K instructions
- Reason: Added issuer revocation status check for extra security
- New threshold: 900K (includes 20% headroom)
- Mainnet cost impact: Negligible (~$0.001 increase per proof registration)
```

## Regression Detection

Two tests deliberately exceed thresholds to prove gates work:
- `budget_gate_detects_cpu_regression` - Fails with artificially low CPU limit
- `budget_gate_detects_memory_regression` - Fails with artificially low memory limit

These tests use `#[should_panic]` to expect failure.

## CI Integration

Resource budget tests run automatically in CI on every PR:
- Budget tests execute via `cargo test --workspace`
- WASM size checks can be added to CI workflow if needed

## Optimization Guidelines

If tests fail due to genuine regressions:

1. **Profile the operation**: Use `cargo flamegraph` or Soroban profiling tools
2. **Check for**:
   - Unnecessary clones or allocations
   - Redundant storage reads
   - Inefficient data structures
   - Unintended debug code left in release builds
3. **Common optimizations**:
   - Use references instead of clones
   - Batch storage operations
   - Cache frequently-accessed data
   - Remove unnecessary validations

## Security Considerations

**Do not weaken security to pass budget tests**:
- ✅ Optimize data structures
- ✅ Reduce redundant operations
- ✅ Improve algorithms
- ❌ Skip authorization checks
- ❌ Remove TTL extensions
- ❌ Disable validation logic

If a security-critical operation is expensive, increase the threshold and document why.

## Network Assumptions

These tests measure resource usage in the Soroban test environment. Actual mainnet costs depend on:
- Network congestion
- Stellar network parameters
- Soroban fee market dynamics

Tests ensure operations stay well below hard limits, providing safety margin for mainnet variability.

## Maintenance

- **Review quarterly**: Check if thresholds need adjustment as SDK evolves
- **Update on SDK upgrades**: Re-baseline after major Soroban SDK updates
- **Track trends**: Monitor if resource usage grows over time without feature additions

## Related Documentation

- [Storage Model](../../docs/storage-model.md) - Storage keys and TTL policies
- [Threat Model](../../docs/threat-model.md) - Security considerations
- [Backend Integration](../../docs/backend-integration.md) - Integration patterns

## Support

If budget tests fail unexpectedly:
1. Check recent changes for unintended performance impact
2. Run tests with `--nocapture` to see exact measurements
3. Compare measurements against thresholds to understand magnitude
4. Review [Soroban resource documentation](https://soroban.stellar.org/docs/fundamentals-and-concepts/resource-limits-fees)
