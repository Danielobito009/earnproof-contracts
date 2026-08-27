//! Failures at each cross-contract read boundary.
//!
//! The table in [`crate::harness`] numbers the three reads `register_proof`
//! performs. This module walks that table: for each boundary it drives a
//! failure *before* it, *inside* it, and *after* it, and asserts the same thing
//! every time — the invocation is rejected, it publishes nothing, and the
//! complete footprint is exactly what it was beforehand.
//!
//! "Inside" is the case that needs help from [`crate::mocks`]. The real
//! dependencies never fail a read: `is_paused`, `is_schema_version_approved`
//! and `is_active_address` all answer successfully whatever their state and
//! report their verdict in the returned `bool`. A substitute dependency is the
//! only way to make the read itself fail, and it is also what makes the
//! *ordering* of the steps observable: a dependency that rejects every call
//! turns "which check ran first" into an assertion rather than a claim.

use earnproof_shared::{ProofError, ProofRecord, ProofStatus, TTL_THRESHOLD_LEDGERS};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, BytesN, IntoVal};

use crate::harness::{
    assert_unchanged, commitment, hash, outcome_of, proof_key, Deployment, Rejection,
    APPROVED_SCHEMA,
};
use crate::mocks::{
    ConfigRequiringAuth, ConfigRequiringAuthClient, MalformedIssuerRead, MalformedPauseRead,
    MalformedSchemaRead, RecordingConfig, RecordingConfigClient, RejectsIssuerRead,
    RejectsPauseRead, RejectsSchemaRead,
};

// ---------------------------------------------------------------------------
// The reconstructed key this crate measures TTLs through
// ---------------------------------------------------------------------------

#[test]
fn the_reconstructed_proof_key_addresses_the_stored_record() {
    // Every proof TTL assertion in this crate reads through `proof_key`. If the
    // encoding of `proof-registry`'s private `DataKey::Proof(..)` ever changed,
    // the key would address nothing, and those assertions would quietly degrade
    // to comparing `None` with `None`. This test is what stops that.
    let deployment = Deployment::new();
    let proof_id = deployment.register(0x11);

    let key = proof_key(&deployment.env, &proof_id);
    let stored: Option<ProofRecord> = deployment
        .env
        .as_contract(&deployment.proofs.address, || {
            deployment.env.storage().persistent().get(&key)
        });
    let via_getter = deployment.proofs.get_proof(&proof_id);

    assert_eq!(
        stored,
        Some(via_getter),
        "the reconstructed storage key must address the same record the public getter returns"
    );
}

// ---------------------------------------------------------------------------
// Before the first read: authorization
// ---------------------------------------------------------------------------

#[test]
fn missing_authorization_is_rejected_before_any_dependency_is_read() {
    // An empty mock authorises nothing, so `issuer_address.require_auth()` at
    // the top of `register_proof` fails. The substituted dependency rejects
    // every call, so a rejection that is *typed* would mean a read had already
    // happened; `Aborted` here is the authorization failure itself.
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(RejectsPauseRead, ()), issuers)
    });
    deployment.env.mock_auths(&[]);

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x21));

    assert_eq!(
        rejection,
        Rejection::Aborted,
        "an unauthorised registration must be rejected by the host, \
         not resolved into a proof-registry verdict"
    );
}

#[test]
fn nested_authorization_failure_rolls_back_the_registration() {
    // The dependency demands authorization from an address the transaction
    // never authorised. `proof-registry` cannot anticipate that requirement and
    // must not proceed past it.
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        let config = env.register(ConfigRequiringAuth, ());
        ConfigRequiringAuthClient::new(env, &config).set_guardian(&Address::generate(env));
        (config, issuers)
    });
    let proof_id = hash(&deployment.env, 0x22);
    let before = deployment.footprint(&proof_id);

    let rejection = register_with_root_auth_only(&deployment, &proof_id);

    // Events before state: the environment reports only the most recent
    // invocation, and `footprint` invokes the getters.
    let events = deployment.env.events().all().events().len();
    let after = deployment.footprint(&proof_id);

    assert_eq!(
        rejection,
        Rejection::Aborted,
        "an unsatisfied nested authorization must abort the registration"
    );
    assert_eq!(events, 0, "a rejected registration published an event");
    assert_unchanged(&before, &after);
}

