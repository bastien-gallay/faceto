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
