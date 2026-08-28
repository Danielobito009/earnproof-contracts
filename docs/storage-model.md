# Storage Model

This document is the authoritative reference for every on-chain storage key used by the EarnProof Soroban contracts. It is intended for auditors, indexer authors, and backend maintainers.

**See Also:** [Executable Examples](./executable-examples.md) for runnable demonstrations of storage model behavior and contract invocation patterns.

Covered contracts:

- [`protocol-config`](../contracts/protocol-config/src/lib.rs)
- [`issuer-registry`](../contracts/issuer-registry/src/lib.rs)
- [`proof-registry`](../contracts/proof-registry/src/lib.rs)

Shared types referenced throughout this document are defined in [`packages/shared/src/lib.rs`](../packages/shared/src/lib.rs).

---

## TTL Constants

All TTL values are defined in `packages/shared/src/lib.rs` and imported by every contract.

| Constant | Value (ledgers) | Approximate wall-clock time\* |
|---|---|---|
| `TTL_THRESHOLD_LEDGERS` | `50,000` | ~3 days |
| `TTL_EXTEND_TO_LEDGERS` | `500,000` | ~29 days |

\* Assumes a 5-second average ledger close time on Stellar. Actual times vary with network conditions.

### Extension policy

Both instance and persistent storage use the same threshold/target pair. When an entry's remaining TTL falls below `TTL_THRESHOLD_LEDGERS`, the next qualifying operation extends it to `TTL_EXTEND_TO_LEDGERS`. This is a one-sided bump: the TTL is set to `TTL_EXTEND_TO_LEDGERS` only if the current TTL is below the threshold; entries already above the threshold are not shortened.

Soroban does not automatically extend TTLs. Every entry will expire and be archived unless a contract call explicitly extends it before the ledger cutoff.

---

## Storage Classes

| Class | Scope | Typical use |
|---|---|---|
| **Instance** | Tied to the contract instance; expires with the contract itself | Small, frequently read configuration values |
| **Persistent** | Independent per-key TTL; survives independent of the contract instance | Long-lived records (issuers, proofs, schema flags) |
| **Temporary** | No explicit TTL extension; reclaimed by the network automatically | Not used in the current contracts |

---

## `protocol-config` Contract

### Instance storage

All instance keys share the contract instance's TTL. The instance TTL is extended on every write call that mutates state.

| `DataKey` variant | Rust type | Description |
|---|---|---|
| `Admin` | `Address` | Stellar address of the protocol administrator. Set on `initialize`; replaced by `set_admin`. |
| `Paused` | `bool` | Global protocol pause flag. `true` while the protocol is paused. Initialized to `false`. |
| `ConfigVersion` | `u32` | Monotonically incrementing counter. Starts at `1` and is bumped on every state-mutating call (`set_admin`, `pause`, `unpause`, `approve_schema_version`, `deprecate_schema_version`). |

### Persistent storage

| `DataKey` variant | Rust type | Description |
|---|---|---|
| `SchemaVersion(u32)` | `bool` | Approval flag for a credential schema version. `true` = approved; `false` = deprecated. Key contains the version number. |

### Lifecycle events

| Event | Effect on storage |
|---|---|
| `initialize(admin)` | Writes `Admin`, `Paused = false`, `ConfigVersion = 1` to instance. Extends instance TTL. |
| `set_admin(new_admin)` | Overwrites `Admin`. Bumps `ConfigVersion`. Extends instance TTL. |
| `pause()` | Sets `Paused = true`. Bumps `ConfigVersion`. Extends instance TTL. |
| `unpause()` | Sets `Paused = false`. Bumps `ConfigVersion`. Extends instance TTL. |
| `approve_schema_version(version)` | Writes `SchemaVersion(version) = true` to persistent. Extends `SchemaVersion(version)` TTL. Bumps `ConfigVersion`. Extends instance TTL. |
| `deprecate_schema_version(version)` | Overwrites `SchemaVersion(version) = false` in persistent. Extends `SchemaVersion(version)` TTL. Bumps `ConfigVersion`. Extends instance TTL. |

Note: `deprecate_schema_version` does **not** delete the key. The key persists with a `false` value so that indexers and auditors can distinguish "never seen" (key absent) from "explicitly deprecated" (key present, value `false`).

### Reads and TTL extension

