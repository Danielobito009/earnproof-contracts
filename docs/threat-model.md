# Threat model

Scope: the Soroban contracts in this repository. Backend and network threats are
named here only where they bound what the contracts can promise.

The companion document is `docs/security-review/README.md`, which maps each
control below to the exact code and test that backs it. This document says *what
we are defending against*; the index says *what evidence exists*.

## What is being protected

The contracts custody no funds. What they hold is **trust state** — the answers
relying parties depend on:

- Is this issuer active?
- Was this proof commitment registered, and has it been revoked?
- Is this schema version approved?
- Are sensitive operations paused?

An attacker's goal is to make one of those answers wrong: to have a revoked
credential verify, to have a compromised issuer keep issuing, or to make valid
credentials fail.

## Trust boundaries

```
  relying party ──read──▶ [ contracts ]  ◀──write── issuer (Stellar key)
                               ▲   ▲
                               │   └──write── administrator (Stellar key)
                               │
                       backend ┘  (constructs hashes, submits transactions)
```

Three boundaries matter:

1. **Administrator key → contract.** One key per contract, no multisig or
   timelock at the contract layer. Full authority over pause, schema approval,
   issuer status, and admin revocation.
2. **Issuer key → contract.** Registers and revokes proofs under its own
   identity. Constrained by issuer status and the pause flag.
3. **Backend → contract.** Supplies all hashes. The contracts never verify how a
   hash was derived.

## Assumptions

These are relied upon and **not** enforced on-chain. If one is false, the
corresponding control below provides no protection.

| # | Assumption | If false |
|---|---|---|
| A1 | The Soroban host enforces `require_auth` correctly | All authorization fails |
| A2 | A rejected invocation is atomic — no partial writes persist | Rejected calls could leave inconsistent state |
| A3 | Administrator keys are held securely and, where required, are multisig at the Stellar account level | Single-key compromise is total for that contract |
| A4 | The backend derives `proof_id_hash`, `commitment_hash`, and `metadata_hash` correctly and without collisions | Distinct proofs could collide, or a proof could be unaddressable |
| A5 | Contract addresses supplied to `proof-registry::initialize` are the genuine registries | An attacker-controlled callee could report "not paused, issuer active" unconditionally |
| A6 | Ledger timestamps are approximately accurate | Expiry checks are meaningless |
| A7 | Deployed WASM corresponds to this source tree | The reviewed code is not the running code |

