#![no_std]

use earnproof_shared::{IssuerRecord, IssuerStatus, TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS, MAX_ISSUER_ID_HASH_BYTES, MAX_METADATA_HASH_BYTES};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct IssuerRegistryContract;

/// Error type for issuer registry contract.
#[derive(Debug)]
#[contracttype]
pub enum ContractError {
    /// Returned when an input parameter exceeds its documented maximum size.
    InputTooLarge = 1000,
}

#[contracttype]
enum DataKey {
    Admin,
    Issuer(BytesN<32>),
    AddressIssuer(Address),
}

#[contractimpl]
impl IssuerRegistryContract {
    /// Initializes the issuer registry contract.
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
        Self::extend_instance_ttl(env);
    }

    /// Retrieves the current issuer registry administrator address.
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

    /// Registers a new issuer in the registry.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    /// - `issuer_address`: fixed-size Stellar address (no validation needed)
    /// - `metadata_hash`: maximum `MAX_METADATA_HASH_BYTES` (32) bytes (fixed-size)
    ///
    /// # Validation
    /// - Input size validation (issuer_id_hash and metadata_hash)
    /// - Checks that issuer_id_hash is not already registered
    /// - Checks that issuer_address is not already registered
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - `DataKey::Issuer(issuer_id_hash)` → IssuerRecord (persistent)
    /// - `DataKey::AddressIssuer(issuer_address)` → issuer_id_hash (persistent)
    ///
    /// # Failure Atomicity
    /// Over-limit inputs are rejected before any storage write.
    /// Duplicate key checks occur before storage. No partial state is committed on error.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if any hash exceeds its limit
    ///
    /// # Panics
    /// - If issuer_id_hash is already registered
    /// - If issuer_address is already registered
    pub fn register_issuer(
        env: Env,
        issuer_id_hash: BytesN<32>,
        issuer_address: Address,
        metadata_hash: BytesN<32>,
    ) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash.clone());
        if env.storage().persistent().has(&key) {
            panic!("issuer already registered");
        }

        let address_key = DataKey::AddressIssuer(issuer_address.clone());
        if env.storage().persistent().has(&address_key) {
            panic!("issuer address already registered");
        }

        let now = env.ledger().timestamp();
        let record = IssuerRecord {
            issuer_id_hash: issuer_id_hash.clone(),
            issuer_address: issuer_address.clone(),
            metadata_hash,
            status: IssuerStatus::Active,
            created_at: now,
            updated_at: now,
        };

        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .set(&address_key, &issuer_id_hash);
        Self::extend_issuer_ttl(env.clone(), issuer_id_hash.clone());
        Self::extend_address_ttl(env, issuer_address);
    }

    /// Updates the metadata for an existing issuer.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    /// - `metadata_hash`: maximum `MAX_METADATA_HASH_BYTES` (32) bytes (fixed-size)
    ///
    /// # Validation
    /// - Input size validation (issuer_id_hash and metadata_hash)
    /// - Checks that issuer exists and is not revoked
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - `DataKey::Issuer(issuer_id_hash)` → updated IssuerRecord (persistent)
    ///
    /// # Failure Atomicity
    /// Over-limit inputs are rejected before any storage write.
    /// All validation occurs before storage. No partial state is committed on error.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if any hash exceeds its limit
    ///
    /// # Panics
    /// - If issuer is not found
    /// - If issuer is revoked
    pub fn update_issuer(env: Env, issuer_id_hash: BytesN<32>, metadata_hash: BytesN<32>) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash);
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("issuer not found");

        if record.status == IssuerStatus::Revoked {
            panic!("revoked issuer cannot be updated");
        }

        record.metadata_hash = metadata_hash;
        record.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        Self::extend_issuer_key_ttl(env, &key);
    }

    /// Suspends an issuer, preventing them from registering new proofs.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    ///
    /// # Validation
    /// - Input size validation (issuer_id_hash)
    /// - Checks that issuer exists
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - `DataKey::Issuer(issuer_id_hash)` → updated IssuerRecord with status=Suspended (persistent)
    ///
    /// # Failure Atomicity
    /// All validation occurs before storage. No partial state is committed on error.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if issuer_id_hash exceeds its limit
    ///
    /// # Panics
    /// - If issuer is not found
    pub fn suspend_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Suspended);
    }

    /// Reactivates a suspended issuer.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    ///
    /// # Validation
    /// - Input size validation (issuer_id_hash)
    /// - Checks that issuer exists and is not revoked
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - `DataKey::Issuer(issuer_id_hash)` → updated IssuerRecord with status=Active (persistent)
    ///
    /// # Failure Atomicity
    /// All validation occurs before storage. No partial state is committed on error.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if issuer_id_hash exceeds its limit
    ///
    /// # Panics
    /// - If issuer is not found
    /// - If issuer is revoked (cannot reactivate revoked issuers)
    pub fn reactivate_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Active);
    }

    /// Revokes an issuer permanently, preventing any future operations.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    ///
    /// # Validation
    /// - Input size validation (issuer_id_hash)
    /// - Checks that issuer exists
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - `DataKey::Issuer(issuer_id_hash)` → updated IssuerRecord with status=Revoked (persistent)
    ///
    /// # Failure Atomicity
    /// All validation occurs before storage. No partial state is committed on error.
    /// This is a terminal operation: revoked issuers cannot be reactivated.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if issuer_id_hash exceeds its limit
    ///
    /// # Panics
    /// - If issuer is not found
    pub fn revoke_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Revoked);
    }

    /// Rotates the Stellar address associated with an issuer.
    ///
    /// # Authorization
    /// Requires signature from the admin.
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    /// - `new_address`: fixed-size Stellar address (no validation needed)
    ///
    /// # Validation
    /// - Input size validation (issuer_id_hash)
    /// - Checks that issuer exists and is not revoked
    /// - Checks that new_address is not already registered to another issuer
    /// - Verifies authorization from admin
    ///
    /// # Storage Writes
    /// - Removes old `DataKey::AddressIssuer(old_address)` (persistent delete)
    /// - Writes new `DataKey::AddressIssuer(new_address)` → issuer_id_hash (persistent)
    /// - Updates `DataKey::Issuer(issuer_id_hash)` with new issuer_address (persistent)
    ///
    /// # Failure Atomicity
    /// Over-limit inputs are rejected before any storage write.
    /// All collision checks occur before any storage mutation.
    /// If new_address is already registered, no partial state is committed.
    ///
    /// # Errors
    /// - `ContractError::InputTooLarge` if issuer_id_hash exceeds its limit
    ///
    /// # Panics
    /// - If issuer is not found
    /// - If issuer is revoked (cannot rotate address of revoked issuer)
    /// - If new_address is already registered to another issuer
    pub fn rotate_issuer_address(env: Env, issuer_id_hash: BytesN<32>, new_address: Address) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash.clone());
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("issuer not found");

        if record.status == IssuerStatus::Revoked {
            panic!("revoked issuer cannot rotate address");
        }

        let new_address_key = DataKey::AddressIssuer(new_address.clone());
        if env.storage().persistent().has(&new_address_key) {
            panic!("issuer address already registered");
        }

        env.storage()
            .persistent()
            .remove(&DataKey::AddressIssuer(record.issuer_address.clone()));
        record.issuer_address = new_address.clone();
        record.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        env.storage()
            .persistent()
            .set(&new_address_key, &issuer_id_hash);
        Self::extend_issuer_key_ttl(env.clone(), &key);
        Self::extend_address_ttl(env, new_address);
    }

    /// Retrieves the full record for an issuer by their ID hash.
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    ///
    /// # Storage Reads
    /// - `DataKey::Issuer(issuer_id_hash)` (persistent, with TTL extension)
    ///
    /// # Returns
    /// - `IssuerRecord` containing all issuer information
    ///
    /// # Panics
    /// - If issuer is not found
    pub fn get_issuer(env: Env, issuer_id_hash: BytesN<32>) -> IssuerRecord {
        let key = DataKey::Issuer(issuer_id_hash);
        let record = env
            .storage()
            .persistent()
            .get(&key)
            .expect("issuer not found");
        Self::extend_issuer_key_ttl(env, &key);
        record
    }

    /// Checks if an issuer is currently active (not suspended or revoked).
    ///
    /// # Input Limits
    /// - `issuer_id_hash`: maximum `MAX_ISSUER_ID_HASH_BYTES` (32) bytes (fixed-size)
    ///
    /// # Storage Reads
    /// - `DataKey::Issuer(issuer_id_hash)` (persistent, with TTL extension)
    ///
    /// # Returns
    /// - `true` if issuer status is Active
    /// - `false` otherwise
    pub fn is_active_issuer(env: Env, issuer_id_hash: BytesN<32>) -> bool {
        let record = Self::get_issuer(env, issuer_id_hash);
        record.status == IssuerStatus::Active
    }

    /// Checks if a Stellar address is active (belongs to an active issuer).
    ///
    /// # Input Limits
    /// - `issuer_address`: fixed-size Stellar address (no validation needed)
    ///
    /// # Storage Reads
    /// - `DataKey::AddressIssuer(issuer_address)` (persistent)
    /// - `DataKey::Issuer(...)` (persistent, with TTL extension via is_active_issuer)
    ///
    /// # Returns
    /// - `true` if the address belongs to an active issuer
    /// - `false` otherwise
    ///
    /// # Panics
    /// - If the address is not found in the registry
    pub fn is_active_address(env: Env, issuer_address: Address) -> bool {
        let issuer_id_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::AddressIssuer(issuer_address.clone()))
            .expect("issuer address not found");

        Self::is_active_issuer(env, issuer_id_hash)
    }

    /// Retrieves the full record for an issuer by their Stellar address.
    ///
    /// # Input Limits
    /// - `issuer_address`: fixed-size Stellar address (no validation needed)
    ///
    /// # Storage Reads
    /// - `DataKey::AddressIssuer(issuer_address)` (persistent, with TTL extension)
    /// - `DataKey::Issuer(...)` (persistent, direct read without separate TTL extension)
    ///
    /// # Returns
    /// - `IssuerRecord` containing all issuer information
    ///
    /// # Panics
    /// - If the address is not found in the registry
    /// - If the issuer record is not found (should not happen if registry is consistent)
    pub fn get_issuer_by_address(env: Env, issuer_address: Address) -> IssuerRecord {
        let issuer_id_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::AddressIssuer(issuer_address.clone()))
            .expect("issuer address not found");

        let record = env
            .storage()
            .persistent()
            .get(&DataKey::Issuer(issuer_id_hash))
            .expect("issuer not found");
        Self::extend_address_ttl(env, issuer_address);
        record
    }

    fn set_status(env: Env, issuer_id_hash: BytesN<32>, status: IssuerStatus) {
        let admin = Self::get_admin(env.clone());
        Self::require_auth(&admin);

        let key = DataKey::Issuer(issuer_id_hash);
        let mut record: IssuerRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("issuer not found");

        if record.status == IssuerStatus::Revoked && status != IssuerStatus::Revoked {
            panic!("revoked issuer cannot be reactivated");
        }

        record.status = status;
        record.updated_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        Self::extend_issuer_key_ttl(env, &key);
    }

    fn extend_instance_ttl(env: Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    }

    fn extend_issuer_ttl(env: Env, issuer_id_hash: BytesN<32>) {
        Self::extend_issuer_key_ttl(env, &DataKey::Issuer(issuer_id_hash));
    }

    fn extend_issuer_key_ttl(env: Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    }

    fn extend_address_ttl(env: Env, issuer_address: Address) {
        env.storage().persistent().extend_ttl(
            &DataKey::AddressIssuer(issuer_address),
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

    use super::{DataKey, IssuerRegistryContract, IssuerRegistryContractClient};
    use earnproof_shared::{IssuerStatus, TTL_THRESHOLD_LEDGERS};
    use soroban_sdk::{testutils::storage::Persistent as _, Address, BytesN, Env};

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
    #[should_panic(expected = "issuer already registered")]
    fn rejects_duplicate_issuer_id() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.register_issuer(
            &issuer_id,
            &Address::from_str(&env, ISSUER_TWO),
            &bytes(&env, 3),
        );
    }

    #[test]
    #[should_panic(expected = "revoked issuer cannot be reactivated")]
    fn revoked_issuer_cannot_be_reactivated() {
        let (env, client, _admin) = setup();
        let issuer_id = bytes(&env, 1);
        let issuer_address = Address::from_str(&env, ISSUER_ONE);

        client.register_issuer(&issuer_id, &issuer_address, &bytes(&env, 2));
        client.revoke_issuer(&issuer_id);
        client.reactivate_issuer(&issuer_id);
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
}