#[test]
fn root_authorization_alone_registers_against_dependencies_that_demand_none() {
    // Attributability for the test above. The single mocked authorization entry
    // has to be sufficient on its own, or the rejection there would be
    // explained by a malformed entry rather than by the nested requirement.
    let deployment = Deployment::new();
    let proof_id = hash(&deployment.env, 0x23);

    let rejection = register_with_root_auth_only(&deployment, &proof_id);

    assert_eq!(rejection, Rejection::Accepted);
    assert_eq!(deployment.proofs.get_proof(&proof_id).proof_id_hash, proof_id);
}

/// Attempts a registration carrying exactly one authorization entry: the
/// issuer's, for the root `register_proof` invocation and nothing beneath it.
fn register_with_root_auth_only(deployment: &Deployment, proof_id: &BytesN<32>) -> Rejection {
    let env = &deployment.env;
    let commitment_hash = commitment(env, 0xC0);
    let expires_at = deployment.expiry();
    let args: soroban_sdk::Vec<soroban_sdk::Val> = (
        proof_id.clone(),
        commitment_hash.clone(),
        deployment.issuer.clone(),
        APPROVED_SCHEMA,
        expires_at,
    )
        .into_val(env);

    env.mock_auths(&[MockAuth {
        address: &deployment.issuer,
        invoke: &MockAuthInvoke {
            contract: &deployment.proofs.address,
            fn_name: "register_proof",
            args,
            sub_invokes: &[],
        },
    }]);

    outcome_of(|| {
        deployment.proofs.try_register_proof(
            proof_id,
            &commitment_hash,
            &deployment.issuer,
            &APPROVED_SCHEMA,
            &expires_at,
        )
    })
}

// ---------------------------------------------------------------------------
// Before the first read: argument validation
//
// Both cases run against a dependency that rejects every call, so a typed
// rejection is proof the local check ran first. Had `protocol-config` been
// consulted before the argument was validated, the verdict would be `Aborted`.
// ---------------------------------------------------------------------------

#[test]
fn zero_schema_version_is_rejected_before_the_protocol_config_is_read() {
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(RejectsPauseRead, ()), issuers)
    });

    let rejection = deployment.assert_rejected_and_atomic_with(
        &hash(&deployment.env, 0x24),
        &deployment.issuer,
        0,
        deployment.expiry(),
    );

    assert_eq!(rejection, Rejection::Typed(ProofError::InvalidSchemaVersion));
}

#[test]
fn an_expired_proof_is_rejected_before_the_protocol_config_is_read() {
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(RejectsPauseRead, ()), issuers)
    });
    let now = deployment.env.ledger().timestamp();

    let rejection = deployment.assert_rejected_and_atomic_with(
        &hash(&deployment.env, 0x25),
        &deployment.issuer,
        APPROVED_SCHEMA,
        now,
    );

    assert_eq!(rejection, Rejection::Typed(ProofError::ProofExpired));
}

// ---------------------------------------------------------------------------
// Inside a read: the dependency rejects the call
// ---------------------------------------------------------------------------

#[test]
fn registration_rolls_back_when_the_pause_read_fails() {
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(RejectsPauseRead, ()), issuers)
    });

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x31));

    assert_eq!(
        rejection,
        Rejection::Aborted,
        "a dependency rejection must not be decoded as a proof-registry error"
    );
}

#[test]
fn registration_rolls_back_when_the_schema_read_fails() {
    // Boundary 1 has already succeeded when this one fails, so the rollback has
    // real work to do.
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(RejectsSchemaRead, ()), issuers)
    });

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x32));

    assert_eq!(rejection, Rejection::Aborted);
}

