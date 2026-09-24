/// TTL Boundary Test Harness
///
/// Provides deterministic ledger advancement and TTL testing utilities for
/// boundary testing (pre-expiry, at-expiry, post-expiry, restoration).
///
/// Soroban SDK 27.0.0 TTL model:
/// - `extend_ttl(threshold, extend_to)`: Extends TTL if current TTL <= threshold
///   - threshold: ledgers; if current TTL is at or below this, extension is triggered
///   - extend_to: relative ledgers from current sequence; absolute expiry = current_seq + extend_to
/// - Expiry semantics: entry is expired when ledger.sequence > expiry_ledger (exclusive boundary)
///   - At expiry_ledger: still valid
///   - At expiry_ledger + 1: expired
///
/// Test boundary conditions:
/// - pre_expiry: ledger = expiry - 1 (entry still valid)
/// - at_expiry: ledger = expiry (entry still valid, at the boundary)
/// - post_expiry: ledger = expiry + 1 (entry expired)
/// - restoration: use env.storage().persistent().restore() to restore expired footprint
use soroban_sdk::{testutils::Ledger as _, testutils::LedgerInfo, Env};

#[allow(dead_code)]
pub struct TtlTestHarness;

#[allow(dead_code)]
impl TtlTestHarness {
    /// Advance the ledger to a specific sequence number and update timestamp accordingly.
    /// Returns the new sequence number.
    pub fn advance_to_ledger(env: &Env, sequence: u32) -> u32 {
        env.ledger().set(LedgerInfo {
            protocol_version: 27,
            sequence_number: sequence,
            timestamp: (sequence as u64) * 5, // Assume 5-second blocks
            network_id: Default::default(),
            base_reserve: 5000,
            min_temp_entry_ttl: 16,
            min_persistent_entry_ttl: 4096,
            max_entry_ttl: 6_312_000,
        });
        sequence
    }

    /// Advance ledger by N ledgers from current position.
    pub fn advance_by_ledgers(env: &Env, count: u32) -> u32 {
        let current = env.ledger().sequence();
        Self::advance_to_ledger(env, current + count)
    }

    /// Get the current ledger sequence number.
    pub fn current_ledger(env: &Env) -> u32 {
        env.ledger().sequence()
    }

    /// Calculate the expiry ledger for an entry, given:
    /// - threshold: TTL_THRESHOLD_LEDGERS
    /// - extend_to: TTL_EXTEND_TO_LEDGERS
    /// - current_ledger: current ledger sequence when extend_ttl() is called
    ///
    /// The Soroban SDK computes:
    ///   new_ttl_ledgers = max(current_ttl, extend_to)
    ///   expiry = current_ledger + new_ttl_ledgers
    ///
    /// For a fresh entry with no prior TTL, assume current_ttl = 0:
    ///   expiry = current_ledger + extend_to
    pub fn calculate_expiry(current_ledger: u32, _threshold: u32, extend_to: u32) -> u32 {
        current_ledger + extend_to
    }

    /// Pre-expiry ledger: entry is still valid (at expiry - 1).
    pub fn pre_expiry_ledger(expiry: u32) -> u32 {
        expiry.saturating_sub(1)
    }

    /// At-expiry ledger: entry is at the boundary but still valid (at expiry).
    pub fn at_expiry_ledger(expiry: u32) -> u32 {
        expiry
    }

    /// Post-expiry ledger: entry is expired (at expiry + 1).
    pub fn post_expiry_ledger(expiry: u32) -> u32 {
        expiry + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_ledger_advancement() {
        let env = Env::default();
        let _initial = TtlTestHarness::current_ledger(&env);

        TtlTestHarness::advance_to_ledger(&env, 1000);
        assert_eq!(TtlTestHarness::current_ledger(&env), 1000);

        TtlTestHarness::advance_by_ledgers(&env, 50);
        assert_eq!(TtlTestHarness::current_ledger(&env), 1050);
    }

    #[test]
    fn harness_expiry_calculation() {
        let current = 100;
        let threshold = 50_000;
        let extend_to = 500_000;

        let expiry = TtlTestHarness::calculate_expiry(current, threshold, extend_to);
        assert_eq!(expiry, current + extend_to);

        assert_eq!(TtlTestHarness::pre_expiry_ledger(expiry), expiry - 1);
        assert_eq!(TtlTestHarness::at_expiry_ledger(expiry), expiry);
        assert_eq!(TtlTestHarness::post_expiry_ledger(expiry), expiry + 1);
    }
}
