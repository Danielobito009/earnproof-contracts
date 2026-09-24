# Contract upgrade and migration strategy

This document is the operational companion to
[`docs/compatibility.md`](compatibility.md). Compatibility classifies *what
changed*; this document covers *how you actually ship it* — the upgrade
mechanism, what it can and cannot carry across, migration strategies for the
cases where it can't, testing procedures, and rollback.

## What "upgrade" means in these contracts

Each of the three contracts (`protocol-config`, `issuer-registry`,
`proof-registry`) exposes an admin-gated, allowlist-then-apply upgrade path:

```
approve_upgrade(wasm_hash, new_version)   # admin allowlists a WASM hash + target version
upgrade_contract(wasm_hash)               # admin applies it: env.deployer().update_current_contract_wasm(wasm_hash)
revoke_upgrade(wasm_hash)                 # admin removes an allowlisted hash without applying it
is_upgrade_allowed(wasm_hash) -> bool     # read-only check
```

`upgrade_contract` calls Soroban's native
`env.deployer().update_current_contract_wasm()`. **This is a real, in-place
code swap** — the contract ID does not change, and every existing
storage entry, every dependency address other contracts hold, and every
event subscription an indexer has already set up continues to point at the
same contract.

This contradicts a claim in [`docs/compatibility.md`](compatibility.md) and
the release template
([`docs/releases/TEMPLATE.md`](releases/TEMPLATE.md)), both of which state
"these contracts have no upgrade mechanism." That statement predates (or
was simply written without accounting for) `approve_upgrade`/
`upgrade_contract`, which are real, tested entry points on all three
contracts today. It is being corrected here rather than silently — anyone
who read that line and concluded "a breaking change always means a new
contract ID" was told something that stopped being true. What compatibility.md
gets right, and what remains true after this correction, is the harder
half of the problem: **the upgrade mechanism swaps code, not storage.**

## What the upgrade mechanism can and cannot do

| Can | Cannot |
|---|---|
| Replace the WASM behind an existing contract ID | Transform an already-stored value from an old shape to a new one |
| Preserve the contract ID, so dependency addresses in `protocol-config`/`issuer-registry`/`proof-registry` stay valid | Add a field to a stored struct without breaking every existing entry's decode |
| Preserve event subscriptions an indexer already has | Guarantee a downgrade path — `new_version` must be strictly greater than current, by design (`upgrade would not advance contract version`) |
| Advance `ContractVersion` atomically with the code swap, so `is_upgrade_allowed`/version checks stay consistent | Run arbitrary migration code — there is no "migration function" hook; the new WASM's code runs, unmodified, against the old storage layout |

The practical rule, stated in [`docs/compatibility.md`](compatibility.md)'s
Storage section and repeated here because it is the one migration teams
get wrong: **an ABI or event change can go through `upgrade_contract`
cleanly. A storage change cannot, unless the new code is written to
tolerate the old encoding.**

## Upgrade process

1. **Classify the change** using [`docs/compatibility.md`](compatibility.md).
   If it touches Storage and is anything other than "adding a new key
   variant," stop here — go to [Storage migration](#storage-migration-strategy)
   instead of the in-place path below.
2. **Build and record the artifact.** `stellar contract build`, then record
   the WASM SHA-256. The recorded hash must be the hash of what is actually
   deployed — this is enforced by `scripts/verify-manifest.ps1 -Release`.
