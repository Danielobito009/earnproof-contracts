#![no_std]

use earnproof_shared::{TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS, MAX_SCHEMA_VERSION, MIN_SCHEMA_VERSION};
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env};

#[contract]
pub struct ProtocolConfigContract;

/// Error type for protocol configuration contract.
#[derive(Debug)]
#[contracttype]
pub enum ContractError {
    /// Returned when an input parameter exceeds its documented maximum size.
    InputTooLarge = 1000,
}

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

/// Validates that a version number is within acceptable range.
/// Called BEFORE any storage write to ensure atomicity.
///
/// # Arguments
/// * `version` - The schema version to validate
///
/// # Errors
/// Returns error if version is outside the valid range.
fn validate_schema_version(version: u32) -> Result<(), ContractError> {
    if version < MIN_SCHEMA_VERSION || version > MAX_SCHEMA_VERSION {
        return Err(ContractError::InputTooLarge);
    }
    Ok(())
}

#[contractimpl]
impl ProtocolConfigContract {
    /// Initializes the protocol configuration contract.
    ///
    /// # Authorization
    /// Requires signature from `admin`.
    ///
    /// # Input Limits
    /// - `admin`: fixed-size Stellar address (no validation needed)
    ///
    /// # Validation
    /// - Checks that contract is not already initialized
    /// - Verifies authorization from admin address
    ///
    /// # Storage Writes
    /// - `DataKey::Admin` → admin address (instance)
    /// - `DataKey::Paused` → false (instance)
    /// - `DataKey::ConfigVersion` → 1 (instance)
    ///
    /// # Events Emitted
    /// - `Initialized { admin }`
    ///
    /// # Failure Atomicity
    /// All validation occurs before any storage write. On error,
    /// no partial state is committed.
    ///
    /// # Panics
    /// - If contract is already initialized
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

    /// Retrieves the current protocol administrator address.
    ///
    /// # Storage Reads
    /// - `DataKey::Admin` (instance, no TTL extension)
    ///
    /// # Panics
    /// - If contract is not initialized
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    /// Updates the protocol administrator to a new address.
    ///
    /// # Authorization
    /// Requires signature from the current admin.
    ///
    /// # Input Limits
    /// - `new_admin`: fixed-size Stellar address (no validation needed)
    ///
    /// # Validation
    /// - Verifies authorization from current admin
    ///
    /// # Storage Writes
    /// - `DataKey::Admin` → new_admin (instance, overwrite)
    /// - `DataKey::ConfigVersion` → incremented (instance)
    ///
    /// # Events Emitted
    /// - `AdminChanged { new_admin }`
    ///
    /// # Failure Atomicity
    /// All validation occurs before storage writes. On error,
    /// no partial state is committed.
    pub fn set_admin(env: Env, new_admin: Address) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Self::bump_config_version(env.clone());
        AdminChanged { new_admin }.publish(&env);
    }

    /// Checks if the protocol is currently paused.
    ///
    /// # Storage Reads
    /// - `DataKey::Paused` (instance, no TTL extension)
    ///
    /// # Returns
    /// - `true` if protocol is paused
    /// - `false` if protocol is active (default)
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Pauses the protocol, preventing new proofs from being registered.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Validation
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - `DataKey::Paused` → true (instance)
    /// - `DataKey::ConfigVersion` → incremented (instance)
    ///
    /// # Events Emitted
    /// - `Paused { paused: true }`
    ///
    /// # Failure Atomicity
    /// All validation occurs before storage writes. On error,
    /// no partial state is committed.
    pub fn pause(env: Env) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Paused, &true);
        Self::bump_config_version(env.clone());
        Paused { paused: true }.publish(&env);
    }

    /// Resumes the protocol, allowing new proofs to be registered.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Validation
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - `DataKey::Paused` → false (instance)
    /// - `DataKey::ConfigVersion` → incremented (instance)
    ///
    /// # Events Emitted
    /// - `Unpaused { paused: false }`
    ///
    /// # Failure Atomicity
    /// All validation occurs before storage writes. On error,
    /// no partial state is committed.
    pub fn unpause(env: Env) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        Self::bump_config_version(env.clone());
        Unpaused { paused: false }.publish(&env);
    }

    /// Approves a credential schema version for use in proof registration.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `version`: maximum `MAX_SCHEMA_VERSION` (u32::MAX)
    ///
    /// # Validation
    /// - Input size validation (version in valid range)
    /// - Version must be >= MIN_SCHEMA_VERSION (1)
    ///
    /// # Storage Writes
    /// - `DataKey::SchemaVersion(version)` → true (persistent)
    /// - `DataKey::ConfigVersion` → incremented (instance)
    ///
    /// # Events Emitted
    /// - `SchemaApproved { version }`
    ///
    /// # Failure Atomicity
    /// Over-limit inputs are rejected before any storage write.
    /// No partial state is committed on error.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if version is out of range
    ///
    /// # Panics
    /// - If version < MIN_SCHEMA_VERSION (checked by ensure_nonzero_version)
    pub fn approve_schema_version(env: Env, version: u32) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        validate_schema_version(version).expect("invalid version");
        Self::ensure_nonzero_version(version);
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion(version), &true);
        Self::extend_schema_ttl(env.clone(), version);
        Self::bump_config_version(env.clone());
        SchemaApproved { version }.publish(&env);
    }

    /// Deprecates a credential schema version, preventing it from being used in new proofs.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `version`: maximum `MAX_SCHEMA_VERSION` (u32::MAX)
    ///
    /// # Validation
    /// - Input size validation (version in valid range)
    /// - Version must be >= MIN_SCHEMA_VERSION (1)
    ///
    /// # Storage Writes
    /// - `DataKey::SchemaVersion(version)` → false (persistent, overwrite)
    /// - `DataKey::ConfigVersion` → incremented (instance)
    ///
    /// # Events Emitted
    /// - `SchemaDeprecated { version }`
    ///
    /// # Failure Atomicity
    /// Over-limit inputs are rejected before any storage write.
    /// No partial state is committed on error.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if version is out of range
    ///
    /// # Panics
    /// - If version < MIN_SCHEMA_VERSION (checked by ensure_nonzero_version)
    pub fn deprecate_schema_version(env: Env, version: u32) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);
        validate_schema_version(version).expect("invalid version");
        Self::ensure_nonzero_version(version);
        env.storage()
            .persistent()
            .set(&DataKey::SchemaVersion(version), &false);
        Self::extend_schema_ttl(env.clone(), version);
        Self::bump_config_version(env.clone());
        SchemaDeprecated { version }.publish(&env);
    }

    /// Checks if a credential schema version is currently approved.
    ///
    /// # Input Limits
    /// - `version`: maximum `MAX_SCHEMA_VERSION` (u32::MAX)
    ///
    /// # Storage Reads
    /// - `DataKey::SchemaVersion(version)` (persistent, with TTL extension if key exists)
    ///
    /// # Returns
    /// - `true` if version is approved
    /// - `false` if version is deprecated or never seen
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

    /// Retrieves the current protocol configuration version counter.
    ///
    /// # Storage Reads
    /// - `DataKey::ConfigVersion` (instance, no TTL extension)
    ///
    /// # Returns
    /// - Version counter (incremented on each state mutation)
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
        address.require_auth();
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
        env.mock_all_auths();
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
