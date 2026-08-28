# Contract Invariants

This directory is the reviewed, code-linked state specification. Rust source is authoritative for enforcement; these documents describe the reachable states, guards, writes, events, and deliberately impossible transitions. A reference such as `contracts/proof-registry/src/lib.rs::register_proof` is checked by `scripts/check-doc-links.py`.

## Enforcement boundary

On-chain code enforces initialization, authorization, hashes-as-identifiers, status guards, schema approval, pause containment, expiry, duplicate prevention, and Soroban transaction atomicity. Backend policy must validate source payloads before hashing, protect private source documents, choose approved schemas, monitor TTLs, and ensure callers interpret inclusive expiry correctly. Backend policy cannot make an on-chain rejection succeed.

## Cross-contract assumptions

`proof-registry` trusts only the immutable-at-runtime addresses recorded by `initialize`; calls fail closed when a referenced contract is missing, expired, incompatible, paused, or returns false. A successful registration requires an active issuer address, an approved nonzero schema, and `expires_at > ledger.timestamp()`. A later validity query uses `ledger.timestamp() <= expires_at`; revocation is checked independently and dominates expiry. Soroban invocation rollback means a failed nested call leaves no proof, event, or dependency mutation.

## Privacy invariant

Storage and events may contain hashes, public issuer addresses, statuses, schema versions, and ledger timestamps only. Exact income, payment history, wallet identity as a private identity claim, and secret material are forbidden. The issuer wallet address is a public authorization key, not a private credential; backend source data never crosses the contract boundary. Positive and negative privacy coverage is in `tests/events/src/ghost.rs::a_rejected_call_changes_neither_events_nor_storage` and the per-operation `*_emits_no_event` tests.

See [protocol configuration](protocol-config.md), [issuer lifecycle](issuer-registry.md), and [proof lifecycle](proof-registry.md). Time interval rules are in [time semantics](../time-semantics.md).