3. **Write the release note** from
   [`docs/releases/TEMPLATE.md`](releases/TEMPLATE.md), including migration,
   rollback, and containment. For a breaking change, get the four
   [governance](compatibility.md#breaking-change-governance) items in place
   first — approval, migration plan, rollback plan, containment notes —
   before step 4.
4. **Allowlist:** admin calls `approve_upgrade(new_wasm_hash, new_version)`
   on each contract being upgraded. This emits `UpgradeAllowlisted` and does
   not change behavior yet — it is a staged, auditable commitment to a
   specific hash and version, separable in time from applying it.
5. **Verify the allowlist took effect:** `is_upgrade_allowed(wasm_hash)`
   returns `true`. Cross-check against the release note's recorded hash
   before proceeding — an allowlisted hash that doesn't match the release
   note is a sign something was built differently than what was reviewed.
6. **Apply:** admin calls `upgrade_contract(wasm_hash)`. This consumes the
   allowlist entry (replay-safe — the same hash cannot be applied twice
   without re-approving), swaps the WASM, advances `ContractVersion`, and
   emits `ContractUpgraded { new_wasm_hash, old_contract_version,
   new_contract_version, upgraded_by }`.
7. **Verify post-upgrade** using
   [`scripts/verify-manifest.ps1 -Live`](../scripts/verify-manifest.ps1)
   (read-only — see [issue #96](https://github.com/veridatum-labs/earnproof-contracts/issues/96)):
   confirm the contract responds correctly to its existing read-only calls
   against pre-upgrade state (e.g. `get_proof`/`get_issuer` for a record
   created before the upgrade still decodes and returns the expected
   values).
8. **Notify backend** of the new minimum version per the release note's
   `backend_compatibility` field, if it changed.

## Storage migration strategy

### Resumable migration checkpoints

All three contracts expose `begin_migration`, `advance_migration`, and
`get_migration_status`. The versioned status record contains the target
contract version, current cursor, total item count, and completion flag.
Governance starts a checkpoint before processing any bounded migration work,
then commits at most `MAX_MIGRATION_BATCH` (100) items per invocation.

Each step supplies the cursor it read. A replay of an already committed batch
returns the current status unchanged; a gap, overlap, rollback, zero-sized
batch, oversized batch, or advance beyond the declared total is rejected.
Normal state-changing operations are unavailable while a checkpoint is
incomplete. `upgrade_contract` also rejects an incomplete checkpoint or one
for a different target version, and consumes the completed record only after
the upgrade version advances. A failed or interrupted transaction therefore
resumes from the last committed cursor rather than restarting an unbounded
traversal.

There is no migration hook in the upgrade mechanism (see above) — a storage
layout change is only ever safe under `upgrade_contract` if the new code's
deserialization tolerates the old bytes (for example, an `Option<T>` field
that decodes `None` for entries written before it existed, which the
Soroban SDK's contract-type derive does not currently do implicitly). Two
real strategies exist when it doesn't:

### Strategy A — new key variant, dual-read

Adding a *new* `DataKey` variant is Additive (per
[`docs/compatibility.md`](compatibility.md#2-storage)) and safe under
`upgrade_contract`. Existing entries under the old key keep working
unmodified; new writes go to the new key; reads check the new key first
and fall back to the old one. This defers the actual migration — old
entries are never rewritten — in exchange for permanent dual-read logic in
every accessor that touches the migrated field. Appropriate when the
field is read infrequently or the old entries will naturally expire (see
`TTL_THRESHOLD_LEDGERS`/`TTL_EXTEND_TO_LEDGERS`).

### Strategy B — new contract ID, off-chain re-anchoring

When the old data must not be read forward at all (the new shape isn't a
superset of the old one, or an old entry's data is being deliberately
narrowed), deploy the new WASM to a **new** contract ID rather than
`upgrade_contract`-ing the existing one. This is the "no upgrade path"
scenario `docs/compatibility.md` describes for genuinely incompatible
storage changes — the old contract ID keeps running the old code
indefinitely (or is `pause()`-d), and every consumer (backend, indexer,
dependency addresses in the other two contracts) is re-pointed to the new
ID. This is expensive specifically because it is not a code swap: every
existing off-chain reference to the old contract ID needs updating, which
is why [`docs/compatibility.md`](compatibility.md) treats a breaking
storage change as the costliest class of change in this system.

## Example: schema version deprecation flow

Schema versions live in `protocol-config`, keyed by `DataKey::SchemaVersion(u32)`,
and are a governance decision (`approve_schema_version`/
`deprecate_schema_version`) rather than a code change — no `upgrade_contract`
call is needed for this specific flow, which is why it's worth walking
through separately from the WASM-upgrade process above.

1. Admin calls `deprecate_schema_version(version)`. This does not touch or
   invalidate any existing `Proof` records already anchored under that
   schema version — `proof-registry`'s `is_valid_proof`/`get_proof` never
   re-check schema approval after registration, only `register_proof` does
   (via `is_schema_version_approved` at write time).
2. `SchemaDeprecated { version }` is emitted. Backend/indexers watching this
   event stop offering the deprecated version for new submissions.
3. New `register_proof` calls against the deprecated version now fail —
   `is_schema_version_approved` returns `false`, and `proof-registry`
   rejects the registration before it reaches storage.
4. Existing proofs anchored under the deprecated version remain fully valid
   and queryable indefinitely; deprecation is forward-only (blocks new
   writes) and does not retroactively invalidate history. If a schema
   version must be actively repudiated (not just deprecated), that is a
   different, currently undefined operation — flagging this gap rather
   than inventing a `revoke_schema_version` that does not exist in the
   contract today.
5. Rollback: `approve_schema_version(version)` re-approves it. Since
   deprecation never touched existing records, re-approval fully restores
   the prior state with no data loss.

## Example: issuer registry migration scenario

`issuer-registry` supports address rotation for an existing issuer without
changing its identity (`issuer_id_hash`) or losing its proof history:

```
rotate_issuer_address(issuer_id_hash, old_address, new_address)
```

1. The issuer (or an admin, depending on authorization configured for this
   call — see the entry point's `require_auth` target in
   `contracts/issuer-registry/src/lib.rs`) initiates rotation when a
   signing key is compromised, expiring, or being moved to new
   infrastructure.
2. The registry updates `IssuerRecord.issuer_address` in place, keyed by
   the unchanged `issuer_id_hash` — every `Proof` record in
   `proof-registry` that references this issuer by `issuer_address`
   continues to resolve correctly, because `is_active_address` and
   `get_issuer_by_address` are re-derived from current state, not cached
   at proof-registration time.
3. This is **not** a storage migration in the ABI/upgrade sense above — no
   WASM changes, no new contract ID. It is the registry's existing,
   intended mechanism for exactly this operational need, which is why an
   issuer changing custody of their signing key is a Semantic event
   (behavior changes, interface doesn't) rather than a Breaking one.
4. Verification: after rotation, `is_active_address(new_address)` returns
   `true` and `is_active_address(old_address)` returns `false`;
   `get_issuer(issuer_id_hash)` reflects the new address; any proof
   registered before rotation is unaffected and still resolves via
   `get_proof`.

## Upgrade testing procedures

Before any `approve_upgrade`/`upgrade_contract` sequence reaches a network
where consumers depend on the result:

1. **Unit and integration tests pass** — `cargo test --workspace` — against
   the new WASM's source, including any new tests the change itself
   requires per [`docs/compatibility.md`](compatibility.md)'s classification.
2. **Build the artifact and record its hash** exactly as it will be
   deployed — `stellar contract build`, not a debug build; the two produce
   different bytes.
3. **Rehearse the upgrade sequence against a sandbox/testnet deployment**
   of the *current* production contract state (not a fresh contract) —
   `approve_upgrade` → `upgrade_contract` → re-run
   `scripts/verify-manifest.ps1 -Live` against it. This is the only way to
   catch a storage-decode failure before it happens against real state,
   since `cargo test` exercises the new contract's own test fixtures, not
   an existing contract's accumulated storage.
4. **Confirm the downgrade guard fires as expected** — attempting to
   `approve_upgrade` a version not strictly greater than current must
   panic with `new_version must be greater than current contract version`,
   and attempting to `upgrade_contract` a hash whose allowlisted version is
   not greater than current must panic with `upgrade would not advance
   contract version`. Both guards exist specifically to prevent an
   accidental or malicious downgrade; a rehearsal that skips confirming
   they still fire is incomplete.
5. **Confirm events** — `ContractUpgraded` carries `old_contract_version`
   and `new_contract_version`; verify both match the rehearsal's expected
   values, since a mismatch here would mean the version bookkeeping and
   the actual applied hash have drifted apart.

## Rollback procedures

Rollback differs by change class, and getting the two conflated is the
single most costly mistake this document exists to prevent:

- **The upgrade mechanism itself can be "rolled back"** by allowlisting and
  applying the previous WASM hash as a new "upgrade" — nothing prevents
  `approve_upgrade(previous_hash, current_version + 1)` followed by
  `upgrade_contract(previous_hash)`, since the version guard only requires
  monotonic increase, not that the code be new. **This only restores
  correct behavior if storage is unchanged between the two versions** —
  the same downgrade-guard-by-version-number does not, and cannot, protect
  against a storage-shape mismatch between the code being restored and the
  data that accumulated while the newer code ran.
- **A storage-incompatible change has no in-place rollback.** Per
  [`docs/compatibility.md`](compatibility.md#why-rollback-is-hard-here):
  "redeploy the old WASM" only qualifies as a rollback if state travels
  with it, and for an incompatible storage change it does not. The
  practical rollback is the same as Strategy B above: a new contract ID
  running the old code, with consumers re-pointed, and the data written
  under the new (now-rolled-back) shape either discarded or migrated by
  hand depending on what it represents.
- **Every release note must state which case applies** — this is the
  `## Rollback` section required by
  [`docs/releases/TEMPLATE.md`](releases/TEMPLATE.md), and
  `scripts/verify-manifest.ps1 -Release` fails a release note claiming a
  breaking change without one.

## Coordination with backend during upgrade

An upgrade that changes any of the three things
[`docs/compatibility.md`](compatibility.md#backend-compatibility) lists —
invocation signatures, hash construction, or schema versions — requires
backend coordination, not just a contract-side release note:

1. Backend must be updated to a version compatible with the new contract
   **before** `upgrade_contract` is called, if the change is Breaking on
   invocation (the backend would otherwise submit calls the new WASM
   rejects at the ABI level).
2. If the change is Additive or Semantic only, backend can update on its
   own schedule after the upgrade, per the release note's
   `backend_compatibility` field.
3. Hash construction (`proof_id_hash`, `commitment_hash`, `metadata_hash`)
   changes are invisible on-chain — the contracts treat these as opaque
   `BytesN<32>` and never verify their construction (see
   [`docs/encoding.md`](encoding.md)) — so a hashing change must ship to
   backend and contracts in lockstep, verified against the shared test
   vectors in `tests/fixtures/encoding/` (see
   [issue #98](https://github.com/veridatum-labs/earnproof-contracts/issues/98)),
   not discovered after proofs stop matching in production.

## Timeline estimates

These are planning estimates for a maintainer sequencing an upgrade, not
service-level commitments:

| Step | Estimate |
|---|---|
| Classification + release note drafting | 1–2 hours for an additive change; a full day for a breaking change requiring governance sign-off |
| Build + hash recording | Minutes |
| Sandbox/testnet rehearsal (§ Upgrade testing procedures) | 1–2 hours, including the downgrade-guard checks |
| Allowlist → verify → apply on target network | Minutes, but do not compress the gap between `approve_upgrade` and `upgrade_contract` — that gap is what makes the allowlist auditable before it's irreversible |
| Post-upgrade verification | 15–30 minutes running `scripts/verify-manifest.ps1 -Live` |
| Backend coordination lead time | Highly variable; for a hash-construction change, backend and contract deploys should be scheduled together, not sequentially |

A breaking change should budget at least one full business day between
"release note approved" and "applied to a network consumers depend on,"
to leave room for the governance sign-offs in
[`docs/compatibility.md`](compatibility.md#breaking-change-governance) to
actually happen rather than being rushed alongside the technical steps.
