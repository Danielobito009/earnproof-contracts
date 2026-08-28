//! Issuer Registry Contract Invocation Examples
//!
//! This module contains executable documentation examples demonstrating
//! how to invoke the IssuerRegistry contract methods. All examples use
//! synthetic identifiers and run in a local sandbox environment.
//!
//! # Examples
//!
//! ## Initialization
//!
//! Initialize the issuer registry contract with an admin address:
//!
//! ```no_run
//! # extern crate std;
//! use soroban_sdk::{Address, BytesN, Env};
//! use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
//!
//! # #[test]
//! fn example_initialize_issuer_registry() {
//!     let env = Env::default();
//!     env.mock_all_auths();
//!
//!     let contract_id = env.register(IssuerRegistryContract, ());
//!     let client = IssuerRegistryContractClient::new(&env, &contract_id);
//!
//!     // Use a synthetic admin address for "sandbox" network
//!     let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR");
//!
//!     // Initialize the issuer registry
//!     client.initialize(&admin);
//!
//!     // Verify admin is set
//!     assert_eq!(client.get_admin(), admin);
//! }
//! ```
//!
//! ## Register Issuer
//!
//! Register an issuer with a unique ID hash and address:
//!
//! ```no_run
//! # extern crate std;
//! use soroban_sdk::{Address, BytesN, Env};
//! use earnproof_shared::IssuerStatus;
//! use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
//!
//! # #[test]
//! fn example_register_issuer() {
//!     let env = Env::default();
//!     env.mock_all_auths();
//!
//!     let contract_id = env.register(IssuerRegistryContract, ());
//!     let client = IssuerRegistryContractClient::new(&env, &contract_id);
//!
//!     let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR");
//!     client.initialize(&admin);
//!
//!     // Create synthetic issuer identifiers
//!     let issuer_id_hash = BytesN::from_array(&env, &[1u8; 32]); // sha256("test-issuer-123")
//!     let issuer_address = Address::from_str(&env, "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U");
//!     let metadata_hash = BytesN::from_array(&env, &[2u8; 32]); // sha256(issuer_metadata_json)
//!
//!     // Register the issuer
//!     client.register_issuer(&issuer_id_hash, &issuer_address, &metadata_hash);
//!
//!     // Verify issuer was registered with Active status
//!     let record = client.get_issuer(&issuer_id_hash);
//!     assert_eq!(record.issuer_id_hash, issuer_id_hash);
//!     assert_eq!(record.issuer_address, issuer_address);
//!     assert_eq!(record.metadata_hash, metadata_hash);
//!     assert_eq!(record.status, IssuerStatus::Active);
//!
//!     // Issuer can be looked up by address
//!     assert!(client.is_active_address(&issuer_address));
//! }
//! ```
//!
//! ## Issuer Lifecycle: Suspend and Reactivate
//!
//! Suspend an issuer and later reactivate it:
//!
//! ```no_run
//! # extern crate std;
//! use soroban_sdk::{Address, BytesN, Env};
//! use earnproof_shared::IssuerStatus;
//! use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
//!
//! # #[test]
//! fn example_suspend_and_reactivate() {
//!     let env = Env::default();
//!     env.mock_all_auths();
//!
//!     let contract_id = env.register(IssuerRegistryContract, ());
//!     let client = IssuerRegistryContractClient::new(&env, &contract_id);
//!
//!     let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR");
//!     client.initialize(&admin);
//!
//!     let issuer_id_hash = BytesN::from_array(&env, &[1u8; 32]);
//!     let issuer_address = Address::from_str(&env, "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U");
//!     let metadata_hash = BytesN::from_array(&env, &[2u8; 32]);
//!
//!     client.register_issuer(&issuer_id_hash, &issuer_address, &metadata_hash);
//!     assert!(client.is_active_issuer(&issuer_id_hash));
//!
//!     // Suspend the issuer
//!     client.suspend_issuer(&issuer_id_hash);
//!     assert!(!client.is_active_issuer(&issuer_id_hash));
//!     let record = client.get_issuer(&issuer_id_hash);
//!     assert_eq!(record.status, IssuerStatus::Suspended);
//!
//!     // Reactivate the issuer
//!     client.reactivate_issuer(&issuer_id_hash);
//!     assert!(client.is_active_issuer(&issuer_id_hash));
//!     let record = client.get_issuer(&issuer_id_hash);
//!     assert_eq!(record.status, IssuerStatus::Active);
//! }
//! ```
//!
//! ## Issuer Lifecycle: Revoke (Terminal)
//!
//! Revoke an issuer (terminal state - cannot be reactivated):
//!
//! ```no_run
//! # extern crate std;
//! use soroban_sdk::{Address, BytesN, Env};
//! use earnproof_shared::IssuerStatus;
//! use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
//!
//! # #[test]
//! fn example_revoke_issuer() {
//!     let env = Env::default();
//!     env.mock_all_auths();
//!
//!     let contract_id = env.register(IssuerRegistryContract, ());
//!     let client = IssuerRegistryContractClient::new(&env, &contract_id);
//!
//!     let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR");
//!     client.initialize(&admin);
//!
//!     let issuer_id_hash = BytesN::from_array(&env, &[1u8; 32]);
//!     let issuer_address = Address::from_str(&env, "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U");
//!     let metadata_hash = BytesN::from_array(&env, &[2u8; 32]);
//!
//!     client.register_issuer(&issuer_id_hash, &issuer_address, &metadata_hash);
//!
//!     // Revoke the issuer (terminal state)
//!     client.revoke_issuer(&issuer_id_hash);
//!     let record = client.get_issuer(&issuer_id_hash);
//!     assert_eq!(record.status, IssuerStatus::Revoked);
//!     assert!(!client.is_active_issuer(&issuer_id_hash));
//! }
//! ```
//!
//! ## Update Issuer Metadata
//!
//! Update an issuer's metadata hash:
//!
//! ```no_run
//! # extern crate std;
//! use soroban_sdk::{Address, BytesN, Env};
//! use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
//!
//! # #[test]
//! fn example_update_issuer_metadata() {
//!     let env = Env::default();
//!     env.mock_all_auths();
//!
//!     let contract_id = env.register(IssuerRegistryContract, ());
//!     let client = IssuerRegistryContractClient::new(&env, &contract_id);
//!
//!     let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR");
//!     client.initialize(&admin);
//!
//!     let issuer_id_hash = BytesN::from_array(&env, &[1u8; 32]);
//!     let issuer_address = Address::from_str(&env, "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U");
//!     let metadata_hash_v1 = BytesN::from_array(&env, &[2u8; 32]);
//!
//!     client.register_issuer(&issuer_id_hash, &issuer_address, &metadata_hash_v1);
//!
//!     // Update to new metadata
//!     let metadata_hash_v2 = BytesN::from_array(&env, &[3u8; 32]);
//!     client.update_issuer(&issuer_id_hash, &metadata_hash_v2);
//!
//!     let record = client.get_issuer(&issuer_id_hash);
//!     assert_eq!(record.metadata_hash, metadata_hash_v2);
//! }
//! ```
//!
//! ## Rotate Issuer Address
//!
//! Rotate an issuer to a new signing address:
//!
//! ```no_run
//! # extern crate std;
//! use soroban_sdk::{Address, BytesN, Env};
//! use issuer_registry::{IssuerRegistryContract, IssuerRegistryContractClient};
//!
//! # #[test]
//! fn example_rotate_issuer_address() {
//!     let env = Env::default();
//!     env.mock_all_auths();
//!
//!     let contract_id = env.register(IssuerRegistryContract, ());
//!     let client = IssuerRegistryContractClient::new(&env, &contract_id);
//!
//!     let admin = Address::from_str(&env, "GCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR");
//!     client.initialize(&admin);
//!
//!     let issuer_id_hash = BytesN::from_array(&env, &[1u8; 32]);
//!     let issuer_address_old = Address::from_str(&env, "GCATS5YOVB6ROX2WUNKGNQ2MP3GMXDMKSG2O4N5CLX3A6W4PZGZZI55U");
//!     let metadata_hash = BytesN::from_array(&env, &[2u8; 32]);
//!
//!     client.register_issuer(&issuer_id_hash, &issuer_address_old, &metadata_hash);
//!
//!     // Rotate to a new address
//!     let issuer_address_new = Address::from_str(&env, "GDWUSKGGFDI4FRXK5EBTRECZSVQSSWJHHJOGH6JWG3AUMFFMQ435DIAG");
//!     client.rotate_issuer_address(&issuer_id_hash, &issuer_address_new);
//!
//!     let record = client.get_issuer(&issuer_id_hash);
//!     assert_eq!(record.issuer_address, issuer_address_new);
//!     assert!(client.is_active_address(&issuer_address_new));
//! }
//! ```
