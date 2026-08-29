#![no_std]

use earnproof_shared::{
    ContractError, ProofError, ProofRecord, ProofStatus, TTL_EXTEND_TO_LEDGERS,
    TTL_THRESHOLD_LEDGERS,
};
use soroban_sdk::{contract, contractclient, contractimpl, contracttype, Address, BytesN, Env};

#[contractclient(name = "ProtocolConfigContractClient")]
pub trait ProtocolConfigInterface {
    fn is_paused(env: Env) -> bool;
    fn is_schema_version_approved(env: Env, version: u32) -> bool;
}

#[contractclient(name = "IssuerRegistryContractClient")]
pub trait IssuerRegistryInterface {
    fn is_active_address(env: Env, issuer_address: Address) -> bool;
}

#[contract]
pub struct ProofRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    IssuerRegistry,
    ProtocolConfig,
    Proof(BytesN<32>),
    /// Allowlist entry: maps a WASM hash to the target contract version.
    AllowedWasm(BytesN<32>),
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

#[contractimpl]
impl ProofRegistryContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        issuer_registry: Address,
        protocol_config: Address,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::IssuerRegistry, &issuer_registry);
        env.storage()
            .instance()
            .set(&DataKey::ProtocolConfig, &protocol_config);
        env.storage()
            .instance()
            .set(&DataKey::ContractVersion, &1_u32);
        Self::extend_instance_ttl(env);
        Ok(())
    }

    pub fn register_proof(
        env: Env,
        proof_id_hash: BytesN<32>,
        commitment_hash: BytesN<32>,
        issuer_address: Address,
        schema_version: u32,
        expires_at: u64,
    ) -> Result<(), ProofError> {
        Self::require_auth(&issuer_address);

        if schema_version == 0 {
            return Err(ProofError::InvalidSchemaVersion);
        }

        if expires_at <= env.ledger().timestamp() {
            return Err(ProofError::ProofExpired);
        }

        let protocol_config =
            Self::get_protocol_config(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        let protocol_client = ProtocolConfigContractClient::new(&env, &protocol_config);
        if protocol_client.is_paused() {
            return Err(ProofError::InvalidSchemaVersion); // Use existing error for protocol paused state
        }

        if !protocol_client.is_schema_version_approved(&schema_version) {
            return Err(ProofError::SchemaVersionNotApproved);
        }

        let issuer_registry =
            Self::get_issuer_registry(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
        let issuer_client = IssuerRegistryContractClient::new(&env, &issuer_registry);
        if !issuer_client.is_active_address(&issuer_address) {
            return Err(ProofError::InvalidSchemaVersion); // Simplified - issuer inactive
        }

        let key = DataKey::Proof(proof_id_hash.clone());
        if env.storage().persistent().has(&key) {
            return Err(ProofError::ProofAlreadyRegistered);
        }

        let now = env.ledger().timestamp();
        let record = ProofRecord {
            proof_id_hash,
            commitment_hash,
            issuer_address,
            status: ProofStatus::Active,
            schema_version,
            expires_at,
            created_at: now,
            revoked_at: 0,
        };

        env.storage().persistent().set(&key, &record);
        Self::extend_proof_key_ttl(env, &key);
        Ok(())
    }

    pub fn revoke_proof(env: Env, proof_id_hash: BytesN<32>) -> Result<(), ProofError> {
        Self::set_revoked(env, proof_id_hash, false)
    }

    pub fn admin_revoke_proof(env: Env, proof_id_hash: BytesN<32>) -> Result<(), ProofError> {
        Self::set_revoked(env, proof_id_hash, true)
    }

    pub fn get_proof(env: Env, proof_id_hash: BytesN<32>) -> Result<ProofRecord, ProofError> {
        let key = DataKey::Proof(proof_id_hash);
        let record = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ProofError::ProofNotFound)?;
        Self::extend_proof_key_ttl(env, &key);
        Ok(record)
    }

    pub fn is_valid_proof(env: Env, proof_id_hash: BytesN<32>) -> bool {
        match Self::get_proof(env.clone(), proof_id_hash) {
            Ok(record) => {
                record.status == ProofStatus::Active
                    && env.ledger().timestamp() <= record.expires_at
            }
            Err(_) => false,
        }
    }

    pub fn is_revoked(env: Env, proof_id_hash: BytesN<32>) -> bool {
        match Self::get_proof(env, proof_id_hash) {
            Ok(record) => record.status == ProofStatus::Revoked,
            Err(_) => false,
        }
    }

    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)
    }

    pub fn get_issuer_registry(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::IssuerRegistry)
            .ok_or(ContractError::NotInitialized)
    }

    pub fn get_protocol_config(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&DataKey::ProtocolConfig)
            .ok_or(ContractError::NotInitialized)
    }

    // ── upgrade governance ────────────────────────────────────────────────────

    /// Returns the stored monotonic contract version.  Starts at 1.
    pub fn get_contract_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ContractVersion)
            .unwrap_or(0)
    }

    /// Admin-only: add `wasm_hash` to the upgrade allowlist.
    ///
    /// `new_version` must be strictly greater than the current contract
    /// version to prevent pre-approving a downgrade.
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

    /// Admin-only: remove a hash from the allowlist without applying it.
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
    /// Requirements:
    /// 1. Caller is the admin.
    /// 2. `wasm_hash` is on the allowlist.
    /// 3. Target version is strictly greater than current (downgrade guard).
    ///
    /// On success the allowlist entry is consumed and `ContractVersion` is
    /// advanced.
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

        // Consume allowlist entry before applying to prevent replay.
        env.storage()
            .instance()
            .remove(&DataKey::AllowedWasm(wasm_hash.clone()));

        env.deployer().update_current_contract_wasm(wasm_hash.clone());

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

    // ── private helpers ───────────────────────────────────────────────────────

    fn set_revoked(env: Env, proof_id_hash: BytesN<32>, by_admin: bool) {
        let key = DataKey::Proof(proof_id_hash.clone());
        let mut record: ProofRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ProofError::ProofNotFound)?;

        if by_admin {
            let admin = Self::get_admin(env.clone()).map_err(|_| ProofError::ProofNotFound)?;
            Self::require_auth(&admin);
        } else {
            Self::require_auth(&record.issuer_address);
        }

        if record.status == ProofStatus::Revoked {
            return Err(ProofError::ProofAlreadyRevoked);
        }

        record.status = ProofStatus::Revoked;
        record.revoked_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        Self::extend_proof_key_ttl(env, &key);
        Ok(())
    }

    fn extend_instance_ttl(env: Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    }

    fn extend_proof_key_ttl(env: Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD_LEDGERS, TTL_EXTEND_TO_LEDGERS);
    }

    fn require_auth(address: &Address) {
        address.require_auth();
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::{DataKey, ProofRegistryContract, ProofRegistryContractClient};
    use earnproof_shared::{ProofStatus, TTL_THRESHOLD_LEDGERS};
    use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
    use protocol_config::{ProtocolConfigContract, ProtocolConfigContractClient};
    use soroban_sdk::{testutils::storage::Persistent as _, Address, BytesN, Env};

    const ADMIN: &str = "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR";
    const ISSUER: &str = "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U";

    fn bytes(env: &Env, value: u8) -> BytesN<32> {
        BytesN::from_array(env, &[value; 32])
    }

    fn setup() -> (
        Env,
        ProofRegistryContractClient<'static>,
        ProtocolConfigContractClient<'static>,
        IssuerRegistryContractClient<'static>,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let protocol_config_id = env.register(ProtocolConfigContract, ());
        let protocol_config_client = ProtocolConfigContractClient::new(&env, &protocol_config_id);
        let issuer_registry_id = env.register(IssuerRegistryContract, ());
        let issuer_registry_client = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);
        let issuer_id = bytes(&env, 9);

        protocol_config_client.initialize(&admin);
        protocol_config_client.approve_schema_version(&1);
        issuer_registry_client.initialize(&admin);
        issuer_registry_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));
        client.initialize(&admin, &issuer_registry_id, &protocol_config_id);

        (
            env,
            client,
            protocol_config_client,
            issuer_registry_client,
            issuer_registry_id,
        )
    }

    // ── existing tests ────────────────────────────────────────────────────────

    #[test]
    fn registers_and_validates_proof() {
        let (env, client, _protocol_config, _issuer_registry, issuer_registry_id) = setup();
        let proof_id = bytes(&env, 1);
        let commitment = bytes(&env, 2);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &commitment, &issuer, &1, &2_000);

        let record = client.get_proof(&proof_id);
        assert_eq!(record.proof_id_hash, proof_id);
        assert_eq!(record.commitment_hash, commitment);
        assert_eq!(record.issuer_address, issuer);
        assert_eq!(record.status, ProofStatus::Active);
        assert_eq!(client.get_issuer_registry(), issuer_registry_id);
        assert!(client.is_valid_proof(&proof_id));
        assert!(!client.is_revoked(&proof_id));
    }

    #[test]
    fn issuer_can_revoke_proof() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);
        client.revoke_proof(&proof_id);

        let record = client.get_proof(&proof_id);
        assert_eq!(record.status, ProofStatus::Revoked);
        assert!(client.is_revoked(&proof_id));
        assert!(!client.is_valid_proof(&proof_id));
    }

    #[test]
    fn rejects_expired_proof() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &1,
            &0,
        );
        assert_eq!(result, Err(Ok(ProofError::ProofExpired)));
    }

    #[test]
    fn rejects_duplicate_proof_id() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);

        let result = client.try_register_proof(&proof_id, &bytes(&env, 3), &issuer, &1, &2_000);
        assert_eq!(result, Err(Ok(ProofError::ProofAlreadyRegistered)));
    }

    #[test]
    fn rejects_unapproved_schema_version() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &2,
            &2_000,
        );
        assert_eq!(result, Err(Ok(ProofError::SchemaVersionNotApproved)));
    }

    #[test]
    fn rejects_registration_when_protocol_is_paused() {
        let (env, client, protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;
        protocol_config.pause();

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &1,
            &2_000,
        );
        assert_eq!(result, Err(Ok(ProofError::InvalidSchemaVersion)));
    }

    #[test]
    fn rejects_inactive_issuer_address() {
        let (env, client, _protocol_config, issuer_registry, _issuer_registry_id) = setup();
        use earnproof_shared::ProofError;
        let inactive_issuer = Address::from_str(
            &env,
            "GBXHUHG5FGYLPD6RHL2MKWMP572O6KUXCZXDZJXS4T57ZTMAKBN7DWXN",
        );
        issuer_registry.register_issuer(&bytes(&env, 10), &inactive_issuer, &bytes(&env, 11));
        issuer_registry.suspend_issuer(&bytes(&env, 10));

        let result = client.try_register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &inactive_issuer,
            &1,
            &2_000,
        );
        assert_eq!(result, Err(Ok(ProofError::InvalidSchemaVersion)));
    }

    #[test]
    fn extends_proof_storage_ttl() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);

        env.as_contract(&client.address, || {
            assert!(
                env.storage()
                    .persistent()
                    .get_ttl(&DataKey::Proof(proof_id.clone()))
                    > TTL_THRESHOLD_LEDGERS
            );
        });
    }

    // ── upgrade governance tests ──────────────────────────────────────────────

    #[test]
    fn contract_version_initialized_to_one() {
        let (_env, client, ..) = setup();
        assert_eq!(client.get_contract_version(), 1);
    }

    #[test]
    fn approve_and_check_allowlist() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0xab);

        assert!(!client.is_upgrade_allowed(&hash));
        client.approve_upgrade(&hash, &2);
        assert!(client.is_upgrade_allowed(&hash));
    }

    #[test]
    fn revoke_removes_from_allowlist() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0xcd);

        client.approve_upgrade(&hash, &2);
        client.revoke_upgrade(&hash);
        assert!(!client.is_upgrade_allowed(&hash));
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn approve_upgrade_rejects_downgrade_version() {
        let (env, client, ..) = setup();
        client.approve_upgrade(&bytes(&env, 1), &1);
    }

    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_contract_rejects_non_allowlisted_hash() {
        let (env, client, ..) = setup();
        client.upgrade_contract(&bytes(&env, 0xff));
    }

    #[test]
    #[should_panic]
    fn upgrade_contract_requires_admin_auth() {
        let env = Env::default();
        let protocol_config_id = env.register(ProtocolConfigContract, ());
        let issuer_registry_id = env.register(IssuerRegistryContract, ());
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let issuer = Address::from_str(&env, ISSUER);
        let issuer_id = bytes(&env, 9);

        env.mock_all_auths();
        let pc_client = ProtocolConfigContractClient::new(&env, &protocol_config_id);
        let ir_client = IssuerRegistryContractClient::new(&env, &issuer_registry_id);
        pc_client.initialize(&admin);
        pc_client.approve_schema_version(&1);
        ir_client.initialize(&admin);
        ir_client.register_issuer(&issuer_id, &issuer, &bytes(&env, 8));
        client.initialize(&admin, &issuer_registry_id, &protocol_config_id);

        let hash = BytesN::from_array(&env, &[0xde; 32]);
        client.approve_upgrade(&hash, &2);
        env.set_auths(&[]);

        client.upgrade_contract(&hash);
    }

    #[test]
    fn upgrade_advances_version_and_consumes_allowlist() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0x42);

        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);

        assert_eq!(client.get_contract_version(), 2);
        assert!(!client.is_upgrade_allowed(&hash));
    }

    #[test]
    #[should_panic(expected = "wasm hash not on allowlist")]
    fn upgrade_hash_cannot_be_replayed() {
        let (env, client, ..) = setup();
        let hash = bytes(&env, 0x42);

        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);
        client.upgrade_contract(&hash);
    }

    /// Persistent proof state must survive an upgrade.
    #[test]
    fn state_preserved_across_upgrade() {
        let (env, client, ..) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);
        assert!(client.is_valid_proof(&proof_id));

        let hash = bytes(&env, 0x77);
        client.approve_upgrade(&hash, &2);
        client.upgrade_contract(&hash);

        assert!(client.is_valid_proof(&proof_id));
        assert_eq!(client.get_contract_version(), 2);
    }

    #[test]
    #[should_panic(expected = "new_version must be greater than current contract version")]
    fn cannot_re_approve_old_version_after_upgrade() {
        let (env, client, ..) = setup();
        let hash_v2 = bytes(&env, 0x01);
        let old_hash = bytes(&env, 0x02);

        client.approve_upgrade(&hash_v2, &2);
        client.upgrade_contract(&hash_v2);

        client.approve_upgrade(&old_hash, &1);
    }
}
