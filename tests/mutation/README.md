# Mutation testing (bounded security profile)

EarnProof uses [cargo-mutants](https://mutants.rs/) as a second line of defence
for the security-critical authorization and validation branches of the three
on-chain contracts. A green test suite can still miss a deleted `require_auth`,
an inverted status check, a skipped expiry check, or a success event emitted on
failure. Mutation testing injects those bugs mechanically and asks the test
suite to catch them.

## What is tested

The bounded profile lives in [`.cargo/mutants.toml`](../../.cargo/mutants.toml)
and restricts mutants to `contracts/**/src/lib.rs`. The reviewed target set is
the authorization/validation surface of:

| Control | Where |
| --- | --- |
| `require_auth` / admin equality | `initialize`, `set_admin`, `pause`, `unpause`, `register_*`, `*_revoke`, `set_status`, `set_revoked`, `rotate_issuer_address` |
| Pause containment | `is_paused`, `register_proof` (`is_paused` gate) |
| Issuer status | `is_active_issuer`, `is_active_address`, `set_status` transition guard |
| Schema approval | `approve_schema_version`, `is_schema_version_approved`, `register_proof` approval gate |
| Duplicate registration | `register_issuer`, `register_proof` storage guards |
| Expiry | `register_proof` (`expires_at`), `is_valid_proof` (`timestamp <= expires_at`) |
| Revocation | `set_status`, `set_revoked`, `is_revoked` |

The config sets `test_workspace = true` because the authorization regressions
are only observable through the integration suites in `tests/` — the in-contract
unit tests call `env.mock_all_auths()`, so a removed `require_auth` would be
invisible to them.

## How to run

```powershell
# Install a pinned, reproducible cargo-mutants and run the bounded profile.
.\scripts\mutation-test.ps1

# Prove the gate catches seeded "removed auth" and "inverted check" mutations.
.\scripts\mutation-test.ps1 -SelfTest
```

Running `cargo mutants` directly also works (it reads `.cargo/mutants.toml`):

```powershell
cargo mutants
```

The reviewed policy is **zero missed mutants** in the bounded set. CI enforces
this on every PR and on a weekly schedule (see `.github/workflows/ci.yml`).
Missed mutants are fixed with a test, or explicitly justified in the PR.

## Seeded mutations

`seeds/` contains two hand-written mutations that stand in for the two most
dangerous classes the gate must catch:

- [`removed-require-auth.patch`](seeds/removed-require-auth.patch) — deletes the
  `require_auth` call in `ProtocolConfigContract::pause`, so *anyone* could pause
  the protocol.
- [`inverted-validity-check.patch`](seeds/inverted-validity-check.patch) — flips
  `record.status == ProofStatus::Active` to `!=` in `is_valid_proof`, so every
  active proof reads as invalid.

`mutation-test.ps1 -SelfTest` applies each seed to the working tree, runs
`cargo test --workspace`, and asserts that the suite **fails** — i.e. that the
mutation is caught. If a seed is not caught, the gate is broken and the self-test
fails.

> The seed patches are examples, not an exhaustive list. They pin the *classes*
> of bug (removed authorization, inverted validity) that the mutation gate is
> accountable for catching. When the contract source around them changes, the
> patches are updated in the same PR so the self-test keeps passing.

## Reproducibility

- cargo-mutants version is pinned to `27.1.0` in `scripts/mutation-test.ps1` and
  installed with `--locked`.
- The generated report lives in `mutants.out/` (git-ignored): `outcomes.json`
  carries the per-mutant verdicts and the summary used to compute the score,
  `diff/` carries the exact source change for each mutant.
