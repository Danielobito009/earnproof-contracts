#![no_std]

use earnproof_shared::{ProofRecord, ProofStatus, TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS};
use issuer_registry::IssuerRegistryContractClient;
use protocol_config::ProtocolConfigContractClient;
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct ProofRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    IssuerRegistry,
    ProtocolConfig,
    Proof(BytesN<32>),
}

#[contractimpl]
impl ProofRegistryContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        issuer_registry: Address,
        protocol_config: Address,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::IssuerRegistry, &issuer_registry);
        env.storage()
            .instance()
            .set(&DataKey::ProtocolConfig, &protocol_config);
        Self::extend_instance_ttl(env);
    }

    pub fn register_proof(
        env: Env,
        proof_id_hash: BytesN<32>,
        commitment_hash: BytesN<32>,
        issuer_address: Address,
        schema_version: u32,
        expires_at: u64,
    ) {
        Self::require_auth(&issuer_address);

        if schema_version == 0 {
            panic!("schema version must be greater than zero");
        }

        if expires_at <= env.ledger().timestamp() {
            panic!("proof expiration must be in the future");
        }

        let protocol_config = Self::get_protocol_config(env.clone());
        let protocol_client = ProtocolConfigContractClient::new(&env, &protocol_config);
        if protocol_client.is_paused() {
            panic!("protocol is paused");
        }

        if !protocol_client.is_schema_version_approved(&schema_version) {
            panic!("schema version is not approved");
        }

        let issuer_registry = Self::get_issuer_registry(env.clone());
        let issuer_client = IssuerRegistryContractClient::new(&env, &issuer_registry);
        if !issuer_client.is_active_address(&issuer_address) {
            panic!("issuer is not active");
        }

        let key = DataKey::Proof(proof_id_hash.clone());
        if env.storage().persistent().has(&key) {
            panic!("proof already registered");
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
    }

    pub fn revoke_proof(env: Env, proof_id_hash: BytesN<32>) {
        Self::set_revoked(env, proof_id_hash, false);
    }

    pub fn admin_revoke_proof(env: Env, proof_id_hash: BytesN<32>) {
        Self::set_revoked(env, proof_id_hash, true);
    }

    pub fn get_proof(env: Env, proof_id_hash: BytesN<32>) -> ProofRecord {
        let key = DataKey::Proof(proof_id_hash);
        let record = env
            .storage()
            .persistent()
            .get(&key)
            .expect("proof not found");
        Self::extend_proof_key_ttl(env, &key);
        record
    }

    pub fn is_valid_proof(env: Env, proof_id_hash: BytesN<32>) -> bool {
        let record = Self::get_proof(env.clone(), proof_id_hash);
        record.status == ProofStatus::Active && env.ledger().timestamp() <= record.expires_at
    }

    pub fn is_revoked(env: Env, proof_id_hash: BytesN<32>) -> bool {
        let record = Self::get_proof(env, proof_id_hash);
        record.status == ProofStatus::Revoked
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn get_issuer_registry(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::IssuerRegistry)
            .expect("issuer registry not configured")
    }

    pub fn get_protocol_config(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::ProtocolConfig)
            .expect("protocol config not configured")
    }

    fn set_revoked(env: Env, proof_id_hash: BytesN<32>, by_admin: bool) {
        let key = DataKey::Proof(proof_id_hash.clone());
        let mut record: ProofRecord = env
            .storage()
            .persistent()
            .get(&key)
            .expect("proof not found");

        if by_admin {
            let admin = Self::get_admin(env.clone());
            Self::require_auth(&admin);
        } else {
            Self::require_auth(&record.issuer_address);
        }

        if record.status == ProofStatus::Revoked {
            panic!("proof already revoked");
        }

        record.status = ProofStatus::Revoked;
        record.revoked_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &record);
        Self::extend_proof_key_ttl(env, &key);
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
        #[cfg(not(test))]
        address.require_auth();

        #[cfg(test)]
        let _ = address;
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
    #[should_panic(expected = "proof expiration must be in the future")]
    fn rejects_expired_proof() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();

        client.register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &1,
            &0,
        );
    }

    #[test]
    #[should_panic(expected = "proof already registered")]
    fn rejects_duplicate_proof_id() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);
        client.register_proof(&proof_id, &bytes(&env, 3), &issuer, &1, &2_000);
    }

    #[test]
    #[should_panic(expected = "schema version is not approved")]
    fn rejects_unapproved_schema_version() {
        let (env, client, _protocol_config, _issuer_registry, _issuer_registry_id) = setup();

        client.register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &2,
            &2_000,
        );
    }

    #[test]
    #[should_panic(expected = "protocol is paused")]
    fn rejects_registration_when_protocol_is_paused() {
        let (env, client, protocol_config, _issuer_registry, _issuer_registry_id) = setup();
        protocol_config.pause();

        client.register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &Address::from_str(&env, ISSUER),
            &1,
            &2_000,
        );
    }

    #[test]
    #[should_panic(expected = "issuer is not active")]
    fn rejects_inactive_issuer_address() {
        let (env, client, _protocol_config, issuer_registry, _issuer_registry_id) = setup();
        let inactive_issuer = Address::from_str(
            &env,
            "GBXHUHG5FGYLPD6RHL2MKWMP572O6KUXCZXDZJXS4T57ZTMAKBN7DWXN",
        );
        issuer_registry.register_issuer(&bytes(&env, 10), &inactive_issuer, &bytes(&env, 11));
        issuer_registry.suspend_issuer(&bytes(&env, 10));

        client.register_proof(
            &bytes(&env, 1),
            &bytes(&env, 2),
            &inactive_issuer,
            &1,
            &2_000,
        );
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
}
