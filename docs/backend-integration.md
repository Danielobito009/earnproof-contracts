# Backend Integration

This document lists the contract calls the EarnProof API should use when writing proof commitments, reading issuer status, and validating public proof state.

**See Also:** [Executable Examples](./executable-examples.md) for runnable demonstrations of all contract invocation patterns.

## Error Handling

All contracts return typed Soroban error codes instead of panic strings. Backend integrations must map these machine-readable codes to appropriate HTTP status codes and user-facing messages.

### Error Code Ranges

Error codes are allocated to prevent collisions across contracts:

- **Common errors (1-99)**: Shared across all contracts
- **Protocol Config errors (100-199)**: Protocol-specific errors
- **Issuer Registry errors (200-299)**: Issuer-specific errors
- **Proof Registry errors (300-399)**: Proof-specific errors

### Common Contract Errors (1-99)

| Code | Error Name | Description | Suggested HTTP Status | Safe API Response |
|------|------------|-------------|----------------------|-------------------|
| 1 | AlreadyInitialized | Contract already initialized | 409 Conflict | "Contract is already initialized" |
| 2 | NotInitialized | Contract not initialized | 500 Internal Server Error | "Service temporarily unavailable" |
| 20 | Unauthorized | Caller lacks required authorization | 403 Forbidden | "Insufficient permissions" |
| 40 | AlreadyExists | Resource already exists | 409 Conflict | "Resource already exists" |
| 41 | NotFound | Resource not found | 404 Not Found | "Resource not found" |
| 42 | InvalidState | Operation invalid for current state | 400 Bad Request | "Operation not permitted in current state" |
| 60 | InvalidInput | Invalid input parameters | 400 Bad Request | "Invalid input provided" |
| 80 | ProtocolPaused | Protocol is paused | 503 Service Unavailable | "Service temporarily paused" |

### Issuer Registry Errors (200-299)

| Code | Error Name | Description | Suggested HTTP Status | Safe API Response |
|------|------------|-------------|----------------------|-------------------|
| 200 | IssuerAlreadyRegistered | Issuer ID already registered | 409 Conflict | "Issuer already registered" |
| 201 | IssuerNotFound | Issuer not found | 404 Not Found | "Issuer not found" |
| 202 | IssuerAddressAlreadyRegistered | Issuer address already in use | 409 Conflict | "Issuer address already registered" |
| 203 | IssuerAddressNotFound | Issuer address not found | 404 Not Found | "Issuer address not found" |
| 204 | IssuerRevoked | Issuer has been revoked | 403 Forbidden | "Issuer has been revoked" |
| 205 | IssuerInactive | Issuer is not active | 403 Forbidden | "Issuer is not active" |
| 206 | InvalidTransition | Invalid status transition | 400 Bad Request | "Invalid status transition" |

### Proof Registry Errors (300-399)

| Code | Error Name | Description | Suggested HTTP Status | Safe API Response |
|------|------------|-------------|----------------------|-------------------|
| 300 | ProofAlreadyRegistered | Proof ID already registered | 409 Conflict | "Proof already registered" |
| 301 | ProofNotFound | Proof not found | 404 Not Found | "Proof not found" |
| 302 | ProofAlreadyRevoked | Proof has already been revoked | 400 Bad Request | "Proof already revoked" |
| 303 | ProofExpired | Proof expiration is invalid | 400 Bad Request | "Invalid proof expiration" |
| 304 | InvalidSchemaVersion | Schema version is invalid | 400 Bad Request | "Invalid schema version" |
| 305 | SchemaVersionNotApproved | Schema version not approved | 400 Bad Request | "Schema version not approved" |

### Error Handling Best Practices

1. **Never expose raw contract errors to end users**: Map error codes to safe, user-friendly messages
2. **Log full error context server-side**: Include contract error codes, transaction IDs, and context for debugging
3. **No sensitive data in error messages**: Do not include personal information, wallet addresses, or internal IDs in user-facing errors
4. **Consistent HTTP status codes**: Use the suggested HTTP status codes for consistency
5. **Graceful degradation**: Handle unexpected error codes gracefully with generic error messages

## Protocol Config

Contract responsibility:

- Store protocol administrator.
- Store pause state.
- Store approved schema versions.
- Expose a configuration version counter.

Backend reads:

```text
get_admin() -> Address
is_paused() -> bool
is_schema_version_approved(version: u32) -> bool
get_config_version() -> u32
```

Backend writes:

```text
approve_schema_version(version: u32)
deprecate_schema_version(version: u32)
pause()
unpause()
```

Admin authorization is required for writes.

### Protocol Config Events

The protocol-config contract emits typed events on every state mutation. Backend indexers should subscribe to these topics for real-time protocol lifecycle tracking.