A4, A5, and A7 are the assumptions an external reviewer should push hardest on.
A7 in particular is currently unverifiable — see
[#17](https://github.com/veridatum-labs/earnproof-contracts/issues/17).

## Threats and controls

Grouped by the attacker's goal. Each row names the control and its evidence
status; full detail is in `docs/security-review/README.md`.

### T1 — Seize administrative control

| Attack | Control | Status |
|---|---|---|
| Call `initialize` on a live contract to reset the admin | Re-init guard at `protocol-config:50`, `issuer-registry:76`, `proof-registry:36` | Implemented, untested |
| Call a privileged function without the admin key | `require_auth(&admin)` on every privileged path | Implemented, **authorization untested** |
| Retain authority after being rotated out | Admin read from storage at call time, never cached | Implemented, untested |
| Rotate authority to yourself after removal | `set_admin` requires the *current* admin | Implemented, untested |

**Residual risk.** No test in this repository fails if a `require_auth` call is
deleted. This is the largest single gap —
[#34](https://github.com/veridatum-labs/earnproof-contracts/issues/34),
[#38](https://github.com/veridatum-labs/earnproof-contracts/issues/38).

### T2 — Forge or launder issuer trust

| Attack | Control | Status |
|---|---|---|
| Register yourself as an issuer | `register_issuer` is admin-gated | Implemented, auth untested |
| Reuse an address already bound to another issuer | Uniqueness check at `issuer-registry:107`, `:194` | Implemented, Tested |
| Reactivate an issuer after revocation | Revocation is terminal, `issuer-registry:258` | Implemented, Tested |
| Keep issuing from a rotated-out key | Old address mapping removed at `issuer-registry:200` | Implemented, partially tested |
| Register a proof as a suspended or revoked issuer | `is_active_address` check at `proof-registry:79` | Implemented, Tested |

### T3 — Falsify proof state

| Attack | Control | Status |
|---|---|---|
| Register a proof under an unapproved schema | `is_schema_version_approved` check at `proof-registry:75` | Implemented, Tested |
| Register an already-expired proof | Expiry check at `proof-registry:65` | Implemented, Tested |
| Overwrite an existing proof record | Duplicate check at `proof-registry:86` | Implemented, Tested |
| Revoke another issuer's proof | `require_auth(&record.issuer_address)` at `proof-registry:166` | Implemented, auth untested |
| Erase revocation by revoking twice | Already-revoked check at `proof-registry:171` | Implemented, untested |

### T4 — Defeat or abuse containment

| Attack | Control | Status |
|---|---|---|
| Register new proofs while the protocol is paused | Pause check at `proof-registry:71` | Implemented, Tested |
| Freeze the protocol as a denial of service | `pause` is admin-gated; unpause always available to the admin | Implemented, auth untested |
| Block revocation during an incident | Revocation paths do not consult the pause flag, by design | Implemented, untested |
| Strand a paused contract by rotating to an unusable address | **None** — see accepted risk below | Accepted risk |

### T5 — Extract private data from the chain

| Attack | Control | Status |
|---|---|---|
| Read income, amounts, or payment history from storage | Only hashes and addresses are ever stored | Implemented, Tested |
| Recover an identifier from an error message | All panics are fixed string literals; no argument is interpolated | Implemented, Tested |
| Recover an identifier from an event | Every event field is a hash, address, version, or timestamp | Implemented, Tested |
| Correlate activity by address | **None** — issuer addresses are public by design | Accepted risk |

The last row is worth stating plainly: on-chain activity for a given issuer
address is publicly linkable. That is inherent to a public ledger and is not a
defect. It is one reason subject identifiers are hashed rather than stored.

## Accepted risks

Recorded so a reviewer does not report them as findings without knowing they were
considered.

**Admin stranding.** `set_admin` accepts any address without verifying the
successor can authorise. An operator can permanently strand a paused contract.
Mitigated by observability, not prevention: every rotation advances
`config_version` and emits `AdminChanged`. Coordinated rotation is
[#11](https://github.com/veridatum-labs/earnproof-contracts/issues/11).

**Single admin key per contract.** No contract-level multisig or timelock.
Depends on A3.

**No upgrade path.** A contract bug requires redeployment and off-chain trust
migration. Strategy is
[#12](https://github.com/veridatum-labs/earnproof-contracts/issues/12).

**Registry instance TTL.** Neither registry extends its instance TTL after
`initialize`. A registry idle for roughly 29 days can have its instance entry
archived while individual records survive. Acceptable for an active testnet
deployment; **must be re-evaluated before mainnet**. Boundary tests are
[#35](https://github.com/veridatum-labs/earnproof-contracts/issues/35).

**Panic strings rather than typed errors.** Callers cannot distinguish failure
causes; a cross-contract rejection surfaces only as
`Error(WasmVm, InvalidAction)`.
[#10](https://github.com/veridatum-labs/earnproof-contracts/issues/10).

## Out of scope

- **Backend security.** Authentication, rate limiting, key custody, webhook
  delivery, and every privacy control over data that never reaches the chain
  live in `earnproof-backend`.
- **Network security.** Consensus, ordering, fee markets, RPC availability, and
  the correctness of the Soroban host.
- **Hash construction.** The contracts treat all 32-byte inputs as opaque.
  Vectors are unpublished —
  [#43](https://github.com/veridatum-labs/earnproof-contracts/issues/43).
- **Mainnet.** `SECURITY.md` scopes the project to testnet. Several accepted
  risks above would require re-classification first.

## Maintenance

Refresh this document whenever the evidence index is refreshed; the two are
reviewed together. See the refresh checklist in
`docs/security-review/README.md`.
