use std::collections::HashSet;

use super::flat::{Fb, FbError, FbVector};
use super::path_hash;

const SLOT_FILE_HASHES: usize = 0;
const SLOT_PACK_PATHS: usize = 1;
const SLOT_FILE_INFOS: usize = 2;
const SLOT_INFO_PACK_INDEX: usize = 0;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("\"{path}\" is not in the archive index.")]
    PathNotInIndex { path: String },
    #[error("The archive index is malformed: {0}")]
    Malformed(String),
}

impl From<FbError> for IndexError {
    fn from(error: FbError) -> Self {
        IndexError::Malformed(error.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub hash: u64,
    pub pack: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedIndex {
    pub entries: Vec<IndexEntry>,
    pub pack_paths: Vec<String>,
}

fn file_vectors(fb: &Fb<'_>, root: usize) -> Result<(FbVector, FbVector), IndexError> {
    let hashes = fb.u64_vector(root, SLOT_FILE_HASHES)?;
    let infos = fb.offset_vector(root, SLOT_FILE_INFOS)?;
    if hashes.len != infos.len {
        return Err(IndexError::Malformed(format!(
            "{} file hashes but {} file infos",
            hashes.len, infos.len
        )));
    }
    Ok((hashes, infos))
}

pub fn parse_index(raw_trpfd: &[u8]) -> Result<ParsedIndex, IndexError> {
    let fb = Fb::new(raw_trpfd);
    let root = fb.root()?;
    let (hashes, infos) = file_vectors(&fb, root)?;
    let entries = fb
        .u64s(hashes)
        .zip(fb.offsets(infos)?)
        .map(|(hash, info)| {
            let pack = fb.u64_field(info, SLOT_INFO_PACK_INDEX, 0)?;
            Ok(IndexEntry { hash, pack })
        })
        .collect::<Result<_, FbError>>()?;
    let pack_paths = fb
        .offsets(fb.offset_vector(root, SLOT_PACK_PATHS)?)?
        .into_iter()
        .map(|pos| fb.string_at(pos).map(str::to_owned))
        .collect::<Result<_, FbError>>()?;
    Ok(ParsedIndex {
        entries,
        pack_paths,
    })
}

/// Compacts both file vectors in place, leaving the buffer the same length with stale bytes in
/// the slack, because the rest of the buffer stays addressed by its original offsets.
pub fn remove_from_index(raw_trpfd: &[u8], paths: &[&str]) -> Result<Vec<u8>, IndexError> {
    let fb = Fb::new(raw_trpfd);
    let root = fb.root()?;
    let (hashes_vector, infos_vector) = file_vectors(&fb, root)?;
    let hashes: Vec<u64> = fb.u64s(hashes_vector).collect();
    let info_targets = fb.offsets(infos_vector)?;

    let present: HashSet<u64> = hashes.iter().copied().collect();
    let mut removed = HashSet::new();
    for path in paths {
        let hash = path_hash(path);
        if !present.contains(&hash) {
            return Err(IndexError::PathNotInIndex {
                path: (*path).to_owned(),
            });
        }
        removed.insert(hash);
    }

    let mut out = raw_trpfd.to_vec();
    let mut kept = 0;
    for (hash, target) in hashes.into_iter().zip(info_targets) {
        if removed.contains(&hash) {
            continue;
        }
        write_u64(&mut out, hashes_vector.start + kept * 8, hash);
        let slot = infos_vector.start + kept * 4;
        let relative = u32::try_from(target - slot)
            .map_err(|_| IndexError::Malformed("file info offset overflows".to_owned()))?;
        write_u32(&mut out, slot, relative);
        kept += 1;
    }
    let kept = kept as u32;
    if hashes_vector.len > 0 {
        write_u32(&mut out, hashes_vector.start - 4, kept);
        write_u32(&mut out, infos_vector.start - 4, kept);
    }
    Ok(out)
}

fn write_u32(buf: &mut [u8], at: usize, value: u32) {
    buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(buf: &mut [u8], at: usize, value: u64) {
    buf[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
pub(crate) mod test_data {
    use super::super::flat::build::{buffer, Field, Node};
    use super::*;

    pub(crate) fn index_bytes(files: &[(u64, u64)], packs: &[&str]) -> Vec<u8> {
        buffer(&Node::Table(vec![
            Field::Ref(Node::U64s(files.iter().map(|f| f.0).collect())),
            Field::Ref(Node::Refs(
                packs.iter().map(|p| Node::Str((*p).to_owned())).collect(),
            )),
            Field::Ref(Node::Refs(
                files
                    .iter()
                    .map(|f| Node::Table(vec![Field::U64(f.1)]))
                    .collect(),
            )),
        ]))
    }

    pub(crate) fn hashes(paths: &[&str]) -> Vec<u64> {
        paths.iter().map(|p| path_hash(p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::test_data::*;
    use super::*;

    const PATHS: [&str; 4] = ["a.bin", "b.bin", "c.bin", "d.bin"];

    fn sample() -> Vec<u8> {
        let hashes = hashes(&PATHS);
        let files: Vec<(u64, u64)> = hashes
            .iter()
            .enumerate()
            .map(|(i, hash)| (*hash, i as u64 % 2))
            .collect();
        index_bytes(&files, &["pack0", "pack1"])
    }

    #[test]
    fn removal_preserves_order_mapping_and_length() {
        let raw = sample();
        let before = parse_index(&raw).unwrap();
        let out = remove_from_index(&raw, &["a.bin", "c.bin"]).unwrap();
        assert_eq!(out.len(), raw.len());

        let after = parse_index(&out).unwrap();
        assert_eq!(after.entries, [before.entries[1], before.entries[3]]);
        assert_eq!(after.pack_paths, before.pack_paths);
    }

    #[test]
    fn removing_a_duplicate_path_removes_it_once() {
        let out = remove_from_index(&sample(), &["b.bin", "b.bin"]).unwrap();
        assert_eq!(parse_index(&out).unwrap().entries.len(), 3);
    }

    #[test]
    fn unknown_path_is_named_in_the_error() {
        let error = remove_from_index(&sample(), &["a.bin", "nope.bin"]).unwrap_err();
        assert!(matches!(error, IndexError::PathNotInIndex { path } if path == "nope.bin"));
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(matches!(parse_index(&[0xFF; 16]), Err(IndexError::Malformed(_))));
        assert!(remove_from_index(&[0xFF; 16], &[]).is_err());
    }

    #[test]
    fn removing_nothing_is_an_identity() {
        assert_eq!(remove_from_index(&sample(), &[]).unwrap(), sample());
    }
}