| Event | Topic | Payload | Emitted by | Fixture |
| --- | --- | --- | --- | --- |
| Initialized | `Initialized` | `admin: Address` | `initialize` | [initialized.json](../tests/fixtures/events/protocol-config/v1/initialized.json) |
| AdminChanged | `AdminChanged` | `new_admin: Address` | `set_admin` | [admin-changed.json](../tests/fixtures/events/protocol-config/v1/admin-changed.json) |
| Paused | `Paused` | `paused: bool` | `pause` | [paused.json](../tests/fixtures/events/protocol-config/v1/paused.json) |
| Unpaused | `Unpaused` | `paused: bool` | `unpause` | [unpaused.json](../tests/fixtures/events/protocol-config/v1/unpaused.json) |
| SchemaApproved | `SchemaApproved` | `version: u32` | `approve_schema_version` | [schema-approved.json](../tests/fixtures/events/protocol-config/v1/schema-approved.json) |
| SchemaDeprecated | `SchemaDeprecated` | `version: u32` | `deprecate_schema_version` | [schema-deprecated.json](../tests/fixtures/events/protocol-config/v1/schema-deprecated.json) |

See [tests/fixtures/events/](../tests/fixtures/events/) for the fixture schema, versioning rules, and compatibility guarantees.

## Issuer Registry

Contract responsibility:

- Store approved issuer records.
- Store issuer status.
- Store public metadata hash.
- Rotate issuer wallet addresses.
- Resolve issuer records by ID hash or Stellar address.

Backend reads:

```text
get_issuer(issuer_id_hash: BytesN<32>) -> IssuerRecord
get_issuer_by_address(issuer_address: Address) -> IssuerRecord
is_active_issuer(issuer_id_hash: BytesN<32>) -> bool
is_active_address(issuer_address: Address) -> bool
```

Backend writes:

```text
register_issuer(issuer_id_hash: BytesN<32>, issuer_address: Address, metadata_hash: BytesN<32>)
update_issuer(issuer_id_hash: BytesN<32>, metadata_hash: BytesN<32>)
suspend_issuer(issuer_id_hash: BytesN<32>)
reactivate_issuer(issuer_id_hash: BytesN<32>)
revoke_issuer(issuer_id_hash: BytesN<32>)
rotate_issuer_address(issuer_id_hash: BytesN<32>, new_address: Address)
```

Admin authorization is required for writes.

### Issuer Registry Events

The issuer-registry contract currently emits no typed events. State changes are stored on-chain via `IssuerRecord` updates but are not announced via Soroban event topics. Future contract versions are expected to add events for issuer lifecycle transitions. See [tests/fixtures/events/issuer-registry/](../tests/fixtures/events/issuer-registry/).
Every successful mutation emits exactly one typed event. Failed, unauthorized, or duplicate operations emit no success event.

All payloads contain only public hashes, addresses, status, and timestamps. No personal data, salary, or payment amounts are included.

#### `IssuerRegistered`

Emitted when an issuer is successfully registered for the first time.

Topic: `issuer_registered`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `issuer_id_hash` | `BytesN<32>` | SHA-256 hash of the issuer's internal ID |
| `issuer_address` | `Address` | On-chain Stellar wallet address |
| `metadata_hash` | `BytesN<32>` | SHA-256 hash of the issuer's public metadata |
| `created_at` | `u64` | Ledger timestamp at registration time |

#### `IssuerMetadataUpdated`

Emitted when the issuer's public metadata hash is replaced.

Topic: `issuer_metadata_updated`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `issuer_id_hash` | `BytesN<32>` | SHA-256 hash of the issuer's internal ID |
| `metadata_hash` | `BytesN<32>` | New SHA-256 hash of the issuer's public metadata |
| `updated_at` | `u64` | Ledger timestamp at update time |

#### `IssuerSuspended`

Emitted when an active or previously-suspended issuer is suspended.

Topic: `issuer_suspended`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `issuer_id_hash` | `BytesN<32>` | SHA-256 hash of the issuer's internal ID |
| `updated_at` | `u64` | Ledger timestamp at suspension time |

#### `IssuerReactivated`

Emitted when a suspended issuer is restored to active status.

Topic: `issuer_reactivated`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `issuer_id_hash` | `BytesN<32>` | SHA-256 hash of the issuer's internal ID |
| `updated_at` | `u64` | Ledger timestamp at reactivation time |

#### `IssuerRevoked`

Emitted when an issuer is permanently revoked. Revocation is irreversible.

Topic: `issuer_revoked`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `issuer_id_hash` | `BytesN<32>` | SHA-256 hash of the issuer's internal ID |
| `updated_at` | `u64` | Ledger timestamp at revocation time |

#### `IssuerAddressRotated`

Emitted when the issuer's on-chain wallet address is rotated to a new address. Both old and new addresses are included so indexers can update their mapping atomically without scanning storage.

Topic: `issuer_address_rotated`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `issuer_id_hash` | `BytesN<32>` | SHA-256 hash of the issuer's internal ID |
| `old_address` | `Address` | Previous on-chain wallet address |
| `new_address` | `Address` | Replacement on-chain wallet address |
| `updated_at` | `u64` | Ledger timestamp at rotation time |

