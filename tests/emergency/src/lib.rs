//! Adversarial pause and emergency-recovery test suite.
//!
//! This crate contains no production code. It exists so the workspace can host
//! cross-contract emergency scenarios that no single contract crate can express
//! on its own: `protocol-config` owns the pause switch, while `proof-registry`
//! and `issuer-registry` are the contracts whose behaviour that switch is meant
//! to contain.
//!
//! The scenarios modelled here are described in `docs/emergency-operations.md`.
//! Every assertion in the test modules maps to a documented operational
//! guarantee; when a guarantee changes, the document and these tests must change
//! together.
//!
//! Privacy note: these tests deliberately use synthetic hashes and addresses.
//! No fixture in this crate encodes a real wallet, a real proof identifier, or
//! any off-chain payload. The contracts themselves only ever store hashes, and
//! the assertions below check that the observable event and error surface stays
//! at that level.

#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod harness;

#[cfg(test)]
mod pause_matrix;

#[cfg(test)]
mod admin_rotation;

#[cfg(test)]
mod sequences;
