# Storage TTL, Expiration, and Restoration

## Observable TTL status

Each contract exposes `get_instance_ttl_status`. Protocol config also exposes
`get_schema_ttl_status(version)`, issuer registry exposes
`get_issuer_ttl_status(id_hash)` and `get_address_ttl_status(address)`, and
proof registry exposes `get_proof_ttl_status(id_hash)`. Callers must name a
critical key; contracts never enumerate record identifiers.

Soroban does not expose live-entry TTL inspection to contract WASM. The
contracts therefore store a conservative absolute `live_until` checkpoint
whenever they extend an instance or persistent entry. Status queries subtract
the current ledger with saturating arithmetic and return `Healthy`,
`NearExpiry`, or `Missing` plus the remaining ledgers and threshold. A missing
checkpoint (including state created before this feature) is reported as
`Missing`, never guessed. Governance can call `refresh_instance_ttl` after a
migration or restoration to establish instance tracking.

Status queries only read the target and checkpoint keys. They do not call
`extend_ttl`; keepalive automation must use the existing paths that explicitly
maintain the relevant entry.

This document is the operator guide for keeping EarnProof contract state alive on Stellar, and the reference for what happens when it is not. It answers three questions for every storage key the contracts use:

1. How long does the entry live, and what extends it?
2. What does a reader observe once it has expired?
3. Who is responsible for preventing that, and what does it cost?

The behaviour described here is pinned by the deterministic tests in [`tests/ttl/`](../tests/ttl/src/lib.rs). Every ledger number in this document appears as an assertion in that crate; if the contracts change, those tests fail before this document goes stale.

The key inventory itself - which contract owns which key and what it holds - is in the [Storage Model](./storage-model.md). This document covers only lifetime.

---

## The two constants

Both constants live in [`packages/shared/src/lib.rs`](../packages/shared/src/lib.rs) and are shared by all three contracts.

| Constant | Value | Approximate wall-clock\* |
|---|---|---|
| `TTL_THRESHOLD_LEDGERS` | `50,000` | ~3 days |
| `TTL_EXTEND_TO_LEDGERS` | `500,000` | ~29 days |

\* At a 5-second average ledger close time. Ledger sequence is the only thing the contracts actually depend on; wall-clock figures are for planning.

An entry written at ledger `S` and extended to the target lives until ledger `S + 500,000` **inclusive**. Its remaining TTL at ledger `L` is `S + 500,000 - L`.

---

## Extension is conditional, one-sided, and never automatic

`extend_ttl(threshold, extend_to)` rewrites an entry's live-until ledger only when **both** of the following hold:

- the remaining TTL is **at or below** `threshold`, and
- the new live-until ledger is further out than the current one.

Three consequences follow, each of which is a test:

| Remaining TTL when the entry is read | Effect of the read |
|---|---|
| `50,001` (threshold + 1) | No change. The entry keeps its existing live-until ledger. |
| `50,000` (exactly the threshold) | Extended to `500,000`. |
| `0` (final live ledger) | Extended to `500,000`. |

Repeated calls in the same ledger are idempotent, and a read can never shorten an entry. Nothing on the network extends an entry implicitly: an entry that is never touched will expire, no matter how busy the rest of the contract is.

---

## Key inventory: durability, trigger, and behaviour after expiry

Every key below is either **instance** or **persistent** durability. No contract uses temporary storage, and a test asserts that all three temporary stores stay empty. Instance and persistent entries share the same archival rules, so "restored on access" applies uniformly.

### `protocol-config`

| Key | Durability | Extension trigger | Threshold / target TTL | After expiry |
|---|---|---|---|---|
| `Admin` | Instance | Every state-mutating call (`initialize`, `set_admin`, `pause`, `unpause`, both schema calls) | 50,000 / 500,000 | Restored on next access |
| `Paused` | Instance | Same as `Admin` (shared instance entry) | 50,000 / 500,000 | Restored on next access |
| `ConfigVersion` | Instance | Same as `Admin` (shared instance entry) | 50,000 / 500,000 | Restored on next access |
| `SchemaVersion(u32)` | Persistent | `approve_schema_version`, `deprecate_schema_version`, and `is_schema_version_approved` when the key exists | 50,000 / 500,000 | Restored on next access |

