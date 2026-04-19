//! Vertex-buffer checksum used as a secondary mesh identifier.
//!
//! Mods are primarily matched to in-game draws by `(prim_count, vert_count)`.
//! When two distinct meshes share those counts, the mod replaces both. This
//! module provides a CRC32 hash over the entire bytes of a vertex buffer,
//! which mods can optionally declare (`VBChecksum: 0xXXXX`) to disambiguate.
//!
//! The algorithm name is surfaced to managed code / mod yaml as the literal
//! `crc32-full` so that future algorithm changes can be detected by tooling.

/// String tag for the hashing algorithm emitted into snapshot metadata.
/// Update this if the algorithm ever changes.
pub const ALGORITHM_NAME: &str = "crc32-full";

/// Compute a CRC32 of all bytes in `data`.
///
/// Returns `0` for an empty slice (which is also the reserved "unknown"
/// value in the bound-VB lookup path). Callers that must distinguish a
/// genuine zero from "not hashed" should track that separately.
pub fn compute(data: &[u8]) -> u32 {
    let mut h = crc32fast::Hasher::new();
    h.update(data);
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(compute(&[]), 0);
    }

    #[test]
    fn deterministic() {
        let data = b"the quick brown fox jumps over the lazy dog";
        let a = compute(data);
        let b = compute(data);
        assert_eq!(a, b);
    }

    #[test]
    fn single_bit_flip_changes_hash() {
        let mut data = vec![0u8; 64];
        for (i, b) in data.iter_mut().enumerate() {
            *b = i as u8;
        }
        let base = compute(&data);
        data[17] ^= 0x01;
        let flipped = compute(&data);
        assert_ne!(base, flipped, "single bit flip must change hash");
    }

    #[test]
    fn differs_by_content() {
        assert_ne!(compute(b"abc"), compute(b"abd"));
    }

    #[test]
    fn algorithm_name_is_crc32_full() {
        assert_eq!(ALGORITHM_NAME, "crc32-full");
    }
}
