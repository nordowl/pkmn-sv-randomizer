mod archive;
mod flat;
mod index;

pub use archive::{Archive, ArchiveError, ArchiveOpenError};
pub use index::{parse_index, remove_from_index, IndexEntry, IndexError, ParsedIndex};

// Trinity's basis, which differs from the standard FNV-1a 64 basis (...2325).
const FNV_BASIS: u64 = 0xCBF2_9CE4_8422_2645;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

pub fn path_hash(path: &str) -> u64 {
    path.bytes().fold(FNV_BASIS, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_hash_matches_the_python_probe() {
        assert_eq!(path_hash(""), FNV_BASIS);
        assert_eq!(path_hash("a"), 0xAF66_BC4C_8606_CF2C);
        assert_eq!(
            path_hash("world/data/trainer/trdata/trdata_array.bin"),
            0xC9BA_95BB_E9B7_6E05
        );
    }
}
