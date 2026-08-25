# Backend Integration

This document lists the contract calls the EarnProof API should use when writing proof commitments, reading issuer status, and validating public proof state.

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