## Proof Registry

Contract responsibility:

- Store proof commitment records.
- Reject duplicate proof IDs.
- Reject expired proof registrations.
- Revoke proof records.
- Expose issuer registry and protocol config contract references.

Backend reads:

```text
get_proof(proof_id_hash: BytesN<32>) -> ProofRecord
is_valid_proof(proof_id_hash: BytesN<32>) -> bool
is_revoked(proof_id_hash: BytesN<32>) -> bool
get_issuer_registry() -> Address
get_protocol_config() -> Address
```

Backend writes:

```text
register_proof(
  proof_id_hash: BytesN<32>,
  commitment_hash: BytesN<32>,
  issuer_address: Address,
  schema_version: u32,
  expires_at: u64
)
revoke_proof(proof_id_hash: BytesN<32>)
admin_revoke_proof(proof_id_hash: BytesN<32>)
```

Issuer authorization is required for normal proof registration and revocation. Admin authorization is required for administrative revocation.

### Proof Registry Events

The proof-registry contract currently emits no typed events. Proof lifecycle changes (registration, revocation) are stored on-chain via `ProofRecord` updates but are not announced via Soroban event topics. Future contract versions are expected to add events for proof registration and revocation. See [tests/fixtures/events/proof-registry/](../tests/fixtures/events/proof-registry/).
Every successful mutation emits exactly one typed event. Failed, unauthorized, duplicate, expired, or paused-protocol operations emit no success event.

All payloads contain only public hashes, addresses, schema version, timestamps, and expiration. No payment amounts, wallet history, personal names, or raw credential data are included.

#### `ProofRegistered`

Emitted when a proof commitment is successfully registered.

Topic: `proof_registered`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `proof_id_hash` | `BytesN<32>` | SHA-256 hash of the proof's internal ID |
| `commitment_hash` | `BytesN<32>` | SHA-256 hash of the canonical credential payload (without signature) |
| `issuer_address` | `Address` | On-chain address of the issuer that registered the proof |
| `schema_version` | `u32` | Approved schema version used for this proof |
| `expires_at` | `u64` | Ledger timestamp after which the proof is no longer valid |
| `created_at` | `u64` | Ledger timestamp at registration time |

#### `ProofRevokedByIssuer`

Emitted when the issuer that originally registered a proof revokes it. Distinguishable from admin revocation by the event name `proof_revoked_by_issuer`.

Topic: `proof_revoked_by_issuer`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `proof_id_hash` | `BytesN<32>` | SHA-256 hash of the proof's internal ID |
| `issuer_address` | `Address` | On-chain address of the revoking issuer |
| `revoked_at` | `u64` | Ledger timestamp at revocation time |

#### `ProofRevokedByAdmin`

Emitted when an admin revokes a proof. Distinguishable from issuer revocation by the event name `proof_revoked_by_admin`.

Topic: `proof_revoked_by_admin`

Payload fields:

| Field | Type | Description |
|---|---|---|
| `proof_id_hash` | `BytesN<32>` | SHA-256 hash of the proof's internal ID |
| `admin_address` | `Address` | On-chain address of the admin that performed the revocation |
| `revoked_at` | `u64` | Ledger timestamp at revocation time |

## Event Replay and Indexer Expectations

- Events are emitted only on success. Any panic or authorization failure before state mutation guarantees no success event is emitted.
- Each mutation emits at most one event. Indexers should not expect batched or partial emissions.
- Topics follow the snake_case convention derived automatically from the struct name by the `#[contractevent]` macro. No custom topic overrides are applied.
- Indexers can identify the actor type for revocations from the topic alone (`proof_revoked_by_issuer` vs `proof_revoked_by_admin`) without decoding the payload.
- Event data is encoded as a Soroban `Map` with field name keys in alphabetical order (the default `data_format = "map"` behavior of `#[contractevent]`).
- To replay from genesis, query the Horizon or RPC event endpoint for the contract address and filter by topic. Events are permanently available at the ledger they were emitted and do not expire.
- Address rotation events include both `old_address` and `new_address` so indexers can rebuild the address-to-issuer mapping without reading contract storage.

## Hashing Rules

The backend should hash public identifiers before passing them to contracts:

```text
proof_id_hash = sha256(proof_id)
issuer_id_hash = sha256(issuer_id)
commitment_hash = sha256(canonical_credential_payload_without_signature)
metadata_hash = sha256(canonical_public_issuer_metadata)
```

## On-Chain Data Boundary

Do not send exact income, raw transaction lists, personal names, emails, or full wallet history to contracts. Store only hashes, status, schema version, issuer address, expiration, and timestamps.

For the complete list of every storage key, TTL policy, lifecycle event, and privacy boundary see the [Storage Model](./storage-model.md) reference.
