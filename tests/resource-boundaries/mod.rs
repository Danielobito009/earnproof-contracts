//! Resource boundary tests for EarnProof contracts.
//!
//! These tests verify:
//! 1. Exact-limit inputs stay within CPU/memory budgets
//! 2. Over-limit inputs are rejected before any storage write (atomicity)
//! 3. Budget-exhaustion paths commit no storage or events
//!
//! Resource evidence is separated by contract and operation.
//! Each test measures and prints CPU instruction count and memory usage.

pub mod protocol_config_resources;
pub mod issuer_registry_resources;
pub mod proof_registry_resources;
