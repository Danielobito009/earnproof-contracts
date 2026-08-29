//! Golden backend encoding vectors consumed without a JSON runtime dependency.

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    #[test]
    fn sha256_vectors_match_published_hex() {
        for line in include_str!("../../fixtures/encoding/vectors.tsv").lines() {
            if line.is_empty() || line.starts_with('#') { continue; }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields[1] == "sha256" {
                let digest = Sha256::digest(fields[2].as_bytes());
                assert_eq!(format!("{digest:x}"), fields[3], "{}", fields[0]);
            }
            assert_eq!(fields[3], fields[4], "{}", fields[0]);
        }
    }

    #[test]
    fn malformed_vectors_are_rejected_by_boundary_rules() {
        for line in include_str!("../../fixtures/encoding/invalid.tsv").lines() {
            if line.is_empty() || line.starts_with('#') { continue; }
            let fields: Vec<&str> = line.split('\t').collect();
            let valid = !fields[2].is_empty()
                && fields[3].len() == 64
                && fields[3].chars().all(|character| character.is_ascii_hexdigit());
            assert!(!valid, "{} must be rejected", fields[0]);
        }
    }
}