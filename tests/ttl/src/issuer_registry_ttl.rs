//! Issuer Registry TTL Boundary Tests

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::harness::TtlTestHarness;
    use earnproof_shared::{IssuerStatus, TTL_THRESHOLD_LEDGERS};
    use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
    use soroban_sdk::{Address, BytesN, Env};

    const ADMIN: &str = "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR";
    const ISSUER_1: &str = "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U";

    fn bytes(env: &Env, value: u8) -> BytesN<32> {
        BytesN::from_array(env, &[value; 32])
    }

    fn admin_addr(env: &Env) -> Address {
        Address::from_str(env, ADMIN)
    }

    fn issuer_addr(env: &Env) -> Address {
        Address::from_str(env, ISSUER_1)
    }

    fn setup(env: &Env) -> (IssuerRegistryContractClient<'static>, Address, Address) {
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(env, &contract_id);
        let admin = admin_addr(env);
        let issuer_1 = issuer_addr(env);
        client.initialize(&admin);
        (client, admin, issuer_1)
    }

    // ── Instance Storage (Admin) ────

    #[test]
    fn instance_admin_pre_expiry_readable() {
        let env = Env::default();
        let (client, admin, _issuer_1) = setup(&env);

        let current_ledger = TtlTestHarness::current_ledger(&env);
        let expiry =
            TtlTestHarness::calculate_expiry(current_ledger, TTL_THRESHOLD_LEDGERS, 500_000);
        let pre_expiry = TtlTestHarness::pre_expiry_ledger(expiry);
        TtlTestHarness::advance_to_ledger(&env, pre_expiry);

        let retrieved = client.get_admin();
        assert_eq!(retrieved, admin);
    }

    #[test]
    fn instance_admin_at_expiry_readable() {
        let env = Env::default();
        let (client, admin, _issuer_1) = setup(&env);

        let current_ledger = TtlTestHarness::current_ledger(&env);
        let expiry =
            TtlTestHarness::calculate_expiry(current_ledger, TTL_THRESHOLD_LEDGERS, 500_000);
        let at_expiry = TtlTestHarness::at_expiry_ledger(expiry);
        TtlTestHarness::advance_to_ledger(&env, at_expiry);

        let retrieved = client.get_admin();
        assert_eq!(retrieved, admin);
    }

    /// Post-expiry: the SDK 27 test host auto-restores expired persistent
    /// entries on access, so the admin read still succeeds.
    #[test]
    fn instance_admin_post_expiry_auto_restored() {
        let env = Env::default();
        let (client, admin, _issuer_1) = setup(&env);

        let current_ledger = TtlTestHarness::current_ledger(&env);
        let expiry =
            TtlTestHarness::calculate_expiry(current_ledger, TTL_THRESHOLD_LEDGERS, 500_000);
        let post_expiry = TtlTestHarness::post_expiry_ledger(expiry);
        TtlTestHarness::advance_to_ledger(&env, post_expiry);

        let result = client.try_get_admin();
        assert_eq!(
            result,
            Ok(Ok(admin)),
            "test host auto-restores expired persistent entries"
        );
    }

    // ── Persistent Storage: Issuer(hash) ────

    #[test]
    fn persistent_issuer_record_pre_expiry_readable() {
        let env = Env::default();
        let (client, _admin, issuer_1) = setup(&env);

        let issuer_id = bytes(&env, 1);
        client.register_issuer(&issuer_id, &issuer_1, &bytes(&env, 2));

        let current_ledger = TtlTestHarness::current_ledger(&env);
        let expiry =
            TtlTestHarness::calculate_expiry(current_ledger, TTL_THRESHOLD_LEDGERS, 500_000);
        let pre_expiry = TtlTestHarness::pre_expiry_ledger(expiry);
        TtlTestHarness::advance_to_ledger(&env, pre_expiry);

        let record = client.get_issuer(&issuer_id);
        assert_eq!(record.issuer_id_hash, issuer_id);
        assert_eq!(record.status, IssuerStatus::Active);
    }

    /// Post-expiry: the SDK 27 test host auto-restores expired persistent
    /// entries on access, so the issuer record read still succeeds.
    #[test]
    fn persistent_issuer_record_post_expiry_auto_restored() {
        let env = Env::default();
        let (client, _admin, issuer_1) = setup(&env);

        let issuer_id = bytes(&env, 5);
        client.register_issuer(&issuer_id, &issuer_1, &bytes(&env, 6));

        let current_ledger = TtlTestHarness::current_ledger(&env);
        let expiry =
            TtlTestHarness::calculate_expiry(current_ledger, TTL_THRESHOLD_LEDGERS, 500_000);
        let post_expiry = TtlTestHarness::post_expiry_ledger(expiry);
        TtlTestHarness::advance_to_ledger(&env, post_expiry);

        let result = client.try_get_issuer(&issuer_id);
        let record = result
            .expect("read succeeds after auto-restore")
            .expect("issuer found");
        assert_eq!(
            record.issuer_address, issuer_1,
            "test host auto-restores expired persistent entries"
        );
    }

    // ── Persistent Storage: AddressIssuer(addr) ────

    /// Post-expiry: the SDK 27 test host auto-restores expired persistent
    /// entries on access, so the address→issuer mapping is still active.
    #[test]
    fn persistent_address_issuer_post_expiry_auto_restored() {
        let env = Env::default();
        let (client, _admin, issuer_1) = setup(&env);

        let issuer_id = bytes(&env, 11);
        client.register_issuer(&issuer_id, &issuer_1, &bytes(&env, 12));

        let current_ledger = TtlTestHarness::current_ledger(&env);
        let expiry =
            TtlTestHarness::calculate_expiry(current_ledger, TTL_THRESHOLD_LEDGERS, 500_000);
        let post_expiry = TtlTestHarness::post_expiry_ledger(expiry);
        TtlTestHarness::advance_to_ledger(&env, post_expiry);

        assert!(
            client.is_active_address(&issuer_1),
            "test host auto-restores expired persistent entries"
        );
    }

    // ── Cross-Entry Consistency ────

    #[test]
    fn issuer_and_address_entries_expire_together() {
        let env = Env::default();
        let (client, _admin, issuer_1) = setup(&env);

        let issuer_id = bytes(&env, 13);
        client.register_issuer(&issuer_id, &issuer_1, &bytes(&env, 14));

        let current_ledger = TtlTestHarness::current_ledger(&env);
        let expiry =
            TtlTestHarness::calculate_expiry(current_ledger, TTL_THRESHOLD_LEDGERS, 500_000);
        let post_expiry = TtlTestHarness::post_expiry_ledger(expiry);
        TtlTestHarness::advance_to_ledger(&env, post_expiry);

        // The SDK 27 test host auto-restores expired persistent entries on
        // access, so both entries remain readable and active after the expiry
        // boundary.
        let result_by_id = client.try_get_issuer(&issuer_id);
        let record = result_by_id
            .expect("read succeeds after auto-restore")
            .expect("issuer found");
        assert_eq!(record.issuer_address, issuer_1);

        let is_active = client.is_active_address(&issuer_1);
        assert!(is_active);
    }
}
