# EarnProof Contracts

EarnProof is an open-source, privacy-focused income and payment verification protocol built on Stellar.

This repository contains the Soroban contracts that support issuer trust, proof commitments, revocation status, and protocol configuration for EarnProof.

## Product Role

The contracts provide public status and trust primitives without storing private income data on-chain.

Contracts should answer questions such as:

- Is this issuer active?
- Was this proof commitment registered?
- Has this proof been revoked?
- Is this schema version approved?
- Are sensitive protocol operations paused?

Contracts must not calculate income, store salaries, store raw payment history, or custody user funds.

## Current Scope

Implemented:

- Rust workspace
- Shared on-chain record types
- `protocol-config` contract
- `issuer-registry` contract with issuer registration, status transitions, address rotation, and lookup helpers
- `proof-registry` contract with proof registration, expiration validation, revocation state, issuer checks, protocol pause checks, schema approval checks, and lookup helpers
- Typed protocol configuration events
- Storage TTL extension policy for durable and temporary entries
- Testnet deployment scripts using `stellar contract build`, issuer registration, and manifest validation
- Contract tests exercise authorization through Soroban mocked auth instead of compiling out `require_auth`
- Buildable contract crates against `soroban-sdk`
- Code-linked lifecycle invariants in [`docs/invariants/`](docs/invariants/)
- Deterministic ledger-time boundary tests and backend encoding vectors
- Generated API reference in [`docs/reference/`](docs/reference/)

The `protocol-config` contract currently supports:

- `initialize`
- `get_admin`
- `set_admin`
- `pause`
- `unpause`
- `is_paused`
- `approve_schema_version`
- `deprecate_schema_version`
- `is_schema_version_approved`
- `get_config_version`

Next:

- Run backend contract anchoring against deployed contract IDs
- Publish explorer links for deployment and proof lifecycle transactions

## Tech Stack

- Rust
- Soroban SDK
- Stellar testnet

## Repository Structure

```text
contracts/
  issuer-registry/
  proof-registry/
  protocol-config/
packages/
  shared/
scripts/
tests/
docs/
```

## Local Setup

```bash
cargo build
cargo test
```

Formatting:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
```

## Current Validation Status

The repository now pins a stable Rust toolchain in `rust-toolchain.toml` and CI runs formatting, clippy, tests, and build.

The current test suite covers protocol configuration defaults and schema changes, issuer registration/status transitions/duplicate prevention, proof registration/expiration/revocation/duplicate prevention, cross-contract dependency checks, storage TTL behavior, and authorization paths through mocked Soroban auth.

The current testnet deployment manifest is checked in at `scripts/deployment-manifest.testnet.json`.

Live testnet contract IDs:

- `protocol-config`: `CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A`
- `issuer-registry`: `CB73TVWVJIIVNTKLWSHZB5NL2UIF3B3EUL4YH4MUD6EYX6SFIHE77D2F`
- `proof-registry`: `CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK`

The remaining readiness blocker is live backend anchoring against the deployed proof registry.

The `protocol-config` contract uses typed `#[contractevent]` events. Deployment automation is available under `scripts/`.

## On-Chain Privacy Boundary

Contracts must not store:

- Exact salary
- Exact payment amount
- Full wallet history
- Personal name
- Email address
- Employment documents
- Raw transaction lists
- Unencrypted personal information

Contracts may store:

- Proof ID hash
- Commitment hash
- Issuer address
- Status
- Expiration
- Schema version
- Timestamp
- Public metadata hash

## Security Requirements

- Authorization checks on every state mutation.
- Duplicate registration prevention.
- Status transitions must be explicit.
- Proof validity must respect expiration and revocation.
- Issuer-backed proof operations must reject inactive issuers.
- Sensitive operations should respect protocol pause state.
- Mainnet deployment should wait for independent review.

## Documentation

- [Storage Model](docs/storage-model.md) — every DataKey variant, TTL policy, lifecycle events, and privacy boundaries
- [Backend Integration](docs/backend-integration.md) — contract calls and hashing rules for the EarnProof API
- [Threat Model and Security Review Checklist](docs/threat-model.md) — comprehensive security analysis, threat enumeration, controls, and mainnet release gates

## Related Repositories

- `earnproof-frontend`: Public app, worker dashboard, issuer UI, verifier UI, and admin UI.
- `earnproof-backend`: API, payment indexing, proof generation, credential signing, and verification.
- `earnproof-sdk`: Future TypeScript SDK for integrations.
- `earnproof-specification`: Future credential and verification standard.
