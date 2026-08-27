//! Cross-contract failure atomicity and error propagation.
//!
//! `proof-registry::register_proof` is the only entry point in this workspace
//! that reads across a contract boundary before it writes. Every other
//! operation decides from its own storage. That makes registration the single
//! place where a dependency can fail *after* the invocation has already done
//! work, and therefore the only place where partial state is even conceivable.
//!
//! This crate contains no production code. It exists because none of the three
//! contract crates can express these scenarios alone: the failure has to be
//! injected into a callee while the assertions are made against the caller.
//!
//! What the suite protects:
//!
//! - A failure at any cross-contract boundary is rejected, not absorbed.
//! - A rejected registration leaves the complete observable footprint —
//!   proof record, entry TTL, instance TTLs, the `protocol-config` version
//!   counter, and the issuer record — byte-for-byte as it was.
//! - A rejected registration publishes no event.
//! - The rollback reaches the *callee*: a dependency that wrote to its own
//!   storage before the registration failed keeps none of that write.
//! - Invalid, stale, unknown, and version-incompatible dependency references
//!   fail closed.
//! - Errors surface as stable machine-readable codes, never as panic text.
//!
//! Privacy note: every fixture here is a synthetic hash or a generated address.
//! No proof identifier, wallet, or off-chain payload in this crate corresponds
//! to anything real.

#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod harness;

#[cfg(test)]
mod mocks;

#[cfg(test)]
mod boundaries;

#[cfg(test)]
mod references;

#[cfg(test)]
mod races;
