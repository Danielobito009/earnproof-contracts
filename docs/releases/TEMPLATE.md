# Release <version>

<!--
  Copy this file to docs/releases/<version>.md and fill every field.

  Validation: pwsh -File scripts/verify-manifest.ps1 -Manifest <manifest> -Release docs/releases/<version>.md

  The validator checks that every required field is present, that the recorded
  hashes and contract IDs match the manifest, and that no credential-shaped
  material appears. Recording a hash is not enough — it has to be the hash of
  the artifact that was actually deployed.

  NEVER include: admin secret keys, seed phrases, signing material, private RPC
  endpoints, API keys, or internal hostnames. A release note is published, and
  publication is irreversible.
-->

- **Release:** <version>
- **Date:** <YYYY-MM-DD>
- **Commit:** <full 40-character source commit>
- **Network:** <stellar-testnet>
- **Classification:** <additive | semantic | breaking>

## Toolchain

- **Rust:** <channel, e.g. stable>
- **soroban-sdk:** <version>
- **Build target:** wasm32v1-none
- **Build command:** `stellar contract build`

## Artifacts

| Contract | Version | Contract ID | WASM SHA-256 |
|---|---|---|---|
| protocol-config | <x.y.z> | <C...> | <64 hex> |
| issuer-registry | <x.y.z> | <C...> | <64 hex> |
| proof-registry | <x.y.z> | <C...> | <64 hex> |

**Approved schema versions:** <e.g. 1>

## Changes

<!--
  One entry per change, each with its class from docs/compatibility.md:
  ABI, Storage, Events, Errors, Authorization, Resource, or Semantic.

  Say what changed and why a consumer should care. "Refactored internals" is
  not a change entry unless behaviour moved; if it did, it is Semantic.
-->

| Class | Contract | Change | Consumer impact |
|---|---|---|---|
| <class> | <contract> | <what changed> | <what a consumer must do> |

## Migration

<!--
  The exact steps a consumer takes. If none are required, write "None." and say
  why — an empty section reads as an oversight.

  These contracts have no upgrade mechanism, so any storage change means
  redeployment and off-chain trust migration. Say so plainly if it applies.
-->

None.

## Backend compatibility

<!--
  State a minimum backend version, or "Unchanged".

  The backend depends on contract behaviour through invocation signatures,
  hash construction, and schema versions. A release touching any of those and
  claiming "unchanged" is wrong in a way that surfaces as a production outage
  rather than a failed build.
-->

Unchanged.

## Rollback

<!--
  How to return to the previous artifact, or why it is not possible.

  "Redeploy the previous WASM" is only a rollback if state is compatible.
  Because these contracts have no upgrade path, redeploying produces a NEW
  contract ID: the old ID keeps running the new code, and state does not
  travel. Say that plainly when it applies.
-->

<Steps, or an explicit statement that rollback is not possible and what that costs.>

## Containment

<!--
  What an operator does if this release misbehaves in production.

  Usually pause() on protocol-config. A change to the pause path itself needs a
  different answer, and that answer belongs here.
-->

<Containment procedure.>

## Governance

<!--
  Required for a breaking change. Additive and semantic releases may record
  "Not required — <class> release."
-->

- **Breaking change approved by:** <maintainer name, or "not required">
- **Security review:** <required | not required> — <reviewer, or why not>

## Verification

```
$ cargo fmt --all --check
$ cargo clippy --workspace --all-targets
$ cargo test --workspace
$ cargo build --workspace
$ pwsh -File scripts/verify-manifest.ps1 -Manifest <manifest> -Release docs/releases/<version>.md
```

<Paste the actual output. A verification section with commands but no output is
not evidence.>
