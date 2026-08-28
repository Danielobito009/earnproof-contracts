# Local development

How to run the EarnProof contracts against a local Soroban sandbox: a
reproducible environment sitting between `cargo test` and a testnet deployment.

## Why a sandbox

`cargo test` runs contracts in-process. It is fast and it covers logic well, but
it never exercises WASM installation, contract IDs, or cross-contract wiring —
the parts that only exist once something is deployed.

A testnet deployment exercises all of that, but it costs real time, needs a
funded account, and leaves state behind on a shared network.

The local sandbox gives you the second without the cost of the third. It is
where you find out that a contract builds, installs, deploys, initializes, and
answers the way you expected — before anyone else sees it.

## Prerequisites

| Requirement | Why | Install |
|---|---|---|
| Rust (stable) | Builds the contracts | <https://rustup.rs> |
| `wasm32v1-none` target | The Soroban build target | `rustup target add wasm32v1-none` |
| Stellar CLI | Builds, deploys, invokes | `cargo install --locked stellar-cli` |
| PowerShell 7+ | The harness is `pwsh` | <https://aka.ms/powershell> |
| Docker or Podman | Runs the local network | <https://docs.docker.com/get-docker/> |

The harness requires **PowerShell 7**, not Windows PowerShell 5.1. This matches
the rest of `scripts/`, which already uses PowerShell 7 syntax.

Check what you have:

```powershell
cargo --version
stellar --version
pwsh --version
docker --version
```

## Start the local network

```powershell
stellar container start local
```

This runs a Stellar quickstart container with RPC and a friendbot. It takes
around a minute on first run while the image is pulled.

Confirm it is up:

```powershell
stellar network ls
```

Stop it when you are done:

```powershell
stellar container stop local
```

## Run the harness

```powershell
pwsh -File scripts/local-sandbox/run-sandbox.ps1
```

One command:

1. validates prerequisites;
2. generates and funds a throwaway identity;
3. builds optimized WASM;
4. deploys `protocol-config`, `issuer-registry`, `proof-registry` in dependency
   order;
5. initializes admins and approves schema version 1;
6. exercises a synthetic lifecycle;
7. writes a disposable manifest.

### Options

| Flag | Effect |
|---|---|
| `-SkipBuild` | Reuse existing WASM. Useful when iterating on the harness rather than the contracts. |
| `-KeepState` | Keep the throwaway identity after the run. Without it the identity is removed. |
| `-Output <path>` | Where to write the manifest. Defaults to a gitignored path. |
| `-MaxRetries <n>` | Retry attempts for transient RPC errors. Defaults to 5. |

```powershell
# Iterating on the harness: skip the rebuild and keep the identity.
pwsh -File scripts/local-sandbox/run-sandbox.ps1 -SkipBuild -KeepState
```

## What the lifecycle covers

Each step asserts its result. A step that deployed and printed without checking
anything would tell you the CLI ran, not that the contract behaved.

| Step | Assertion |
|---|---|
| `register_issuer` | `is_active_issuer` returns true |
| `register_proof` | `is_valid_proof` returns true |
| `pause` | `is_paused` returns true |
| `register_proof` while paused | **rejected** — the run fails if it succeeds |
| `is_valid_proof` while paused | still available; verification does not go dark |
| `admin_revoke_proof` while paused | succeeds; `is_revoked` returns true |
| `unpause` | `is_paused` returns false |
| `register_proof` after unpause | succeeds |
| `is_revoked` after unpause | still true; revocation outlives the pause |

The pause steps are the interesting ones. Pausing blocks new registrations but
deliberately leaves revocation and reads available, so an operator can contain
an incident without losing the tools needed to resolve it. The harness proves
that asymmetry rather than assuming it.

## Safety

The harness handles credentials the way you would want a script that generates
keys to handle them.

**It only runs against `local`.** Any other network — testnet, futurenet,
mainnet, or a custom name — is rejected before anything is built, deployed, or
generated. The harness creates throwaway identities and deploys unreviewed
artifacts; neither belongs on a shared network. For testnet, use
[`scripts/deploy-testnet.ps1`](../scripts/deploy-testnet.ps1), which takes an
existing funded identity you control.

**It reads no credentials.** No environment variable, no key file, no argument.
The identity is generated at run time and funded by the local friendbot.

**It prints no secret.** Output contains contract IDs, the throwaway public
address, and synthetic hashes. The secret key is held by the Stellar CLI under
`.stellar/`, which is gitignored, and the harness never invokes
`stellar keys show`.

**Its values are synthetic.** Every hash is derived from a fixed literal such as
`earnproof-sandbox:proof:1`. Nothing resembles a real wallet, proof, or
credential, and the derivation is printed so you can confirm that yourself.

**Its output is disposable.** The manifest is written to a gitignored path and
carries `"disposable": true` plus a warning in its own body, so it cannot be
mistaken for a deployment record if it is pasted somewhere.

## Output

```
=== Sandbox run complete ===
Contract IDs (safe to share):
  protocol-config: C...
  issuer-registry: C...
  proof-registry:  C...

Synthetic values (derived from fixed literals, not real data):
  issuerIdHash: 3f2a...
  proofIdHash:  9c81...

Disposable manifest: scripts/local-sandbox/.sandbox-manifest.json
```

Contract IDs and synthetic values are labelled separately, so a reader copying
something out of the output knows which category it belongs to.

## Repeat runs

Runs are disposable rather than idempotent. Each one deploys fresh contracts
with new IDs, so a second run does not collide with the first.

Contract state is not reset between runs — that is what the fresh deployment
achieves. To start completely clean:

```powershell
stellar container stop local
stellar container start local
```

## Smoke test

```powershell
pwsh -File scripts/local-sandbox/run-sandbox.tests.ps1
```

Validates the harness without needing Docker or a running container: it checks
that the script parses, that the network guard rejects every non-local network
before any side effect, that no credential is read or printed, that the
lifecycle covers every required step, and that the output is disposable.

It deliberately does **not** perform a deployment. That needs a running
container and is what the manual run above is for. The smoke test proves the
harness is safe and structurally correct to run, not that a deployment
succeeded — a distinction worth keeping, because a smoke test that overclaims
is worse than none.

## Troubleshooting

**`Required command 'stellar' was not found`** — install the CLI with
`cargo install --locked stellar-cli` and confirm it is on `PATH`.

**`The Stellar CLI could not list networks`** — the container is not running.
Start it with `stellar container start local`.

**`Expected WASM artifact was not found`** — you passed `-SkipBuild` without a
prior build. Run once without it.

**Transient RPC errors** — a container that has just started is briefly
unavailable. The harness retries with backoff; raise `-MaxRetries` if your
machine is slow.

**`This harness only runs against the local sandbox network`** — working as
intended. See [Safety](#safety).

## Related

- [`scripts/README.md`](../scripts/README.md) — all deployment scripts
- [`scripts/deploy-testnet.ps1`](../scripts/deploy-testnet.ps1) — testnet deployment
- [`docs/storage-model.md`](storage-model.md) — storage keys and TTL
- [`docs/backend-integration.md`](backend-integration.md) — backend contract usage
