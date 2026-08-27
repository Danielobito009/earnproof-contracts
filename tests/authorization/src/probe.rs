#![cfg(test)]

use crate::harness::{hash, Deployment};
use soroban_sdk::testutils::storage::Instance as _;

/// Diagnostic probe that prints instance storage keys and TTLs for each
/// contract. Useful during development; does not assert anything.
#[test]
fn probe_all_keys() {
    let d = Deployment::new();
    let _ = hash(&d.env, 1);

    // Instance storage (correctly filtered by contract address by the SDK).
    for (label, addr) in [
        ("config", &d.config_address),
        ("issuers", &d.issuers_address),
        ("proofs", &d.proofs_address),
    ] {
        d.env.as_contract(addr, || {
            let all = d.env.storage().instance().all();
            std::println!(
                "{} instance keys: {:?}",
                label,
                all.keys().iter().collect::<std::vec::Vec<_>>()
            );
            for _key in all.keys().iter() {
                let ttl = d.env.storage().instance().get_ttl();
                std::println!("  instance TTL: {}", ttl);
            }
        });
    }
}