| Function | Storage read | Extends TTL? |
|---|---|---|
| `get_admin()` | Instance: `Admin` | No |
| `is_paused()` | Instance: `Paused` | No |
| `get_config_version()` | Instance: `ConfigVersion` | No |
| `is_schema_version_approved(version)` | Persistent: `SchemaVersion(version)` | **Yes** — extends `SchemaVersion(version)` if the key exists |

Instance storage reads do not call `extend_ttl` individually. Instance TTL is only extended by write operations.

---

## `issuer-registry` Contract

### Instance storage

| `DataKey` variant | Rust type | Description |
|---|---|---|
| `Admin` | `Address` | Stellar address of the registry administrator. Set on `initialize`; not rotatable through a public method in this contract. |

### Persistent storage

| `DataKey` variant | Rust type | Description |
|---|---|---|
| `Issuer(BytesN<32>)` | `IssuerRecord` | Full issuer record keyed by the 32-byte SHA-256 hash of the issuer's internal identifier. |
| `AddressIssuer(Address)` | `BytesN<32>` | Reverse index from an issuer's Stellar address to their `issuer_id_hash`. Used by address-based lookups. |

#### `IssuerRecord` fields (defined in `packages/shared`)

| Field | Type | Description |
|---|---|---|
| `issuer_id_hash` | `BytesN<32>` | SHA-256 hash of the issuer's internal ID. |
| `issuer_address` | `Address` | Current Stellar address associated with this issuer. |
| `metadata_hash` | `BytesN<32>` | SHA-256 hash of the issuer's canonical public metadata payload. |
| `status` | `IssuerStatus` | Current status: `Active`, `Suspended`, or `Revoked`. |
| `created_at` | `u64` | Ledger timestamp at registration. |
| `updated_at` | `u64` | Ledger timestamp of the last state change. |

#### `IssuerStatus` values

| Value | Meaning |
|---|---|
| `Active` | Issuer may register and sign proofs. |
| `Suspended` | Issuer operations are temporarily blocked; reactivation is possible. |
| `Revoked` | Issuer is permanently disabled. No further status changes or address rotation are allowed. |

### Lifecycle events

| Event | Effect on storage |
|---|---|
| `initialize(admin)` | Writes `Admin` to instance. Extends instance TTL. |
| `register_issuer(issuer_id_hash, issuer_address, metadata_hash)` | Writes `Issuer(issuer_id_hash)` and `AddressIssuer(issuer_address)` to persistent. Extends both entry TTLs. |
| `update_issuer(issuer_id_hash, metadata_hash)` | Overwrites `metadata_hash` and `updated_at` in `Issuer(issuer_id_hash)`. Extends `Issuer` TTL. Blocked if status is `Revoked`. |
| `suspend_issuer(issuer_id_hash)` | Sets `status = Suspended` and updates `updated_at` in `Issuer(issuer_id_hash)`. Extends `Issuer` TTL. |
| `reactivate_issuer(issuer_id_hash)` | Sets `status = Active` and updates `updated_at`. Extends `Issuer` TTL. Blocked if current status is `Revoked`. |
| `revoke_issuer(issuer_id_hash)` | Sets `status = Revoked` and updates `updated_at`. Extends `Issuer` TTL. Terminal: no further status transitions or address rotation are possible. |
| `rotate_issuer_address(issuer_id_hash, new_address)` | Removes old `AddressIssuer(old_address)`. Writes `AddressIssuer(new_address)`. Updates `issuer_address` and `updated_at` in `Issuer(issuer_id_hash)`. Extends both `Issuer` and `AddressIssuer(new_address)` TTLs. Blocked if status is `Revoked`. |

### Reads and TTL extension

| Function | Storage read | Extends TTL? |
|---|---|---|
| `get_admin()` | Instance: `Admin` | No |
| `get_issuer(issuer_id_hash)` | Persistent: `Issuer(issuer_id_hash)` | **Yes** — extends `Issuer` entry |
| `get_issuer_by_address(issuer_address)` | Persistent: `AddressIssuer(issuer_address)`, then `Issuer(...)` | **Yes** — extends `AddressIssuer` entry only; does **not** separately extend the `Issuer` entry |
| `is_active_issuer(issuer_id_hash)` | Delegates to `get_issuer` | **Yes** — via `get_issuer` |
| `is_active_address(issuer_address)` | Persistent: `AddressIssuer(issuer_address)`, then delegates to `is_active_issuer` | **Yes** — extends `AddressIssuer`; extends `Issuer` via `is_active_issuer` |

