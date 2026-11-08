use std::collections::{HashMap, VecDeque};
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::flat::{Fb, FbError};
use super::index::{parse_index, IndexError};
use super::path_hash;
use crate::container::{le_u64, Storage};

const ONEPACK_MAGIC: &[u8; 8] = b"ONEPACK\0";
const COMPRESSION_NONE: i8 = -1;
const COMPRESSION_OODLE_KRAKEN: i8 = 3;
const MAX_UNCOMPRESSED_FILE: u64 = 1 << 32;
const PACK_CACHE_BYTES: usize = 128 << 20;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveOpenError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error("{0}")]
    Malformed(String),
}

impl From<FbError> for ArchiveOpenError {
    fn from(error: FbError) -> Self {
        ArchiveOpenError::Malformed(error.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("The game archive has no file \"{path}\".")]
    NotFound { path: String },
    #[error("Could not read \"{path}\" from the game archive: {source}")]
    Io { path: String, source: io::Error },
    #[error("\"{path}\" uses compression type {kind}, which is not supported.")]
    UnsupportedCompression { path: String, kind: i8 },
    #[error("\"{path}\" is corrupt in the game archive: {detail}")]
    Corrupt { path: String, detail: String },
}

#[derive(Clone, Copy)]
struct PackExtent {
    start: u64,
    len: u64,
}

pub struct Archive {
    trpfs: Arc<dyn Storage>,
    pack_of_file: HashMap<u64, usize>,
    packs: Vec<PackExtent>,
    cache: Mutex<PackCache>,
}

impl Archive {
    pub fn open(trpfd: &[u8], trpfs: Arc<dyn Storage>) -> Result<Archive, ArchiveOpenError> {
        let index = parse_index(trpfd)?;
        let packs = read_pack_extents(&*trpfs, &index.pack_paths)?;

        let mut pack_of_file = HashMap::with_capacity(index.entries.len());
        for entry in &index.entries {
            let pack = usize::try_from(entry.pack)
                .ok()
                .filter(|&pack| pack < packs.len())
                .ok_or_else(|| {
                    ArchiveOpenError::Malformed(format!(
                        "file {:016X} points at pack {}, but there are only {} packs",
                        entry.hash,
                        entry.pack,
                        packs.len()
                    ))
                })?;
            pack_of_file.insert(entry.hash, pack);
        }

        Ok(Archive {
            trpfs,
            pack_of_file,
            packs,
            cache: Mutex::new(PackCache::new(PACK_CACHE_BYTES)),
        })
    }

    pub fn contains(&self, path: &str) -> bool {
        self.pack_of_file.contains_key(&path_hash(path))
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, ArchiveError> {
        self.read_hashed(path_hash(path), path)
    }

    /// Reads a file known only by its path hash, as the index lists it.
    pub fn read_hash(&self, hash: u64) -> Result<Vec<u8>, ArchiveError> {
        self.read_hashed(hash, &format!("{hash:016X}"))
    }

    fn read_hashed(&self, hash: u64, path: &str) -> Result<Vec<u8>, ArchiveError> {
        let pack = *self
            .pack_of_file
            .get(&hash)
            .ok_or_else(|| ArchiveError::NotFound {
                path: path.to_owned(),
            })?;
        let bytes = self.pack_bytes(pack).map_err(|source| ArchiveError::Io {
            path: path.to_owned(),
            source,
        })?;
        extract_file(&bytes, hash, path)
    }

    pub fn file_count(&self) -> usize {
        self.pack_of_file.len()
    }

    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }

    /// Each pack's byte range in `data.trpfs`, in index order.
    pub fn pack_extents(&self) -> Vec<(u64, u64)> {
        self.packs.iter().map(|p| (p.start, p.len)).collect()
    }

    /// The lock is released while decrypting, so readers of different packs run in parallel.
    /// Two readers missing the same pack may both decrypt it, which only wastes time.
    fn pack_bytes(&self, pack: usize) -> io::Result<Arc<Vec<u8>>> {
        if let Some(bytes) = self.lock_cache().get(pack) {
            return Ok(bytes);
        }
        let extent = self.packs[pack];
        let bytes = Arc::new(self.trpfs.read_vec(extent.start, extent.len)?);
        self.lock_cache().insert(pack, bytes.clone());
        Ok(bytes)
    }

