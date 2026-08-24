#![no_std]

use earnproof_shared::{TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS};
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct ProtocolConfigContract;

#[contracttype]
enum DataKey {
    Admin,
    Paused,
    ConfigVersion,
    SchemaVersion(u32),
    /// Allowlist entry: maps a WASM hash to the target contract version it
    /// must install.  Only hashes pre-approved by the admin may be applied.
    AllowedWasm(BytesN<32>),
    /// Monotonically-increasing contract version stored in instance storage.
    /// Prevents installing an older (or equal) version over a newer one.
    ContractVersion,
}

// ── existing events ─────────────────────────────────────────────────────────

#[contractevent]
pub struct Initialized {
    pub admin: Address,
}

#[contractevent]
pub struct AdminChanged {
    pub new_admin: Address,
}

#[contractevent]
pub struct Paused {
    pub paused: bool,
}

#[contractevent]
pub struct Unpaused {
    pub paused: bool,
}

#[contractevent]
pub struct SchemaApproved {
    pub version: u32,
}

#[contractevent]
pub struct SchemaDeprecated {
    pub version: u32,
}

// ── upgrade events ───────────────────────────────────────────────────────────

/// Emitted when the admin adds a WASM hash to the upgrade allowlist.
#[contractevent]
pub struct UpgradeAllowlisted {
    pub wasm_hash: BytesN<32>,
    pub new_contract_version: u32,
    pub approved_by: Address,
}

/// Emitted when the admin removes a WASM hash from the allowlist without
/// applying it (e.g. rolling back an approved-but-not-yet-applied hash).
#[contractevent]
pub struct UpgradeRevoked {
    pub wasm_hash: BytesN<32>,
    pub revoked_by: Address,
}

/// Emitted when a WASM upgrade is successfully applied.
#[contractevent]
pub struct ContractUpgraded {
    pub new_wasm_hash: BytesN<32>,
    pub old_contract_version: u32,
    pub new_contract_version: u32,
    pub upgraded_by: Address,
}

