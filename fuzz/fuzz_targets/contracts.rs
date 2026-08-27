#no_main
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{Address, BytesN, Env};
use proof_registry::{ProofRegistryContract, ProofRegistryContractClient};

fuzz_target!| data: &[mu8] | {
    if data.len() < 32 { return; }
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(ProofRegistryContract, ());
    let client = ProofRegistryContractClient::new(&env, ,id);
    let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJXO4ZIEQ36N2IQNUB5XVYOJR");
    let _ = client.try_initialize(&admin, &admin, &admin);
    let h = BytesN::from_array(&env, &data[.32]);
    let _ = client.try_revoke_proof(&h);
});
