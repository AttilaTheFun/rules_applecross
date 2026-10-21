//! CoreFoundation's string hash, from the published CoreFoundation sources
//! (`__CFStrHashCharacters` in CFString.c).
//!
//! Writers whose output follows a hash table's iteration order (binary
//! property lists, keyed archives) need it to place keys where Apple's do.

/// Strings up to this many UTF-16 units hash every unit; longer ones hash the
/// first, middle and last 32.
const HASH_EVERYTHING_LIMIT: usize = 96;

/// `CFHash` of a string, for 64-bit CoreFoundation.
pub fn string_hash(s: &str) -> u64 {
    let units: Vec<u16> = s.encode_utf16().collect();
    hash_units(&units)
}

fn four(result: u64, u: &[u16]) -> u64 {
    result
        .wrapping_mul(67_503_105)
        .wrapping_add((u[0] as u64).wrapping_mul(16_974_593))
        .wrapping_add((u[1] as u64).wrapping_mul(66_049))
        .wrapping_add((u[2] as u64).wrapping_mul(257))
        .wrapping_add(u[3] as u64)
}

pub fn hash_units(u: &[u16]) -> u64 {
    let len = u.len();
    let mut result = len as u64;
    if len <= HASH_EVERYTHING_LIMIT {
        let end4 = len & !3;
        let mut i = 0;
        while i < end4 {
            result = four(result, &u[i..i + 4]);
            i += 4;
        }
        while i < len {
            result = result.wrapping_mul(257).wrapping_add(u[i] as u64);
            i += 1;
        }
    } else {
        for start in [0, (len >> 1) - 16, len - 32] {
            let mut i = start;
            while i < start + 32 {
                result = four(result, &u[i..i + 4]);
                i += 4;
            }
        }
    }
    result.wrapping_add(result << (len & 31))
}