#[contractimpl]
impl ProtocolConfigContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .set(&DataKey::ConfigVersion, &1_u32);
        env.storage()
            .instance()
            .set(&DataKey::ContractVersion, &1_u32);
        Self::extend_instance_ttl(env.clone());
        Initialized { admin }.publish(&env);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn set_admin(env: Env, new_admin: Address) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Self::bump_config_version(env.clone());
        AdminChanged { new_admin }.publish(&env);
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    pub fn pause(env: Env) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Paused, &true);
        Self::bump_config_version(env.clone());
        Paused { paused: true }.publish(&env);
    }

    pub fn unpause(env: Env) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        Self::bump_config_version(env.clone());
        Unpaused { paused: false }.publish(&env);
    }

    pub fn approve_schema_version(env: Env, version: u32) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        Self::ensure_nonzero_version(version);
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion(version), &true);
        Self::extend_schema_ttl(env.clone(), version);
        Self::bump_config_version(env.clone());
        SchemaApproved { version }.publish(&env);
    }

    pub fn deprecate_schema_version(env: Env, version: u32) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        Self::ensure_nonzero_version(version);
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion(version), &false);
        Self::extend_schema_ttl(env.clone(), version);
        Self::bump_config_version(env.clone());
        SchemaDeprecated { version }.publish(&env);
    }

    pub fn is_schema_version_approved(env: Env, version: u32) -> bool {
        if version == 0 {
            return false;
        }

        let key = DataKey::SchemaVersion(version);
        let approved = env.storage().persistent().get(&key).unwrap_or(false);
        if env.storage().persistent().has(&key) {
            env.storage().persistent().extend_ttl(
                &key,
                TTL_THRESHOLD_LEDGERS,
                TTL_EXTEND_TO_LEDGERS,
            );
        }
        approved
    }

    pub fn get_config_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ConfigVersion)
            .unwrap_or(0)
    }

    // ── upgrade governance ───────────────────────────────────────────────────

    /// Returns the stored monotonic contract version (separate from the
    /// config-mutation counter).  Starts at 1 after `initialize`.
    pub fn get_contract_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ContractVersion)
            .unwrap_or(0)
    }

    /// Admin-only: add `wasm_hash` to the upgrade allowlist and record the
    /// `new_version` that must be installed by that WASM.
    ///
    /// `new_version` must be strictly greater than the currently stored
    /// contract version so that a downgrade cannot be pre-approved.
    pub fn approve_upgrade(env: Env, wasm_hash: BytesN<32>, new_version: u32) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);

        let current = Self::get_contract_version(env.clone());
        if new_version <= current {
            panic!("new_version must be greater than current contract version");
        }

        env.storage()
            .instance()
            .set(&DataKey::AllowedWasm(wasm_hash.clone()), &new_version);
        Self::extend_instance_ttl(env.clone());

        UpgradeAllowlisted {
            wasm_hash,
            new_contract_version: new_version,
            approved_by: admin,
        }
        .publish(&env);
    }

    /// Admin-only: remove a previously allowlisted WASM hash without applying
    /// it.  Safe to call even if the hash was never allowlisted.
    pub fn revoke_upgrade(env: Env, wasm_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);

        env.storage()
            .instance()
            .remove(&DataKey::AllowedWasm(wasm_hash.clone()));

        UpgradeRevoked {
            wasm_hash,
            revoked_by: admin,
        }
        .publish(&env);
    }

    /// Returns true when `wasm_hash` is on the allowlist.
    pub fn is_upgrade_allowed(env: Env, wasm_hash: BytesN<32>) -> bool {
        env.storage()
            .instance()
            .has(&DataKey::AllowedWasm(wasm_hash))
    }

    /// Admin-only: apply an in-place WASM upgrade.
    ///
    /// Requirements (all checked on-chain before the upgrade is applied):
    /// 1. Caller is the admin.
    /// 2. `wasm_hash` is on the allowlist (pre-approved via `approve_upgrade`).
    /// 3. The target version stored in the allowlist entry is strictly greater
    ///    than the current `ContractVersion` (downgrade guard).
    ///
    /// On success the new WASM is installed, `ContractVersion` is advanced,
    /// the allowlist entry is consumed (removed), and a `ContractUpgraded`
    /// event is emitted.
    pub fn upgrade_contract(env: Env, wasm_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);

        let new_version: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AllowedWasm(wasm_hash.clone()))
            .expect("wasm hash not on allowlist");

        let old_version = Self::get_contract_version(env.clone());
        if new_version <= old_version {
            panic!("upgrade would not advance contract version");
        }

        // Consume the allowlist entry before applying so re-entrancy cannot
        // replay the same hash.
        env.storage()
            .instance()
            .remove(&DataKey::AllowedWasm(wasm_hash.clone()));

        // Apply the WASM upgrade.  This replaces the executable code while
        // leaving all stored state intact.
        env.deployer().update_current_contract_wasm(wasm_hash.clone());

        // Record the new version.
        env.storage()
            .instance()
            .set(&DataKey::ContractVersion, &new_version);
        Self::extend_instance_ttl(env.clone());

        ContractUpgraded {
            new_wasm_hash: wasm_hash,
            old_contract_version: old_version,
            new_contract_version: new_version,
            upgraded_by: admin,
        }
        .publish(&env);
    }

    // ── private helpers ──────────────────────────────────────────────────────

    fn ensure_nonzero_version(version: u32) {
        if version == 0 {
            panic!("schema version must be greater than zero");
        }
    }

    fn bump_config_version(env: Env) {
        let current = Self::get_config_version(env.clone());
        env.storage()
            .instance()
            .set(&DataKey::ConfigVersion, &(current + 1));
        Self::extend_instance_ttl(env);
    }

    fn extend_instance_ttl(env: Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    }

    fn extend_schema_ttl(env: Env, version: u32) {
        env.storage().persistent().extend_ttl(
            &DataKey::SchemaVersion(version),
            TTL_THRESHOLD_LEDGERS,
            TTL_EXTEND_TO_LEDGERS,
        );
    }

    fn require_auth(address: &Address) {
        address.require_auth();
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::{DataKey, ProtocolConfigContract, ProtocolConfigContractClient};
    use earnproof_shared::TTL_THRESHOLD_LEDGERS;
    use soroban_sdk::{testutils::storage::Persistent as _, Address, BytesN, Env};

    const ADMIN: &str = "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR";
    const OTHER: &str = "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U";

    fn bytes(env: &Env, value: u8) -> BytesN<32> {
        BytesN::from_array(env, &[value; 32])
    }

    fn setup() -> (Env, ProtocolConfigContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(ProtocolConfigContract, ());
        let client = ProtocolConfigContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        client.initialize(&admin);
        (env, client, admin)
    }

    // ── existing tests ────────────────────────────────────────────────────────

    #[test]
    fn initializes_config_defaults() {
        let (_env, client, admin) = setup();

        assert_eq!(client.get_admin(), admin);
        assert!(!client.is_paused());
        assert_eq!(client.get_config_version(), 1);
        assert!(!client.is_schema_version_approved(&1));
        // contract version initialized to 1
        assert_eq!(client.get_contract_version(), 1);
    }

    #[test]
    fn pause_and_unpause_bump_config_version() {
        let (_env, client, _admin) = setup();

        client.pause();
        assert!(client.is_paused());
        assert_eq!(client.get_config_version(), 2);

        client.unpause();
        assert!(!client.is_paused());
        assert_eq!(client.get_config_version(), 3);
    }

    #[test]
    fn schema_versions_can_be_approved_and_deprecated() {
        let (_env, client, _admin) = setup();

        client.approve_schema_version(&1);
        assert!(client.is_schema_version_approved(&1));

        client.deprecate_schema_version(&1);
        assert!(!client.is_schema_version_approved(&1));
    }

    #[test]
    #[should_panic(expected = "schema version must be greater than zero")]
    fn rejects_zero_schema_version() {
        let (_env, client, _admin) = setup();
        client.approve_schema_version(&0);
    }

    #[test]
    fn extends_schema_storage_ttl() {
        let (env, client, _admin) = setup();

        client.approve_schema_version(&7);

        env.as_contract(&client.address, || {
            assert!(
                env.storage()
                    .persistent()
                    .get_ttl(&DataKey::SchemaVersion(7))
                    > TTL_THRESHOLD_LEDGERS
            );
        });
    }

    // ── upgrade governance tests ──────────────────────────────────────────────

    #[test]
    fn approve_and_check_allowlist() {
        let (env, client, _admin) = setup();
        let hash = bytes(&env, 0xab);

        assert!(!client.is_upgrade_allowed(&hash));
        client.approve_upgrade(&hash, &2);
        assert!(client.is_upgrade_allowed(&hash));
    }

    #[test]
    fn revoke_removes_from_allowlist() {
        let (env, client, _admin) = setup();
        let hash = bytes(&env, 0xcd);

        client.approve_upgrade(&hash, &2);
        assert!(client.is_upgrade_allowed(&hash));

        client.revoke_upgrade(&hash);
        assert!(!client.is_upgrade_allowed(&hash));
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn approve_upgrade_rejects_downgrade_version() {
        let (env, client, _admin) = setup();
        // current version is 1; attempting to allowlist version 1 is rejected
        client.approve_upgrade(&bytes(&env, 1), &1);
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn approve_upgrade_rejects_lower_version() {
        let (env, client, _admin) = setup();
        // current version is 1; version 0 must be rejected
        client.approve_upgrade(&bytes(&env, 1), &0);
    }

    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_contract_rejects_non_allowlisted_hash() {
        let (env, client, _admin) = setup();
        client.upgrade_contract(&bytes(&env, 0xff));
    }

    /// Verifies that `upgrade_contract` enforces admin authorization before
    /// doing anything.  `mock_all_auths` is intentionally NOT used here.
    #[test]
    #[should_panic]
    fn upgrade_contract_requires_admin_auth() {
        let env = Env::default();
        let contract_id = env.register(ProtocolConfigContract, ());
        let client = ProtocolConfigContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        // Initialize using mocked auths.
        env.mock_all_auths();
        client.initialize(&admin);
        let hash = BytesN::from_array(&env, &[0xde; 32]);
        client.approve_upgrade(&hash, &2);
        env.set_auths(&[]);

        // Attempt upgrade without auth — must panic.
        client.upgrade_contract(&hash);
    }

    /// Verifies that the allowlist entry is consumed after a successful upgrade
    /// so the same hash cannot be replayed.
    ///
    /// Note: in the test environment `update_current_contract_wasm` is a no-op
    /// (the WASM is not actually swapped) but all surrounding state transitions
    /// — version bump, allowlist removal, event emission — are fully exercised.
    #[test]
    fn upgrade_contract_advances_version_and_consumes_allowlist() {
        let (env, client, _admin) = setup();
        let hash = bytes(&env, 0x42);

        assert_eq!(client.get_contract_version(), 1);
        client.approve_upgrade(&hash, &2);
        assert!(client.is_upgrade_allowed(&hash));

        client.upgrade_contract(&hash);

        // Version must have advanced.
        assert_eq!(client.get_contract_version(), 2);
        // Allowlist entry must have been consumed.
        assert!(!client.is_upgrade_allowed(&hash));
    }

    /// After a successful upgrade the same hash cannot be applied a second
    /// time (allowlist entry was consumed, and the version guard would also
    /// block it even if re-approved with the same version).
    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_contract_hash_cannot_be_replayed() {
        let (env, client, _admin) = setup();
        let hash = bytes(&env, 0x42);

        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);
        // Second application must fail — entry was consumed.
        client.upgrade_contract(&hash);
    }

    /// State written before an upgrade is still readable after.
    #[test]
    fn state_preserved_across_upgrade() {
        let (env, client, _admin) = setup();

        // Write some state before the upgrade.
        client.approve_schema_version(&3);
        client.pause();
        assert!(client.is_paused());
        assert!(client.is_schema_version_approved(&3));

        // Perform upgrade.
        let hash = bytes(&env, 0x77);
        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);

        // State must be intact after upgrade.
        assert!(client.is_paused());
        assert!(client.is_schema_version_approved(&3));
        assert_eq!(client.get_contract_version(), 2);
    }

    /// An upgrade approved with version N cannot be reused to downgrade from
    /// a later version M > N even if the hash is re-added to the allowlist.
    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn cannot_re_approve_old_version_after_upgrade() {
        let (env, client, _admin) = setup();
        let hash_v2 = bytes(&env, 0x01);
        let old_hash = bytes(&env, 0x02);

        // Upgrade to version 2.
        client.approve_upgrade(&hash_v2, &2);
        client.upgrade_contract(&hash_v2);
        assert_eq!(client.get_contract_version(), 2);

        // Attempt to allowlist a hash that would install version 1 — rejected.
        client.approve_upgrade(&old_hash, &1);
    }

    /// `approve_upgrade` by a non-admin must be rejected.
    #[test]
    #[should_panic]
    fn approve_upgrade_requires_admin_auth() {
        let env = Env::default();
        let contract_id = env.register(ProtocolConfigContract, ());
        let client = ProtocolConfigContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let other = Address::from_str(&env, OTHER);

        env.mock_all_auths();
        client.initialize(&admin);

        // Only authorize `other`, not `admin` — should panic.
        env.mock_auths(&[soroban_sdk::testutils::MockAuth {
            address: &other,
            invoke: &soroban_sdk::testutils::MockAuthInvoke {
                contract: &client.address,
                fn_name: "approve_upgrade",
                args: soroban_sdk::vec![
                    &env,
                    soroban_sdk::IntoVal::into_val(
                        &BytesN::from_array(&env, &[0xaa; 32]),
                        &env
                    ),
                    soroban_sdk::IntoVal::into_val(&2_u32, &env),
                ]
                .into(),
                sub_invokes: &[],
            },
        }]);
        client.approve_upgrade(&BytesN::from_array(&env, &[0xaa; 32]), &2);
    }
}
