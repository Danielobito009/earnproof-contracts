#![no_std]

use earnproof_shared::{ProofRecord, ProofStatus};
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
    }

    pub fn revoke_proof(env: Env, proof_id_hash: BytesN<32>) {
        Self::set_revoked(env, proof_id_hash, false);
    }

    pub fn admin_revoke_proof(env: Env, proof_id_hash: BytesN<32>) {
        Self::set_revoked(env, proof_id_hash, true);
    }

    pub fn get_proof(env: Env, proof_id_hash: BytesN<32>) -> ProofRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Proof(proof_id_hash))
            .expect("proof not found")
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

    use super::{ProofRegistryContract, ProofRegistryContractClient};
    use earnproof_shared::ProofStatus;
    use soroban_sdk::{Address, BytesN, Env};

    const ADMIN: &str = "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR";
    const ISSUER: &str = "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U";
    const ISSUER_REGISTRY: &str = "GDWUSKGGFDI4FRXK5EBTRECZSVQSSWJHHJOGH6JWG3AUMFFMQ435DIAG";
    const PROTOCOL_CONFIG: &str = "GDFJHLAXAUMHA4OWPOB4P7YO72AQR2HMIUYFOXLXE2DZGM633K7HZDQP";

    fn bytes(env: &Env, value: u8) -> BytesN<32> {
        BytesN::from_array(env, &[value; 32])
    }

    fn setup() -> (Env, ProofRegistryContractClient<'static>, Address, Address) {
        let env = Env::default();
        let contract_id = env.register(ProofRegistryContract, ());
        let client = ProofRegistryContractClient::new(&env, &contract_id);
        let admin = Address::from_str(&env, ADMIN);
        let issuer_registry = Address::from_str(&env, ISSUER_REGISTRY);
        let protocol_config = Address::from_str(&env, PROTOCOL_CONFIG);
        client.initialize(&admin, &issuer_registry, &protocol_config);
        (env, client, admin, issuer_registry)
    }

    #[test]
    fn registers_and_validates_proof() {
        let (env, client, _admin, issuer_registry) = setup();
        let proof_id = bytes(&env, 1);
        let commitment = bytes(&env, 2);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &commitment, &issuer, &1, &2_000);

        let record = client.get_proof(&proof_id);
        assert_eq!(record.proof_id_hash, proof_id);
        assert_eq!(record.commitment_hash, commitment);
        assert_eq!(record.issuer_address, issuer);
        assert_eq!(record.status, ProofStatus::Active);
        assert_eq!(client.get_issuer_registry(), issuer_registry);
        assert!(client.is_valid_proof(&proof_id));
        assert!(!client.is_revoked(&proof_id));
    }

    #[test]
    fn issuer_can_revoke_proof() {
        let (env, client, _admin, _issuer_registry) = setup();
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
        let (env, client, _admin, _issuer_registry) = setup();

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
        let (env, client, _admin, _issuer_registry) = setup();
        let proof_id = bytes(&env, 1);
        let issuer = Address::from_str(&env, ISSUER);

        client.register_proof(&proof_id, &bytes(&env, 2), &issuer, &1, &2_000);
        client.register_proof(&proof_id, &bytes(&env, 3), &issuer, &1, &2_000);
    }
}
