# Security Policy

EarnProof contracts affect public proof status and issuer trust state. Please report vulnerabilities privately before opening a public issue.

## Reporting a Vulnerability

Email the maintainers at `security@veridatum.dev` with:

- affected contract and commit;
- vulnerability description;
- reproduction steps;
- expected impact;
- suggested remediation, if known.

Do not include private keys, seed phrases, or real payment records in reports.

## Supported Scope

The current project targets Stellar testnet only. Mainnet deployment and production security claims are out of scope until explicitly documented and reviewed.

## For Security Reviewers

Two documents are maintained for external review and are the intended starting point:

- [`docs/threat-model.md`](docs/threat-model.md) — what is being protected, the trust boundaries, the assumptions the contracts rely on but do not enforce, and the threats each control addresses.
- [`docs/security-review/README.md`](docs/security-review/README.md) — a commit-specific evidence index mapping assets, entry points, privileges, invariants, errors, events, storage, TTL, and cross-contract calls to exact repository paths, with an explicit status on every claim.

The index distinguishes an implemented control from a tested one, and records accepted risks and open gaps rather than omitting them. Each open gap links to a tracking issue. Both documents carry a refresh checklist and are commit-stamped; if the stamped commit does not match the tree you are reviewing, request a refresh before starting.

Known limitations are stated in both documents. The most significant at present: authorization is implemented on every privileged entry point but is not proven by any test, because the suite runs under `mock_all_auths` ([#34](https://github.com/veridatum-labs/earnproof-contracts/issues/34)), and deployed WASM hashes are not independently reproducible from source ([#17](https://github.com/veridatum-labs/earnproof-contracts/issues/17)).