    fn lock_cache(&self) -> MutexGuard<'_, PackCache> {
        // The cache only ever holds complete entries, so it stays valid after a panic elsewhere.
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn read_pack_extents(
    trpfs: &dyn Storage,
    pack_paths: &[String],
) -> Result<Vec<PackExtent>, ArchiveOpenError> {
    let head = trpfs.read_vec(0, 0x10)?;
    if head[..8] != ONEPACK_MAGIC[..] {
        return Err(ArchiveOpenError::Malformed(
            "the pack file has no ONEPACK header".to_owned(),
        ));
    }
    let metadata_offset = le_u64(&head, 8)?;
    let metadata_size = trpfs
        .len()
        .checked_sub(metadata_offset)
        .ok_or_else(|| ArchiveOpenError::Malformed("pack metadata is out of bounds".to_owned()))?;
    let metadata = trpfs.read_vec(metadata_offset, metadata_size)?;

    let fb = Fb::new(&metadata);
    let root = fb.root()?;
    let hashes = fb.u64s(fb.u64_vector(root, 0)?);
    let offsets: Vec<u64> = fb.u64s(fb.u64_vector(root, 1)?).collect();
    let start_of_pack: HashMap<u64, u64> = hashes.zip(offsets.iter().copied()).collect();

    let mut boundaries = offsets;
    boundaries.push(metadata_offset);
    boundaries.sort_unstable();
    boundaries.dedup();

    pack_paths
        .iter()
        .map(|path| {
            let malformed = |what: &str| ArchiveOpenError::Malformed(format!("pack \"{path}\" {what}"));
            let start = *start_of_pack
                .get(&path_hash(path))
                .ok_or_else(|| malformed("is missing from the pack file"))?;
            let end = boundaries
                .get(boundaries.partition_point(|&offset| offset <= start))
                .ok_or_else(|| malformed("starts after the last pack"))?;
            Ok(PackExtent {
                start,
                len: end - start,
            })
        })
        .collect()
}

struct PackEntry<'a> {
    compression: i8,
    uncompressed_size: u64,
    data: &'a [u8],
}

fn find_pack_entry(pack: &[u8], hash: u64) -> Result<Option<PackEntry<'_>>, FbError> {
    let fb = Fb::new(pack);
    let root = fb.root()?;
    let Some(position) = fb.u64s(fb.u64_vector(root, 0)?).position(|h| h == hash) else {
        return Ok(None);
    };
    let tables = fb.offsets(fb.offset_vector(root, 1)?)?;
    let Some(&table) = tables.get(position) else {
        return Ok(None);
    };
    Ok(Some(PackEntry {
        compression: fb.i8_field(table, 1, COMPRESSION_NONE)?,
        uncompressed_size: fb.u64_field(table, 3, 0)?,
        data: fb.bytes_field(table, 4)?,
    }))
}

fn extract_file(pack: &[u8], hash: u64, path: &str) -> Result<Vec<u8>, ArchiveError> {
    let corrupt = |detail: String| ArchiveError::Corrupt {
        path: path.to_owned(),
        detail,
    };
    let entry = find_pack_entry(pack, hash)
        .map_err(|error| corrupt(error.to_string()))?
        .ok_or_else(|| corrupt("the pack named by the index does not contain it".to_owned()))?;
    match entry.compression {
        COMPRESSION_NONE => Ok(entry.data.to_vec()),
        COMPRESSION_OODLE_KRAKEN => {
            decompress_kraken(entry.data, entry.uncompressed_size).map_err(corrupt)
        }
        kind => Err(ArchiveError::UnsupportedCompression {
            path: path.to_owned(),
            kind,
        }),
    }
}

