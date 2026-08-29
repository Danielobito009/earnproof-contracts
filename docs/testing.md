# Testing and coverage

What CI runs, what coverage measures, and what's deliberately excluded (#66).

## What CI runs

Every PR touching the workspace runs (`.github/workflows/ci.yml`, `contracts`
job): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`,
`cargo test --workspace`, `cargo build --workspace`.

A separate `coverage` job runs `cargo-llvm-cov` (pinned to `0.8.7` — see the
workflow file for why an unpinned install isn't used), generates both a
human-readable summary and a machine-readable JSON report, checks the
critical-path gates below, and uploads both files as a build artifact
(`coverage-report`, 30-day retention). No source code, secrets, or coverage
data leave GitHub Actions — nothing is uploaded to an external coverage
service.

## Running coverage locally

```bash
cargo install cargo-llvm-cov --version 0.8.7 --locked
cargo llvm-cov --workspace --summary-only
```

Add `--html` for a browsable per-line report, or `--json --output-path
coverage.json` to reproduce exactly what CI checks.

## Critical-path gates

`scripts/check-coverage-gates.py` reads the JSON report and fails if any
contract's **region** coverage — the metric that catches an untested branch a
line- or function-level number can miss — drops below its gate:

| File | Minimum | Measured when introduced |
|---|---|---|
| `contracts/issuer-registry/src/lib.rs` | 90.0% | 98.77% |
| `contracts/proof-registry/src/lib.rs` | 90.0% | 98.61% |
| `contracts/protocol-config/src/lib.rs` | 90.0% | 97.51% |

Minimums are set below the measured figure at introduction, not at it — an
unrelated one-line change to an already-well-tested branch shouldn't fail CI
over noise, while an actual regression (new logic added with no test
reaching it) still gets caught. Each contract's real coverage is already
close to complete: the specific 1-11 missed regions per file are almost
entirely defensive `unreachable!()`/internal-invariant branches that
`mock_all_auths()`-based tests can't reach without deliberately corrupting
storage first — see the `#[cfg(test)]` module in each contract for what
*is* covered (initialization, every state transition, every documented
error path, TTL renewal, event emission).

Authorization, validation, state-mutation, and error branches are exercised
directly: every `require_auth` call site has a test asserting the exact
address it demands (see `contracts/issuer-registry/src/lib.rs`'s
`revoke_issuer_rejects_a_valid_signature_from_the_issuer_itself` for the
pattern, and `tests/emergency/src/admin_rotation.rs`'s
`assert_authorized_by` helper for scoped-auth assertions against the real
invocation tree rather than a blanket `mock_all_auths()`), every documented
error variant in `earnproof_shared::{ContractError, IssuerError, ProofError}`
has a test that triggers it, and every state-changing entry point has a
paired "emits exactly one event" / "emits no event on rejection" assertion
(see [`docs/events.md`](./events.md)).

## What's excluded, and why

**`packages/shared/src/lib.rs`** is excluded from the gates above (not from
the report — it still appears in the summary, at whatever llvm-cov measures
for it). It contains only `#[contracterror]`/`#[contracttype]` declarations
and constants — zero `pub fn` or `impl` blocks of its own — so there is no
executable logic for llvm-cov to attribute coverage to directly. The
derive-macro-generated (de)serialization code these types produce *is*
exercised, but only observably through the three contracts that use them,
which is exactly what the three gates above already measure.

**Generated client bindings** (the `*Client` structs `#[contractimpl]`
generates) and **host glue code** are not separately gated for the same
reason: they have no logic of their own to regress independently of the
contract method they wrap.

## Test layout

- `contracts/*/src/lib.rs` — unit tests per contract, in an inline
  `#[cfg(test)] mod test`, using `env.mock_all_auths()` for the majority of
  behavioral coverage.
- `tests/cross-contract/` — cross-contract wiring, using scoped `MockAuth`/
  `MockAuthInvoke` (see [`docs/troubleshooting.md`](./troubleshooting.md#authorization-failures)
  for the authorization-scoping concepts these tests exercise).
- `tests/emergency/`, `tests/budgets/`, `tests/events/`, `tests/time/`,
  `tests/encoding/`, `tests/event-fixtures/` — one workspace member per
  concern, each independently coverage-measured but not separately gated
  (they exercise the same three contracts the gates above already cover).
