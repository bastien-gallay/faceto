//! FNV-1a content hashing, hand-rolled (zero deps): the short hex tag that keys the
//! recent-model ring and answers `/model-version`.

/// FNV-1a 64-bit, first 12 hex chars — a stable, dependency-free content version token.
pub(crate) fn fnv12(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv12_is_deterministic_and_twelve_hex_chars() {
        // FNV-1a offset basis, for empty input.
        assert_eq!(fnv12(b""), "cbf29ce48422");
        let h = fnv12(b"faceto");
        assert_eq!(h.len(), 12);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(fnv12(b"faceto"), h);
        assert_ne!(fnv12(b"faceto"), fnv12(b"faceto "));
    }
}