> **Indexer note:** `get_issuer_by_address` reads the `AddressIssuer` reverse-index and then loads the `Issuer` record directly without going through `get_issuer`. The `Issuer` entry's TTL is therefore **not** extended by `get_issuer_by_address`. If an indexer only ever calls `get_issuer_by_address`, the underlying `Issuer` entry will not be kept alive. To keep both entries alive, call `get_issuer` or `is_active_issuer` as well.

---

## `proof-registry` Contract

### Instance storage

| `DataKey` variant | Rust type | Description |
|---|---|---|
| `Admin` | `Address` | Stellar address of the proof registry administrator. |
| `IssuerRegistry` | `Address` | Contract address of the deployed `issuer-registry` instance. Set once at `initialize`. |
| `ProtocolConfig` | `Address` | Contract address of the deployed `protocol-config` instance. Set once at `initialize`. |

### Persistent storage

| `DataKey` variant | Rust type | Description |
|---|---|---|
| `Proof(BytesN<32>)` | `ProofRecord` | Full proof record keyed by the 32-byte SHA-256 hash of the proof's internal identifier. |

#### `ProofRecord` fields (defined in `packages/shared`)

| Field | Type | Description |
|---|---|---|
| `proof_id_hash` | `BytesN<32>` | SHA-256 hash of the proof's internal ID. |
| `commitment_hash` | `BytesN<32>` | SHA-256 hash of the canonical credential payload (excluding the signature). |
| `issuer_address` | `Address` | Stellar address of the issuer who registered this proof. |
| `status` | `ProofStatus` | Current status: `Active` or `Revoked`. |
| `schema_version` | `u32` | Credential schema version used for this proof. |
| `expires_at` | `u64` | Ledger timestamp after which this proof is no longer valid. |
| `created_at` | `u64` | Ledger timestamp at registration. |
| `revoked_at` | `u64` | Ledger timestamp at revocation. `0` if not revoked. |

#### `ProofStatus` values

| Value | Meaning |
|---|---|
| `Active` | Proof is registered and not revoked. Validity also depends on `expires_at`. |
| `Revoked` | Proof has been explicitly revoked. Terminal: no further status changes are possible. |

### Lifecycle events

| Event | Effect on storage |
|---|---|
| `initialize(admin, issuer_registry, protocol_config)` | Writes `Admin`, `IssuerRegistry`, `ProtocolConfig` to instance. Extends instance TTL. |
| `register_proof(proof_id_hash, commitment_hash, issuer_address, schema_version, expires_at)` | Validates: protocol not paused, schema version approved, issuer active, `expires_at` in the future, no duplicate. Writes `Proof(proof_id_hash)` to persistent with `status = Active`, `revoked_at = 0`. Extends `Proof` TTL. |
| `revoke_proof(proof_id_hash)` | Requires auth from `issuer_address` stored in the record. Sets `status = Revoked`, updates `revoked_at`. Extends `Proof` TTL. Panics if already revoked. |
| `admin_revoke_proof(proof_id_hash)` | Requires auth from `Admin`. Same state change as `revoke_proof`. |
| Expiration (no explicit call) | `is_valid_proof` returns `false` once `ledger.timestamp() > expires_at`. The `Proof` record is **not deleted**; it remains on-chain as an expired `Active` record. |

Note: There is no `update_proof` operation. Proof records are immutable after registration except for the `status` and `revoked_at` fields set during revocation.

### Reads and TTL extension

| Function | Storage read | Extends TTL? |
|---|---|---|
| `get_admin()` | Instance: `Admin` | No |
| `get_issuer_registry()` | Instance: `IssuerRegistry` | No |
| `get_protocol_config()` | Instance: `ProtocolConfig` | No |
| `get_proof(proof_id_hash)` | Persistent: `Proof(proof_id_hash)` | **Yes** — extends `Proof` entry |
| `is_valid_proof(proof_id_hash)` | Delegates to `get_proof` | **Yes** — via `get_proof` |
| `is_revoked(proof_id_hash)` | Delegates to `get_proof` | **Yes** — via `get_proof` |

---

## Cross-Contract TTL Dependencies

The `proof-registry` contract holds references to `issuer-registry` and `protocol-config` in its instance storage. These addresses are read by `get_issuer_registry()` and `get_protocol_config()`, which do **not** extend the proof registry's instance TTL. The instance TTL is only extended when `register_proof` writes a new proof.

