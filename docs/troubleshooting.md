# Contract client integration troubleshooting

Symptom-to-cause workflows for backend and indexer teams integrating against
the EarnProof contracts, without guessing from a raw host error.

Every example below uses a synthetic address (`GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF`,
the same placeholder [`scripts/deployment-manifest.example.json`](../scripts/deployment-manifest.example.json)
uses) and a versioned contract interface. Never paste a real secret, seed
phrase, or signed transaction into a bug report or log excerpt — see
[Safe evidence](#safe-evidence) below for what is and isn't safe to share.

## How to use this guide

1. Find the category matching your symptom.
2. Follow the workflow to identify the layer at fault (client, RPC, network,
   or contract).
3. Jump to [Escalation](#escalation) once you know which layer is responsible.

## Invocation and simulation failures

**Symptom:** a `simulateTransaction` or `sendTransaction` call fails before
you ever see a contract error code.

| Cause | How to tell | Fix |
|---|---|---|
| Wrong contract ID for the network | Simulation fails immediately with a "contract not found" style host error | Confirm the contract ID against the manifest for the network you're targeting (`scripts/deployment-manifest.testnet.json` for testnet). A mainnet ID against testnet RPC (or vice versa) fails the same way as a typo. |
| Function name or argument count/order mismatch | Simulation fails with an argument-count or type-mismatch error before any contract logic runs | Diff your call against the contract's actual exported function signature in `contracts/<name>/src/lib.rs`, not against a cached copy of an older interface. |
| Footprint / read-write set exceeded | Simulation succeeds but returns a footprint your client didn't request, or the real submission fails after a successful simulation | Re-simulate immediately before submitting — don't reuse a stale simulation's footprint. |

Reproduce locally: [`docs/local-development.md`](./local-development.md)
describes the local sandbox flow (`cargo test` for logic-only, the sandbox
for install/deploy/invoke wiring).

## Authorization failures

**Symptom:** the call reaches the contract but is rejected with an
authorization-related error.

Every privileged entry point requires the caller's `require_auth()` to
succeed for the specific address the contract checks against — usually the
protocol admin or the issuer/proof owner, never a blanket "any signer."

| Cause | How to tell | Fix |
|---|---|---|
| Signed with the wrong key | The address the client signed with doesn't match the address the contract expects for this operation | Confirm which address the operation actually requires — e.g. issuer operations require the issuer's own address, not the protocol admin's, and vice versa. |
| Nested/cross-contract call missing its own auth | A call that itself invokes another contract (e.g. proof-registry checking issuer-registry state) fails only on the nested call | The outer call's auth does not automatically authorize the inner one; each `require_auth()` in the invocation tree needs its own valid entry. |
| Replayed or stale authorization entry | An authorization that worked once is rejected on a later call | Soroban authorization entries are scoped to one invocation; build a fresh one per call rather than reusing a captured entry. |

`IssuerError` and `ProofError` (`packages/shared/src/lib.rs`) do not encode
*why* an authorization failed — that detail comes from the host's own
`require_auth` failure, before any contract-level error is reached. If you
only see `Unauthorized` (code 20, `ContractError`), the call reached contract
logic and failed a stored-address comparison; if you see a bare host auth
error with no contract error code, it never reached that comparison.

## Decoding and type-mismatch failures

**Symptom:** the call succeeds on-chain, but your client fails to decode the
return value or an event payload.

| Cause | How to tell | Fix |
|---|---|---|
| Client interface out of sync with the deployed contract | Decoding fails on a field that doesn't exist, or succeeds but produces obviously wrong values | Check the contract's `schemaVersions` (also in the deployment manifest) against what your client expects. See [`docs/compatibility.md`](./compatibility.md) for what a schema version actually promises. |
| Hash/bytes field decoded as the wrong type | A 32-byte hash decodes as garbled text, or a `BytesN<32>` is read as a variable-length `Bytes` | Match the exact field type from the contract source, not from an older or hand-written type mapping. |

## TTL / storage-expiry failures

**Symptom:** a read that worked before now returns "not found," or a write
fails referencing an expired entry.

See [`docs/storage-model.md`](./storage-model.md) for which storage durability
class (instance / persistent / temporary) each entry lives in and its TTL
extension behavior. A "not found" for an entry you know was written is, in
order of likelihood: (1) it genuinely expired and was never touched again to
extend its TTL, (2) you're reading against the wrong network/contract
instance, (3) it was legitimately revoked or removed by contract logic.

## Event failures

**Symptom:** an indexer expected an event that never arrived, or received one
it didn't expect.

[`docs/events.md`](./events.md) documents the two guarantees every
EarnProof event follows: exactly-once on success, and silence on failure — a
rejected invocation never publishes a partial or failure-shaped event. If
your indexer is missing an event for a transaction it can see succeeded
on-chain, check that you're reading from the correct network's RPC and that
your polling window covers the ledger the transaction landed in — not a
contract-side gap, since none is possible per that doc's guarantees.

## Network / RPC failures

**Symptom:** intermittent failures with no consistent contract-level cause.

| Cause | How to tell | Fix |
|---|---|---|
| Transient RPC unavailability | Fails once, succeeds on retry with identical inputs | Retry with backoff; this is the same pattern `scripts/verify-manifest.ps1`'s live-check mode already uses (`-MaxRetries`, `-TimeoutSeconds`). |
| Genuine network partition or RPC outage | Fails consistently across retries, across different endpoints | Not a client or contract defect — escalate to network status, don't keep retrying against a single endpoint indefinitely. |

## Escalation

Once you've identified the failing layer:

- **Client defect** (wrong ID, stale interface, wrong type decoding): fix in
  the integrating client; not an EarnProof contract issue.
- **RPC defect** (malformed responses, inconsistent simulation results): file
  against the RPC provider, not this repository.
- **Network defect** (partition, outage): wait or switch endpoints; not
  actionable in this repository.
- **Contract defect** (a documented invariant from
  [`docs/invariants/`](./invariants/) or [`docs/events.md`](./events.md) is
  violated): file an issue here with the reproduction below.

## Safe evidence

When filing an issue or sharing diagnostic output:

- **Safe to share:** contract IDs, network identifiers, transaction hashes,
  ledger sequence numbers, error codes, synthetic/test addresses, and the
  exact CLI command you ran (with real addresses replaced by synthetic ones
  matching the style in this guide).
- **Never share:** secret keys, seed phrases, signed transaction envelopes
  from a funded account, or any real user's wallet address or transaction
  history. `scripts/verify-manifest.ps1` already rejects secret-shaped
  content in deployment manifests and release notes for the same reason —
  apply the same standard to bug reports and logs.

A minimal reproduction should use the local sandbox
([`docs/local-development.md`](./local-development.md)) with synthetic
addresses generated fresh for the report, not addresses copied from a real
deployment.
