# Proof Registry Specification

## States and transitions

Each proof is `Absent`, `Active`, or terminal `Revoked`. `expires_at` is a ledger timestamp, not wall-clock time. Registration requires `expires_at > now`; validity is inclusive through `expires_at` and false after it.

| Transition | Guard | Side effects and event | Impossible transition |
|---|---|---|---|
| `initialize` | Admin key absent; admin authenticates | Stores admin and dependency addresses | Second initialization |
| `register_proof` | Issuer authenticates; nonzero approved schema; protocol unpaused; active issuer; `expires_at > now`; proof id absent | Writes Active record and TTL; no success event currently emitted | Any failed guard; duplicate id |
| `revoke_proof` | Stored issuer authenticates; proof exists and is Active | Sets Revoked and `revoked_at=now` | Missing or already revoked proof |
| `admin_revoke_proof` | Current proof admin authenticates; proof exists and is Active | Same status write as issuer revocation | Missing or already revoked proof |
| `is_valid_proof` | Readable record | Returns `status == Active && now <= expires_at` | Revoked proof is never valid, even before expiry |

Implementation: `contracts/proof-registry/src/lib.rs::register_proof`, `set_revoked`, `is_valid_proof`, `get_proof`. Positive tests: `contracts/proof-registry/src/lib.rs::registers_and_validates_proof` and `issuer_can_revoke_proof`. Negative tests: `rejects_expired_proof`, `rejects_duplicate_proof_id`, `rejects_unapproved_schema_version`, `rejects_registration_when_protocol_is_paused`, and `tests/time/src/lib.rs::revocation_dominates_expiration`.

Cross-contract failure is explicit: missing or incompatible references abort registration; dependency false results are mapped to the current typed proof errors. The operation is atomic, so no proof record, TTL extension, or event remains after rejection. See `tests/cross-contract/src/boundaries.rs::registration_rolls_back_when_the_pause_read_fails` and `a_failed_registration_does_not_extend_the_schema_version_ttl`.