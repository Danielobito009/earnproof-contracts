# Contract Threat Model and Security Review Checklist

This document provides a structured threat model covering trust boundaries, privileged actors, cross-contract calls, upgrade/admin risk, denial of service, TTL expiry, and backend assumptions for the EarnProof Soroban contracts.

**Intended audience**: Security auditors, maintainers, backend developers, and deployment engineers.

**Document status**: Living document; updated as contracts evolve.

**Last reviewed**: 2026-08-26

---

## Table of Contents

1. [Assets and Trust Model](#assets-and-trust-model)
2. [Actors and Roles](#actors-and-roles)
3. [Trust Assumptions](#trust-assumptions)
4. [Entry Points and Attack Surface](#entry-points-and-attack-surface)
5. [Cross-Contract Boundaries](#cross-contract-boundaries)
6. [Threat Analysis](#threat-analysis)
7. [Privacy Analysis](#privacy-analysis)
8. [Mainnet Release Gates](#mainnet-release-gates)
9. [Security Review Checklist](#security-review-checklist)
10. [References](#references)

---

## Assets and Trust Model

### Assets Under Protection

1. **Issuer Trust State**
   - Issuer approval status (Active/Suspended/Revoked)
   - Issuer-to-address mappings
   - Issuer metadata integrity

2. **Proof Commitment Registry**
   - Proof existence and validity status
   - Revocation state
   - Expiration enforcement

3. **Protocol Configuration**
   - Admin control
   - Schema version approvals
   - Global pause state

4. **User Privacy**
   - No on-chain leakage of exact income amounts
   - No storage of personal identifying information
   - No raw payment history on-chain

### Non-Assets (Out of Scope)

- **Funds/Assets**: Contracts do not custody XLM or other Stellar assets
- **Computation**: Contracts do not perform income calculations or aggregate financial data
- **Key Material**: Contracts do not store private keys or signing secrets

---

## Actors and Roles

### Privileged Actors

| Actor | Scope | Privileges | Attack Impact if Compromised |
|-------|-------|-----------|----------------------------|
| **Protocol Admin** | `protocol-config` | - Change admin<br>- Pause/unpause protocol<br>- Approve/deprecate schema versions | **Critical**: Can halt all proof registrations, manipulate schema approvals, transfer admin control |
| **Issuer Registry Admin** | `issuer-registry` | - Register new issuers<br>- Update issuer metadata<br>- Suspend/reactivate/revoke issuers<br>- Rotate issuer addresses | **High**: Can register malicious issuers, revoke legitimate issuers, disrupt issuer operations |
| **Proof Registry Admin** | `proof-registry` | - Revoke any proof (admin revocation) | **Medium**: Can invalidate proofs but cannot create false proofs or modify existing proof data |

### Non-Privileged Actors

| Actor | Privileges | Constraints |
|-------|-----------|-------------|
| **Active Issuer** | - Register proofs under their own address<br>- Revoke proofs they registered | - Cannot register proofs while suspended or revoked<br>- Cannot revoke proofs registered by other issuers<br>- Must be marked Active in issuer-registry |
| **Public Readers** | - Read issuer status<br>- Check proof validity<br>- Query protocol configuration | - No write access<br>- No privileged information access |
| **Backend Indexers** | - Monitor events<br>- Query contract state | - Read-only operations<br>- Must properly handle TTL expiration |

---

## Trust Assumptions

### Explicit Trust Requirements

1. **Admin Key Security**
   - Protocol admin and registry admin keys must be secured
   - **Assumed**: Multi-sig or hardware wallet custody for mainnet
   - **Risk**: Single compromised admin key = full contract control

2. **Backend Hash Integrity**
   - Backend must correctly hash identifiers before contract calls
   - **Assumed**: Backend hashing implementation matches contract expectations
   - **Risk**: Incorrect hashing = inability to verify proofs

3. **Issuer Vetting**
   - Registry admin validates issuer legitimacy before registration
   - **Assumed**: Off-chain due diligence process exists
   - **Risk**: Malicious issuer registration = fraudulent proof issuance

4. **Cross-Contract Reference Stability**
   - `proof-registry` instance storage points to correct `issuer-registry` and `protocol-config` addresses
   - **Assumed**: Deployment manifest validates contract addresses
   - **Risk**: Incorrect reference = bypassed validation checks

5. **Stellar Network Availability**
   - Horizon/RPC availability for TTL extension and state reads
   - **Assumed**: Network liveness for time-sensitive operations
   - **Risk**: Network downtime = inability to extend TTLs before expiration

### Trust Boundaries

| Boundary | Trusted Side | Untrusted Side | Enforcement Mechanism |
|----------|--------------|----------------|---------------------|
| Admin operations | Admin address | All other addresses | `require_auth(&admin)` |
| Issuer registration | Issuer Registry Admin | All callers | `require_auth(&admin)` |
| Proof registration | Active issuer address | Suspended/revoked issuers, non-issuers | `is_active_address()` check + `require_auth(&issuer_address)` |
| Schema validation | Approved versions | Unapproved versions | `is_schema_version_approved()` check |
| Protocol pause | Paused = no registrations | Active = registrations allowed | `is_paused()` check |
| Cross-contract calls | Contract instance storage references | External caller-provided addresses | Stored references set at `initialize` |

---

## Entry Points and Attack Surface

### `protocol-config` Contract

| Entry Point | Authorization | State Mutation | Attack Vectors |
|-------------|---------------|----------------|----------------|
| `initialize(admin)` | Caller becomes admin | Sets admin, pause=false, version=1 | **Initialization race**: First caller wins; deploy scripts must call immediately |
| `set_admin(new_admin)` | Current admin only | Transfers admin control | **Admin takeover**: Compromised admin can transfer control |
| `pause()` | Admin only | Sets paused=true | **DoS**: Malicious admin halts all proof registrations |
| `unpause()` | Admin only | Sets paused=false | **Premature unblocking**: Admin unpauses during incident response |
| `approve_schema_version(v)` | Admin only | Marks version approved | **Unapproved schema injection**: Malicious admin approves vulnerable schema |
| `deprecate_schema_version(v)` | Admin only | Marks version deprecated | **Schema revocation DoS**: Admin deprecates active schemas |

**Read-only** (no auth): `get_admin`, `is_paused`, `get_config_version`, `is_schema_version_approved`

### `issuer-registry` Contract

| Entry Point | Authorization | State Mutation | Attack Vectors |
|-------------|---------------|----------------|----------------|
| `initialize(admin)` | Caller becomes admin | Sets admin | **Initialization race**: First caller wins |
| `register_issuer(...)` | Admin only | Creates issuer record | **Malicious issuer registration**: Admin registers attacker-controlled issuer |
| `update_issuer(...)` | Admin only | Updates metadata hash | **Metadata manipulation**: Admin updates metadata to point to malicious data |
| `suspend_issuer(...)` | Admin only | Sets status=Suspended | **Issuer DoS**: Admin suspends legitimate issuer |
| `reactivate_issuer(...)` | Admin only | Sets status=Active | **Malicious reactivation**: Admin reactivates compromised issuer |
| `revoke_issuer(...)` | Admin only | Sets status=Revoked (terminal) | **Permanent issuer DoS**: Admin revokes legitimate issuer |
| `rotate_issuer_address(...)` | Admin only | Updates issuer address | **Address hijacking**: Admin rotates address to attacker-controlled wallet |

**Read-only** (no auth): `get_admin`, `get_issuer`, `get_issuer_by_address`, `is_active_issuer`, `is_active_address`

### `proof-registry` Contract

| Entry Point | Authorization | State Mutation | Attack Vectors |
|-------------|---------------|----------------|----------------|
| `initialize(admin, issuer_registry, protocol_config)` | Caller becomes admin | Sets admin + cross-contract refs | **Initialization race**: First caller wins<br>**Incorrect references**: Malicious contracts referenced |
| `register_proof(...)` | Issuer address only | Creates proof record | **Duplicate registration**: Prevented by contract<br>**Expired proof**: Prevented by `expires_at` check<br>**Inactive issuer bypass**: Prevented by `is_active_address` check<br>**Paused protocol bypass**: Prevented by `is_paused` check |
| `revoke_proof(proof_id)` | Issuer address that registered proof | Sets status=Revoked | **Unauthorized revocation**: Prevented by auth check<br>**Double revocation**: Prevented by contract |
| `admin_revoke_proof(proof_id)` | Admin only | Sets status=Revoked | **Admin abuse**: Admin can revoke any proof |

**Read-only** (no auth): `get_admin`, `get_issuer_registry`, `get_protocol_config`, `get_proof`, `is_valid_proof`, `is_revoked`

---

## Cross-Contract Boundaries

### Cross-Contract Call Flow

```
proof-registry.register_proof()
    ├─> protocol-config.is_paused()           [external call]
    ├─> protocol-config.is_schema_version_approved(version) [external call]
    └─> issuer-registry.is_active_address(issuer_address)  [external call]
```

### Security Properties

1. **Reference Immutability**
   - Contract addresses stored in `proof-registry` instance storage
   - Set once at initialization, not modifiable
   - **Control**: Deployment validation required

2. **No Reentrancy**
   - Soroban does not support reentrancy during cross-contract calls
   - State changes committed before external calls return
   - **Control**: Soroban runtime enforcement

3. **Call Depth Limits**
   - Soroban enforces maximum call depth
   - Current call chain depth: 3 (proof-registry → issuer-registry/protocol-config)
   - **Control**: Well below Soroban limits

### Cross-Contract Threats

| Threat | Impact | Implemented Control | Test Coverage |
|--------|--------|-------------------|---------------|
| **Malicious contract reference** | Bypass all validation checks | Deployment manifest validation | `scripts/verify-manifest.tests.ps1` |
| **Contract address typo** | Calls fail or hit wrong contract | Pre-deployment address verification | Deployment script validation |
| **Referenced contract upgrade** | Behavior changes post-deployment | Immutable references + upgrade coordination | Manual review required |
| **TTL expiry of referenced contract** | Cross-contract calls fail | Backend must extend TTLs | Monitoring (not automated) |

---

## Threat Analysis

### T1: Authorization Bypass

**Description**: Attacker attempts to execute privileged operations without proper authorization.

**Attack Vectors**:
- Forge admin signatures
- Exploit missing `require_auth` calls
- Replay valid authorization from different context

**Implemented Controls**:
- ✅ Every state-mutating function calls `require_auth(&address)`
- ✅ Soroban SDK validates signatures cryptographically
- ✅ Authorization context tied to current invocation (no replay)

**Test Coverage**:
- ✅ All tests use `env.mock_all_auths()` to exercise auth paths
- ✅ Contracts compile with auth checks (not compile-gated)

**Residual Risk**: **Low** — Soroban SDK handles signature validation

**Open Issues**: None

**Status**: ✅ **Mitigated**

---

### T2: Duplicate Registration (Replay Attacks)

**Description**: Attacker attempts to register the same issuer or proof multiple times.

**Attack Vectors**:
- Submit identical issuer registration twice
- Submit identical proof registration twice
- Cause state inconsistency between forward/reverse indexes

**Implemented Controls**:
- ✅ `issuer-registry`: Checks `has(&Issuer(issuer_id_hash))` and `has(&AddressIssuer(address))` before registration
- ✅ `proof-registry`: Checks `has(&Proof(proof_id_hash))` before registration
- ✅ Rejection returns error; no partial state written

**Test Coverage**:
- ✅ `issuer-registry`: `rejects_duplicate_issuer_id`
- ✅ `proof-registry`: `rejects_duplicate_proof_id`

**Residual Risk**: **Low** — Duplicate checks enforced

**Open Issues**: None

**Status**: ✅ **Mitigated**

---

### T3: Malicious Issuer Behavior

**Description**: Compromised or malicious issuer attempts to issue fraudulent proofs or manipulate proof state.

**Attack Vectors**:
- Register proofs with false income data
- Revoke legitimate proofs from other issuers
- Continue issuing proofs after suspension

**Implemented Controls**:
- ✅ Proof registration requires `is_active_address(&issuer_address)` check
- ✅ Revocation requires auth from issuer who registered the proof
- ✅ Suspended/revoked issuers cannot register new proofs
- ⚠️ **Gap**: Contracts cannot validate proof content; off-chain verification required

**Test Coverage**:
- ✅ `proof-registry`: `rejects_inactive_issuer_address`
- ✅ Authorization tests cover issuer-initiated revocation

**Residual Risk**: **Medium** — Malicious issuer can issue proofs with false claims until suspended

**Accepted Risk**: Proof content validation is off-chain; verifiers must check issuer trust status

**Open Issues**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21) — Backend issuer monitoring

**Status**: ⚠️ **Partially mitigated** — Requires off-chain monitoring

---

### T4: Compromised Admin Key

**Description**: Attacker gains control of protocol admin, issuer registry admin, or proof registry admin key.

**Attack Vectors**:
- Phishing admin private key
- Compromising deployment machine
- Social engineering admin key holder

**Impact**:
- **Protocol admin**: Pause protocol, manipulate schema approvals, transfer admin
- **Issuer registry admin**: Register malicious issuers, revoke legitimate issuers
- **Proof registry admin**: Revoke any proof

**Implemented Controls**:
- ⚠️ **Testnet**: Single-key admin (acceptable for testnet)
- ❌ **Mainnet**: Multi-sig or hardware wallet custody required (not yet implemented)

**Test Coverage**: N/A (operational control)

**Residual Risk**: **Critical for mainnet** — Single compromised key = full control

**Required Control**: Multi-sig admin (e.g., 3-of-5) or hardware wallet custody

**Open Issues**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21) — Mainnet admin custody policy

**Status**: ❌ **Mainnet blocker** — Multi-sig required before mainnet

---

### T5: Protocol Pause Abuse / Denial of Service

**Description**: Malicious or compromised admin pauses protocol to prevent legitimate proof registrations.

**Attack Vectors**:
- Admin calls `pause()` during normal operations
- Admin refuses to call `unpause()` during incident

**Implemented Controls**:
- ✅ Only admin can pause/unpause
- ✅ Pause state queryable via `is_paused()`
- ⚠️ **Gap**: No time-lock or multi-sig requirement for pause

**Test Coverage**:
- ✅ `proof-registry`: `rejects_registration_when_protocol_is_paused`
- ✅ `protocol-config`: `pause_and_unpause_bump_config_version`

**Residual Risk**: **Medium** — Admin can DoS proof registration

**Accepted Risk**: Intentional design; admin must be trusted

**Mitigation**: Mainnet admin should use multi-sig or DAO governance

**Open Issues**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21) — Governance model for pause

**Status**: ⚠️ **Accepted risk** — Requires trusted admin

---

### T6: Stale Cross-Contract References

**Description**: Referenced contracts (`issuer-registry`, `protocol-config`) become unavailable due to TTL expiry or upgrade.

**Attack Vectors**:
- Instance storage of referenced contract expires
- Referenced contract upgraded to incompatible version
- Referenced contract address incorrect at deployment

**Implemented Controls**:
- ✅ Contract addresses set once at `proof-registry.initialize()`
- ✅ Deployment scripts validate contract addresses before initialization
- ⚠️ **Gap**: No automated TTL extension for referenced contracts
- ⚠️ **Gap**: No upgrade coordination mechanism

**Test Coverage**:
- ✅ `scripts/verify-manifest.tests.ps1` validates deployment manifest
- ❌ No automated tests for TTL expiry scenarios

**Residual Risk**: **Medium** — TTL expiry would halt proof registration

**Required Control**: Backend monitoring + manual TTL extension for referenced contracts

**Open Issues**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21) — TTL monitoring automation

**Status**: ⚠️ **Manual mitigation** — Monitoring required

---

### T7: TTL Expiration and Data Loss

**Description**: Contract storage entries expire due to insufficient TTL extension, causing data loss or availability issues.

**Attack Vectors**:
- Backend fails to extend TTL for frequently accessed entries
- Network downtime prevents TTL extension before expiry
- Low-activity issuers/proofs expire due to infrequent access

**Implemented Controls**:
- ✅ TTL constants defined: `TTL_THRESHOLD_LEDGERS = 50,000`, `TTL_EXTEND_TO_LEDGERS = 500,000`
- ✅ Read operations (`get_issuer`, `get_proof`) extend TTL
- ⚠️ **Gap**: No automated TTL monitoring
- ⚠️ **Gap**: Expired entries require Soroban state archival recovery

**Test Coverage**:
- ✅ `issuer-registry`: `extends_issuer_storage_ttl`
- ✅ `proof-registry`: `extends_proof_storage_ttl`

**Residual Risk**: **Medium** — Low-activity entries may expire

**Required Control**: Backend TTL monitoring + periodic reads to extend TTL

**Open Issues**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21) — TTL monitoring service

**Status**: ⚠️ **Manual mitigation** — Monitoring required

---

### T8: Invalid State Transitions

**Description**: Attacker attempts to force invalid issuer status transitions or proof state changes.

**Attack Vectors**:
- Reactivate revoked issuer
- Update metadata of revoked issuer
- Rotate address of revoked issuer
- Double-revoke proof

**Implemented Controls**:
- ✅ Revoked issuer operations blocked: `if record.status == IssuerStatus::Revoked { return Err(...) }`
- ✅ Invalid status transitions rejected: `if record.status == Revoked && new_status != Revoked { return Err(InvalidTransition) }`
- ✅ Double-revoke proof blocked: `if record.status == ProofStatus::Revoked { return Err(ProofAlreadyRevoked) }`

**Test Coverage**:
- ✅ `issuer-registry`: `revoked_issuer_cannot_be_reactivated`, `rotate_revoked_issuer_address_emits_no_event`, `update_revoked_issuer_emits_no_event`
- ✅ Tests assert typed error codes

**Residual Risk**: **Low** — State transition validation enforced

**Open Issues**: None

**Status**: ✅ **Mitigated**

---

### T9: Expired Proof Acceptance

**Description**: Verifier accepts proof after expiration timestamp.

**Attack Vectors**:
- Backend caches `is_valid_proof` result past expiration
- Off-chain verifier ignores expiration check
- Time manipulation (not possible on Stellar)

**Implemented Controls**:
- ✅ `is_valid_proof` checks `env.ledger().timestamp() <= record.expires_at`
- ✅ Expiration validation on every call (no caching in contract)
- ⚠️ **Gap**: Backend must not cache validity status across expiration boundary

**Test Coverage**:
- ✅ `proof-registry`: `rejects_expired_proof` (registration time check)
- ⚠️ **Gap**: No test for post-registration expiration validation

**Residual Risk**: **Low** — Contract enforces expiration; backend must respect it

**Required Control**: Backend must query `is_valid_proof` at verification time, not cache

**Open Issues**: None (backend implementation responsibility)

**Status**: ✅ **Mitigated** (assuming backend compliance)

---

### T10: Event Consumer Manipulation

**Description**: Off-chain indexer or event consumer is deceived by fake events or misses legitimate events.

**Attack Vectors**:
- Attacker submits transaction with event-like data (not possible; events are contract-emitted only)
- Indexer misses event due to network partition
- Indexer processes events out of order

**Implemented Controls**:
- ✅ Events emitted via `#[contractevent]` macro (Soroban enforced)
- ✅ Only successful transactions emit events (Soroban enforced)
- ✅ Failed calls emit no success events
- ⚠️ **Gap**: Indexer must handle network partitions and reorgs

**Test Coverage**:
- ✅ Event emission tests verify exactly one event per successful mutation
- ✅ Failed mutations emit zero success events

**Residual Risk**: **Low** — Event integrity guaranteed by Soroban; indexer reliability is operational concern

**Required Control**: Indexer must implement retry logic and reorg handling

**Open Issues**: None (backend implementation responsibility)

**Status**: ✅ **Mitigated** (contract-side)

---

### T11: Unapproved Schema Version Bypass

**Description**: Attacker registers proof with unapproved or deprecated schema version.

**Attack Vectors**:
- Submit proof with version = 0
- Submit proof with deprecated version
- Submit proof with never-approved version

**Implemented Controls**:
- ✅ `register_proof` checks `if schema_version == 0 { return Err(InvalidSchemaVersion) }`
- ✅ `register_proof` calls `protocol_config.is_schema_version_approved(&schema_version)`
- ✅ Rejected if version not explicitly approved

**Test Coverage**:
- ✅ `proof-registry`: `rejects_unapproved_schema_version`
- ✅ `protocol-config`: `rejects_zero_schema_version`

**Residual Risk**: **Low** — Schema approval enforced

**Open Issues**: None

**Status**: ✅ **Mitigated**

---

### T12: Deployment and Supply Chain Attacks

**Description**: Attacker compromises deployment process or build artifacts.

**Attack Vectors**:
- Inject malicious code during build
- Deploy incorrect contract addresses
- Tamper with deployment manifest
- Use compromised Rust toolchain or dependencies

**Implemented Controls**:
- ✅ Rust toolchain pinned: `rust-toolchain.toml`
- ✅ `Cargo.lock` committed to repository
- ✅ Deployment manifest validation: `scripts/verify-manifest.ps1`
- ⚠️ **Gap**: No reproducible build verification
- ⚠️ **Gap**: No multi-party deployment verification

**Test Coverage**:
- ✅ `scripts/verify-manifest.tests.ps1` validates manifest structure

**Residual Risk**: **Medium** — Single deployer can deploy malicious build

**Required Control**: Reproducible builds + multi-party verification for mainnet

**Open Issues**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21) — Reproducible build process

**Status**: ❌ **Mainnet blocker** — Reproducible builds required

---

### T13: Resource Exhaustion / Griefing

**Description**: Attacker floods contracts with low-value operations to exhaust resources or increase costs.

**Attack Vectors**:
- Register many low-activity issuers
- Register many short-lived proofs
- Repeatedly query contract state

**Implemented Controls**:
- ✅ Soroban resource limits (CPU, memory, ledger I/O)
- ✅ Transaction fees required for all operations
- ⚠️ **Gap**: No rate limiting at contract level
- ⚠️ **Gap**: No minimum TTL or minimum expiration duration

**Test Coverage**: N/A (Soroban runtime enforced)

**Residual Risk**: **Low** — Griefing limited by transaction fees

**Accepted Risk**: Soroban fee market provides economic defense

**Open Issues**: None (monitoring recommended)

**Status**: ✅ **Accepted risk** — Fee market mitigation

---

## Privacy Analysis

### Privacy Requirements

EarnProof contracts must not store:
- Exact income amounts
- Exact payment amounts
- Raw transaction history
- Personal names
- Email addresses
- Employment documents
- Unencrypted personal information

### On-Chain Data Review

| Data Item | Stored? | Privacy Risk | Control |
|-----------|---------|--------------|---------|
| Exact income | ❌ No | None | Backend hashes commitment |
| Proof ID | ❌ No (hash only) | None | SHA-256 hash prevents reversal |
| Commitment payload | ❌ No (hash only) | None | SHA-256 hash prevents reversal |
| Issuer internal ID | ❌ No (hash only) | None | SHA-256 hash prevents reversal |
| Issuer metadata | ❌ No (hash only) | None | SHA-256 hash; metadata hosted off-chain |
| Issuer Stellar address | ✅ Yes | Low | Public blockchain address; pseudonymous |
| Proof schema version | ✅ Yes | Low | Version number reveals credential type but not content |
| Proof expiration timestamp | ✅ Yes | Low | Timestamp range may infer employment period |
| Proof creation timestamp | ✅ Yes | Low | Registration time public information |
| Proof status (Active/Revoked) | ✅ Yes | Low | Status reveals proof lifecycle but not content |

### Privacy Controls

- ✅ All identifiers hashed with SHA-256 before storage
- ✅ No personal identifying information in events
- ✅ No personal identifying information in storage keys
- ✅ Metadata and payloads stored off-chain
- ✅ Backend integration docs specify hashing rules

### Privacy Test Coverage

- ✅ Event fixture tests verify no personal data in event payloads
- ✅ Storage model documentation explicitly lists excluded data types

### Residual Privacy Risks

| Risk | Impact | Mitigation |
|------|--------|-----------|
| **Timing analysis** | Proof registration patterns may reveal employment events | Low impact; timestamps are inherent to blockchain |
| **Address correlation** | Issuer addresses may be linkable to known entities | Accepted; public blockchain property |
| **Schema version inference** | Schema version reveals credential type | Low impact; necessary for validation |

**Privacy Status**: ✅ **Compliant** — No path stores exact income or personal identity

---

## Mainnet Release Gates

Before mainnet deployment, the following gates must be satisfied:

### 1. Independent Security Audit

- ❌ **Status**: Not completed
- **Requirement**: Third-party security audit by Soroban-experienced firm
- **Scope**: All contracts, cross-contract interactions, authorization logic, TTL policies
- **Deliverable**: Audit report with findings resolution plan

### 2. Admin Custody Model

- ❌ **Status**: Not implemented
- **Requirement**: Multi-sig (3-of-5 or stronger) or DAO governance for all admin keys
- **Scope**: Protocol admin, issuer registry admin, proof registry admin
- **Deliverable**: Multi-sig wallet deployment + key ceremony documentation

### 3. Upgrade Policy

- ❌ **Status**: Not defined
- **Requirement**: Documented upgrade process including:
  - Time-lock for admin changes
  - Community notice period
  - Rollback procedure
  - Backwards compatibility requirements
- **Deliverable**: `docs/upgrade-policy.md`

### 4. Monitoring and Incident Response

- ❌ **Status**: Not implemented
- **Requirement**:
  - TTL monitoring for all contracts
  - Admin action alerting
  - Issuer suspension monitoring
  - Proof registration rate monitoring
  - Incident response runbook
- **Deliverable**: Monitoring dashboard + runbook

### 5. TTL and Resource Analysis

- ❌ **Status**: Not completed
- **Requirement**:
  - Worst-case TTL extension costs
  - Resource usage profiling under load
  - Fee estimation for all operations
- **Deliverable**: Resource analysis document

### 6. Reproducible Build Artifacts

- ❌ **Status**: Not implemented
- **Requirement**:
  - Deterministic build process
  - Multi-party build verification
  - WASM artifact hash publication
- **Deliverable**: Reproducible build instructions + artifact hashes

### 7. Backend Integration Testing

- ⚠️ **Status**: Partial
- **Requirement**:
  - Backend can successfully interact with deployed contracts
  - Error handling tested for all failure modes
  - Rate limiting and retry logic implemented
- **Deliverable**: Backend integration test suite passing against deployed contracts

### 8. Deployment Acknowledgement

- ❌ **Status**: Not completed
- **Requirement**:
  - Mainnet deployment checklist completed
  - All signers acknowledge deployment risks
  - Emergency contact list established
- **Deliverable**: Signed deployment acknowledgement document

### 9. Post-Deployment Verification

- ❌ **Status**: Not completed
- **Requirement**:
  - Contract addresses published
  - Initial state verified (admin addresses, pause state, etc.)
  - Cross-contract references verified
  - Sample transactions executed successfully
- **Deliverable**: Post-deployment verification report

### 10. Containment and Rollback Limits

- ❌ **Status**: Not defined
- **Requirement**:
  - Document what can be rolled back (e.g., admin actions)
  - Document what cannot be rolled back (e.g., registered proofs)
  - Incident containment procedures
- **Deliverable**: Rollback limitations document

### Deployment Safety

- ✅ **Testnet default**: Deployment scripts default to testnet
- ✅ **Fail-closed**: Scripts explicitly require `--network testnet` flag
- ⚠️ **Mainnet guard**: Mainnet deployment should require multi-party approval

---

## Security Review Checklist

Use this checklist to verify security controls against the current codebase.

### Authorization

- [x] Every state-mutating function calls `require_auth`
  - **Reference**: `contracts/*/src/lib.rs` — search for `require_auth`
  - **Test**: All tests use `env.mock_all_auths()`
  
- [x] Admin functions check admin address before execution
  - **Reference**: `Self::get_admin()` calls in admin functions
  - **Test**: Admin operations in test suite

- [x] Issuer operations validate issuer status
  - **Reference**: `proof-registry` checks `is_active_address`
  - **Test**: `rejects_inactive_issuer_address`

### Duplicate Prevention

- [x] Issuer registration checks for duplicate ID and address
  - **Reference**: `issuer-registry/src/lib.rs:register_issuer`
  - **Test**: `rejects_duplicate_issuer_id`

- [x] Proof registration checks for duplicate ID
  - **Reference**: `proof-registry/src/lib.rs:register_proof`
  - **Test**: `rejects_duplicate_proof_id`

### State Transitions

- [x] Revoked issuers cannot be updated, reactivated, or rotated
  - **Reference**: `issuer-registry/src/lib.rs` — `if status == Revoked` checks
  - **Test**: `revoked_issuer_cannot_be_reactivated`, `update_revoked_issuer_emits_no_event`, `rotate_revoked_issuer_address_emits_no_event`

- [x] Revoked proofs cannot be revoked again
  - **Reference**: `proof-registry/src/lib.rs:set_revoked`
  - **Test**: Double-revoke attempt in test suite (implicit)

### Validation

- [x] Proof expiration must be in the future
  - **Reference**: `proof-registry/src/lib.rs:register_proof`
  - **Test**: `rejects_expired_proof`

- [x] Schema version must be greater than zero
  - **Reference**: `protocol-config/src/lib.rs:ensure_nonzero_version`
  - **Test**: `rejects_zero_schema_version`

- [x] Schema version must be approved
  - **Reference**: `proof-registry/src/lib.rs:register_proof`
  - **Test**: `rejects_unapproved_schema_version`

### Cross-Contract

- [x] Protocol pause state checked before proof registration
  - **Reference**: `proof-registry/src/lib.rs:register_proof`
  - **Test**: `rejects_registration_when_protocol_is_paused`

- [x] Issuer activity checked before proof registration
  - **Reference**: `proof-registry/src/lib.rs:register_proof`
  - **Test**: `rejects_inactive_issuer_address`

- [x] Contract references set at initialization and immutable
  - **Reference**: `proof-registry/src/lib.rs:initialize`
  - **Test**: Deployment manifest validation

### TTL Management

- [x] Read operations extend persistent storage TTL
  - **Reference**: `get_issuer`, `get_proof` call `extend_ttl`
  - **Test**: `extends_issuer_storage_ttl`, `extends_proof_storage_ttl`

- [x] Write operations extend instance and persistent TTL
  - **Reference**: Every mutation calls `extend_*_ttl`
  - **Test**: TTL tests

### Privacy

- [x] No exact income stored on-chain
  - **Reference**: `packages/shared/src/lib.rs` — only hash types
  - **Test**: Storage model documentation review

- [x] No personal identifying information in storage
  - **Reference**: Storage model lists only hashes and addresses
  - **Test**: Privacy boundary documentation review

- [x] No personal identifying information in events
  - **Reference**: Event definitions in contracts
  - **Test**: `protocol_config_no_private_data_in_fixtures`

### Testing

- [x] Unit tests cover all entry points
  - **Reference**: `contracts/*/src/lib.rs` test modules
  - **Test**: 41 tests pass

- [x] Error cases tested with typed error codes
  - **Reference**: Tests use `try_*` methods and assert error codes
  - **Test**: All error tests pass

- [x] Authorization paths exercised
  - **Reference**: Tests use `env.mock_all_auths()`
  - **Test**: All tests pass

- [x] Cross-contract integration tested
  - **Reference**: `proof-registry` tests set up full contract stack
  - **Test**: Proof registration tests

### Deployment

- [x] Deployment scripts default to testnet
  - **Reference**: `scripts/deploy-testnet.ps1`
  - **Test**: Script inspection

- [x] Manifest validation enforced
  - **Reference**: `scripts/verify-manifest.ps1`
  - **Test**: `scripts/verify-manifest.tests.ps1`

- [x] Rust toolchain pinned
  - **Reference**: `rust-toolchain.toml`
  - **Test**: CI build consistency

- [x] Dependencies locked
  - **Reference**: `Cargo.lock` committed
  - **Test**: Build reproducibility

### Open Gaps

- [ ] Multi-sig admin custody (Mainnet blocker)
  - **Issue**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21)
  
- [ ] Independent security audit (Mainnet blocker)
  - **Issue**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21)

- [ ] Reproducible build process (Mainnet blocker)
  - **Issue**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21)

- [ ] Automated TTL monitoring
  - **Issue**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21)

- [ ] Upgrade policy documentation
  - **Issue**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21)

- [ ] Incident response runbook
  - **Issue**: [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21)

---

## References

- [Storage Model](./storage-model.md) — Authoritative storage key reference
- [Backend Integration](./backend-integration.md) — Contract call patterns and error handling
- [Security Policy](../SECURITY.md) — Vulnerability reporting process
- [Soroban Documentation](https://soroban.stellar.org/docs) — Soroban security best practices
- Issue [#21](https://github.com/veridatum-labs/earnproof-contracts/issues/21) — Mainnet readiness tracking

---

**Document Maintenance**: This threat model should be updated whenever:
- New contracts are added
- Entry points or authorization logic changes
- Cross-contract interactions are modified
- New security controls are implemented
- Security issues are discovered and resolved

**Next Review Date**: Before mainnet deployment (no date set)