A schema version that was never approved has **no entry at all**. `is_schema_version_approved` returns `false` for it and does not create one, so an absent flag and an archived flag are not confusable: the archived one comes back with its stored value, the absent one stays absent.

### `issuer-registry`

| Key | Durability | Extension trigger | Threshold / target TTL | After expiry |
|---|---|---|---|---|
| `Admin` | Instance | `initialize` only | 50,000 / 500,000 | Restored on next access |
| `Issuer(BytesN<32>)` | Persistent | `register_issuer`, `update_issuer`, any status change, `rotate_issuer_address`, `get_issuer`, `is_active_issuer`, `is_active_address` | 50,000 / 500,000 | Restored on next access |
| `AddressIssuer(Address)` | Persistent | `register_issuer`, `rotate_issuer_address` (new address only), `get_issuer_by_address`, `is_active_address` | 50,000 / 500,000 | Restored on next access |

**Operator note.** `get_issuer_by_address` reads the issuer record but extends only the reverse index. An indexer that polls exclusively through `get_issuer_by_address` will keep the index alive while letting the record it points at archive. Call `get_issuer` or `is_active_address` instead, or in addition, if you are relying on read traffic to keep issuer records live.

`rotate_issuer_address` removes the old `AddressIssuer` entry outright. A removed entry is gone; it is not archived, and it does not come back.

### `proof-registry`

| Key | Durability | Extension trigger | Threshold / target TTL | After expiry |
|---|---|---|---|---|
| `Admin` | Instance | `initialize` only | 50,000 / 500,000 | Restored on next access |
| `IssuerRegistry` | Instance | `initialize` only (shared instance entry) | 50,000 / 500,000 | Restored on next access |
| `ProtocolConfig` | Instance | `initialize` only (shared instance entry) | 50,000 / 500,000 | Restored on next access |
| `Proof(BytesN<32>)` | Persistent | `register_proof`, `revoke_proof`, `admin_revoke_proof`, `get_proof`, `is_valid_proof`, `is_revoked` | 50,000 / 500,000 | Restored on next access |

