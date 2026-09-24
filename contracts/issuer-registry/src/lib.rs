#![no_std]

use earnproof_shared::{
    ContractError, IssuerError, IssuerRecord, IssuerStatus, MigrationStatus, TtlStatus,
    MAX_MIGRATION_BATCH, MIGRATION_STATUS_VERSION, TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS,
};
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct IssuerRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    Issuer(BytesN<32>),
    AddressIssuer(Address),
    IssuerTtl(BytesN<32>),
    AddressTtl(Address),
    InstanceLiveUntil,
    /// Allowlist entry: maps a WASM hash to the target contract version.
    AllowedWasm(BytesN<32>),
    MigrationStatus,
    /// Monotonically-increasing contract version.  Prevents downgrade.
    ContractVersion,
}

// ── upgrade events ────────────────────────────────────────────────────────────

/// Emitted when the admin adds a WASM hash to the upgrade allowlist.
#[contractevent]
pub struct UpgradeAllowlisted {
    pub wasm_hash: BytesN<32>,
    pub new_contract_version: u32,
    pub approved_by: Address,
}

/// Emitted when the admin removes a WASM hash from the allowlist without
/// applying it.
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

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Emitted when an issuer is successfully registered.
#[contractevent]
pub struct IssuerRegistered {
    pub issuer_id_hash: BytesN<32>,
    pub issuer_address: Address,
    pub metadata_hash: BytesN<32>,
    pub created_at: u64,
}

/// Emitted when an issuer's public metadata hash is updated.
#[contractevent]
pub struct IssuerMetadataUpdated {
    pub issuer_id_hash: BytesN<32>,
    pub metadata_hash: BytesN<32>,
    pub updated_at: u64,
}

/// Emitted when an issuer is suspended.
#[contractevent]
pub struct IssuerSuspended {
    pub issuer_id_hash: BytesN<32>,
    pub updated_at: u64,
}

/// Emitted when a suspended issuer is reactivated.
#[contractevent]
pub struct IssuerReactivated {
    pub issuer_id_hash: BytesN<32>,
    pub updated_at: u64,
}

/// Emitted when an issuer is permanently revoked.
#[contractevent]
pub struct IssuerRevoked {
    pub issuer_id_hash: BytesN<32>,
    pub updated_at: u64,
}

/// Emitted when an issuer's on-chain wallet address is rotated.
/// Both old and new addresses are included so indexers can update their mapping
/// without scanning storage.
#[contractevent]
pub struct IssuerAddressRotated {
    pub issuer_id_hash: BytesN<32>,
    pub old_address: Address,
    pub new_address: Address,
    pub updated_at: u64,
}

// ---------------------------------------------------------------------------
// Contract implementation
// ---------------------------------------------------------------------------

