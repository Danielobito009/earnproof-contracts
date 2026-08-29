#![no_std]

//! Deterministic ledger-time boundary tests. All timestamps are explicit.

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use earnproof_shared::ProofError;
    use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
    use proof_registry::{ProofRegistryContract, ProofRegistryContractClient};
    use protocol_config::{ProtocolConfigContract, ProtocolConfigContractClient};
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::{Address, BytesN, Env};

    const NOW: u64 = 1_000;

    struct Fixture {
        env: Env,
        proofs: ProofRegistryContractClient<'static>,
        config: ProtocolConfigContractClient<'static>,
        issuer: Address,
    }

    fn bytes(env: &Env, value: u8) -> BytesN<32> { BytesN::from_array(env, &[value; 32]) }

    fn fixture() -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(NOW);
        let admin = Address::generate(&env);
        let issuer = Address::generate(&env);
        let config_id = env.register(ProtocolConfigContract, ());
        let config = ProtocolConfigContractClient::new(&env, &config_id);
        config.initialize(&admin);
        config.approve_schema_version(&1);
        let issuers_id = env.register(IssuerRegistryContract, ());
        let issuers = IssuerRegistryContractClient::new(&env, &issuers_id);
        issuers.initialize(&admin);
        issuers.register_issuer(&bytes(&env, 1), &issuer, &bytes(&env, 2));
        let proofs_id = env.register(ProofRegistryContract, ());
        let proofs = ProofRegistryContractClient::new(&env, &proofs_id);
        proofs.initialize(&admin, &issuers_id, &config_id);
        Fixture { env, proofs, config, issuer }
    }

    fn register(fixture: &Fixture, id: u8, expires_at: u64) {
        fixture.proofs.register_proof(
            &bytes(&fixture.env, id), &bytes(&fixture.env, id.wrapping_add(10)),
            &fixture.issuer, &1, &expires_at,
        );
    }

    #[test]
    fn validity_is_inclusive_at_expiration_and_false_after() {
        let fixture = fixture();
        register(&fixture, 1, NOW + 10);
        fixture.env.ledger().set_timestamp(NOW + 10);
        assert!(fixture.proofs.is_valid_proof(&bytes(&fixture.env, 1)));
        fixture.env.ledger().set_timestamp(NOW + 11);
        assert!(!fixture.proofs.is_valid_proof(&bytes(&fixture.env, 1)));
    }

    #[test]
    fn registration_requires_strictly_future_expiration() {
        let fixture = fixture();
        for (id, expires_at) in [(1, NOW - 1), (2, NOW), (3, 0)] {
            assert_eq!(fixture.proofs.try_register_proof(
                &bytes(&fixture.env, id), &bytes(&fixture.env, id + 10),
                &fixture.issuer, &1, &expires_at,
            ), Err(Ok(ProofError::ProofExpired)));
        }
    }

    #[test]
    fn revocation_dominates_expiration() {
        let fixture = fixture();
        register(&fixture, 4, NOW + 10);
        fixture.env.ledger().set_timestamp(NOW + 10);
        fixture.proofs.revoke_proof(&bytes(&fixture.env, 4));
        fixture.env.ledger().set_timestamp(NOW + 11);
        assert!(!fixture.proofs.is_valid_proof(&bytes(&fixture.env, 4)));
        assert!(fixture.proofs.is_revoked(&bytes(&fixture.env, 4)));
    }

    #[test]
    fn zero_schema_and_pause_are_deterministic_guards() {
        let fixture = fixture();
        assert_eq!(fixture.proofs.try_register_proof(
            &bytes(&fixture.env, 5), &bytes(&fixture.env, 6), &fixture.issuer, &0, &(NOW + 1)
        ), Err(Ok(ProofError::InvalidSchemaVersion)));
        fixture.config.pause();
        assert_eq!(fixture.proofs.try_register_proof(
            &bytes(&fixture.env, 7), &bytes(&fixture.env, 8), &fixture.issuer, &1, &(NOW + 1)
        ), Err(Ok(ProofError::InvalidSchemaVersion)));
    }

    #[test]
    fn maximum_timestamp_is_representable_without_interval_overflow() {
        let fixture = fixture();
        register(&fixture, 9, u64::MAX);
        assert!(fixture.proofs.is_valid_proof(&bytes(&fixture.env, 9)));
    }
}