# Ledger-Time Semantics

All time-sensitive behavior reads `Env::ledger().timestamp()`. Tests set this value explicitly through the reusable fixture in `tests/time/src/lib.rs`.

Registration uses an exclusive lower bound: `expires_at > now`. A value in the past, zero, or exactly equal to the current timestamp is rejected with `ProofExpired`. Validity uses an inclusive upper bound: an Active proof is valid when `now <= expires_at` and invalid when `now > expires_at`. Revocation dominates both cases and is terminal. Schema approval and pause are state guards, not time intervals.

`u64::MAX` is representable because registration performs no `now + interval` arithmetic; clients must reject values that overflow their own interval calculations before submitting. No wall-clock, sequence-number assumption, or far-future special case is used on-chain.

Positive and negative boundary coverage is in `tests/time/src/lib.rs::validity_is_inclusive_at_expiration_and_false_after`, `registration_requires_strictly_future_expiration`, `revocation_dominates_expiration`, and `maximum_timestamp_is_representable_without_interval_overflow`.