The `proof-registry` makes cross-contract calls to `issuer-registry` (`is_active_address`) and `protocol-config` (`is_paused`, `is_schema_version_approved`) during `register_proof`. These calls extend the TTLs of the addressed contracts under their own extension policies.

---

## Privacy Boundaries

EarnProof is designed so that private financial data never reaches the chain. The table below is an explicit record of what is and is not stored.

### Data stored on-chain

| Data item | Contract | Storage key | Notes |
|---|---|---|---|
| Protocol admin address | `protocol-config` | `Admin` (instance) | Stellar address only |
| Protocol pause state | `protocol-config` | `Paused` (instance) | Boolean flag |
| Config change counter | `protocol-config` | `ConfigVersion` (instance) | Opaque integer |
| Schema version approval flag | `protocol-config` | `SchemaVersion(u32)` (persistent) | Boolean per version number |
| Issuer registry admin address | `issuer-registry` | `Admin` (instance) | Stellar address only |
| Issuer ID hash | `issuer-registry` | `Issuer(BytesN<32>)` (persistent) | SHA-256 of internal ID, not the ID itself |
| Issuer Stellar address | `issuer-registry` | `Issuer` record field + `AddressIssuer` (persistent) | Public Stellar address |
| Issuer public metadata hash | `issuer-registry` | `Issuer` record field | SHA-256 of public metadata blob, not the blob itself |
| Issuer status | `issuer-registry` | `Issuer` record field | `Active`, `Suspended`, or `Revoked` |
| Issuer registration and update timestamps | `issuer-registry` | `Issuer` record fields | Ledger timestamps |
| Proof registry admin address | `proof-registry` | `Admin` (instance) | Stellar address only |
| Issuer registry contract address | `proof-registry` | `IssuerRegistry` (instance) | Contract address |
| Protocol config contract address | `proof-registry` | `ProtocolConfig` (instance) | Contract address |
| Proof ID hash | `proof-registry` | `Proof(BytesN<32>)` (persistent) | SHA-256 of internal proof ID |
| Proof commitment hash | `proof-registry` | `Proof` record field | SHA-256 of canonical credential payload without signature |
| Issuer address on proof | `proof-registry` | `Proof` record field | Stellar address of issuing party |
| Proof status | `proof-registry` | `Proof` record field | `Active` or `Revoked` |
| Proof schema version | `proof-registry` | `Proof` record field | Integer version number |
| Proof expiration timestamp | `proof-registry` | `Proof` record field | Ledger timestamp |
| Proof creation and revocation timestamps | `proof-registry` | `Proof` record fields | Ledger timestamps; `revoked_at = 0` if not revoked |

### Data intentionally absent from on-chain storage

| Data item | Reason |
|---|---|
| Exact income amount | Private financial data; only a commitment hash is stored |
| Exact payment amount | Private financial data; only a commitment hash is stored |
| Raw transaction history | Private financial data |
| Wallet transaction lists | Private financial data |
| Personal name | Personal identifying information |
| Email address | Personal identifying information |
| Employment documents | Personal identifying information |
| Credential payload contents | Stored off-chain; only the SHA-256 commitment hash appears on-chain |
| Signature bytes | Stored off-chain with the credential; not required for on-chain status checks |
| Unencrypted personal information | Any form |

The backend must hash all identifiers and payloads before passing them to contract calls. See the [Backend Integration guide](./backend-integration.md) for the required hashing rules.

---

## Storage Key Summary

| Contract | `DataKey` variant | Storage class | Value type |
|---|---|---|---|
| `protocol-config` | `Admin` | Instance | `Address` |
| `protocol-config` | `Paused` | Instance | `bool` |
| `protocol-config` | `ConfigVersion` | Instance | `u32` |
| `protocol-config` | `SchemaVersion(u32)` | Persistent | `bool` |
| `issuer-registry` | `Admin` | Instance | `Address` |
| `issuer-registry` | `Issuer(BytesN<32>)` | Persistent | `IssuerRecord` |
| `issuer-registry` | `AddressIssuer(Address)` | Persistent | `BytesN<32>` |
| `proof-registry` | `Admin` | Instance | `Address` |
| `proof-registry` | `IssuerRegistry` | Instance | `Address` |
| `proof-registry` | `ProtocolConfig` | Instance | `Address` |
| `proof-registry` | `Proof(BytesN<32>)` | Persistent | `ProofRecord` |
