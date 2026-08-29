# Issuer Registry Specification

## States and transitions

An issuer is `Active`, `Suspended`, or terminal `Revoked`. Registration creates one issuer record and one address reverse index. Address rotation atomically removes the old index and writes the new one.

| Transition | Guard | Side effects and event | Impossible transition |
|---|---|---|---|
| `register_issuer` | Current admin; id and address absent | Writes record and reverse index as Active; emits `IssuerRegistered` | Duplicate id/address |
| `update_issuer` | Current admin; record exists and is not Revoked | Changes only metadata and `updated_at`; emits `IssuerMetadataUpdated` | Update of revoked issuer |
| `suspend_issuer` | Current admin; record exists | Status Suspended; emits `IssuerSuspended` | Missing issuer |
| `reactivate_issuer` | Current admin; record exists and is not Revoked | Status Active; emits `IssuerReactivated` | Revoked to Active: `InvalidTransition` |
| `revoke_issuer` | Current admin; record exists | Status Revoked; emits `IssuerRevoked` | Revoked issuer can never become active |
| `rotate_issuer_address` | Current admin; exists, not Revoked, new address absent | Removes old reverse index, updates record, writes new index; emits `IssuerAddressRotated` | Reuse of an address or rotate revoked issuer |

Implementation: `contracts/issuer-registry/src/lib.rs::register_issuer`, `update_issuer`, `set_status`, `rotate_issuer_address`, `get_issuer_by_address`. Positive tests: `contracts/issuer-registry/src/lib.rs::registers_and_reads_active_issuer` and `status_transitions_reject_reactivated_revoked_issuer`. Negative tests: `rejects_duplicate_issuer_id`, `revoked_issuer_cannot_be_reactivated`, and `tests/events/src/ghost.rs::a_rejected_call_changes_neither_events_nor_storage`.

The reverse index is public address metadata. It must not be used to infer or store a person's identity, income, or payment history.