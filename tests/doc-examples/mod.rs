//! Executable Documentation Examples for EarnProof Contracts
//!
//! This module contains comprehensive, runnable documentation examples
//! that demonstrate how to invoke each contract method. All examples:
//!
//! - Use synthetic identifiers (e.g., "test-issuer-123", "proof-id-456")
//! - Run in a local Soroban sandbox environment
//! - Validate expected outputs: return values, status changes, and event emissions
//! - Are executable as documentation tests (`cargo test --doc`)
//!
//! ## Running Examples
//!
//! To run all documentation examples:
//! ```sh
//! cargo test --doc --package earnproof-contracts
//! ```
//!
//! To run examples for a specific contract:
//! ```sh
//! cargo test --doc protocol_config
//! cargo test --doc issuer_registry
//! cargo test --doc proof_registry
//! cargo test --doc integration
//! ```
//!
//! ## Organization
//!
//! Examples are organized by contract:
//!
//! - **`protocol_config.rs`** — Protocol initialization, schema approval, pause operations, admin changes
//! - **`issuer_registry.rs`** — Issuer registration, lifecycle transitions, address rotation
//! - **`proof_registry.rs`** — Proof registration, revocation (issuer and admin), validity checks
//! - **`integration.rs`** — Cross-contract workflows and error cases
//!
//! ## Key Patterns
//!
//! ### Hashing Requirements
//!
//! All public identifiers must be hashed before passing to contracts:
//!
//! ```text
//! proof_id_hash = sha256(proof_id)
//! issuer_id_hash = sha256(issuer_id)
//! commitment_hash = sha256(canonical_credential_payload)
//! metadata_hash = sha256(canonical_public_issuer_metadata)
//! ```
//!
//! In examples, we use synthetic `BytesN<32>` values to represent hashes.
//!
//! ### Authorization Patterns
//!
//! - **Protocol-Config writes:** Require admin authorization (pause, unpause, set_admin, approve/deprecate schema)
//! - **Issuer-Registry writes:** Require admin authorization (register, update, suspend, reactivate, revoke, rotate)
//! - **Proof-Registry writes:** Require issuer authorization (revoke_proof) or admin authorization (admin_revoke_proof)
//!
//! In sandbox examples, `env.mock_all_auths()` simulates authorization for all addresses.
//!
//! ### Lifecycle Patterns
//!
//! **Issuer Lifecycle:**
//! 1. `register_issuer` (Active)
//! 2. `suspend_issuer` (Suspended) ← optional
//! 3. `reactivate_issuer` (Active) ← only from Suspended
//! 4. `revoke_issuer` (Revoked) ← terminal state
//!
//! **Proof Lifecycle:**
//! 1. `register_proof` (Active, not yet expired)
//! 2. `revoke_proof` or `admin_revoke_proof` (Revoked) ← terminal state
//! 3. Expiration is implicit (status remains Active, but `is_valid_proof` returns false after expiry)
//!
//! ## Error Conditions
//!
//! Documentation examples demonstrate:
//!
//! - **Initialization errors:** "already initialized", "schema version must be greater than zero"
//! - **Duplicate errors:** "issuer already registered", "issuer address already registered", "proof already registered"
//! - **State transition errors:** "revoked issuer cannot be updated", "revoked issuer cannot be reactivated"
//! - **Validation errors:** "proof expiration must be in the future", "protocol is paused", "issuer is not active"
//! - **Revocation errors:** "proof already revoked"
//!
//! ## Modules

pub mod integration;
pub mod issuer_registry;
pub mod proof_registry;
pub mod protocol_config;
