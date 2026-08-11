#![no_std]

use earnproof_shared::{TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS};
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env};

#[contract]
pub struct ProtocolConfigContract;

#[contracttype]
enum DataKey {
    Admin,
    Paused,
    ConfigVersion,
    SchemaVersion(u32),
}

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
        #[cfg(not(test))]
        address.require_auth();

        #[cfg(test)]
        let _ = address;
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::{DataKey, ProtocolConfigContract, ProtocolConfigContractClient};
    use earnproof_shared::TTL_THRESHOLD_LEDGERS;
    use soroban_sdk::{testutils::storage::Persistent as _, Address, Env};

    const ADMIN: &str = "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR";

    fn setup() -> (Env, ProtocolConfigContractClient<'static>, Address) {
        let env = Env::default();
        let contract_id = env.register(ProtocolConfigContract, ());
        let client = ProtocolConfigContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn initializes_config_defaults() {
        let (_env, client, admin) = setup();

        assert_eq!(client.get_admin(), admin);
        assert!(!client.is_paused());
        assert_eq!(client.get_config_version(), 1);
        assert!(!client.is_schema_version_approved(&1));
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
}
