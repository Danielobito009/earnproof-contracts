# Configuration digests

Each contract exposes `get_config_digest_version() -> u32` and
`get_config_digest() -> BytesN<32>`. Version 1 digests are SHA-256 over the
Soroban XDR encoding of one tuple. The first two fields are always the digest
version and a contract-specific domain symbol, so values from different
contracts cannot collide.

| Contract | Domain | Covered fields, in encoding order |
| --- | --- | --- |
| `protocol-config` | `earnproof_protocol_config` | admin, paused, config version, contract version |
| `issuer-registry` | `earnproof_issuer_registry` | admin, contract version |
| `proof-registry` | `earnproof_proof_registry` | admin, issuer-registry address, protocol-config address, contract version |

Schema approvals are represented by the protocol config's monotonic config
version. Tooling comparing a release manifest must compare both the digest and
digest version; a later encoding version may add fields or change their order.

The `earnproof-shared` crate provides `protocol_config_digest`,
`issuer_registry_digest`, and `proof_registry_digest` for deployment tooling.
They use the same XDR encoder and SHA-256 host function as the contracts. The
known-vector contract tests pin the encoding and verify that contract queries
and host-side calculations are identical.