#[test]
fn registration_rolls_back_when_the_issuer_read_fails() {
    // Both `protocol-config` reads have succeeded by this point: the last
    // boundary before the write.
    let deployment = Deployment::with_dependency_addresses(|env, config, _issuers| {
        (config, env.register(RejectsIssuerRead, ()))
    });

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x33));

    assert_eq!(rejection, Rejection::Aborted);
}

// ---------------------------------------------------------------------------
// Inside a read: the dependency answers with the wrong type
//
// The failure lands in the caller's conversion of the return value rather than
// in the callee, which is a distinct path through the host from a rejection.
// ---------------------------------------------------------------------------

#[test]
fn a_malformed_pause_result_rolls_back_the_registration() {
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(MalformedPauseRead, ()), issuers)
    });

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x41));

    assert_eq!(
        rejection,
        Rejection::Aborted,
        "a return value that is not the declared type must fail closed"
    );
}

#[test]
fn a_malformed_schema_result_rolls_back_the_registration() {
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(MalformedSchemaRead, ()), issuers)
    });

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x42));

    assert_eq!(rejection, Rejection::Aborted);
}

#[test]
fn a_malformed_issuer_result_rolls_back_the_registration() {
    let deployment = Deployment::with_dependency_addresses(|env, config, _issuers| {
        (config, env.register(MalformedIssuerRead, ()))
    });

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x43));

    assert_eq!(rejection, Rejection::Aborted);
}

// ---------------------------------------------------------------------------
// After a read: the dependency answered, and its answer says no
//
// These are the rejections integrators actually see in production. Unlike the
// cases above they carry a typed `ProofError`, which is the surface
// `docs/backend-integration.md` tells them to map.
// ---------------------------------------------------------------------------

#[test]
fn a_paused_protocol_is_rejected_with_a_typed_error() {
    let deployment = Deployment::new();
    deployment.config.pause();

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x51));

    // `register_proof` reports the pause as `InvalidSchemaVersion` (code 304),
    // not as `ContractError::ProtocolPaused` (code 80). 304 is what callers
    // observe today and what `contracts/proof-registry` asserts in its own
    // tests, so it is what this test pins. Re-mapping it would be a Semantic
    // change under `docs/compatibility.md` and belongs to a release.
    assert_eq!(rejection, Rejection::Typed(ProofError::InvalidSchemaVersion));
}

#[test]
fn an_unapproved_schema_version_is_rejected_with_a_typed_error() {
    let deployment = Deployment::new();
    deployment.config.deprecate_schema_version(&APPROVED_SCHEMA);

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x52));

    assert_eq!(
        rejection,
        Rejection::Typed(ProofError::SchemaVersionNotApproved)
    );
}

#[test]
fn an_inactive_issuer_is_rejected_with_a_typed_error() {
    let deployment = Deployment::new();
    deployment.issuers.suspend_issuer(&deployment.issuer_id);

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x53));

    // As with the pause, the inactive issuer is reported as code 304 rather
    // than `IssuerError::IssuerInactive` (code 205).
    assert_eq!(rejection, Rejection::Typed(ProofError::InvalidSchemaVersion));
}

#[test]
fn a_duplicate_registration_is_rejected_after_every_dependency_read() {
    // The last rejection point, past all three boundaries. The footprint check
    // inside `assert_rejected_and_atomic` is what matters here: the attempt
    // carries a different commitment hash, so an overwrite would be visible.
    let deployment = Deployment::new();
    let proof_id = deployment.register(0x61);

    let rejection = deployment.assert_rejected_and_atomic(&proof_id);

    assert_eq!(
        rejection,
        Rejection::Typed(ProofError::ProofAlreadyRegistered)
    );
    assert_eq!(
        deployment.proofs.get_proof(&proof_id).status,
        ProofStatus::Active
    );
}

// ---------------------------------------------------------------------------
// The rollback reaches the callee
// ---------------------------------------------------------------------------