#[contractimpl]
impl IssuerRegistryContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        Self::require_valid_admin(&admin)?;
        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::ContractVersion, &1_u32);
        Self::extend_instance_ttl(env);
        Ok(())
    }

    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)
    }

    pub fn register_issuer(
        env: Env,
        issuer_id_hash: BytesN<32>,
        issuer_address: Address,
        metadata_hash: BytesN<32>,
    ) -> Result<(), IssuerError> {
        Self::assert_operational(&env);
        let admin = Self::get_admin(env.clone()).map_err(|_| IssuerError::IssuerNotFound)?;
        Self::require_valid_issuer_address(&issuer_address)?;
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash.clone());
        if env.storage().persistent().has(&key) {
            return Err(IssuerError::IssuerAlreadyRegistered);
        }

        let address_key = DataKey::AddressIssuer(issuer_address.clone());
        if env.storage().persistent().has(&address_key) {
            return Err(IssuerError::IssuerAddressAlreadyRegistered);
        }

        let now = env.ledger().timestamp();
        let record = IssuerRecord {
            issuer_id_hash: issuer_id_hash.clone(),
            issuer_address: issuer_address.clone(),
            metadata_hash: metadata_hash.clone(),
            status: IssuerStatus::Active,
            created_at: now,
            updated_at: now,
        };

        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .set(&address_key, &issuer_id_hash);
        Self::extend_issuer_ttl(env.clone(), issuer_id_hash.clone());
        Self::extend_address_ttl(env.clone(), issuer_address.clone());

        IssuerRegistered {
            issuer_id_hash,
            issuer_address,
            metadata_hash,
            created_at: now,
        }
        .publish(&env);
        Ok(())
    }

    pub fn update_issuer(
        env: Env,
        issuer_id_hash: BytesN<32>,
        metadata_hash: BytesN<32>,
    ) -> Result<(), IssuerError> {
        Self::assert_operational(&env);
        let admin = Self::get_admin(env.clone()).map_err(|_| IssuerError::IssuerNotFound)?;
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash.clone());
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(IssuerError::IssuerNotFound)?;

        if record.status == IssuerStatus::Revoked {
            return Err(IssuerError::IssuerRevoked);
        }

        let now = env.ledger().timestamp();
        record.metadata_hash = metadata_hash.clone();
        record.updated_at = now;
        env.storage().persistent().set(&key, &record);
        Self::extend_issuer_key_ttl(env.clone(), &key);

        IssuerMetadataUpdated {
            issuer_id_hash,
            metadata_hash,
            updated_at: now,
        }
        .publish(&env);
        Ok(())
    }

    pub fn suspend_issuer(env: Env, issuer_id_hash: BytesN<32>) -> Result<(), IssuerError> {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Suspended)
    }

    pub fn reactivate_issuer(env: Env, issuer_id_hash: BytesN<32>) -> Result<(), IssuerError> {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Active)
    }

    pub fn revoke_issuer(env: Env, issuer_id_hash: BytesN<32>) -> Result<(), IssuerError> {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Revoked)
    }

    pub fn rotate_issuer_address(
        env: Env,
        issuer_id_hash: BytesN<32>,
        new_address: Address,
    ) -> Result<(), IssuerError> {
        Self::assert_operational(&env);
        let admin = Self::get_admin(env.clone()).map_err(|_| IssuerError::IssuerNotFound)?;
        Self::require_valid_issuer_address(&new_address)?;
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash.clone());
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(IssuerError::IssuerNotFound)?;

        if record.status == IssuerStatus::Revoked {
            return Err(IssuerError::IssuerRevoked);
        }
        if new_address == record.issuer_address {
            return Err(IssuerError::InvalidAddress);
        }

        let new_address_key = DataKey::AddressIssuer(new_address.clone());
        if env.storage().persistent().has(&new_address_key) {
            return Err(IssuerError::IssuerAddressAlreadyRegistered);
        }

        let old_address = record.issuer_address.clone();
        env.storage()
            .persistent()
            .remove(&DataKey::AddressIssuer(old_address.clone()));
        env.storage()
            .persistent()
            .remove(&DataKey::AddressTtl(old_address.clone()));
        record.issuer_address = new_address.clone();
        let now = env.ledger().timestamp();
        record.updated_at = now;
        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .set(&new_address_key, &issuer_id_hash);
        Self::extend_issuer_key_ttl(env.clone(), &key);
        Self::extend_address_ttl(env.clone(), new_address.clone());

        IssuerAddressRotated {
            issuer_id_hash,
            old_address,
            new_address,
            updated_at: now,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get_issuer(env: Env, issuer_id_hash: BytesN<32>) -> Result<IssuerRecord, IssuerError> {
        let key = DataKey::Issuer(issuer_id_hash);
        let record = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(IssuerError::IssuerNotFound)?;
        Self::extend_issuer_key_ttl(env, &key);
        Ok(record)
    }

    pub fn is_active_issuer(env: Env, issuer_id_hash: BytesN<32>) -> bool {
        match Self::get_issuer(env, issuer_id_hash) {
            Ok(record) => record.status == IssuerStatus::Active,
            Err(_) => false,
        }
    }

    pub fn is_active_address(env: Env, issuer_address: Address) -> bool {
        let issuer_id_hash: Option<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&DataKey::AddressIssuer(issuer_address.clone()));

        match issuer_id_hash {
            Some(id) => Self::is_active_issuer(env, id),
            None => false,
        }
    }

    // ── upgrade governance ────────────────────────────────────────────────────

    /// Returns the stored monotonic contract version.  Starts at 1.
    pub fn get_contract_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ContractVersion)
            .unwrap_or(0)
    }

    pub fn get_migration_status(env: Env) -> Option<MigrationStatus> {
        env.storage().instance().get(&DataKey::MigrationStatus)
    }

    pub fn begin_migration(
        env: Env,
        target_contract_version: u32,
        total_items: u32,
    ) -> Result<MigrationStatus, ContractError> {
        let admin = Self::get_admin(env.clone())?;
        Self::require_auth(&admin);
        if target_contract_version <= Self::get_contract_version(env.clone()) || total_items == 0 {
            return Err(ContractError::InvalidInput);
        }
        if let Some(status) = Self::get_migration_status(env.clone()) {
            return if status.target_contract_version == target_contract_version
                && status.total_items == total_items
            {
                Ok(status)
            } else {
                Err(ContractError::InvalidState)
            };
        }
        let status = MigrationStatus {
            status_version: MIGRATION_STATUS_VERSION,
            target_contract_version,
            cursor: 0,
            total_items,
            complete: false,
        };
        env.storage()
            .instance()
            .set(&DataKey::MigrationStatus, &status);
        Self::extend_instance_ttl(env);
        Ok(status)
    }

    pub fn advance_migration(
        env: Env,
        expected_cursor: u32,
        processed_items: u32,
    ) -> Result<MigrationStatus, ContractError> {
        let admin = Self::get_admin(env.clone())?;
        Self::require_auth(&admin);
        if processed_items == 0 || processed_items > MAX_MIGRATION_BATCH {
            return Err(ContractError::InvalidInput);
        }
        let mut status =
            Self::get_migration_status(env.clone()).ok_or(ContractError::InvalidState)?;
        if expected_cursor < status.cursor {
            return if expected_cursor.saturating_add(processed_items) <= status.cursor {
                Ok(status)
            } else {
                Err(ContractError::InvalidState)
            };
        }
        if expected_cursor != status.cursor || status.complete {
            return Err(ContractError::InvalidState);
        }
        let next = status
            .cursor
            .checked_add(processed_items)
            .ok_or(ContractError::InvalidInput)?;
        if next > status.total_items {
            return Err(ContractError::InvalidInput);
        }
        status.cursor = next;
        status.complete = next == status.total_items;
        env.storage()
            .instance()
            .set(&DataKey::MigrationStatus, &status);
        Self::extend_instance_ttl(env);
        Ok(status)
    }

    pub fn get_config_digest_version() -> u32 {
        earnproof_shared::CONFIG_DIGEST_VERSION
    }

    pub fn get_config_digest(env: Env) -> Result<BytesN<32>, ContractError> {
        let admin = Self::get_admin(env.clone())?;
        Ok(earnproof_shared::issuer_registry_digest(
            &env,
            &admin,
            Self::get_contract_version(env.clone()),
        ))
    }

    pub fn get_instance_ttl_status(env: Env) -> TtlStatus {
        earnproof_shared::ttl_status(
            env.ledger().sequence(),
            env.storage().instance().has(&DataKey::Admin),
            env.storage().instance().get(&DataKey::InstanceLiveUntil),
        )
    }

    pub fn get_issuer_ttl_status(env: Env, issuer_id_hash: BytesN<32>) -> TtlStatus {
        earnproof_shared::ttl_status(
            env.ledger().sequence(),
            env.storage()
                .persistent()
                .has(&DataKey::Issuer(issuer_id_hash.clone())),
            env.storage()
                .persistent()
                .get(&DataKey::IssuerTtl(issuer_id_hash)),
        )
    }

    pub fn get_address_ttl_status(env: Env, issuer_address: Address) -> TtlStatus {
        earnproof_shared::ttl_status(
            env.ledger().sequence(),
            env.storage()
                .persistent()
                .has(&DataKey::AddressIssuer(issuer_address.clone())),
            env.storage()
                .persistent()
                .get(&DataKey::AddressTtl(issuer_address)),
        )
    }

    pub fn refresh_instance_ttl(env: Env) -> Result<TtlStatus, ContractError> {
        let admin = Self::get_admin(env.clone())?;
        Self::require_auth(&admin);
        Self::extend_instance_ttl(env.clone());
        Ok(Self::get_instance_ttl_status(env))
    }

    /// Admin-only: add `wasm_hash` to the upgrade allowlist.
    ///
    /// `new_version` must be strictly greater than the current contract
    /// version to prevent pre-approving a downgrade.
    pub fn approve_upgrade(env: Env, wasm_hash: BytesN<32>, new_version: u32) {
        let admin = Self::get_admin(env.clone()).expect("contract not initialized");
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

    /// Admin-only: remove a hash from the allowlist without applying it.
    pub fn revoke_upgrade(env: Env, wasm_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone()).expect("contract not initialized");
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
    /// Requirements:
    /// 1. Caller is the admin.
    /// 2. `wasm_hash` is on the allowlist.
    /// 3. Target version is strictly greater than current (downgrade guard).
    ///
    /// On success the allowlist entry is consumed and `ContractVersion` is
    /// advanced.
    pub fn upgrade_contract(env: Env, wasm_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone()).expect("contract not initialized");
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
        if let Some(status) = Self::get_migration_status(env.clone()) {
            if !status.complete || status.target_contract_version != new_version {
                panic!("required storage migration is incomplete");
            }
        }

        // Consume allowlist entry before applying to prevent replay.
        env.storage()
            .instance()
            .remove(&DataKey::AllowedWasm(wasm_hash.clone()));

        #[cfg(not(test))]
        env.deployer()
            .update_current_contract_wasm(wasm_hash.clone());

        env.storage()
            .instance()
            .set(&DataKey::ContractVersion, &new_version);
        env.storage().instance().remove(&DataKey::MigrationStatus);
        Self::extend_instance_ttl(env.clone());

        ContractUpgraded {
            new_wasm_hash: wasm_hash,
            old_contract_version: old_version,
            new_contract_version: new_version,
            upgraded_by: admin,
        }
        .publish(&env);
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn require_valid_admin(address: &Address) -> Result<(), ContractError> {
        if !earnproof_shared::is_valid_principal_address(address) {
            return Err(ContractError::InvalidInput);
        }
        Ok(())
    }

    fn assert_operational(env: &Env) {
        if Self::get_migration_status(env.clone()).is_some_and(|status| !status.complete) {
            panic!("storage migration in progress");
        }
    }

    fn require_valid_issuer_address(address: &Address) -> Result<(), IssuerError> {
        if !earnproof_shared::is_valid_principal_address(address) {
            return Err(IssuerError::InvalidAddress);
        }
        Ok(())
    }

    fn set_status(
        env: Env,
        issuer_id_hash: BytesN<32>,
        status: IssuerStatus,
    ) -> Result<(), IssuerError> {
        Self::assert_operational(&env);
        let admin = Self::get_admin(env.clone()).map_err(|_| IssuerError::IssuerNotFound)?;
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash.clone());
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(IssuerError::IssuerNotFound)?;

        if record.status == IssuerStatus::Revoked && status != IssuerStatus::Revoked {
            return Err(IssuerError::InvalidTransition);
        }

        record.status = status.clone();
        let now = env.ledger().timestamp();
        record.updated_at = now;
        env.storage().persistent().set(&key, &record);
        Self::extend_issuer_key_ttl(env.clone(), &key);

        match status {
            IssuerStatus::Active => IssuerReactivated {
                issuer_id_hash,
                updated_at: now,
            }
            .publish(&env),
            IssuerStatus::Suspended => IssuerSuspended {
                issuer_id_hash,
                updated_at: now,
            }
            .publish(&env),
            IssuerStatus::Revoked => IssuerRevoked {
                issuer_id_hash,
                updated_at: now,
            }
            .publish(&env),
        }
        Ok(())
    }

    fn extend_instance_ttl(env: Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
        let live_until = Self::tracked_live_until(&env);
        env.storage()
            .instance()
            .set(&DataKey::InstanceLiveUntil, &live_until);
    }

    fn extend_issuer_ttl(env: Env, issuer_id_hash: BytesN<32>) {
        Self::extend_issuer_key_ttl(env, &DataKey::Issuer(issuer_id_hash));
    }

    fn extend_issuer_key_ttl(env: Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
        if let DataKey::Issuer(issuer_id_hash) = key {
            let tracker = DataKey::IssuerTtl(issuer_id_hash.clone());
            let live_until = Self::tracked_live_until(&env);
            env.storage().persistent().set(&tracker, &live_until);
            env.storage().persistent().extend_ttl(
                &tracker,
                TTL_THRESHOLD_LEDGERS,
                TTL_EXTEND_TO_LEDGERS,
            );
        }
    }

    fn extend_address_ttl(env: Env, issuer_address: Address) {
        let tracker = DataKey::AddressTtl(issuer_address.clone());
        let live_until = Self::tracked_live_until(&env);
        env.storage().persistent().extend_ttl(
            &DataKey::AddressIssuer(issuer_address),
            TTL_THRESHOLD_LEDGERS,
            TTL_EXTEND_TO_LEDGERS,
        );
        env.storage().persistent().set(&tracker, &live_until);
        env.storage().persistent().extend_ttl(
            &tracker,
            TTL_THRESHOLD_LEDGERS,
            TTL_EXTEND_TO_LEDGERS,
        );
    }

    fn tracked_live_until(env: &Env) -> u32 {
        env.ledger()
            .sequence()
            .saturating_add(TTL_EXTEND_TO_LEDGERS.min(env.storage().max_ttl()))
    }

    fn require_auth(address: &Address) {
        address.require_auth();
    }

    pub fn get_issuer_by_address(
        env: Env,
        issuer_address: Address,
    ) -> Result<IssuerRecord, IssuerError> {
        let issuer_id_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::AddressIssuer(issuer_address.clone()))
            .ok_or(IssuerError::IssuerAddressNotFound)?;

        let record = env
            .storage()
            .persistent()
            .get(&DataKey::Issuer(issuer_id_hash))
            .ok_or(IssuerError::IssuerNotFound)?;
        Self::extend_address_ttl(env, issuer_address);
        Ok(record)
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::{DataKey, IssuerRegistryContract, IssuerRegistryContractClient};
    use earnproof_shared::{IssuerError, IssuerStatus, TTL_THRESHOLD_LEDGERS};
    use soroban_sdk::{
        testutils::{storage::Persistent as _, Address as _, Events, MockAuth, MockAuthInvoke},
        Address, BytesN, Env, IntoVal,
    };

    const ADMIN: &str = "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR";
    const ISSUER_ONE: &str = "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U";
    const ISSUER_TWO: &str = "GDWUSKGGFDI4FRXK5EBTRECZSVQSSWJHHJOGH6JWG3AUMFFMQ435DIAG";

    fn bytes(env: &Env, value: u8) -> BytesN<32> {
        BytesN::from_array(env, &[value; 32])
    }

    fn setup() -> (Env, IssuerRegistryContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        client.initialize(&admin);
        (env, client, admin)
    }

    // ── existing tests ────────────────────────────────────────────────────────
    // -----------------------------------------------------------------------
    // Existing behavioral tests (preserved)
    // -----------------------------------------------------------------------

    #[test]
    fn registers_and_reads_active_issuer() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let metadata_hash = bytes(&env, 2);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &metadata_hash);

        let record = client.get_issuer(&issuer_id);
        assert_eq!(record.issuer_id_hash, issuer_id);
        assert_eq!(record.issuer_address, issuer_address);
        assert_eq!(record.metadata_hash, metadata_hash);
        assert_eq!(record.status, IssuerStatus::Active);
        assert!(client.is_active_issuer(&issuer_id));
        assert!(client.is_active_address(&issuer_address));
    }

    #[test]
    fn status_transitions_reject_reactivated_revoked_issuer() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.suspend_issuer(&issuer_id);
        assert!(!client.is_active_issuer(&issuer_id));

        client.reactivate_issuer(&issuer_id);
        assert!(client.is_active_issuer(&issuer_id));

        client.revoke_issuer(&issuer_id);
        assert!(!client.is_active_issuer(&issuer_id));
    }

    #[test]
    fn rejects_duplicate_issuer_id() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));

        let result = client.try_register_issuer(
            &issuer_id,
            &Address::from_str(&env, ISSUER_TWO),
            &bytes(&env, 3),
        );
        assert_eq!(result, Err(Ok(IssuerError::IssuerAlreadyRegistered)));
    }

    #[test]
    fn revoked_issuer_cannot_be_reactivated() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.revoke_issuer(&issuer_id);

        let result = client.try_reactivate_issuer(&issuer_id);
        assert_eq!(result, Err(Ok(IssuerError::InvalidTransition)));
    }

    #[test]
    fn extends_issuer_storage_ttl() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));

        env.as_contract(&client.address, || {
            assert!(
                env.storage()
                    .persistent()
                    .get_ttl(&DataKey::Issuer(issuer_id.clone()))
                    > TTL_THRESHOLD_LEDGERS
            );
            assert!(
                env.storage()
                    .persistent()
                    .get_ttl(&DataKey::AddressIssuer(issuer_address.clone()))
                    > TTL_THRESHOLD_LEDGERS
            );
        });
    }

    // ── upgrade governance tests ──────────────────────────────────────────────

    #[test]
    fn contract_version_initialized_to_one() {
        let (_env, client, _admin) = setup();
        assert_eq!(client.get_contract_version(), 1);
    }

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
        client.revoke_upgrade(&hash);
        assert!(!client.is_upgrade_allowed(&hash));
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn approve_upgrade_rejects_downgrade_version() {
        let (env, client, _admin) = setup();
        client.approve_upgrade(&bytes(&env, 1), &1);
    }

    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_contract_rejects_non_allowlisted_hash() {
        let (env, client, _admin) = setup();
        client.upgrade_contract(&bytes(&env, 0xff));
    }

    /// Auth guard: upgrade_contract without admin signature must panic.
    #[test]
    #[should_panic]
    fn upgrade_contract_requires_admin_auth() {
        let env = Env::default();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        env.mock_all_auths();
        client.initialize(&admin);
        let hash = BytesN::from_array(&env, &[0xde; 32]);
        client.approve_upgrade(&hash, &2);
        env.set_auths(&[]);

        client.upgrade_contract(&hash);
    }

    #[test]
    fn upgrade_advances_version_and_consumes_allowlist() {
        let (env, client, _admin) = setup();
        let hash = bytes(&env, 0x42);

        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);

        assert_eq!(client.get_contract_version(), 2);
        assert!(!client.is_upgrade_allowed(&hash));
    }

    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_hash_cannot_be_replayed() {
        let (env, client, _admin) = setup();
        let hash = bytes(&env, 0x42);

        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);
        client.upgrade_contract(&hash);
    }

    /// Persistent issuer state must survive an upgrade.
    #[test]
    fn state_preserved_across_upgrade() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        assert!(client.is_active_issuer(&issuer_id));

        let hash = bytes(&env, 0x77);
        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);

        // Issuer record must still be intact.
        assert!(client.is_active_issuer(&issuer_id));
        assert_eq!(client.get_contract_version(), 2);
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn cannot_re_approve_old_version_after_upgrade() {
        let (env, client, _admin) = setup();
        let hash_v2 = bytes(&env, 0x01);
        let old_hash = bytes(&env, 0x02);

        client.approve_upgrade(&hash_v2, &2);
        client.upgrade_contract(&hash_v2);

        // Attempting to allowlist version 1 after reaching version 2.
        client.approve_upgrade(&old_hash, &1);
    }

    // -----------------------------------------------------------------------
    // Event payload tests
    //
    // The Soroban test environment clears the event buffer at the start of each
    // top-level contract invocation (invocation metering is enabled by default
    // in Env::default()). Therefore env.events().all().events() reflects only
    // the events from the most recent invocation. Tests assert on the count
    // returned by a single invocation rather than a before/after diff.
    //
    // Failed invocations produce no contract events (failed_call events are
    // filtered out by all()). The catch_unwind tests confirm this by asserting
    // that a failed call leaves zero success events.
    // -----------------------------------------------------------------------

    /// register_issuer must emit exactly one event on success.
    #[test]
    fn register_issuer_emits_one_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let metadata_hash = bytes(&env, 2);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &metadata_hash);

        assert_eq!(
            env.events().all().events().len(),
            1,
            "expected exactly one event on registration"
        );
    }

    /// Duplicate registration panics before emitting any success event.
    #[test]
    fn register_issuer_failure_emits_no_success_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));

        // Attempt a duplicate — the invocation must panic.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 3));
        }));
        assert!(result.is_err(), "expected panic on duplicate");
        // Failed invocations emit no contract success events.
        assert_eq!(
            env.events().all().events().len(),
            0,
            "no success event should be emitted on a failed registration"
        );
    }

    /// update_issuer emits exactly one event on success.
    #[test]
    fn update_issuer_emits_one_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);
        let new_metadata = bytes(&env, 99);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.update_issuer(&issuer_id, &new_metadata);

        assert_eq!(
            env.events().all().events().len(),
            1,
            "expected exactly one event on metadata update"
        );
    }

    /// Updating a revoked issuer panics and emits no success event.
    #[test]
    fn update_revoked_issuer_emits_no_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.revoke_issuer(&issuer_id);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.update_issuer(&issuer_id, &bytes(&env, 99));
        }));
        assert!(result.is_err(), "expected panic on revoked issuer update");
        assert_eq!(
            env.events().all().events().len(),
            0,
            "no success event should be emitted on a failed update"
        );
    }

    /// suspend_issuer emits exactly one event.
    #[test]
    fn suspend_issuer_emits_one_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.suspend_issuer(&issuer_id);

        assert_eq!(
            env.events().all().events().len(),
            1,
            "expected exactly one event on suspension"
        );
    }

    /// reactivate_issuer emits exactly one event.
    #[test]
    fn reactivate_issuer_emits_one_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.suspend_issuer(&issuer_id);
        client.reactivate_issuer(&issuer_id);

        assert_eq!(
            env.events().all().events().len(),
            1,
            "expected exactly one event on reactivation"
        );
    }

    /// revoke_issuer emits exactly one event.
    #[test]
    fn revoke_issuer_emits_one_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.revoke_issuer(&issuer_id);

        assert_eq!(
            env.events().all().events().len(),
            1,
            "expected exactly one event on revocation"
        );
    }

    /// rotate_issuer_address emits exactly one event containing both old and new addresses.
    #[test]
    fn rotate_address_emits_one_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let old_address = Address::from_str(&env, ISSUER_ONE);
        let new_address = Address::from_str(&env, ISSUER_TWO);

        client.register_issuer(&issuer_id, &old_address, &bytes(&env, 2));
        client.rotate_issuer_address(&issuer_id, &new_address);

        assert_eq!(
            env.events().all().events().len(),
            1,
            "expected exactly one event on address rotation"
        );
    }

    /// rotate_issuer_address on a revoked issuer panics and emits no success event.
    #[test]
    fn rotate_revoked_issuer_address_emits_no_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let old_address = Address::from_str(&env, ISSUER_ONE);
        let new_address = Address::from_str(&env, ISSUER_TWO);

        client.register_issuer(&issuer_id, &old_address, &bytes(&env, 2));
        client.revoke_issuer(&issuer_id);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.rotate_issuer_address(&issuer_id, &new_address);
        }));
        assert!(result.is_err(), "expected panic on revoked issuer rotation");
        assert_eq!(
            env.events().all().events().len(),
            0,
            "no success event should be emitted on a failed rotation"
        );
    }

    /// Each successful mutation emits exactly one event (full lifecycle).
    /// Each call is checked independently since the event buffer resets per
    /// invocation.
    #[test]
    fn each_mutation_emits_exactly_one_event() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);
        let new_address = Address::from_str(&env, ISSUER_TWO);

        // register
        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        assert_eq!(env.events().all().events().len(), 1);

        // update metadata
        client.update_issuer(&issuer_id, &bytes(&env, 3));
        assert_eq!(env.events().all().events().len(), 1);

        // suspend
        client.suspend_issuer(&issuer_id);
        assert_eq!(env.events().all().events().len(), 1);

        // reactivate
        client.reactivate_issuer(&issuer_id);
        assert_eq!(env.events().all().events().len(), 1);

        // rotate address
        client.rotate_issuer_address(&issuer_id, &new_address);
        assert_eq!(env.events().all().events().len(), 1);

        // revoke
        client.revoke_issuer(&issuer_id);
        assert_eq!(env.events().all().events().len(), 1);
    }

    // -----------------------------------------------------------------------
    // Auth mock-parity (#72)
    //
    // Every test above uses env.mock_all_auths() via setup(), which lets any
    // caller through unconditionally — it can never observe that
    // revoke_issuer actually demands the *admin's* signature specifically.
    // This test scopes mock_auths to a real, valid signer that is not the
    // admin (the registered issuer's own address) and asserts the contract's
    // real require_auth(&admin) check rejects it — proving the issuer's own
    // valid signature cannot authorize an admin-only operation on itself.
    // -----------------------------------------------------------------------

    #[test]
    fn revoke_issuer_rejects_a_valid_signature_from_the_issuer_itself() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        client.initialize(&admin);

        // mock_auths (below) registers a stand-in auth contract at each
        // mocked address, so the address must be one the test Env generated
        // itself — a hardcoded G-string constant (like ISSUER_ONE, used by
        // every other test in this module under mock_all_auths()) is not a
        // valid registration target here.
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::generate(&env);
        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));

        // From here on, only the issuer's own signature is authorized for
        // this specific revoke_issuer invocation — not a blanket
        // mock_all_auths(). The issuer's signature is genuinely valid (it is
        // a real, well-formed authorization the host will accept); it is
        // simply for the wrong address. If require_auth(&admin) were ever
        // weakened to accept any authorized caller, this is what would stop
        // silently passing.
        env.mock_auths(&[MockAuth {
            address: &issuer_address,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "revoke_issuer",
                args: (issuer_id.clone(),).into_val(&env),
                sub_invokes: &[],
            },
        }]);

        let result = client.try_revoke_issuer(&issuer_id);
        assert!(
            result.is_err(),
            "the issuer's own valid signature must not authorize revoking itself; only the admin's signature may"
        );

        // And unrevoked: the rejected call must not have mutated state.
        assert_eq!(client.get_issuer(&issuer_id).status, IssuerStatus::Active);
    }

    // ── numeric boundary tests ────────────────────────────────────────────────

    /// Contract version boundaries for issuer-registry.
    /// While issuer-registry has no direct numeric user inputs, it does support
    /// contract versioning and upgrade governance. This test covers version boundaries.
    #[test]
    fn contract_version_initialized_and_upgradeable() {
        let (env, client, _admin) = setup();

        // Contract version should be initialized to 1
        assert_eq!(client.get_contract_version(), 1);

        // Valid: upgrade to next version
        client.approve_upgrade(&bytes(&env, 1), &2);
        assert!(client.is_upgrade_allowed(&bytes(&env, 1)));
    }

    #[test]
    fn contract_version_upgrade_boundaries() {
        let (env, client, _admin) = setup();

        // Valid: immediate next version
        client.approve_upgrade(&bytes(&env, 1), &2);
        client.upgrade_contract(&bytes(&env, 1));
        assert_eq!(client.get_contract_version(), 2);

        // Valid: large version number
        client.approve_upgrade(&bytes(&env, 2), &u32::MAX);
        client.upgrade_contract(&bytes(&env, 2));
        assert_eq!(client.get_contract_version(), u32::MAX);
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn contract_version_equal_current_rejected() {
        let (env, client, _admin) = setup();
        // Current version is 1; attempting version 1 is rejected
        client.approve_upgrade(&bytes(&env, 1), &1);
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn contract_version_below_current_rejected() {
        let (env, client, _admin) = setup();
        // Current version is 1; attempting version 0 is rejected
        client.approve_upgrade(&bytes(&env, 1), &0);
    }

    /// Test storage invariants: failed boundary cases must not modify state.
    #[test]
    fn failed_upgrade_version_downgrade_leaves_state_unchanged() {
        let (env, client, _admin) = setup();

        let contract_version_before = client.get_contract_version();
        let hash = bytes(&env, 0x88);

        // Attempt to allowlist a downgrade
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.approve_upgrade(&hash, &0);
        }));

        // Must have panicked
        assert!(result.is_err());

        // Contract version must not change
        assert_eq!(
            client.get_contract_version(),
            contract_version_before,
            "contract version must not change on failed upgrade approval"
        );

        // Hash must not be on allowlist
        assert!(
            !client.is_upgrade_allowed(&hash),
            "failed upgrade approval must not add hash to allowlist"
        );
    }

    // ── adversarial initialization tests ───────────────────────────────────────

    /// Verify that first initialization writes exactly the documented state
    /// with no partial writes or missing fields.
    ///
    /// Required behavior: First call to `initialize` results in:
    /// - Admin address set and readable
    /// - ContractVersion = 1
    #[test]
    fn initialization_writes_exactly_documented_state() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        // Perform initialization
        client.initialize(&admin);

        // Verify exact state written
        assert_eq!(client.get_admin(), admin, "admin must be set");
        assert_eq!(
            client.get_contract_version(),
            1,
            "contract version must be exactly 1 after initialization"
        );

        // Verify storage keys are set
        env.as_contract(&contract_id, || {
            let instance = env.storage().instance();
            assert!(
                instance.has(&DataKey::Admin),
                "Admin key must exist in instance storage"
            );
            assert!(
                instance.has(&DataKey::ContractVersion),
                "ContractVersion key must exist in instance storage"
            );
        });
    }

    /// Verify that repeated initialization by any address fails without
    /// altering state or emitting events.
    ///
    /// Required behavior for re-initialization guard:
    /// - Second call to `initialize` with any admin (same or different) panics
    /// - Storage is byte-for-byte unchanged
    /// - No additional events are emitted
    #[test]
    fn reinitialization_by_same_admin_fails_atomically() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        // First initialization succeeds
        client.initialize(&admin);
        let contract_version_after_first = client.get_contract_version();

        // Attempt second initialization with same admin
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&admin);
        }));

        // Must have panicked with "already initialized"
        assert!(result.is_err(), "re-initialization must panic");

        // Verify state is byte-for-byte identical
        assert_eq!(
            client.get_admin(),
            admin,
            "admin must not change after failed re-initialization"
        );
        assert_eq!(
            client.get_contract_version(),
            contract_version_after_first,
            "contract version must not change after failed re-initialization"
        );
    }

    /// Verify that re-initialization by a different address also fails
    /// without state or event changes.
    ///
    /// This tests that the re-initialization guard does not discriminate
    /// based on caller identity — it prevents any re-initialization attempt.
    #[test]
    fn reinitialization_by_different_admin_fails_atomically() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let other = Address::from_str(&env, ISSUER_ONE);

        // First initialization with original admin
        client.initialize(&admin);
        let stored_admin = client.get_admin();
        let contract_version_after_first = client.get_contract_version();

        // Attempt re-initialization with different admin
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&other);
        }));

        // Must have panicked
        assert!(
            result.is_err(),
            "re-initialization by different admin must panic"
        );

        // Verify state is unchanged: original admin must still be stored
        assert_eq!(
            client.get_admin(),
            stored_admin,
            "admin must not change when different address attempts re-initialization"
        );
        assert_eq!(
            client.get_contract_version(),
            contract_version_after_first,
            "contract version must not change after failed re-initialization by different admin"
        );
    }

    /// Verify that the re-initialization guard does not allow partial state
    /// modification on subsequent initialization attempts.
    #[test]
    fn reinitialization_guard_is_absolute() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        // First initialization
        client.initialize(&admin);

        // Multiple re-initialization attempts must all fail
        for attempt in 1..=3 {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                client.initialize(&admin);
            }));

            assert!(
                result.is_err(),
                "re-initialization attempt {} must fail",
                attempt
            );

            // Admin must remain unchanged
            assert_eq!(
                client.get_admin(),
                admin,
                "admin must not change after re-initialization attempt {}",
                attempt
            );
        }
    }

    /// Verify that initialization state is maintained across subsequent
    /// issuer registration and upgrade operations.
    ///
    /// Tests that the initialization state (admin, contract version) is stable
    /// and correct before and after other contract operations.
    #[test]
    fn initialization_state_stable_across_operations() {
        let (env, client, admin) = setup();

        // State immediately after initialization
        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_contract_version(), 1);

        // Perform issuer registration
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);
        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));

        // Admin must remain unchanged
        assert_eq!(
            client.get_admin(),
            admin,
            "admin must not change after issuer registration"
        );
        // Contract version must still be 1 (no upgrade yet)
        assert_eq!(
            client.get_contract_version(),
            1,
            "contract version must not change on issuer registration"
        );
    }

    /// Summary test: issuer-registry initialization spec verification.
    ///
    /// This test serves as executable documentation of what the test matrix
    /// expects from issuer-registry initialization:
    /// - Standalone contract (no dependency addresses)
    /// - Has re-initialization guard
    /// - Does NOT emit an event during initialization
    /// - Sets: admin, contract_version=1
    #[test]
    fn issuer_registry_initialization_spec_summary() {
        // CONTRACT SPEC: issuer-registry
        // - Name: "issuer-registry"
        // - Has re-initialization guard: YES (panics "already initialized")
        // - Emits initialization event: NO
        // - Takes dependency addresses: NO
        // - Dependencies: []
        // - First init writes:
        //   - Admin: passed address (requires auth)
        //   - ContractVersion: 1
        // - Re-init guard: DataKey::Admin presence check; panics if set
        // - Re-init allowed by different admin: NO (guard blocks all)
        // - Invalid config cases: None (no dependencies to validate)

        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(IssuerRegistryContract, ());
        let client = IssuerRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);

        // Verify the spec
        client.initialize(&admin);
        assert_eq!(client.get_admin(), admin);
        assert_eq!(client.get_contract_version(), 1);

        // Re-initialization must fail
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.initialize(&admin)
        }))
        .is_err());
    }

    #[test]
    fn configuration_digest_matches_host_helper_and_version_changes() {
        let (env, client, admin) = setup();
        let initial = client.get_config_digest();
        assert_eq!(
            IssuerRegistryContractClient::get_config_digest_version(&client),
            earnproof_shared::CONFIG_DIGEST_VERSION
        );
        assert_eq!(
            initial,
            earnproof_shared::issuer_registry_digest(&env, &admin, 1)
        );

        let wasm_hash = bytes(&env, 0xd1);
        client.approve_upgrade(&wasm_hash, &2);
        client.upgrade_contract(&wasm_hash);
        assert_ne!(client.get_config_digest(), initial);
    }

    #[test]
    fn ttl_status_tracks_only_caller_named_issuer_entries() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 0xe1);
        let unknown_id = bytes(&env, 0xe2);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        assert_eq!(
            client.get_instance_ttl_status().health,
            earnproof_shared::TtlHealth::Healthy
        );
        assert_eq!(
            client.get_issuer_ttl_status(&unknown_id).health,
            earnproof_shared::TtlHealth::Missing
        );
        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 0xe3));
        assert_eq!(
            client.get_issuer_ttl_status(&issuer_id).health,
            earnproof_shared::TtlHealth::Healthy
        );
        assert_eq!(
            client.get_address_ttl_status(&issuer_address).health,
            earnproof_shared::TtlHealth::Healthy
        );
    }
}
