# Protocol Configuration Specification

## States and transitions

The contract has an uninitialized state and an initialized state with `(admin, paused, config_version)`. Each schema key is independently `Approved`, `Deprecated`, or `Absent`; version zero is permanently invalid.

| Transition | Guard | Side effects and event | Impossible transition |
|---|---|---|---|
| `initialize` | Admin key absent; supplied admin authenticates | Writes admin, `paused=false`, version 1; emits `Initialized` | Second initialization: `AlreadyInitialized`; unauthenticated admin |
| `set_admin` | Initialized; current admin authenticates | Replaces admin, increments version; emits `AdminChanged` | Old admin after rotation |
| `pause` / `unpause` | Initialized; current admin authenticates | Sets flag, increments version; emits `Paused` / `Unpaused` | Unauthorized mutation |
| `approve_schema_version` | Initialized; admin authenticates; version != 0 | Sets persistent key true, bumps TTL/version; emits `SchemaApproved` | Version zero: `InvalidInput` |
| `deprecate_schema_version` | Initialized; admin authenticates; version != 0 | Sets key false, bumps TTL/version; emits `SchemaDeprecated` | Version zero: `InvalidInput` |

Implementations: `contracts/protocol-config/src/lib.rs::initialize`, `set_admin`, `pause`, `unpause`, `approve_schema_version`, `deprecate_schema_version`, `is_schema_version_approved`.

Invariants are: initialization is one-way; only the current admin mutates state; `config_version` increases on every privileged mutation; a zero schema is never approved; reads do not create absent approval keys. Positive tests: `contracts/protocol-config/src/lib.rs::initializes_config_defaults`, `pause_and_unpause_bump_config_version`, `schema_versions_can_be_approved_and_deprecated`. Negative tests: `rejects_zero_schema_version` and `tests/emergency/src/admin_rotation.rs::pause_authority_follows_rotation_and_does_not_stay_with_the_former_admin`.