fn decompress_kraken(data: &[u8], uncompressed_size: u64) -> Result<Vec<u8>, String> {
    if uncompressed_size > MAX_UNCOMPRESSED_FILE {
        return Err(format!("implausible size {uncompressed_size:#x}"));
    }
    let mut out = vec![0; uncompressed_size as usize];
    // oozextract panics on some malformed streams instead of returning an error.
    let decoded = panic::catch_unwind(AssertUnwindSafe(|| {
        oozextract::Extractor::new().read_from_slice(data, &mut out)
    }))
    .map_err(|_| "the Oodle Kraken stream is malformed".to_owned())?;
    let written =
        decoded.map_err(|error| format!("Oodle Kraken decompression failed: {error}"))?;
    if written != out.len() {
        return Err(format!(
            "decompressed to {written} bytes, expected {}",
            out.len()
        ));
    }
    Ok(out)
}

struct PackCache {
    entries: VecDeque<(usize, Arc<Vec<u8>>)>,
    bytes: usize,
    capacity: usize,
}

impl PackCache {
    fn new(capacity: usize) -> PackCache {
        PackCache {
            entries: VecDeque::new(),
            bytes: 0,
            capacity,
        }
    }

    fn get(&mut self, pack: usize) -> Option<Arc<Vec<u8>>> {
        let position = self.entries.iter().position(|(id, _)| *id == pack)?;
        let entry = self.entries.remove(position)?;
        let bytes = entry.1.clone();
        self.entries.push_back(entry);
        Some(bytes)
    }

