# Backend Encoding Vectors

The canonical fixture is [vectors.json](../tests/fixtures/encoding/vectors.json); the tab-separated form is consumed by the Rust test at `tests/encoding/src/lib.rs::sha256_vectors_match_published_hex`.

## Rules

- Text is encoded as UTF-8, with no normalization, trimming, newline, prefix removal, or case folding. Unicode is therefore hashed by its UTF-8 bytes. Empty text and malformed UTF-8 are rejected by backend validation; the contracts accept only already-sized `BytesN<32>` values.
- `proof_id_hash`, `commitment_hash`, `issuer_id_hash`, and `metadata_hash` are SHA-256 digests represented as exactly 32 bytes. The hex form is lowercase for transport only.
- `schema_version` is an unsigned 32-bit integer encoded big-endian when serialized outside Soroban. `expiration` is an unsigned 64-bit ledger timestamp, also big-endian. No signed, little-endian, truncated, or overflowing value is valid.
- `BytesN<32>` is the digest bytes, not the ASCII bytes of its hexadecimal display.

The fixtures contain synthetic values only. They must never be updated with wallets, credentials, secrets, deployment identifiers, income, or payment history. Run `cargo test -p encoding-vector-tests` after changing them and update the independent TypeScript example in `tests/fixtures/encoding/example.ts`.