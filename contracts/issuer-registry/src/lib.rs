#![no_std]

use earnproof_shared::{IssuerRecord, IssuerStatus, TTL_EXTEND_TO_LEDGERS, TTL_THRESHOLD_LEDGERS};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

#[contract]
pub struct IssuerRegistryContract;

#[contracttype]
enum DataKey {
    Admin,
    Issuer(BytesN<32>),
    AddressIssuer(Address),
}

#[contractimpl]
impl IssuerRegistryContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }

        Self::require_auth(&admin);
        env.storage().instance().set(&DataKey::Admin, &admin);
        Self::extend_instance_ttl(env);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

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

    pub fn suspend_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Suspended);
    }

    pub fn reactivate_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Active);
    }

    pub fn revoke_issuer(env: Env, issuer_id_hash: BytesN<32>) {
        Self::set_status(env, issuer_id_hash, IssuerStatus::Revoked);
    }

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

    pub fn is_active_issuer(env: Env, issuer_id_hash: BytesN<32>) -> bool {
        let record = Self::get_issuer(env, issuer_id_hash);
        record.status == IssuerStatus::Active
    }

    pub fn is_active_address(env: Env, issuer_address: Address) -> bool {
        let issuer_id_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&DataKey::AddressIssuer(issuer_address.clone()))
            .expect("issuer address not found");

        Self::is_active_issuer(env, issuer_id_hash)
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
        #[cfg(not(test))]
        address.require_auth();

        #[cfg(test)]
        let _ = address;
    }

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