**Operator note.** `initialize` is the only proof-registry entry point that extends the instance entry. `register_proof` extends the proof entry it wrote, not the instance. A registry that is idle for longer than one archival window therefore pays a restoration for its instance entry on the next call, every window, indefinitely. This is correct but not free; see [Resource costs](#resource-costs) below.

---

## Storage expiry versus credential expiry

These are two unrelated clocks and must not be conflated.

| | Storage TTL | `ProofRecord.expires_at` |
|---|---|---|
| Measured in | Ledger sequence | Ledger timestamp |
| Set by | The contract's extension policy | The issuer at registration |
| Effect when passed | Entry is archived, then restored on access | `is_valid_proof` returns `false` |
| Reversible | Yes, by restoration | No |

A proof whose `expires_at` has passed is still a readable record with `status = Active`. Expiry is a validity rule, not a deletion: `get_proof` keeps returning the record so that a verifier can distinguish "expired on this date" from "never existed". A proof whose storage entry archived and was restored is still expired if its timestamp says so. Restoration never makes an expired credential valid again, and a revoked proof stays revoked with its original `revoked_at` intact.

---

## Restoration

Since protocol 23, accessing an archived **persistent** entry restores it automatically inside the same invocation rather than rejecting the call. What restoration does and does not do:

- **Does** return the entry byte for byte. The record's issuer address, status, timestamps, and commitment hash are exactly what they were.
- **Does** give the restored entry the host minimum TTL (`min_persistent_entry_ttl - 1`, i.e. 4,095 ledgers on a default network). The contract's own extension policy then applies on top, so a call that reads through `get_proof` leaves the entry at the full 500,000.
- **Does not** change authorization. A restored proof can still only be revoked by the issuer recorded on it, or by the registry admin. Authorization is re-evaluated from the restored record on every call.
- **Does not** allow re-registration. `register_proof` on a restored identifier fails with `ProofAlreadyRegistered`, so an archived commitment cannot be quietly replaced with a different one.
- **Does not** apply to temporary storage, which is unrecoverable once expired. The contracts do not use temporary storage, so this is a constraint on future changes rather than on the current deployment.

Restoration is emulated faithfully in the Soroban test environment, which is what makes the assertions in [`tests/ttl/src/restoration.rs`](../tests/ttl/src/restoration.rs) meaningful rather than approximate.

---

## Missing state

Archival is not the only way state can be absent. The contracts distinguish these cases explicitly:

| Situation | Result |
|---|---|
| Proof identifier was never registered | `get_proof` returns `ProofNotFound`; `is_valid_proof` and `is_revoked` return `false` |
| Revocation of an unknown proof | `ProofNotFound`, from both `revoke_proof` and `admin_revoke_proof` |
| Proof registry not initialized | `get_admin`, `get_issuer_registry`, `get_protocol_config` return `NotInitialized`; `register_proof` returns `ProofNotFound` and writes nothing |
| Issuer address not in the reverse index | `is_active_address` returns `false`; `get_issuer_by_address` returns `IssuerAddressNotFound` |
| Schema version never approved | `is_schema_version_approved` returns `false`; `register_proof` returns `SchemaVersionNotApproved` |
| Protocol config not initialized | `is_paused` returns `false` and `is_schema_version_approved` returns `false`, so registration is blocked by the schema check rather than the pause check |

The pause flag defaulting to `false` on an uninitialized config is deliberate: the pause switch is an emergency brake that must be explicitly engaged, and an uninitialized config already blocks every registration because no schema version can be approved. A registry pointed at an empty protocol config therefore fails closed, with `SchemaVersionNotApproved`.

---

## Resource costs

A restoring call is more expensive than a live one. Relative to the same call against a live entry, it adds:

- one **ledger write** per restored entry, even for a call that is otherwise read-only, and
- one **persistent entry rent bump** per restored entry, priced by entry size and by how far the TTL is being extended.

An invocation that restores the proof-registry instance *and* a proof entry pays both. The relationship is asserted rather than estimated: the tests compare `write_entries` and `persistent_entry_rent_bumps` between a live read and a restoring read of the same function. Absolute fee figures depend on network rent pricing and are deliberately not pinned here; use [`scripts/measure-resources.ps1`](../scripts/measure-resources.ps1) against the target network for current numbers.

---

## Operational responsibilities

Extension is the operator's job. Nothing in the protocol does it on a schedule.

| Responsibility | Owner | Cadence |
|---|---|---|
| Keep the three contract instances live | Deployment operator | At least once per 450,000 ledgers (~26 days) |
| Keep active issuer records and reverse indexes live | Registry operator | At least once per 450,000 ledgers, per issuer |
| Keep proofs that must stay verifiable live | Backend / issuer | At least once per 450,000 ledgers, per proof |
| Keep approved schema flags live | Protocol operator | At least once per 450,000 ledgers, per approved version |

**Why 450,000.** Extension only takes effect once the remaining TTL has fallen to 50,000 or below. Touching an entry every `500,000 - 50,000 = 450,000` ledgers lands each touch exactly at the trigger point: any earlier is a no-op that costs a call for nothing, any later risks an archival window. The tests run twenty consecutive cycles at this cadence and assert the entry never leaves the target TTL.

**Recommended practice.**

- Drive extension from reads that the contracts already extend on (`get_proof`, `is_active_address`, `is_schema_version_approved`) rather than from bespoke calls.
- For issuer records, prefer `is_active_address` over `get_issuer_by_address`: it extends both the record and the index.
- Treat archival as acceptable but not free for cold state. Proofs that no longer need to be verifiable can be allowed to archive; they remain restorable, so nothing is lost, and the restoration cost falls on whoever eventually reads them.
- Do not rely on archival as a deletion mechanism. Archived state is preserved, not destroyed, and it comes back with its original contents and its original authorization rules.

---

## Test coverage

| Concern | Tests |
|---|---|
| Extension trigger boundaries, idempotence, long-run cadence | [`tests/ttl/src/extension.rs`](../tests/ttl/src/extension.rs) |
| Pre-expiry, final live ledger, first archived ledger, index divergence | [`tests/ttl/src/expiry.rs`](../tests/ttl/src/expiry.rs) |
| Restoration contents, TTL, cost, authorization, re-registration | [`tests/ttl/src/restoration.rs`](../tests/ttl/src/restoration.rs) |
| Uninitialized contracts, unknown records, unapproved schemas | [`tests/ttl/src/missing_state.rs`](../tests/ttl/src/missing_state.rs) |