    fn insert(&mut self, pack: usize, bytes: Arc<Vec<u8>>) {
        if bytes.len() > self.capacity || self.entries.iter().any(|(id, _)| *id == pack) {
            return;
        }
        while self.bytes + bytes.len() > self.capacity {
            match self.entries.pop_front() {
                Some((_, evicted)) => self.bytes -= evicted.len(),
                None => break,
            }
        }
        self.bytes += bytes.len();
        self.entries.push_back((pack, bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::super::flat::build::{buffer, Field, Node};
    use super::super::index::test_data::index_bytes;
    use super::*;

    fn pack(files: &[(&str, i8, u64, &[u8])]) -> Vec<u8> {
        buffer(&Node::Table(vec![
            Field::Ref(Node::U64s(files.iter().map(|f| path_hash(f.0)).collect())),
            Field::Ref(Node::Refs(
                files
                    .iter()
                    .map(|f| {
                        Node::Table(vec![
                            Field::Absent,
                            Field::I8(f.1),
                            Field::Absent,
                            Field::U64(f.2),
                            Field::Ref(Node::Bytes(f.3.to_vec())),
                        ])
                    })
                    .collect(),
            )),
        ]))
    }

    /// Lays out a `.trpfs`: magic, metadata offset, the packs, then the metadata flatbuffer.
    fn trpfs(packs: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut out = vec![0; 0x10];
        let mut offsets = Vec::new();
        for (_, bytes) in packs {
            offsets.push(out.len() as u64);
            out.extend(bytes);
        }
        let metadata_offset = out.len() as u64;
        out.extend(buffer(&Node::Table(vec![
            Field::Ref(Node::U64s(packs.iter().map(|p| path_hash(p.0)).collect())),
            Field::Ref(Node::U64s(offsets)),
        ])));
        out[..8].copy_from_slice(ONEPACK_MAGIC);
        out[8..16].copy_from_slice(&metadata_offset.to_le_bytes());
        out
    }

    fn archive() -> Archive {
        let first = pack(&[
            ("raw.bin", COMPRESSION_NONE, 3, b"abc"),
            ("zlib.bin", 1, 3, b"abc"),
            ("garbage.bin", COMPRESSION_OODLE_KRAKEN, 64, &[0xFF; 32]),
            ("huge.bin", COMPRESSION_OODLE_KRAKEN, u64::MAX, b"abc"),
        ]);
        let second = pack(&[("other.bin", COMPRESSION_NONE, 2, b"xy")]);
        let files = [
            (path_hash("raw.bin"), 0),
            (path_hash("zlib.bin"), 0),
            (path_hash("garbage.bin"), 0),
            (path_hash("huge.bin"), 0),
            (path_hash("indexed-not-packed.bin"), 1),
            (path_hash("other.bin"), 1),
        ];
        let index = index_bytes(&files, &["pack/a", "pack/b"]);
        let trpfs = trpfs(&[("pack/a", first), ("pack/b", second)]);
        Archive::open(&index, Arc::new(trpfs)).unwrap()
    }

    #[test]
    fn reads_raw_files_across_packs() {
        let archive = archive();
        assert_eq!(archive.file_count(), 6);
        assert_eq!(archive.pack_count(), 2);
        assert_eq!(archive.read("raw.bin").unwrap(), b"abc");
        assert_eq!(archive.read("other.bin").unwrap(), b"xy");
        assert_eq!(archive.read("raw.bin").unwrap(), b"abc");
    }

    #[test]
    fn errors_are_clean_and_specific() {
        let archive = archive();
        assert!(!archive.contains("missing.bin"));
        assert!(matches!(archive.read("missing.bin"), Err(ArchiveError::NotFound { .. })));
        assert!(matches!(
            archive.read("zlib.bin"),
            Err(ArchiveError::UnsupportedCompression { kind: 1, .. })
        ));
        assert!(matches!(archive.read("huge.bin"), Err(ArchiveError::Corrupt { .. })));
        assert!(matches!(
            archive.read("indexed-not-packed.bin"),
            Err(ArchiveError::Corrupt { .. })
        ));
    }

    #[test]
    fn garbage_kraken_data_is_a_clean_error() {
        assert!(matches!(archive().read("garbage.bin"), Err(ArchiveError::Corrupt { .. })));
    }

    #[test]
    fn malformed_kraken_streams_from_real_data_are_errors_not_panics() {
        let Some((keys, paths)) = crate::game::tests::real_game("malformed_kraken_streams") else {
            return;
        };
        let game = crate::game::GameFiles::open(&paths, &keys).unwrap();
        let archive = game.archive();
        let hash = path_hash(crate::game::tests::GIFTS);
        let pack = archive.pack_bytes(archive.pack_of_file[&hash]).unwrap();
        let entry = find_pack_entry(&pack, hash).unwrap().unwrap();
        assert_eq!(entry.compression, COMPRESSION_OODLE_KRAKEN);

        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut errors = 0;
        for _ in 0..3000 {
            let mut mutated = entry.data.to_vec();
            for _ in 0..1 + next() % 8 {
                let at = (next() as usize) % mutated.len();
                mutated[at] ^= (next() | 1) as u8;
            }
            if decompress_kraken(&mutated, entry.uncompressed_size).is_err() {
                errors += 1;
            }
        }
        println!("OK 3000 mutated Kraken streams: {errors} clean errors, no panic escaped");
    }

    #[test]
    fn opening_rejects_bad_magic_and_missing_packs() {
        let index = index_bytes(&[(1, 0)], &["pack/a"]);
        let bad_magic = Archive::open(&index, Arc::new(vec![0u8; 32]));
        assert!(matches!(bad_magic, Err(ArchiveOpenError::Malformed(_))));

        let wrong_pack = trpfs(&[("pack/other", pack(&[]))]);
        assert!(matches!(
            Archive::open(&index, Arc::new(wrong_pack)),
            Err(ArchiveOpenError::Malformed(_))
        ));

        let index = index_bytes(&[(1, 5)], &["pack/a"]);
        let right_pack = trpfs(&[("pack/a", pack(&[]))]);
        assert!(matches!(
            Archive::open(&index, Arc::new(right_pack)),
            Err(ArchiveOpenError::Malformed(_))
        ));
    }

    #[test]
    fn cache_stays_within_capacity_and_evicts_least_recently_used() {
        let mut cache = PackCache::new(10);
        cache.insert(1, Arc::new(vec![0; 4]));
        cache.insert(2, Arc::new(vec![0; 4]));
        assert!(cache.get(1).is_some());
        cache.insert(3, Arc::new(vec![0; 4]));
        assert!(cache.get(2).is_none());
        assert!(cache.get(1).is_some());
        assert!(cache.get(3).is_some());
        assert_eq!(cache.bytes, 8);

        cache.insert(4, Arc::new(vec![0; 11]));
        assert!(cache.get(4).is_none());
        cache.insert(1, Arc::new(vec![0; 4]));
        assert_eq!(cache.bytes, 8);
    }
}
