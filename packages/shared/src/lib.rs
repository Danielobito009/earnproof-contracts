#![no_std]

use soroban_sdk::{contracttype, Address, BytesN};

pub const TTL_THRESHOLD_LEDGERS: u32 = 50_000;
pub const TTL_EXTEND_TO_LEDGERS: u32 = 500_000;

// ── Input Size Limits ─────────────────────────────────────────
// These constants document the maximum sizes for contract inputs.
// All current inputs are fixed-size (BytesN<32>, Address, u32, u64).
// These limits are provided for defensive programming and future
// extensibility when variable-size inputs may be added.
//
// Verified by resource-boundary tests: bulk operations with these
// limits remain within per-transaction Soroban budgets.

/// Maximum size of issuer_id_hash input (BytesN<32>).
/// Currently fixed at 32 bytes; documented for consistency.
pub const MAX_ISSUER_ID_HASH_BYTES: u32 = 32;

/// Maximum size of metadata_hash input (BytesN<32>).
/// Currently fixed at 32 bytes; documented for consistency.
pub const MAX_METADATA_HASH_BYTES: u32 = 32;

/// Maximum size of proof_id_hash input (BytesN<32>).
/// Currently fixed at 32 bytes; documented for consistency.
pub const MAX_PROOF_ID_HASH_BYTES: u32 = 32;

/// Maximum size of commitment_hash input (BytesN<32>).
/// Currently fixed at 32 bytes; documented for consistency.
pub const MAX_COMMITMENT_HASH_BYTES: u32 = 32;

/// Maximum number of issuers that can be registered in a single call.
/// Current implementation registers one issuer per call; this constant
/// documents the limit for defensive programming.
pub const MAX_ISSUERS_PER_CALL: u32 = 1;

/// Maximum number of proofs that can be registered in a single call.
/// Current implementation registers one proof per call; this constant
/// documents the limit for defensive programming.
pub const MAX_PROOFS_PER_CALL: u32 = 1;

/// Maximum schema version number.
/// Version numbers are u32; this allows all valid versions.
/// Version 0 is explicitly rejected by validate_version().
pub const MAX_SCHEMA_VERSION: u32 = u32::MAX;

/// Minimum schema version number (exclusive).
/// Schema version 0 is not allowed (checked explicitly).
pub const MIN_SCHEMA_VERSION: u32 = 1;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssuerStatus {
    Active,
    Suspended,
    Revoked,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofStatus {
    Active,
    Revoked,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuerRecord {
    pub issuer_id_hash: BytesN<32>,
    pub issuer_address: Address,
    pub metadata_hash: BytesN<32>,
    pub status: IssuerStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRecord {
    pub proof_id_hash: BytesN<32>,
    pub commitment_hash: BytesN<32>,
    pub issuer_address: Address,
    pub status: ProofStatus,
    pub schema_version: u32,
    pub expires_at: u64,
    pub created_at: u64,
    pub revoked_at: u64,
}