#[test]
fn a_dependency_write_is_rolled_back_when_the_registration_fails() {
    // `proof-registry`'s own footprint cannot see inside a dependency. This is
    // the assertion that the invocation is atomic across the whole call tree
    // and not merely within the caller.
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(RecordingConfig, ()), issuers)
    });
    let recorder =
        RecordingConfigClient::new(&deployment.env, &deployment.proofs.get_protocol_config());
    // Fail at boundary 3, after the recording read has already written.
    deployment.issuers.suspend_issuer(&deployment.issuer_id);

    let rejection = deployment.assert_rejected_and_atomic(&hash(&deployment.env, 0x71));

    assert_eq!(rejection, Rejection::Typed(ProofError::InvalidSchemaVersion));
    assert!(
        !recorder.was_touched(),
        "a dependency's own write survived a rejected registration; \
         the rollback does not reach the callee"
    );
}

#[test]
fn a_dependency_write_survives_a_committed_registration() {
    // Attributability for the test above: the write has to actually happen when
    // the registration commits, or its absence there proves nothing.
    let deployment = Deployment::with_dependency_addresses(|env, _config, issuers| {
        (env.register(RecordingConfig, ()), issuers)
    });
    let recorder =
        RecordingConfigClient::new(&deployment.env, &deployment.proofs.get_protocol_config());

    assert!(!recorder.was_touched());
    deployment.register(0x72);

    assert!(
        recorder.was_touched(),
        "the dependency read must write when the registration commits"
    );
}

// ---------------------------------------------------------------------------
// TTL extensions performed mid-invocation are rolled back too
// ---------------------------------------------------------------------------

/// Ledger sequence at which `SchemaVersion(APPROVED_SCHEMA)`'s remaining TTL
/// has fallen below `TTL_THRESHOLD_LEDGERS`.
///
/// `initialize` extended the entry to `TTL_EXTEND_TO_LEDGERS` (500,000). At
/// sequence 460,000 it has 40,000 left, which is under the 50,000 threshold, so
/// the next `is_schema_version_approved` re-extends it. Without advancing the
/// sequence the entry is already at the target and a rolled-back extension
/// would be indistinguishable from no extension at all.
const SEQUENCE_NEAR_SCHEMA_EXPIRY: u32 = 460_000;

#[test]
fn a_failed_registration_does_not_extend_the_schema_version_ttl() {
    let deployment = Deployment::new();
    deployment
        .env
        .ledger()
        .set_sequence_number(SEQUENCE_NEAR_SCHEMA_EXPIRY);
    // Fail at boundary 3 — after `is_schema_version_approved` has run and
    // extended the entry as a side effect.
    deployment.issuers.suspend_issuer(&deployment.issuer_id);

    let proof_id = hash(&deployment.env, 0x81);
    let ttl_before = deployment
        .footprint(&proof_id)
        .schema_ttl
        .expect("the approved schema entry exists");
    assert!(
        ttl_before < TTL_THRESHOLD_LEDGERS,
        "the fixture must leave the schema entry below the extension threshold, \
         or this test cannot observe an extension at all"
    );

    // `assert_rejected_and_atomic` compares the schema entry's TTL across the
    // attempt; the guard above is what gives that comparison teeth.
    let rejection = deployment.assert_rejected_and_atomic(&proof_id);

    assert_eq!(rejection, Rejection::Typed(ProofError::InvalidSchemaVersion));
}

#[test]
fn a_committed_registration_does_extend_the_schema_version_ttl() {
    // Attributability for the test above.
    let deployment = Deployment::new();
    deployment
        .env
        .ledger()
        .set_sequence_number(SEQUENCE_NEAR_SCHEMA_EXPIRY);

    let proof_id = hash(&deployment.env, 0x82);
    let before = deployment.footprint(&proof_id);
    deployment.register(0x82);
    let after = deployment.footprint(&proof_id);

    assert!(
        after.schema_ttl > before.schema_ttl,
        "a committed registration must extend the schema entry, \
         otherwise the rollback assertion proves nothing"
    );
}
