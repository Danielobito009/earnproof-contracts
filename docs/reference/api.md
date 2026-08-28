# Generated Contract API

<!-- BEGIN GENERATED: do not edit. -->

## issuer-registry::get_admin

- Parameters: `env: Env`
- Result: `Result<Address, ContractError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::get_issuer

- Parameters: `env: Env, issuer_id_hash: BytesN<32>`
- Result: `Result<IssuerRecord, IssuerError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::get_issuer_by_address

- Parameters: `env: Env, issuer_address: Address,`
- Result: `Result<IssuerRecord, IssuerError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::initialize

- Parameters: `env: Env, admin: Address`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::is_active_address

- Parameters: `env: Env, issuer_address: Address`
- Result: `bool`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::is_active_issuer

- Parameters: `env: Env, issuer_id_hash: BytesN<32>`
- Result: `bool`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::reactivate_issuer

- Parameters: `env: Env, issuer_id_hash: BytesN<32>`
- Result: `Result<(), IssuerError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::register_issuer

- Parameters: `env: Env, issuer_id_hash: BytesN<32>, issuer_address: Address, metadata_hash: BytesN<32>,`
- Result: `Result<(), IssuerError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::revoke_issuer

- Parameters: `env: Env, issuer_id_hash: BytesN<32>`
- Result: `Result<(), IssuerError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::rotate_issuer_address

- Parameters: `env: Env, issuer_id_hash: BytesN<32>, new_address: Address,`
- Result: `Result<(), IssuerError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::suspend_issuer

- Parameters: `env: Env, issuer_id_hash: BytesN<32>`
- Result: `Result<(), IssuerError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## issuer-registry::update_issuer

- Parameters: `env: Env, issuer_id_hash: BytesN<32>, metadata_hash: BytesN<32>,`
- Result: `Result<(), IssuerError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/issuer-registry/src/lib.rs`

## proof-registry::admin_revoke_proof

- Parameters: `env: Env, proof_id_hash: BytesN<32>`
- Result: `Result<(), ProofError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::get_admin

- Parameters: `env: Env`
- Result: `Result<Address, ContractError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::get_issuer_registry

- Parameters: `env: Env`
- Result: `Result<Address, ContractError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::get_proof

- Parameters: `env: Env, proof_id_hash: BytesN<32>`
- Result: `Result<ProofRecord, ProofError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::get_protocol_config

- Parameters: `env: Env`
- Result: `Result<Address, ContractError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::initialize

- Parameters: `env: Env, admin: Address, issuer_registry: Address, protocol_config: Address,`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::is_revoked

- Parameters: `env: Env, proof_id_hash: BytesN<32>`
- Result: `bool`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::is_valid_proof

- Parameters: `env: Env, proof_id_hash: BytesN<32>`
- Result: `bool`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::register_proof

- Parameters: `env: Env, proof_id_hash: BytesN<32>, commitment_hash: BytesN<32>, issuer_address: Address, schema_version: u32, expires_at: u64,`
- Result: `Result<(), ProofError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## proof-registry::revoke_proof

- Parameters: `env: Env, proof_id_hash: BytesN<32>`
- Result: `Result<(), ProofError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/proof-registry/src/lib.rs`

## protocol-config::approve_schema_version

- Parameters: `env: Env, version: u32`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::deprecate_schema_version

- Parameters: `env: Env, version: u32`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::get_admin

- Parameters: `env: Env`
- Result: `Result<Address, ContractError>`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::get_config_version

- Parameters: `env: Env`
- Result: `u32`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::initialize

- Parameters: `env: Env, admin: Address`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::is_paused

- Parameters: `env: Env`
- Result: `bool`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::is_schema_version_approved

- Parameters: `env: Env, version: u32`
- Result: `bool`
- Authorization: none
- Storage effect: read-only
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::pause

- Parameters: `env: Env`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::set_admin

- Parameters: `env: Env, new_admin: Address`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

## protocol-config::unpause

- Parameters: `env: Env`
- Result: `Result<(), ContractError>`
- Authorization: current admin
- Storage effect: documented in lifecycle specification
- Event effect: none unless documented in lifecycle specification
- Failure atomicity: Soroban invocation rollback
- Source: `contracts/protocol-config/src/lib.rs`

<!-- END GENERATED -->
