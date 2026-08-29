# Admin Operations Runbook

## Contracts
Contracts: ProtocolConfig, Issuer, ProofRegistry.
Each contract has an admin address and an `update_admin(new_admin)` function.

## Rotation Interface
- Auth: only current admin can call.
- Rejects: zero address and no-op.
- Emits: `AdminUpdated(old_admin, new_admin)`.

## Order
1. Issuer
2. ProofRegistry
3. ProtocolConfig

Verify after each step.

## Verification
- Query `admin()` on each contract after each rotation.
- Run `./scripts/verify-manifest.ps1` to check the manifest.

## Partial Failure Recovery
- Determine which contracts have already been rotated.
- Retry the remaining ones using the old admin key.
- Complete all rotations before verifying.

## Emergency Key Compromise
- If the current admin key is compromised, immediately rotate all contracts to a fresh securely generated key.
- If the key is lost, use the governed alternative (e.g., time-locked multisig) to reset the admin.
- Audit all configurations after recovery.

## Prevention
- Use a multisig address as the admin to avoid single-key lockout.
