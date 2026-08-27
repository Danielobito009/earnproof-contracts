# Testing

This document describes how to run and extend the EarnProof test suite, and how
the bounded mutation-testing profile protects the authorization and validation
controls of the on-chain contracts.

## Prerequisites

- Rust toolchain from `rust-toolchain.toml` (`stable` + `rustfmt` + `clippy`).
- No running node, network, or local ledger is required for the Rust suites.

## Unit and integration tests

The workspace test suite spans the in-contract unit tests (`.src/lib.rs` under
`contracts/`) and the scenario-based integration suites under `tests/`:

| Suite | Crate | Covers |
| --- | --- | --- |
| `emergency-tests` | `tests/emergency` | pause matrix, admin rotation, revocation and recovery sequences |
| `cross-contract-tests` | `tests/cross-contract` | cross-contract boundaries, races, and references |
| `event-tests` | `tests/events` | event emission, ordering, and compatibility |
| `resource-budget-tests` | `tests/budgets` | Soroban resource (CPU/memory) budgets |

Run everything:

```bash
cargo test --workspace
```

Run a single suite:

```bash
cargo test -p emergency-tests
```

## Formatting, linting, and building

These are the checks CI runs on every pull request:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build --workspace
```

## Mutation testing

A green suite can still miss a removed `require_auth`, an inverted status check,
or a skipped expiry check. Mutation testing injects those bugs and asks the
suite to catch them.

The **bounded profile** in [`.cargo/mutants.toml`](../.cargo/mutants.toml)
limits mutation to `contracts/**/src/lib.rs` — the authorization and validation
branches listed in [`tests/mutation/README.md`](../tests/mutation/README.md) —
and runs the whole workspace suite against every mutant.

Run the profile and enforce the reviewed score:

```powershell
.\scripts\mutation-test.ps1
```

Prove the gate catches the seeded "removed authorization" and "inverted validity
check" mutations:

```powershell
.\scripts\mutation-test.ps1 -SelfTest
```

### Score policy

The reviewed policy is **zero missed mutants** in the bounded set. `cargo mutants`
exits non-zero when any mutant survives, and `mutation-test.ps1` additionally
computes the score from `mutants.out/outcomes.json` and fails if it drops below
`-MinimumScore` (default `100`).

When a mutant survives:

1. Inspect the exact change in `mutants.out/diff/`.
2. Add a test that asserts the correct behaviour at the right abstraction level
   (preferably through a public entry point), or
3. Explicitly justify the survivor in the PR — e.g. the mutant is behaviourally
   indistinguishable from the correct code.

### CI enforcement

The `mutation` job in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
runs the bounded profile and the seeded-mutation self-test on a weekly schedule
and on `workflow_dispatch`, and uploads `mutants.out/` as an artifact. It is
deliberately not part of the fast PR loop so the normal contributor test cycle
stays quick.

### Reproducibility

- `cargo-mutants` is pinned to `27.1.0` and installed with `--locked`.
- `mutants.out/` and `mutants.out.old/` are git-ignored; `outcomes.json` records
  the per-mutant verdicts and the summary used to compute the score.
